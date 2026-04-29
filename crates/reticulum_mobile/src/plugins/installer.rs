use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use thiserror::Error;
use zip::ZipArchive;

use super::{PluginManifest, PluginManifestError, PluginState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_dir: PathBuf,
    pub state: PluginState,
}

#[derive(Debug, Error)]
pub enum PluginInstallerError {
    #[error("missing plugin.toml in package")]
    MissingManifest,
    #[error(transparent)]
    Manifest(#[from] PluginManifestError),
    #[error("required package file is missing: {relative_path}")]
    MissingPackageFile { relative_path: String },
    #[error("invalid package path: {path}")]
    InvalidPackagePath { path: PathBuf },
    #[error("plugin is already installed: {plugin_id}")]
    AlreadyInstalled { plugin_id: String },
    #[error("plugin install I/O failed")]
    Io(#[from] std::io::Error),
    #[error("plugin archive is invalid")]
    InvalidArchive(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone)]
pub struct PluginInstaller {
    install_root: PathBuf,
}

impl PluginInstaller {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
        }
    }

    pub fn install_from_package_dir(
        &self,
        package_dir: impl AsRef<Path>,
        android_abi: &str,
    ) -> Result<InstalledPlugin, PluginInstallerError> {
        let package_dir = package_dir.as_ref();
        let manifest_path = package_dir.join("plugin.toml");
        if !manifest_path.is_file() {
            return Err(PluginInstallerError::MissingManifest);
        }

        let manifest =
            PluginManifest::from_toml_str(fs_err::read_to_string(manifest_path)?.as_str())?;
        let library_path = manifest.android_library_for_abi(android_abi)?;
        require_package_file(package_dir, library_path)?;
        if let Some(settings) = &manifest.settings {
            require_package_file(package_dir, settings.schema.as_str())?;
        }
        for message in &manifest.messages {
            require_package_file(package_dir, message.schema.as_str())?;
        }

        let install_dir = self.install_root.join(manifest.id.as_str());
        if install_dir.exists() {
            return Err(PluginInstallerError::AlreadyInstalled {
                plugin_id: manifest.id.clone(),
            });
        }

        fs_err::create_dir_all(self.install_root.as_path())?;
        let staging_dir = self.staging_install_dir(manifest.id.as_str());
        if staging_dir.exists() {
            fs_err::remove_dir_all(staging_dir.as_path())?;
        }
        if let Err(error) = copy_package_dir(package_dir, staging_dir.as_path()) {
            let _ = fs_err::remove_dir_all(staging_dir.as_path());
            return Err(error);
        }
        if let Err(error) = fs_err::rename(staging_dir.as_path(), install_dir.as_path()) {
            let _ = fs_err::remove_dir_all(staging_dir.as_path());
            return Err(PluginInstallerError::Io(error));
        }

        Ok(InstalledPlugin {
            manifest,
            install_dir,
            state: PluginState::Disabled,
        })
    }

    pub fn install_from_archive(
        &self,
        archive_path: impl AsRef<Path>,
        android_abi: &str,
    ) -> Result<InstalledPlugin, PluginInstallerError> {
        fs_err::create_dir_all(self.install_root.as_path())?;
        let extraction_dir = self.archive_extraction_dir();
        if extraction_dir.exists() {
            fs_err::remove_dir_all(extraction_dir.as_path())?;
        }
        fs_err::create_dir(extraction_dir.as_path())?;

        let result = (|| {
            let archive_file = fs_err::File::open(archive_path.as_ref())?;
            extract_archive(archive_file, extraction_dir.as_path())?;
            self.install_from_package_dir(extraction_dir.as_path(), android_abi)
        })();

        let _ = fs_err::remove_dir_all(extraction_dir.as_path());
        result
    }

    fn staging_install_dir(&self, plugin_id: &str) -> PathBuf {
        self.install_root
            .join(format!(".{plugin_id}.installing-{}", std::process::id()))
    }

    fn archive_extraction_dir(&self) -> PathBuf {
        self.install_root
            .join(format!(".archive-extract-{}", std::process::id()))
    }
}

fn extract_archive<R: Read + Seek>(
    reader: R,
    destination: &Path,
) -> Result<(), PluginInstallerError> {
    let mut archive = ZipArchive::new(reader)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let enclosed_path =
            entry
                .enclosed_name()
                .ok_or_else(|| PluginInstallerError::InvalidPackagePath {
                    path: PathBuf::from(entry.name()),
                })?;
        if is_zip_symlink(&entry) {
            return Err(PluginInstallerError::InvalidPackagePath {
                path: enclosed_path.to_path_buf(),
            });
        }
        let target = destination.join(enclosed_path);
        if entry.is_dir() {
            fs_err::create_dir_all(target.as_path())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut output = fs_err::File::create(target.as_path())?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

fn is_zip_symlink(entry: &zip::read::ZipFile<'_>) -> bool {
    const UNIX_FILE_TYPE_MASK: u32 = 0o170000;
    const UNIX_SYMLINK_TYPE: u32 = 0o120000;
    entry
        .unix_mode()
        .is_some_and(|mode| mode & UNIX_FILE_TYPE_MASK == UNIX_SYMLINK_TYPE)
}

fn require_package_file(
    package_dir: &Path,
    relative_path: &str,
) -> Result<(), PluginInstallerError> {
    let path = package_dir.join(relative_path);
    if path.is_file() {
        return Ok(());
    }
    Err(PluginInstallerError::MissingPackageFile {
        relative_path: relative_path.to_string(),
    })
}

fn copy_package_dir(source: &Path, destination: &Path) -> Result<(), PluginInstallerError> {
    fs_err::create_dir(destination)?;
    for entry in fs_err::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }

        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_package_dir(entry.path().as_path(), target.as_path())?;
        } else if file_type.is_file() {
            fs_err::copy(entry.path(), target)?;
        } else {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }
    }
    Ok(())
}
