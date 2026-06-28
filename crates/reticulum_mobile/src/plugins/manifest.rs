use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::abi::PluginEntrypoints;
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
    #[error("unsupported plugin type: {plugin_type}")]
    InvalidPluginType { plugin_type: String },
    #[error("invalid plugin version: {version}")]
    InvalidPluginVersion { version: String },
    #[error("unsupported REM plugin API version: {rem_api_version}")]
    UnsupportedApiVersion { rem_api_version: String },
    #[error("missing Android library for ABI: {abi}")]
    MissingAndroidLibrary { abi: String },
    #[error("invalid plugin library path: {path}")]
    InvalidLibraryPath { path: String },
    #[error("invalid plugin settings path: {path}")]
    InvalidSettingsPath { path: String },
    #[error("invalid plugin message schema path: {path}")]
    InvalidMessageSchemaPath { path: String },
    #[error("invalid plugin message name: {message_name}")]
    InvalidMessageName { message_name: String },
    #[error("invalid plugin message version for {message_name}: {version}")]
    InvalidMessageVersion {
        message_name: String,
        version: String,
    },
    #[error("duplicate plugin message name: {message_name}")]
    DuplicateMessageName { message_name: String },
    #[error("duplicate plugin message direction for {message_name}: {direction}")]
    DuplicateMessageDirection {
        message_name: String,
        direction: String,
    },
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
    #[serde(default)]
    pub entrypoints: PluginEntrypoints,
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
        if !is_semver_version(self.version.as_str()) {
            return Err(PluginManifestError::InvalidPluginVersion {
                version: self.version.clone(),
            });
        }
        if !is_reverse_dns_id(self.id.as_str()) {
            return Err(PluginManifestError::InvalidPluginId {
                plugin_id: self.id.clone(),
            });
        }
        if self.plugin_type != "native" {
            return Err(PluginManifestError::InvalidPluginType {
                plugin_type: self.plugin_type.clone(),
            });
        }
        if !rem_api_version_supports_current(self.rem_api_version.as_str()) {
            return Err(PluginManifestError::UnsupportedApiVersion {
                rem_api_version: self.rem_api_version.clone(),
            });
        }
        for path in self.library.android.values() {
            validate_relative_archive_path(path).map_err(|()| {
                PluginManifestError::InvalidLibraryPath {
                    path: path.to_string(),
                }
            })?;
        }
        if let Some(settings) = &self.settings {
            validate_relative_archive_path(settings.schema.as_str()).map_err(|()| {
                PluginManifestError::InvalidSettingsPath {
                    path: settings.schema.clone(),
                }
            })?;
        }
        let mut message_names = BTreeSet::new();
        for message in &self.messages {
            message.validate()?;
            if !message_names.insert(message.name.as_str()) {
                return Err(PluginManifestError::DuplicateMessageName {
                    message_name: message.name.clone(),
                });
            }
            validate_relative_archive_path(message.schema.as_str()).map_err(|()| {
                PluginManifestError::InvalidMessageSchemaPath {
                    path: message.schema.clone(),
                }
            })?;
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

pub(crate) fn is_semver_version(value: &str) -> bool {
    Version::parse(value.trim()).is_ok()
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

fn validate_relative_archive_path(value: &str) -> Result<(), ()> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(());
    }
    Ok(())
}
