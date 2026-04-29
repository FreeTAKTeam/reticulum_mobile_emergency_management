use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{PluginLxmfMessage, PluginLxmfMessageError, PluginRegistry, RegisteredPlugin};

#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("plugin not found: {plugin_id}")]
    PluginNotFound { plugin_id: String },
    #[error("permission denied for {plugin_id}: {permission}")]
    PermissionDenied {
        plugin_id: String,
        permission: &'static str,
    },
    #[error(transparent)]
    LxmfMessage(#[from] PluginLxmfMessageError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginEvent {
    pub topic: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone)]
pub struct PluginHostApi {
    registry: PluginRegistry,
    plugin_storage: BTreeMap<(String, String), JsonValue>,
    queued_lxmf_messages: Vec<PluginLxmfMessage>,
    received_lxmf_messages: BTreeMap<String, Vec<PluginLxmfMessage>>,
    subscriptions: BTreeMap<String, BTreeSet<String>>,
    event_inboxes: BTreeMap<String, Vec<PluginEvent>>,
}

impl PluginHostApi {
    pub fn new(registry: PluginRegistry) -> Self {
        Self {
            registry,
            plugin_storage: BTreeMap::new(),
            queued_lxmf_messages: Vec::new(),
            received_lxmf_messages: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            event_inboxes: BTreeMap::new(),
        }
    }

    pub fn get_plugin_storage(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<JsonValue>, PluginHostError> {
        self.require_permission(plugin_id, "storage.plugin")?;
        Ok(self
            .plugin_storage
            .get(&(plugin_id.to_string(), key.to_string()))
            .cloned())
    }

    pub fn set_plugin_storage(
        &mut self,
        plugin_id: &str,
        key: &str,
        value: JsonValue,
    ) -> Result<(), PluginHostError> {
        self.require_permission(plugin_id, "storage.plugin")?;
        self.plugin_storage
            .insert((plugin_id.to_string(), key.to_string()), value);
        Ok(())
    }

    pub fn request_lxmf_send(
        &mut self,
        plugin_id: &str,
        message_name: &str,
        payload: JsonValue,
    ) -> Result<PluginLxmfMessage, PluginHostError> {
        self.require_permission(plugin_id, "lxmf.send")?;
        let plugin = self.require_plugin(plugin_id)?;
        let message = PluginLxmfMessage::new(&plugin.manifest, message_name, payload)?;
        self.queued_lxmf_messages.push(message.clone());
        Ok(message)
    }

    pub fn queued_lxmf_messages(&self) -> &[PluginLxmfMessage] {
        self.queued_lxmf_messages.as_slice()
    }

    pub fn receive_lxmf_fields(
        &mut self,
        fields_bytes: &[u8],
    ) -> Result<Option<PluginLxmfMessage>, PluginHostError> {
        let Some(plugin_id) = PluginLxmfMessage::try_plugin_id_from_fields_bytes(fields_bytes)?
        else {
            return Ok(None);
        };
        self.require_permission(plugin_id.as_str(), "lxmf.receive")?;
        let plugin = self.require_plugin(plugin_id.as_str())?;
        let message = PluginLxmfMessage::from_fields_bytes(&plugin.manifest, fields_bytes)?;
        self.received_lxmf_messages
            .entry(plugin_id)
            .or_default()
            .push(message.clone());
        Ok(Some(message))
    }

    pub fn received_lxmf_messages(&self, plugin_id: &str) -> &[PluginLxmfMessage] {
        self.received_lxmf_messages
            .get(plugin_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn subscribe(&mut self, plugin_id: &str, topic: &str) -> Result<(), PluginHostError> {
        if let Some(permission) = permission_for_topic(topic) {
            self.require_permission(plugin_id, permission)?;
        } else {
            self.require_plugin(plugin_id)?;
        }
        self.subscriptions
            .entry(plugin_id.to_string())
            .or_default()
            .insert(topic.to_string());
        Ok(())
    }

    pub fn deliver_event(
        &mut self,
        topic: &str,
        payload: JsonValue,
    ) -> Result<(), PluginHostError> {
        let plugin_ids = self
            .subscriptions
            .iter()
            .filter_map(|(plugin_id, topics)| topics.contains(topic).then_some(plugin_id.clone()))
            .collect::<Vec<_>>();

        for plugin_id in plugin_ids {
            if let Some(permission) = permission_for_topic(topic) {
                self.require_permission(plugin_id.as_str(), permission)?;
            }
            self.event_inboxes
                .entry(plugin_id)
                .or_default()
                .push(PluginEvent {
                    topic: topic.to_string(),
                    payload: payload.clone(),
                });
        }
        Ok(())
    }

    pub fn plugin_events(&self, plugin_id: &str) -> &[PluginEvent] {
        self.event_inboxes
            .get(plugin_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn require_permission(
        &self,
        plugin_id: &str,
        permission: &'static str,
    ) -> Result<(), PluginHostError> {
        let plugin = self.require_plugin(plugin_id)?;
        let allowed = match permission {
            "storage.plugin" => {
                plugin.manifest.permissions.storage_plugin
                    && plugin.granted_permissions.storage_plugin
            }
            "lxmf.send" => {
                plugin.manifest.permissions.lxmf_send && plugin.granted_permissions.lxmf_send
            }
            "lxmf.receive" => {
                plugin.manifest.permissions.lxmf_receive && plugin.granted_permissions.lxmf_receive
            }
            "messages.read" => {
                plugin.manifest.permissions.messages_read
                    && plugin.granted_permissions.messages_read
            }
            _ => false,
        };
        if allowed {
            return Ok(());
        }
        Err(PluginHostError::PermissionDenied {
            plugin_id: plugin_id.to_string(),
            permission,
        })
    }

    fn require_plugin(&self, plugin_id: &str) -> Result<&RegisteredPlugin, PluginHostError> {
        self.registry
            .get(plugin_id)
            .ok_or_else(|| PluginHostError::PluginNotFound {
                plugin_id: plugin_id.to_string(),
            })
    }
}

fn permission_for_topic(topic: &str) -> Option<&'static str> {
    match topic {
        "rem.message.received" | "rem.message.sent" => Some("messages.read"),
        _ => None,
    }
}
