use std::collections::BTreeSet;

use rmpv::Value as MsgPackValue;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::types::{InstalledPluginRecord, NodeError, PluginMessageDescriptorRecord};

pub const PLUGIN_CUSTOM_TYPE: &str = "org.freetakteam.rem.plugin.v1";
const FIELD_CUSTOM_TYPE: i64 = 0xFB;
const FIELD_CUSTOM_DATA: i64 = 0xFC;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLxmfEnvelope {
    pub plugin_id: String,
    pub message_name: String,
    pub message_version: String,
    pub payload: JsonValue,
}

pub fn validate_plugin_id(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.iter().all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|first| {
                (first.is_ascii_lowercase() || first.is_ascii_digit())
                    && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
            })
        })
}

pub fn validate_discovered_plugin(plugin: &InstalledPluginRecord) -> Result<(), NodeError> {
    let descriptor = &plugin.discovered;
    if !validate_plugin_id(descriptor.plugin_id.as_str())
        || descriptor.display_name.trim().is_empty()
        || descriptor.version.trim().is_empty()
        || descriptor.package_name.trim().is_empty()
        || descriptor.service_class_name.trim().is_empty()
        || descriptor.publisher_fingerprint.trim().is_empty()
    {
        return Err(NodeError::InvalidConfig {});
    }
    let mut names = BTreeSet::new();
    for message in &descriptor.messages {
        if !is_safe_name(message.name.as_str())
            || message.version.trim().is_empty()
            || (!message.send && !message.receive)
            || !message.schema.is_object()
            || !names.insert(message.name.as_str())
        {
            return Err(NodeError::InvalidConfig {});
        }
    }
    Ok(())
}

pub fn encode_plugin_fields(
    plugin: &InstalledPluginRecord,
    message_name: &str,
    payload: JsonValue,
) -> Result<Vec<u8>, NodeError> {
    let descriptor = require_message(plugin, message_name, true)?;
    validate_payload(&payload, &descriptor.schema)?;
    let envelope = PluginLxmfEnvelope {
        plugin_id: plugin.discovered.plugin_id.clone(),
        message_name: descriptor.name.clone(),
        message_version: descriptor.version.clone(),
        payload,
    };
    let envelope_value = rmpv::ext::from_value(
        rmp_serde::from_slice::<MsgPackValue>(
            rmp_serde::to_vec_named(&envelope)
                .map_err(|_| NodeError::InvalidConfig {})?
                .as_slice(),
        )
        .map_err(|_| NodeError::InvalidConfig {})?,
    )
    .map_err(|_| NodeError::InvalidConfig {})?;
    let fields = MsgPackValue::Map(vec![
        (
            MsgPackValue::from(FIELD_CUSTOM_TYPE),
            MsgPackValue::from(PLUGIN_CUSTOM_TYPE),
        ),
        (MsgPackValue::from(FIELD_CUSTOM_DATA), envelope_value),
    ]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InvalidConfig {})
}

pub fn decode_plugin_fields(
    fields_bytes: &[u8],
    plugin: &InstalledPluginRecord,
) -> Result<Option<PluginLxmfEnvelope>, NodeError> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes)
        .map_err(|_| NodeError::InvalidConfig {})?;
    let MsgPackValue::Map(entries) = fields else {
        return Ok(None);
    };
    let custom_type = entries.iter().find_map(|(key, value)| {
        (key.as_i64() == Some(FIELD_CUSTOM_TYPE))
            .then(|| value.as_str())
            .flatten()
    });
    if custom_type != Some(PLUGIN_CUSTOM_TYPE) {
        return Ok(None);
    }
    let data = entries
        .iter()
        .find_map(|(key, value)| (key.as_i64() == Some(FIELD_CUSTOM_DATA)).then_some(value))
        .ok_or(NodeError::InvalidConfig {})?;
    let envelope: PluginLxmfEnvelope =
        rmpv::ext::from_value(data.clone()).map_err(|_| NodeError::InvalidConfig {})?;
    if envelope.plugin_id != plugin.discovered.plugin_id {
        return Err(NodeError::InvalidConfig {});
    }
    let descriptor = require_message(plugin, envelope.message_name.as_str(), false)?;
    if descriptor.version != envelope.message_version {
        return Err(NodeError::InvalidConfig {});
    }
    validate_payload(&envelope.payload, &descriptor.schema)?;
    Ok(Some(envelope))
}

pub fn plugin_id_from_fields(fields_bytes: &[u8]) -> Result<Option<String>, NodeError> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes)
        .map_err(|_| NodeError::InvalidConfig {})?;
    let MsgPackValue::Map(entries) = fields else {
        return Ok(None);
    };
    let custom_type = entries.iter().find_map(|(key, value)| {
        (key.as_i64() == Some(FIELD_CUSTOM_TYPE))
            .then(|| value.as_str())
            .flatten()
    });
    if custom_type != Some(PLUGIN_CUSTOM_TYPE) {
        return Ok(None);
    }
    let data = entries
        .iter()
        .find_map(|(key, value)| (key.as_i64() == Some(FIELD_CUSTOM_DATA)).then_some(value))
        .ok_or(NodeError::InvalidConfig {})?;
    let envelope: PluginLxmfEnvelope =
        rmpv::ext::from_value(data.clone()).map_err(|_| NodeError::InvalidConfig {})?;
    Ok(Some(envelope.plugin_id))
}

