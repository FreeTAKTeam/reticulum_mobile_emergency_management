use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{
    validate_plugin_message_payload, PluginLxmfMessage, PluginLxmfMessageError,
    PluginLxmfOutboundRequest, PluginMessageSchemaMap, PluginRegistry, RegisteredPlugin,
};
use crate::app_state::AppStateStore;
use crate::types::{NodeError, SendMode};

#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("plugin not found: {plugin_id}")]
    PluginNotFound { plugin_id: String },
    #[error("permission denied for {plugin_id}: {permission}")]
    PermissionDenied {
        plugin_id: String,
        permission: &'static str,
    },
    #[error("plugin storage failed: {0}")]
    Storage(#[from] NodeError),
    #[error(transparent)]
    LxmfMessage(#[from] PluginLxmfMessageError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginEvent {
    pub topic: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPermissionCheckLog {
    pub plugin_id: String,
    pub action: &'static str,
    pub permission: &'static str,
    pub allowed: bool,
}

#[derive(Debug, Clone)]
pub struct PluginHostApi {
    registry: PluginRegistry,
    message_schemas: PluginMessageSchemaMap,
    app_state: Option<AppStateStore>,
    plugin_storage: BTreeMap<(String, String), JsonValue>,
    queued_lxmf_messages: Vec<PluginLxmfMessage>,
    queued_lxmf_outbound: Vec<PluginLxmfOutboundRequest>,
    received_lxmf_messages: BTreeMap<String, Vec<PluginLxmfMessage>>,
    subscriptions: BTreeMap<String, BTreeSet<String>>,
    event_inboxes: BTreeMap<String, Vec<PluginEvent>>,
    permission_checks: Vec<PluginPermissionCheckLog>,
}

impl PluginHostApi {
    pub fn new(registry: PluginRegistry) -> Self {
        Self::new_with_message_schemas(registry, PluginMessageSchemaMap::new())
    }

    pub fn new_with_message_schemas(
        registry: PluginRegistry,
        message_schemas: PluginMessageSchemaMap,
    ) -> Self {
        Self::new_inner(registry, message_schemas, None)
    }

    pub(crate) fn new_with_message_schemas_and_app_state_store(
        registry: PluginRegistry,
        message_schemas: PluginMessageSchemaMap,
        app_state: AppStateStore,
    ) -> Self {
        Self::new_inner(registry, message_schemas, Some(app_state))
    }

    fn new_inner(
        registry: PluginRegistry,
        message_schemas: PluginMessageSchemaMap,
        app_state: Option<AppStateStore>,
    ) -> Self {
        Self {
            registry,
            message_schemas,
            app_state,
            plugin_storage: BTreeMap::new(),
            queued_lxmf_messages: Vec::new(),
            queued_lxmf_outbound: Vec::new(),
            received_lxmf_messages: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            event_inboxes: BTreeMap::new(),
            permission_checks: Vec::new(),
        }
    }

    pub fn get_plugin_storage(
        &mut self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<JsonValue>, PluginHostError> {
        self.require_permission(plugin_id, "get_plugin_storage", "storage.plugin")?;
        if let Some(app_state) = self.app_state.as_ref() {
            return app_state
                .get_plugin_storage_value(plugin_id, key)
                .map_err(PluginHostError::Storage);
        }
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
        self.require_permission(plugin_id, "set_plugin_storage", "storage.plugin")?;
        if let Some(app_state) = self.app_state.as_ref() {
            return app_state
                .set_plugin_storage_value(plugin_id, key, &value)
                .map_err(PluginHostError::Storage);
        }
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
        self.require_permission(plugin_id, "request_lxmf_send", "lxmf.send")?;
        let plugin = self.require_plugin(plugin_id)?;
        let message = PluginLxmfMessage::new(&plugin.manifest, message_name, payload)?;
        self.validate_payload(&message)?;
        self.queued_lxmf_messages.push(message.clone());
        Ok(message)
    }

    pub fn request_lxmf_send_to(
        &mut self,
        plugin_id: &str,
        destination_hex: &str,
        message_name: &str,
        payload: JsonValue,
        body_utf8: &str,
        title: Option<String>,
        send_mode: SendMode,
    ) -> Result<PluginLxmfOutboundRequest, PluginHostError> {
        self.require_permission(plugin_id, "request_lxmf_send_to", "lxmf.send")?;
        let plugin = self.require_plugin(plugin_id)?;
        let message = PluginLxmfMessage::new(&plugin.manifest, message_name, payload)?;
        self.validate_payload(&message)?;
        let request =
            message.into_outbound_request(destination_hex, body_utf8, title, send_mode)?;
        self.queued_lxmf_outbound.push(request.clone());
        Ok(request)
    }

    pub fn queued_lxmf_messages(&self) -> &[PluginLxmfMessage] {
        self.queued_lxmf_messages.as_slice()
    }

    pub fn queued_lxmf_outbound_requests(&self) -> &[PluginLxmfOutboundRequest] {
        self.queued_lxmf_outbound.as_slice()
    }

    pub fn drain_queued_lxmf_outbound_requests(&mut self) -> Vec<PluginLxmfOutboundRequest> {
        std::mem::take(&mut self.queued_lxmf_outbound)
    }

    pub fn receive_lxmf_fields(
        &mut self,
        fields_bytes: &[u8],
    ) -> Result<Option<PluginLxmfMessage>, PluginHostError> {
        let Some(plugin_id) = PluginLxmfMessage::try_plugin_id_from_fields_bytes(fields_bytes)?
        else {
            return Ok(None);
        };
        self.require_permission(plugin_id.as_str(), "receive_lxmf_fields", "lxmf.receive")?;
        let plugin = self.require_plugin(plugin_id.as_str())?;
        let message = PluginLxmfMessage::from_fields_bytes(&plugin.manifest, fields_bytes)?;
        self.validate_payload(&message)?;
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
            self.require_permission(plugin_id, "subscribe", permission)?;
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
            self.deliver_event_to_plugin(plugin_id.as_str(), topic, payload.clone())?;
        }
        Ok(())
    }

    pub fn deliver_event_to_plugin(
        &mut self,
        plugin_id: &str,
        topic: &str,
        payload: JsonValue,
    ) -> Result<bool, PluginHostError> {
        self.require_plugin(plugin_id)?;
        if !self
            .subscriptions
            .get(plugin_id)
            .is_some_and(|topics| topics.contains(topic))
        {
            return Ok(false);
        }
        if let Some(permission) = permission_for_topic(topic) {
            self.require_permission(plugin_id, "deliver_event", permission)?;
        }
        self.event_inboxes
            .entry(plugin_id.to_string())
            .or_default()
            .push(PluginEvent {
                topic: topic.to_string(),
                payload,
            });
        Ok(true)
    }

    pub fn plugin_events(&self, plugin_id: &str) -> &[PluginEvent] {
        self.event_inboxes
            .get(plugin_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn permission_checks(&self) -> &[PluginPermissionCheckLog] {
        self.permission_checks.as_slice()
    }

    fn require_permission(
        &mut self,
        plugin_id: &str,
        action: &'static str,
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
        self.permission_checks.push(PluginPermissionCheckLog {
            plugin_id: plugin_id.to_string(),
            action,
            permission,
            allowed,
        });
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

    fn validate_payload(&self, message: &PluginLxmfMessage) -> Result<(), PluginHostError> {
        let Some(schema) = self
            .message_schemas
            .get(&(message.plugin_id.clone(), message.message_name.clone()))
        else {
            return Ok(());
        };
        validate_plugin_message_payload(message.message_name.as_str(), &message.payload, schema)?;
        Ok(())
    }
}

fn permission_for_topic(topic: &str) -> Option<&'static str> {
    match topic {
        "rem.message.received" | "rem.message.sent" => Some("messages.read"),
        "rem.plugin.lxmf.received" => Some("lxmf.receive"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppStateStore;
    use crate::plugins::{PluginManifest, PluginRegistry};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const VALID_MANIFEST: &str = r#"
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[permissions]
storage_plugin = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send"]
schema = "messages/status_test.schema.json"
"#;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_storage_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "reticulum-mobile-plugin-host-{name}-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        path
    }

    fn granted_storage_registry() -> PluginRegistry {
        let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
        let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
        registry
            .grant_permissions("rem.plugin.example_status", |permissions| {
                permissions.storage_plugin = true;
            })
            .expect("grant succeeds");
        registry
    }

    #[test]
    fn host_api_persists_plugin_storage_with_app_state_store() {
        let storage_dir = test_storage_dir("storage-round-trip");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let mut host = PluginHostApi::new_with_message_schemas_and_app_state_store(
            granted_storage_registry(),
            PluginMessageSchemaMap::new(),
            store,
        );

        host.set_plugin_storage("rem.plugin.example_status", "counter", json!(4))
            .expect("write plugin storage");

        let restarted_store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("reopen store");
        let mut restarted = PluginHostApi::new_with_message_schemas_and_app_state_store(
            granted_storage_registry(),
            PluginMessageSchemaMap::new(),
            restarted_store,
        );

        assert_eq!(
            restarted
                .get_plugin_storage("rem.plugin.example_status", "counter")
                .expect("read plugin storage"),
            Some(json!(4))
        );
    }
}
