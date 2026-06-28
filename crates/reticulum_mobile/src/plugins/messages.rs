use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use thiserror::Error;

use super::manifest::{is_semver_version, PluginManifestError};
use super::PluginManifest;
use crate::types::SendMode;

pub const PLUGIN_LXMF_FIELD_KEY: &str = "rem.plugin.message";

pub type PluginMessageSchemaMap = BTreeMap<(String, String), JsonValue>;

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
    #[error("failed to decode plugin LXMF fields")]
    DecodeFields,
    #[error("invalid plugin LXMF field envelope")]
    InvalidEnvelope,
    #[error("plugin LXMF message is for {actual_plugin_id}, expected {expected_plugin_id}")]
    PluginIdMismatch {
        expected_plugin_id: String,
        actual_plugin_id: String,
    },
    #[error("plugin LXMF wire type is {actual_wire_type}, expected {expected_wire_type}")]
    WireTypeMismatch {
        expected_wire_type: String,
        actual_wire_type: String,
    },
    #[error("plugin message direction is not allowed for {message_name}: {direction:?}")]
    DirectionNotAllowed {
        message_name: String,
        direction: PluginMessageDirection,
    },
    #[error("invalid plugin message payload for {message_name}: {reason}")]
    InvalidPayload {
        message_name: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginLxmfMessage {
    pub plugin_id: String,
    pub message_name: String,
    pub wire_type: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginLxmfOutboundRequest {
    pub plugin_id: String,
    pub destination_hex: String,
    pub message_name: String,
    pub wire_type: String,
    pub body_utf8: String,
    pub title: Option<String>,
    pub fields_bytes: Vec<u8>,
    pub send_mode: SendMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginLxmfSendRequest {
    pub plugin_id: String,
    pub destination_hex: String,
    pub message_name: String,
    pub payload: JsonValue,
    pub body_utf8: String,
    pub title: Option<String>,
    pub send_mode: SendMode,
}

impl PluginLxmfMessage {
    pub fn new(
        manifest: &PluginManifest,
        message_name: &str,
        payload: JsonValue,
    ) -> Result<Self, PluginLxmfMessageError> {
        Self::new_for_direction(
            manifest,
            message_name,
            payload,
            PluginMessageDirection::Send,
        )
    }

    pub fn new_for_direction(
        manifest: &PluginManifest,
        message_name: &str,
        payload: JsonValue,
        direction: PluginMessageDirection,
    ) -> Result<Self, PluginLxmfMessageError> {
        let descriptor = declared_message_for_direction(manifest, message_name, direction)?;
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

    pub fn into_outbound_request(
        self,
        destination_hex: impl Into<String>,
        body_utf8: impl Into<String>,
        title: Option<String>,
        send_mode: SendMode,
    ) -> Result<PluginLxmfOutboundRequest, PluginLxmfMessageError> {
        let fields_bytes = self.to_fields_bytes()?;
        Ok(PluginLxmfOutboundRequest {
            plugin_id: self.plugin_id,
            destination_hex: destination_hex.into(),
            message_name: self.message_name,
            wire_type: self.wire_type,
            body_utf8: body_utf8.into(),
            title,
            fields_bytes,
            send_mode,
        })
    }

    pub fn try_plugin_id_from_fields_bytes(
        fields_bytes: &[u8],
    ) -> Result<Option<String>, PluginLxmfMessageError> {
        let fields = decode_fields(fields_bytes)?;
        let Some(envelope) = fields.get(PLUGIN_LXMF_FIELD_KEY) else {
            return Ok(None);
        };
        Ok(Some(required_string(envelope, "plugin_id")?.to_string()))
    }

    pub fn from_fields_bytes(
        manifest: &PluginManifest,
        fields_bytes: &[u8],
    ) -> Result<Self, PluginLxmfMessageError> {
        let fields = decode_fields(fields_bytes)?;
        let envelope = fields
            .get(PLUGIN_LXMF_FIELD_KEY)
            .ok_or(PluginLxmfMessageError::InvalidEnvelope)?;
        let plugin_id = required_string(envelope, "plugin_id")?;
        if plugin_id != manifest.id {
            return Err(PluginLxmfMessageError::PluginIdMismatch {
                expected_plugin_id: manifest.id.clone(),
                actual_plugin_id: plugin_id.to_string(),
            });
        }

        let message_name = required_string(envelope, "message_name")?;
        let wire_type = required_string(envelope, "wire_type")?;
        let payload = envelope
            .get("payload")
            .cloned()
            .ok_or(PluginLxmfMessageError::InvalidEnvelope)?;
        let descriptor = declared_message_for_direction(
            manifest,
            message_name,
            PluginMessageDirection::Receive,
        )?;
        let expected_wire_type = descriptor.wire_type(manifest.id.as_str());
        if wire_type != expected_wire_type {
            return Err(PluginLxmfMessageError::WireTypeMismatch {
                expected_wire_type,
                actual_wire_type: wire_type.to_string(),
            });
        }

        Ok(Self {
            plugin_id: plugin_id.to_string(),
            message_name: message_name.to_string(),
            wire_type: wire_type.to_string(),
            payload,
        })
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
        if !is_semver_version(self.version.as_str()) {
            return Err(PluginManifestError::InvalidMessageVersion {
                message_name: self.name.clone(),
                version: self.version.clone(),
            });
        }
        if self.schema.trim().is_empty() {
            return Err(PluginManifestError::MissingRequiredField {
                field: "messages.schema",
            });
        }
        if self.direction.is_empty() {
            return Err(PluginManifestError::MissingRequiredField {
                field: "messages.direction",
            });
        }
        let mut directions = BTreeSet::new();
        for direction in &self.direction {
            let direction_name = direction.as_manifest_value();
            if !directions.insert(direction_name) {
                return Err(PluginManifestError::DuplicateMessageDirection {
                    message_name: self.name.clone(),
                    direction: direction_name.to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn wire_type(&self, plugin_id: &str) -> String {
        format!("plugin.{plugin_id}.{}", self.name)
    }

    fn allows_direction(&self, direction: PluginMessageDirection) -> bool {
        self.direction.contains(&direction)
    }
}

impl PluginMessageDirection {
    fn as_manifest_value(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Receive => "receive",
        }
    }
}

pub fn validate_plugin_message_payload(
    message_name: &str,
    payload: &JsonValue,
    schema: &JsonValue,
) -> Result<(), PluginLxmfMessageError> {
    validate_schema_value("$", payload, schema).map_err(|reason| {
        PluginLxmfMessageError::InvalidPayload {
            message_name: message_name.to_string(),
            reason,
        }
    })
}

fn validate_schema_value(path: &str, value: &JsonValue, schema: &JsonValue) -> Result<(), String> {
    let Some(schema_object) = schema.as_object() else {
        return Err(format!("{path} schema must be an object"));
    };
    match schema_object.get("type").and_then(JsonValue::as_str) {
        Some("object") => validate_object_schema(path, value, schema_object),
        Some("string") => validate_string_schema(path, value, schema_object),
        Some("number") => value
            .is_number()
            .then_some(())
            .ok_or_else(|| format!("{path} must be a number")),
        Some("integer") => value
            .as_i64()
            .map(|_| ())
            .ok_or_else(|| format!("{path} must be an integer")),
        Some("boolean") => value
            .is_boolean()
            .then_some(())
            .ok_or_else(|| format!("{path} must be a boolean")),
        Some("array") => value
            .is_array()
            .then_some(())
            .ok_or_else(|| format!("{path} must be an array")),
        Some(other) => Err(format!("{path} has unsupported schema type {other}")),
        None => Ok(()),
    }
}

fn validate_object_schema(
    path: &str,
    value: &JsonValue,
    schema: &serde_json::Map<String, JsonValue>,
) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err(format!("{path} must be an object"));
    };
    if let Some(required) = schema.get("required").and_then(JsonValue::as_array) {
        for field in required.iter().filter_map(JsonValue::as_str) {
            if !object.contains_key(field) {
                return Err(format!("{path}.{field} is required"));
            }
        }
    }

    let properties = schema.get("properties").and_then(JsonValue::as_object);
    if schema
        .get("additionalProperties")
        .and_then(JsonValue::as_bool)
        == Some(false)
    {
        if let Some(properties) = properties {
            for key in object.keys() {
                if !properties.contains_key(key) {
                    return Err(format!("{path}.{key} is not allowed"));
                }
            }
        }
    }
    if let Some(properties) = properties {
        for (key, property_schema) in properties {
            if let Some(property_value) = object.get(key) {
                validate_schema_value(
                    format!("{path}.{key}").as_str(),
                    property_value,
                    property_schema,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_string_schema(
    path: &str,
    value: &JsonValue,
    schema: &serde_json::Map<String, JsonValue>,
) -> Result<(), String> {
    let Some(value) = value.as_str() else {
        return Err(format!("{path} must be a string"));
    };
    if let Some(min_length) = schema.get("minLength").and_then(JsonValue::as_u64) {
        if value.chars().count() < min_length as usize {
            return Err(format!("{path} is shorter than {min_length} characters"));
        }
    }
    if let Some(max_length) = schema.get("maxLength").and_then(JsonValue::as_u64) {
        if value.chars().count() > max_length as usize {
            return Err(format!("{path} is longer than {max_length} characters"));
        }
    }
    Ok(())
}

fn declared_message_for_direction<'manifest>(
    manifest: &'manifest PluginManifest,
    message_name: &str,
    direction: PluginMessageDirection,
) -> Result<&'manifest PluginMessageDescriptor, PluginLxmfMessageError> {
    let descriptor = manifest
        .messages
        .iter()
        .find(|message| message.name == message_name)
        .ok_or_else(|| PluginLxmfMessageError::UndeclaredMessage {
            message_name: message_name.to_string(),
        })?;
    if descriptor.allows_direction(direction) {
        return Ok(descriptor);
    }
    Err(PluginLxmfMessageError::DirectionNotAllowed {
        message_name: message_name.to_string(),
        direction,
    })
}

fn decode_fields(fields_bytes: &[u8]) -> Result<JsonValue, PluginLxmfMessageError> {
    rmp_serde::from_slice(fields_bytes).map_err(|_| PluginLxmfMessageError::DecodeFields)
}

fn required_string<'a>(
    envelope: &'a JsonValue,
    field: &str,
) -> Result<&'a str, PluginLxmfMessageError> {
    envelope
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or(PluginLxmfMessageError::InvalidEnvelope)
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
