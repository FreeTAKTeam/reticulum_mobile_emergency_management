use std::collections::BTreeMap;

use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{PluginLxmfMessage, PluginLxmfMessageError, PluginRegistry};

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

#[derive(Debug, Clone)]
pub struct PluginHostApi {
    registry: PluginRegistry,
    plugin_storage: BTreeMap<(String, String), JsonValue>,
    queued_lxmf_messages: Vec<PluginLxmfMessage>,
}

impl PluginHostApi {
    pub fn new(registry: PluginRegistry) -> Self {
        Self {
            registry,
            plugin_storage: BTreeMap::new(),
            queued_lxmf_messages: Vec::new(),
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
        let plugin =
            self.registry
                .get(plugin_id)
                .ok_or_else(|| PluginHostError::PluginNotFound {
                    plugin_id: plugin_id.to_string(),
                })?;
        let message = PluginLxmfMessage::new(&plugin.manifest, message_name, payload)?;
        self.queued_lxmf_messages.push(message.clone());
        Ok(message)
    }

    pub fn queued_lxmf_messages(&self) -> &[PluginLxmfMessage] {
        self.queued_lxmf_messages.as_slice()
    }

    fn require_permission(
        &self,
        plugin_id: &str,
        permission: &'static str,
    ) -> Result<(), PluginHostError> {
        let plugin =
            self.registry
                .get(plugin_id)
                .ok_or_else(|| PluginHostError::PluginNotFound {
                    plugin_id: plugin_id.to_string(),
                })?;
        let allowed = match permission {
            "storage.plugin" => {
                plugin.manifest.permissions.storage_plugin
                    && plugin.granted_permissions.storage_plugin
            }
            "lxmf.send" => {
                plugin.manifest.permissions.lxmf_send && plugin.granted_permissions.lxmf_send
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
}
