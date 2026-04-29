use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::messages::PluginMessageDescriptor;
use super::permissions::PluginPermissions;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginManifestError {
    #[error("failed to parse plugin manifest")]
    ParseToml,
    #[error("missing required field: {field}")]
    MissingRequiredField { field: &'static str },
    #[error("invalid plugin id: {plugin_id}")]
    InvalidPluginId { plugin_id: String },
    #[error("missing Android library for ABI: {abi}")]
    MissingAndroidLibrary { abi: String },
    #[error("invalid plugin library path: {path}")]
    InvalidLibraryPath { path: String },
    #[error("invalid plugin message name: {message_name}")]
    InvalidMessageName { message_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub rem_api_version: String,
    pub plugin_type: String,
    pub library: PluginLibrary,
    #[serde(default)]
    pub settings: Option<PluginSettings>,
    #[serde(default)]
    pub permissions: PluginPermissions,
    #[serde(default)]
    pub messages: Vec<PluginMessageDescriptor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLibrary {
    #[serde(default)]
    pub android: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSettings {
    pub schema: String,
}

impl PluginManifest {
    pub fn from_toml_str(source: &str) -> Result<Self, PluginManifestError> {
        let manifest: Self = toml::from_str(source).map_err(|_| PluginManifestError::ParseToml)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn android_library_for_abi(&self, abi: &str) -> Result<&str, PluginManifestError> {
        let normalized = normalize_android_abi_key(abi);
        self.library
            .android
            .get(normalized.as_str())
            .map(String::as_str)
            .ok_or_else(|| PluginManifestError::MissingAndroidLibrary {
                abi: abi.to_string(),
            })
    }

    fn validate(&self) -> Result<(), PluginManifestError> {
        require_nonempty(self.id.as_str(), "id")?;
        require_nonempty(self.name.as_str(), "name")?;
        require_nonempty(self.version.as_str(), "version")?;
        require_nonempty(self.rem_api_version.as_str(), "rem_api_version")?;
        require_nonempty(self.plugin_type.as_str(), "plugin_type")?;
        if !is_reverse_dns_id(self.id.as_str()) {
            return Err(PluginManifestError::InvalidPluginId {
                plugin_id: self.id.clone(),
            });
        }
        for path in self.library.android.values() {
            validate_relative_archive_path(path)?;
        }
        for message in &self.messages {
            message.validate()?;
        }
        Ok(())
    }
}

fn require_nonempty(value: &str, field: &'static str) -> Result<(), PluginManifestError> {
    if value.trim().is_empty() {
        return Err(PluginManifestError::MissingRequiredField { field });
    }
    Ok(())
}

fn normalize_android_abi_key(abi: &str) -> String {
    abi.trim().replace('-', "_")
}

fn is_reverse_dns_id(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.iter().all(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            (first.is_ascii_lowercase() || first.is_ascii_digit())
                && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        })
}

fn validate_relative_archive_path(value: &str) -> Result<(), PluginManifestError> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(PluginManifestError::InvalidLibraryPath {
            path: value.to_string(),
        });
    }
    Ok(())
}
