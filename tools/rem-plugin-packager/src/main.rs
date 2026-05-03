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
    #[error("invalid package schema file: {path}")]
    InvalidPackageSchema { path: String },
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
    let plugin_type = manifest_string(manifest, "plugin_type")?;
    if plugin_type != "native" {
        return Err(PackagerError::InvalidManifestField {
            field: "plugin_type",
        });
    }
    let rem_api_version = manifest_string(manifest, "rem_api_version")?;
    if !rem_api_version_supports_current(rem_api_version.as_str()) {
        return Err(PackagerError::InvalidManifestField {
            field: "rem_api_version",
        });
    }
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
        let settings_schema_json = read_json_package_file(plugin_dir, settings_schema)?;
        validate_settings_schema_actions(&settings_schema_json, manifest, settings_schema)?;
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
            read_json_package_file(plugin_dir, schema)?;
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

fn read_json_package_file(
    plugin_dir: &Path,
    relative_path: &str,
) -> Result<serde_json::Value, PackagerError> {
    let path = plugin_dir.join(relative_path);
    let contents = fs_err::read(path)?;
    let schema: serde_json::Value = serde_json::from_slice(contents.as_slice()).map_err(|_| {
        PackagerError::InvalidPackageSchema {
            path: relative_path.to_string(),
        }
    })?;
    if schema.is_object() {
        return Ok(schema);
    }
    Err(PackagerError::InvalidPackageSchema {
        path: relative_path.to_string(),
    })
}

fn validate_settings_schema_actions(
    schema: &serde_json::Value,
    manifest: &toml::Value,
    relative_path: &str,
) -> Result<(), PackagerError> {
    let field_ids = settings_field_ids(schema);
    let declared_messages = manifest
        .get("messages")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| message.get("name").and_then(toml::Value::as_str))
        .collect::<BTreeSet<_>>();
    let Some(actions) = schema.get("actions").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };
    for action in actions {
        let Some(action) = action.as_object() else {
            continue;
        };
        let Some(action_type) = action.get("type").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if action_type != "send_lxmf" && action_type != "sendPluginLxmf" {
            continue;
        }
        let Some(message_name) = action
            .get("messageName")
            .and_then(serde_json::Value::as_str)
        else {
            return Err(invalid_schema(relative_path));
        };
        if !declared_messages.contains(message_name) {
            return Err(invalid_schema(relative_path));
        }
        for field_key in ["destinationField", "bodyField"] {
            let Some(field_id) = action.get(field_key).and_then(serde_json::Value::as_str) else {
                return Err(invalid_schema(relative_path));
            };
            if !field_ids.contains(field_id) {
                return Err(invalid_schema(relative_path));
            }
        }
        if let Some(payload_fields) = action
            .get("payloadFields")
            .and_then(serde_json::Value::as_object)
        {
            for value in payload_fields.values() {
                let Some(field_id) = value.as_str() else {
                    return Err(invalid_schema(relative_path));
                };
                if !field_ids.contains(field_id) {
                    return Err(invalid_schema(relative_path));
                }
            }
        }
    }
    Ok(())
}