fn require_message<'a>(
    plugin: &'a InstalledPluginRecord,
    name: &str,
    outbound: bool,
) -> Result<&'a PluginMessageDescriptorRecord, NodeError> {
    plugin
        .discovered
        .messages
        .iter()
        .find(|message| {
            message.name == name
                && if outbound {
                    message.send
                } else {
                    message.receive
                }
        })
        .ok_or(NodeError::InvalidConfig {})
}

fn is_safe_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| {
        first.is_ascii_lowercase()
            && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    })
}

fn validate_payload(value: &JsonValue, schema: &JsonValue) -> Result<(), NodeError> {
    let Some(schema) = schema.as_object() else {
        return Err(NodeError::InvalidConfig {});
    };
    match schema.get("type").and_then(JsonValue::as_str) {
        Some("object") => {
            let object = value.as_object().ok_or(NodeError::InvalidConfig {})?;
            if let Some(required) = schema.get("required").and_then(JsonValue::as_array) {
                if required
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .any(|key| !object.contains_key(key))
                {
                    return Err(NodeError::InvalidConfig {});
                }
            }
            if let Some(properties) = schema.get("properties").and_then(JsonValue::as_object) {
                for (key, property_schema) in properties {
                    if let Some(property) = object.get(key) {
                        validate_payload(property, property_schema)?;
                    }
                }
                if schema
                    .get("additionalProperties")
                    .and_then(JsonValue::as_bool)
                    == Some(false)
                    && object.keys().any(|key| !properties.contains_key(key))
                {
                    return Err(NodeError::InvalidConfig {});
                }
            }
        }
        Some("string") if !value.is_string() => return Err(NodeError::InvalidConfig {}),
        Some("integer") if value.as_i64().is_none() => return Err(NodeError::InvalidConfig {}),
        Some("number") if !value.is_number() => return Err(NodeError::InvalidConfig {}),
        Some("boolean") if !value.is_boolean() => return Err(NodeError::InvalidConfig {}),
        Some("array") if !value.is_array() => return Err(NodeError::InvalidConfig {}),
        Some("string" | "integer" | "number" | "boolean" | "array") | None => {}
        Some(_) => return Err(NodeError::InvalidConfig {}),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DiscoveredPluginRecord, PluginCapabilityRecord, PluginMessageDescriptorRecord,
    };
    use serde_json::json;

    fn plugin() -> InstalledPluginRecord {
        InstalledPluginRecord {
            discovered: DiscoveredPluginRecord {
                plugin_id: "org.freetakteam.rem.plugin.test".to_string(),
                display_name: "Test".to_string(),
                version: "1.0.0".to_string(),
                api_major: 1,
                api_minor: 0,
                package_name: "org.freetakteam.rem.plugin.test".to_string(),
                service_class_name: ".TestPluginService".to_string(),
                publisher_fingerprint: "aa".repeat(32),
                publisher_history: Vec::new(),
                android_permissions: Vec::new(),
                declared_capabilities: PluginCapabilityRecord {
                    lxmf_send: true,
                    lxmf_receive: true,
                    ..PluginCapabilityRecord::default()
                },
                messages: vec![PluginMessageDescriptorRecord {
                    name: "sample".to_string(),
                    version: "1.0.0".to_string(),
                    send: true,
                    receive: true,
                    schema: json!({
                        "type": "object",
                        "required": ["value"],
                        "properties": {"value": {"type": "integer"}},
                        "additionalProperties": false
                    }),
                }],
                configuration_entrypoint: None,
            },
            state: "Running".to_string(),
            trusted: true,
            enabled: true,
            granted_capabilities: PluginCapabilityRecord {
                lxmf_send: true,
                lxmf_receive: true,
                ..PluginCapabilityRecord::default()
            },
            diagnostic: None,
            updated_at_ms: 1,
        }
    }

    #[test]
    fn plugin_custom_fields_round_trip() {
        let plugin = plugin();
        let fields =
            encode_plugin_fields(&plugin, "sample", json!({"value": 82})).expect("encode fields");
        let decoded = decode_plugin_fields(&fields, &plugin)
            .expect("decode fields")
            .expect("plugin envelope");
        assert_eq!(decoded.message_name, "sample");
        assert_eq!(decoded.payload, json!({"value": 82}));
    }

    #[test]
    fn rejects_payload_outside_declared_schema() {
        assert!(encode_plugin_fields(&plugin(), "sample", json!({"value": "bad"})).is_err());
    }
}
