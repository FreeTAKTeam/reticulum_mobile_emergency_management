use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use thiserror::Error;

use super::manifest::PluginManifestError;
use super::PluginManifest;

pub const PLUGIN_LXMF_FIELD_KEY: &str = "rem.plugin.message";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginMessageDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMessageDescriptor {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub direction: Vec<PluginMessageDirection>,
    pub schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginLxmfMessageError {
    #[error("plugin message is not declared: {message_name}")]
    UndeclaredMessage { message_name: String },
    #[error("failed to encode plugin LXMF fields")]
    EncodeFields,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginLxmfMessage {
    pub plugin_id: String,
    pub message_name: String,
    pub wire_type: String,
    pub payload: JsonValue,
}

impl PluginLxmfMessage {
    pub fn new(
        manifest: &PluginManifest,
        message_name: &str,
        payload: JsonValue,
    ) -> Result<Self, PluginLxmfMessageError> {
        let descriptor = manifest
            .messages
            .iter()
            .find(|message| message.name == message_name)
            .ok_or_else(|| PluginLxmfMessageError::UndeclaredMessage {
                message_name: message_name.to_string(),
            })?;
        Ok(Self {
            plugin_id: manifest.id.clone(),
            message_name: descriptor.name.clone(),
            wire_type: descriptor.wire_type(manifest.id.as_str()),
            payload,
        })
    }

    pub fn to_fields_bytes(&self) -> Result<Vec<u8>, PluginLxmfMessageError> {
        let fields = json!({
            PLUGIN_LXMF_FIELD_KEY: {
                "plugin_id": self.plugin_id,
                "message_name": self.message_name,
                "wire_type": self.wire_type,
                "payload": self.payload,
            }
        });
        rmp_serde::to_vec(&fields).map_err(|_| PluginLxmfMessageError::EncodeFields)
    }
}

impl PluginMessageDescriptor {
    pub fn validate(&self) -> Result<(), PluginManifestError> {
        if !is_safe_message_name(self.name.as_str()) {
            return Err(PluginManifestError::InvalidMessageName {
                message_name: self.name.clone(),
            });
        }
        if self.version.trim().is_empty() {
            return Err(PluginManifestError::MissingRequiredField {
                field: "messages.version",
            });
        }
        if self.schema.trim().is_empty() {
            return Err(PluginManifestError::MissingRequiredField {
                field: "messages.schema",
            });
        }
        Ok(())
    }

    pub fn wire_type(&self, plugin_id: &str) -> String {
        format!("plugin.{plugin_id}.{}", self.name)
    }
}

fn is_safe_message_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}