fn settings_field_ids(schema: &serde_json::Value) -> BTreeSet<String> {
    let explicit = schema
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|field| field.get("id").and_then(serde_json::Value::as_str))
        .filter(|field_id| !field_id.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    if !explicit.is_empty() {
        return explicit;
    }
    schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

fn invalid_schema(relative_path: &str) -> PackagerError {
    PackagerError::InvalidPackageSchema {
        path: relative_path.to_string(),
    }
}

fn rem_api_version_supports_current(value: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .all(comparator_allows_current_api)
}

fn comparator_allows_current_api(comparator: &str) -> bool {
    const CURRENT_API_VERSION: (u64, u64, u64) = (1, 0, 0);
    for operator in [">=", "<=", ">", "<", "="] {
        if let Some(raw_version) = comparator.strip_prefix(operator) {
            let Some(version) = parse_api_version(raw_version.trim()) else {
                return false;
            };
            return match operator {
                ">=" => CURRENT_API_VERSION >= version,
                "<=" => CURRENT_API_VERSION <= version,
                ">" => CURRENT_API_VERSION > version,
                "<" => CURRENT_API_VERSION < version,
                "=" => CURRENT_API_VERSION == version,
                _ => false,
            };
        }
    }
    parse_api_version(comparator).is_some_and(|version| version == CURRENT_API_VERSION)
}

fn parse_api_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(label: &str) -> Self {
            let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "rem-packager-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(path.as_path()).expect("temp dir is created");
            Self { path }
        }

        fn path(&self) -> &Path {
            self.path.as_path()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.path.as_path());
        }
    }

    fn write_file(root: &Path, relative_path: &str, contents: &[u8]) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory is created");
        }
        fs::write(path, contents).expect("file is written");
    }

    fn valid_manifest() -> toml::Value {
        r#"
id = "rem.plugin.example_status"
plugin_type = "native"
rem_api_version = ">=1.0.0,<2.0.0"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"
x86_64 = "logic/android/x86_64/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send", "receive"]
schema = "schemas/status_test.schema.json"
"#
        .parse()
        .expect("manifest parses")
    }

    fn write_valid_package(root: &Path) {
        write_file(
            root,
            "logic/android/arm64-v8a/libexample_status_plugin.so",
            b"native-arm64",
        );
        write_file(
            root,
            "logic/android/x86_64/libexample_status_plugin.so",
            b"native-x64",
        );
        write_file(root, "ui/settings.schema.json", br#"{"type":"object"}"#);
        write_file(
            root,
            "schemas/status_test.schema.json",
            br#"{"type":"object"}"#,
        );
    }

    #[test]
    fn parse_args_accepts_allow_missing_libraries_flag_after_output() {
        let args = parse_args([
            "plugins/example-status-plugin".to_string(),
            "output/example-status.remplugin".to_string(),
            "--allow-missing-libraries".to_string(),
        ])
        .expect("args parse");

        assert_eq!(
            args.plugin_dir,
            PathBuf::from("plugins/example-status-plugin")
        );
        assert_eq!(
            args.output_path,
            Some(PathBuf::from("output/example-status.remplugin"))
        );
        assert!(args.allow_missing_libraries);
    }

    #[test]
    fn validate_package_references_requires_android_libraries_by_default() {
        let package = TestTempDir::new("missing-library");
        let manifest = valid_manifest();
        write_file(package.path(), "ui/settings.schema.json", br#"{}"#);
        write_file(package.path(), "schemas/status_test.schema.json", br#"{}"#);

        let err = validate_package_references(package.path(), &manifest, false)
            .expect_err("missing library is rejected");

        assert!(matches!(err, PackagerError::MissingPackageFile { .. }));
    }

    #[test]
    fn validate_package_references_allows_missing_libraries_only_when_requested() {
        let package = TestTempDir::new("allow-missing-library");
        let manifest = valid_manifest();
        write_file(package.path(), "ui/settings.schema.json", br#"{}"#);
        write_file(package.path(), "schemas/status_test.schema.json", br#"{}"#);

        validate_package_references(package.path(), &manifest, true)
            .expect("missing library override is accepted");
    }

    #[test]
    fn validate_package_references_rejects_unsafe_message_schema_path() {
        let package = TestTempDir::new("unsafe-message-schema");
        let manifest = r#"
id = "rem.plugin.example_status"
plugin_type = "native"
rem_api_version = ">=1.0.0,<2.0.0"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send", "receive"]
schema = "../status_test.schema.json"
"#
        .parse()
        .expect("manifest parses");

        let err = validate_package_references(package.path(), &manifest, true)
            .expect_err("unsafe schema path is rejected");

        assert!(matches!(err, PackagerError::UnsafePath { .. }));
    }

    #[test]
    fn validate_package_references_rejects_invalid_message_schema_json() {
        let package = TestTempDir::new("invalid-message-schema-json");
        let manifest = valid_manifest();
        write_valid_package(package.path());
        write_file(
            package.path(),
            "schemas/status_test.schema.json",
            b"not-json",
        );

        validate_package_references(package.path(), &manifest, true)
            .expect_err("invalid message schema json is rejected");
    }

    #[test]
    fn validate_package_references_rejects_invalid_settings_schema_json() {
        let package = TestTempDir::new("invalid-settings-schema-json");
        let manifest = valid_manifest();
        write_valid_package(package.path());
        write_file(package.path(), "ui/settings.schema.json", b"not-json");

        validate_package_references(package.path(), &manifest, true)
            .expect_err("invalid settings schema json is rejected");
    }

    #[test]
    fn validate_package_references_rejects_settings_action_for_undeclared_message() {
        let package = TestTempDir::new("settings-action-undeclared-message");
        let manifest = valid_manifest();
        write_valid_package(package.path());
        write_file(
            package.path(),
            "ui/settings.schema.json",
            br#"{
                "fields": [
                    {"id": "destinationHex", "type": "text"},
                    {"id": "statusMessage", "type": "text"}
                ],
                "actions": [
                    {
                        "id": "sendMissing",
                        "type": "send_lxmf",
                        "messageName": "missing_status",
                        "destinationField": "destinationHex",
                        "bodyField": "statusMessage",
                        "payloadFields": {"message": "statusMessage"}
                    }
                ]
            }"#,
        );

        validate_package_references(package.path(), &manifest, true)
            .expect_err("settings action for undeclared message is rejected");
    }

    #[test]
    fn validate_package_references_rejects_settings_action_for_unknown_field() {
        let package = TestTempDir::new("settings-action-unknown-field");
        let manifest = valid_manifest();
        write_valid_package(package.path());
        write_file(
            package.path(),
            "ui/settings.schema.json",
            br#"{
                "fields": [
                    {"id": "destinationHex", "type": "text"},
                    {"id": "statusMessage", "type": "text"}
                ],
                "actions": [
                    {
                        "id": "sendStatus",
                        "type": "send_lxmf",
                        "messageName": "status_test",
                        "destinationField": "destinationHex",
                        "bodyField": "missingBody",
                        "payloadFields": {"message": "statusMessage"}
                    }
                ]
            }"#,
        );

        validate_package_references(package.path(), &manifest, true)
            .expect_err("settings action for unknown field is rejected");
    }

    #[test]
    fn validate_package_references_rejects_non_native_plugin_type() {
        let package = TestTempDir::new("non-native-plugin-type");
        let manifest = r#"
id = "rem.plugin.example_status"
plugin_type = "web"
rem_api_version = ">=1.0.0,<2.0.0"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"
"#
        .parse()
        .expect("manifest parses");
        write_valid_package(package.path());

        validate_package_references(package.path(), &manifest, true)
            .expect_err("non-native plugin type is rejected");
    }

    #[test]
    fn validate_package_references_rejects_unsupported_rem_api_version() {
        let package = TestTempDir::new("unsupported-rem-api-version");
        let manifest = r#"
id = "rem.plugin.example_status"
plugin_type = "native"
rem_api_version = ">=2.0.0,<3.0.0"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"
"#
        .parse()
        .expect("manifest parses");
        write_valid_package(package.path());

        validate_package_references(package.path(), &manifest, true)
            .expect_err("unsupported API range is rejected");
    }

    #[test]
    fn write_archive_skips_build_and_dependency_directories() {
        let package = TestTempDir::new("archive");
        write_file(
            package.path(),
            "plugin.toml",
            b"id = \"rem.plugin.example_status\"",
        );
        write_valid_package(package.path());
        write_file(package.path(), "target/debug/libignored.so", b"ignored");
        write_file(package.path(), "node_modules/example/index.js", b"ignored");

        let mut cursor = std::io::Cursor::new(Vec::new());
        write_archive(package.path(), &mut cursor).expect("archive writes");
        cursor.set_position(0);
        let mut archive = zip::ZipArchive::new(cursor).expect("archive reads");
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("entry exists");
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .expect("entry contents read");
            names.push(entry.name().to_string());
        }

        assert!(names.contains(&"plugin.toml".to_string()));
        assert!(names.contains(&"logic/android/arm64-v8a/libexample_status_plugin.so".to_string()));
        assert!(!names.iter().any(|name| name.starts_with("target/")));
        assert!(!names.iter().any(|name| name.starts_with("node_modules/")));
    }
}
