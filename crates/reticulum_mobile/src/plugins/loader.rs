use std::path::{Path, PathBuf};

use thiserror::Error;

use super::{PluginManifest, PluginManifestError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLoadCandidate {
    pub manifest: PluginManifest,
    pub install_dir: PathBuf,
    pub library_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginDiscoveryReport {
    pub candidates: Vec<PluginLoadCandidate>,
    pub errors: Vec<PluginLoaderError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginLoaderError {
    #[error("plugin discovery I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("invalid plugin manifest at {path}: {source}")]
    Manifest {
        path: PathBuf,
        source: PluginManifestError,
    },
    #[error("missing native plugin library: {path}")]
    MissingLibrary { path: PathBuf },
    #[error("invalid native plugin library path: {path}")]
    InvalidLibraryPath { path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct PluginLoader {
    install_root: PathBuf,
}

impl PluginLoader {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
        }
    }

    pub fn discover_installed_plugins(
        &self,
        android_abi: &str,
    ) -> Result<PluginDiscoveryReport, PluginLoaderError> {
        let mut report = PluginDiscoveryReport::default();
        if !self.install_root.exists() {
            return Ok(report);
        }

        for entry in fs_err::read_dir(self.install_root.as_path()).map_err(|error| {
            PluginLoaderError::Io {
                path: self.install_root.clone(),
                message: error.to_string(),
            }
        })? {
            let entry = entry.map_err(|error| PluginLoaderError::Io {
                path: self.install_root.clone(),
                message: error.to_string(),
            })?;
            let entry_path = entry.path();
            let file_type = entry.file_type().map_err(|error| PluginLoaderError::Io {
                path: entry_path.clone(),
                message: error.to_string(),
            })?;
            if !file_type.is_dir() {
                continue;
            }

            match self.discover_plugin_dir(entry_path.as_path(), android_abi) {
                Ok(Some(candidate)) => report.candidates.push(candidate),
                Ok(None) => {}
                Err(error) => report.errors.push(error),
            }
        }

        Ok(report)
    }

    fn discover_plugin_dir(
        &self,
        install_dir: &Path,
        android_abi: &str,
    ) -> Result<Option<PluginLoadCandidate>, PluginLoaderError> {
        let manifest_path = install_dir.join("plugin.toml");
        if !manifest_path.exists() {
            return Ok(None);
        }

        let manifest_source = fs_err::read_to_string(manifest_path.as_path()).map_err(|error| {
            PluginLoaderError::Io {
                path: manifest_path.clone(),
                message: error.to_string(),
            }
        })?;
        let manifest =
            PluginManifest::from_toml_str(manifest_source.as_str()).map_err(|source| {
                PluginLoaderError::Manifest {
                    path: manifest_path.clone(),
                    source,
                }
            })?;
        let relative_library_path =
            manifest
                .android_library_for_abi(android_abi)
                .map_err(|source| PluginLoaderError::Manifest {
                    path: manifest_path,
                    source,
                })?;
        let library_path = install_dir.join(relative_library_path);
        validate_installed_file_path(install_dir, library_path.as_path())?;

        Ok(Some(PluginLoadCandidate {
            manifest,
            install_dir: install_dir.to_path_buf(),
            library_path,
        }))
    }
}

fn validate_installed_file_path(
    install_dir: &Path,
    file_path: &Path,
) -> Result<(), PluginLoaderError> {
    if !file_path.is_file() {
        return Err(PluginLoaderError::MissingLibrary {
            path: file_path.to_path_buf(),
        });
    }

    let canonical_install_dir =
        fs_err::canonicalize(install_dir).map_err(|error| PluginLoaderError::Io {
            path: install_dir.to_path_buf(),
            message: error.to_string(),
        })?;
    let canonical_file_path =
        fs_err::canonicalize(file_path).map_err(|error| PluginLoaderError::Io {
            path: file_path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !canonical_file_path.starts_with(canonical_install_dir) {
        return Err(PluginLoaderError::InvalidLibraryPath {
            path: file_path.to_path_buf(),
        });
    }
    Ok(())
}
