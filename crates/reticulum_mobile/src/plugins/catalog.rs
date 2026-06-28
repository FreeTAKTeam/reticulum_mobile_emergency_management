use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{
    PersistedPluginRegistry, PluginLoadCandidate, PluginLoader, PluginLoaderError,
    PluginMessageDescriptor, PluginPermissions, PluginRegistry, PluginRegistryError, PluginState,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCatalogReport {
    pub items: Vec<InstalledPluginDescriptor>,
    pub errors: Vec<PluginCatalogDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    pub rem_api_version: String,
    pub plugin_type: String,
    pub state: PluginState,
    pub library_path: String,
    pub settings: Option<InstalledPluginSettingsDescriptor>,
    pub permissions: PluginPermissions,
    pub granted_permissions: PluginPermissions,
    pub messages: Vec<PluginMessageDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginSettingsDescriptor {
    pub schema_path: String,
    pub schema: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCatalogDiagnostic {
    pub plugin_id: Option<String>,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum PluginCatalogError {
    #[error("plugin catalog I/O failed")]
    Io(#[from] std::io::Error),
    #[error("invalid plug-in settings schema JSON at {path}")]
    InvalidSettingsSchema { path: PathBuf },
    #[error(transparent)]
    Registry(#[from] PluginRegistryError),
}

#[derive(Debug, Clone)]
pub struct PluginCatalog {
    install_root: PathBuf,
}

impl PluginCatalog {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
        }
    }

    pub fn list_installed_plugins(
        &self,
        android_abi: &str,
    ) -> Result<PluginCatalogReport, PluginCatalogError> {
        self.list_installed_plugins_with_state(android_abi, None)
    }

    pub fn list_installed_plugins_with_state(
        &self,
        android_abi: &str,
        persisted: Option<&PersistedPluginRegistry>,
    ) -> Result<PluginCatalogReport, PluginCatalogError> {
        let discovery = PluginLoader::new(self.install_root.as_path())
            .discover_installed_plugins(android_abi)
            .map_err(loader_error_to_catalog_error)?;
        let registry = registry_from_candidates(discovery.candidates.as_slice(), persisted)?;
        let mut report = PluginCatalogReport {
            items: Vec::new(),
            errors: discovery
                .errors
                .into_iter()
                .map(loader_error_to_diagnostic)
                .collect(),
        };

        for candidate in discovery.candidates {
            match descriptor_from_candidate(candidate, &registry) {
                Ok(descriptor) => report.items.push(descriptor),
                Err(error) => report.errors.push(catalog_error_to_diagnostic(error)),
            }
        }

        report
            .items
            .sort_by(|left, right| left.name.cmp(&right.name));
        Ok(report)
    }
}

fn registry_from_candidates(
    candidates: &[PluginLoadCandidate],
    persisted: Option<&PersistedPluginRegistry>,
) -> Result<PluginRegistry, PluginCatalogError> {
    let mut registry = PluginRegistry::from_manifests(
        candidates
            .iter()
            .map(|candidate| candidate.manifest.clone())
            .collect(),
    )
    .map_err(PluginCatalogError::Registry)?;
    if let Some(persisted) = persisted {
        registry.apply_persisted_state(persisted);
    }
    Ok(registry)
}

fn descriptor_from_candidate(
    candidate: PluginLoadCandidate,
    registry: &PluginRegistry,
) -> Result<InstalledPluginDescriptor, PluginCatalogError> {
    let settings = candidate
        .manifest
        .settings
        .as_ref()
        .map(|settings| {
            read_settings_descriptor(candidate.install_dir.as_path(), settings.schema.as_str())
        })
        .transpose()?;
    let library_path = candidate
        .library_path
        .strip_prefix(candidate.install_dir.as_path())
        .unwrap_or(candidate.library_path.as_path())
        .to_string_lossy()
        .replace('\\', "/");
    let registered = registry.get(candidate.manifest.id.as_str());

    Ok(InstalledPluginDescriptor {
        id: candidate.manifest.id.clone(),
        name: candidate.manifest.name,
        version: candidate.manifest.version,
        rem_api_version: candidate.manifest.rem_api_version,
        plugin_type: candidate.manifest.plugin_type,
        state: registered
            .map(|plugin| plugin.state)
            .unwrap_or(PluginState::Disabled),
        library_path,
        settings,
        permissions: candidate.manifest.permissions.clone(),
        granted_permissions: registered
            .map(|plugin| plugin.granted_permissions.clone())
            .unwrap_or_default(),
        messages: candidate.manifest.messages,
    })
}

fn read_settings_descriptor(
    install_dir: &Path,
    schema_path: &str,
) -> Result<InstalledPluginSettingsDescriptor, PluginCatalogError> {
    let path = install_dir.join(schema_path);
    let schema_source = fs_err::read_to_string(path.as_path())?;
    let schema: JsonValue = serde_json::from_str(schema_source.as_str()).map_err(|_| {
        PluginCatalogError::InvalidSettingsSchema {
            path: path.to_path_buf(),
        }
    })?;
    if !schema.is_object() {
        return Err(PluginCatalogError::InvalidSettingsSchema {
            path: path.to_path_buf(),
        });
    }
    Ok(InstalledPluginSettingsDescriptor {
        schema_path: schema_path.to_string(),
        schema,
    })
}

fn loader_error_to_catalog_error(error: PluginLoaderError) -> PluginCatalogError {
    match error {
        PluginLoaderError::Io { message, .. } => {
            PluginCatalogError::Io(std::io::Error::new(std::io::ErrorKind::Other, message))
        }
        PluginLoaderError::Manifest { path, source } => {
            PluginCatalogError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{path:?}: {source}"),
            ))
        }
        PluginLoaderError::MissingLibrary { path }
        | PluginLoaderError::InvalidLibraryPath { path } => PluginCatalogError::Io(
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{path:?}")),
        ),
    }
}

fn loader_error_to_diagnostic(error: PluginLoaderError) -> PluginCatalogDiagnostic {
    match error {
        PluginLoaderError::Io { path, message } => PluginCatalogDiagnostic {
            plugin_id: None,
            path: path.display().to_string(),
            message,
        },
        PluginLoaderError::Manifest { path, source } => PluginCatalogDiagnostic {
            plugin_id: None,
            path: path.display().to_string(),
            message: source.to_string(),
        },
        PluginLoaderError::MissingLibrary { path } => PluginCatalogDiagnostic {
            plugin_id: None,
            path: path.display().to_string(),
            message: "missing native plugin library".to_string(),
        },
        PluginLoaderError::InvalidLibraryPath { path } => PluginCatalogDiagnostic {
            plugin_id: None,
            path: path.display().to_string(),
            message: "native plugin library path escapes plugin directory".to_string(),
        },
    }
}

fn catalog_error_to_diagnostic(error: PluginCatalogError) -> PluginCatalogDiagnostic {
    match error {
        PluginCatalogError::Io(error) => PluginCatalogDiagnostic {
            plugin_id: None,
            path: String::new(),
            message: error.to_string(),
        },
        PluginCatalogError::InvalidSettingsSchema { path } => PluginCatalogDiagnostic {
            plugin_id: None,
            path: path.display().to_string(),
            message: "invalid settings schema JSON".to_string(),
        },
        PluginCatalogError::Registry(error) => PluginCatalogDiagnostic {
            plugin_id: None,
            path: String::new(),
            message: error.to_string(),
        },
    }
}
