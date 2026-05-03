use std::collections::BTreeSet;
use std::env;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

#[derive(Debug, Error)]
enum PackagerError {
    #[error(
        "usage: rem-plugin-packager <plugin-dir> [output.remplugin] [--allow-missing-libraries]"
    )]
    Usage,
    #[error("plugin directory does not exist: {path}")]
    MissingPluginDir { path: PathBuf },
    #[error("missing plugin.toml")]
    MissingManifest,
    #[error("invalid plugin manifest")]
    InvalidManifest,
    #[error("manifest field is missing or invalid: {field}")]
    InvalidManifestField { field: &'static str },
    #[error("unsafe package path: {path}")]
    UnsafePath { path: String },
    #[error("required package file is missing: {path}")]
    MissingPackageFile { path: String },
    #[error("archive I/O failed")]
    Io(#[from] std::io::Error),
    #[error("archive write failed")]
    Zip(#[from] zip::result::ZipError),
    #[error("directory walk failed")]
    Walk(#[from] walkdir::Error),
}

#[derive(Debug)]
struct PackagerArgs {
    plugin_dir: PathBuf,
    output_path: Option<PathBuf>,
    allow_missing_libraries: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), PackagerError> {
    let args = parse_args(env::args().skip(1))?;
    let plugin_dir =
        args.plugin_dir
            .canonicalize()
            .map_err(|_| PackagerError::MissingPluginDir {
                path: args.plugin_dir.clone(),
            })?;
    if !plugin_dir.is_dir() {
        return Err(PackagerError::MissingPluginDir {
            path: args.plugin_dir,
        });
    }

    let manifest_path = plugin_dir.join("plugin.toml");
    if !manifest_path.is_file() {
        return Err(PackagerError::MissingManifest);
    }
    let manifest = fs_err::read_to_string(manifest_path.as_path())?
        .parse::<toml::Value>()
        .map_err(|_| PackagerError::InvalidManifest)?;
    let plugin_id = manifest_string(&manifest, "id")?;
    validate_package_references(&plugin_dir, &manifest, args.allow_missing_libraries)?;

    let output_path = args
        .output_path
        .unwrap_or_else(|| PathBuf::from(format!("{plugin_id}.remplugin")));
    let output_path = absolute_path(output_path)?;
    if output_path.starts_with(plugin_dir.as_path()) {
        return Err(PackagerError::UnsafePath {
            path: output_path.display().to_string(),
        });
    }
    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs_err::create_dir_all(parent)?;
    }
    let archive_file = fs_err::File::create(output_path.as_path())?;
    write_archive(plugin_dir.as_path(), archive_file)?;
    println!("{}", output_path.display());
    Ok(())
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, PackagerError> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(env::current_dir()?.join(path))
}

fn parse_args(raw_args: impl IntoIterator<Item = String>) -> Result<PackagerArgs, PackagerError> {
    let mut plugin_dir = None;
    let mut output_path = None;
    let mut allow_missing_libraries = false;
    for arg in raw_args {
        if arg == "--allow-missing-libraries" {
            allow_missing_libraries = true;
        } else if plugin_dir.is_none() {
            plugin_dir = Some(PathBuf::from(arg));
        } else if output_path.is_none() {
            output_path = Some(PathBuf::from(arg));
        } else {
            return Err(PackagerError::Usage);
        }
    }
    let Some(plugin_dir) = plugin_dir else {
        return Err(PackagerError::Usage);
    };
    Ok(PackagerArgs {
        plugin_dir,
        output_path,
        allow_missing_libraries,
    })
}

fn validate_package_references(
    plugin_dir: &Path,
    manifest: &toml::Value,
    allow_missing_libraries: bool,
) -> Result<(), PackagerError> {
    let mut library_paths = BTreeSet::new();
    let android_libraries = manifest
        .get("library")
        .and_then(|value| value.get("android"))
        .and_then(toml::Value::as_table)
        .ok_or(PackagerError::InvalidManifestField {
            field: "library.android",
        })?;
    for value in android_libraries.values() {
        let path = value.as_str().ok_or(PackagerError::InvalidManifestField {
            field: "library.android.*",
        })?;
        validate_relative_path(path)?;
        library_paths.insert(path.to_string());
    }
    for path in &library_paths {
        if !allow_missing_libraries {
            require_package_file(plugin_dir, path)?;
        }
    }

    if let Some(settings_schema) = manifest
        .get("settings")
        .and_then(|value| value.get("schema"))
        .and_then(toml::Value::as_str)
    {
        validate_relative_path(settings_schema)?;
        require_package_file(plugin_dir, settings_schema)?;
    }

    if let Some(messages) = manifest.get("messages").and_then(toml::Value::as_array) {
        for message in messages {
            let schema = message.get("schema").and_then(toml::Value::as_str).ok_or(
                PackagerError::InvalidManifestField {
                    field: "messages.schema",
                },
            )?;
            validate_relative_path(schema)?;
            require_package_file(plugin_dir, schema)?;
        }
    }
    Ok(())
}

fn manifest_string(manifest: &toml::Value, key: &'static str) -> Result<String, PackagerError> {
    manifest
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(PackagerError::InvalidManifestField { field: key })
}

fn require_package_file(plugin_dir: &Path, relative_path: &str) -> Result<(), PackagerError> {
    let path = plugin_dir.join(relative_path);
    if path.is_file() {
        return Ok(());
    }
    Err(PackagerError::MissingPackageFile {
        path: relative_path.to_string(),
    })
}

fn validate_relative_path(path: &str) -> Result<(), PackagerError> {
    let candidate = Path::new(path);
    if path.trim().is_empty()
        || candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(PackagerError::UnsafePath {
            path: path.to_string(),
        });
    }
    Ok(())
}

fn write_archive<W: Write + Seek>(plugin_dir: &Path, writer: W) -> Result<(), PackagerError> {
    let mut archive = zip::ZipWriter::new(writer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    for entry in WalkDir::new(plugin_dir).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            return Err(PackagerError::UnsafePath {
                path: entry.path().display().to_string(),
            });
        }
        if entry.file_type().is_dir() {
            continue;
        }
        let relative =
            entry
                .path()
                .strip_prefix(plugin_dir)
                .map_err(|_| PackagerError::UnsafePath {
                    path: entry.path().display().to_string(),
                })?;
        if should_skip(relative) {
            continue;
        }
        files.push(relative.to_path_buf());
    }
    files.sort();
    for relative in files {
        let archive_path = relative.to_string_lossy().replace('\\', "/");
        validate_relative_path(archive_path.as_str())?;
        archive.start_file(archive_path, options)?;
        let bytes = fs_err::read(plugin_dir.join(relative))?;
        archive.write_all(bytes.as_slice())?;
    }
    archive.finish()?;
    Ok(())
}

fn should_skip(relative: &Path) -> bool {
    relative.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "target" || name == "node_modules" || name == ".git"
    })
}
