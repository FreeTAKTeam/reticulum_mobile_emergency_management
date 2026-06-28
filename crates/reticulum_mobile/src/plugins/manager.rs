use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use thiserror::Error;

use super::{
    NativePluginLibrary, NativePluginLoadError, PersistedPluginRegistry, PluginHostApi,
    PluginLoadCandidate, PluginLoader, PluginLoaderError, PluginLxmfMessage,
    PluginLxmfOutboundRequest, PluginMessageSchemaMap, PluginRegistry, PluginRegistryError,
    PluginState,
};
use crate::app_state::AppStateStore;

const SANITIZED_RUNTIME_TOPICS: &[&str] = &[
    "rem.message.received",
    "rem.message.sent",
    "rem.plugin.lxmf.received",
    "rem.plugin.started",
    "rem.plugin.stopped",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeDiagnostic {
    pub plugin_id: Option<String>,
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum NativePluginRuntimeError {
    #[error(transparent)]
    Discovery(#[from] PluginLoaderError),
    #[error(transparent)]
    Registry(#[from] PluginRegistryError),
}

#[derive(Debug)]
pub struct NativePluginRuntime {
    registry: PluginRegistry,
    candidates: BTreeMap<String, PluginLoadCandidate>,
    loaded: BTreeMap<String, NativePluginLibrary>,
    message_schemas: PluginMessageSchemaMap,
    app_state: Option<AppStateStore>,
    diagnostics: Vec<PluginRuntimeDiagnostic>,
}

impl NativePluginRuntime {
    pub fn discover(
        install_root: impl Into<PathBuf>,
        android_abi: &str,
        persisted: Option<&PersistedPluginRegistry>,
    ) -> Result<Self, NativePluginRuntimeError> {
        Self::discover_inner(install_root, android_abi, persisted, None)
    }

    pub fn discover_with_app_state_store(
        install_root: impl Into<PathBuf>,
        android_abi: &str,
        persisted: Option<&PersistedPluginRegistry>,
        app_state: AppStateStore,
    ) -> Result<Self, NativePluginRuntimeError> {
        Self::discover_inner(install_root, android_abi, persisted, Some(app_state))
    }

    fn discover_inner(
        install_root: impl Into<PathBuf>,
        android_abi: &str,
        persisted: Option<&PersistedPluginRegistry>,
        app_state: Option<AppStateStore>,
    ) -> Result<Self, NativePluginRuntimeError> {
        let discovery = PluginLoader::new(install_root).discover_installed_plugins(android_abi)?;
        let mut diagnostics = discovery
            .errors
            .into_iter()
            .map(loader_error_to_diagnostic)
            .collect::<Vec<_>>();
        let mut candidates = BTreeMap::new();
        let mut manifests = Vec::new();

        for candidate in discovery.candidates {
            let plugin_id = candidate.manifest.id.clone();
            manifests.push(candidate.manifest.clone());
            candidates.insert(plugin_id, candidate);
        }
        let (message_schemas, schema_diagnostics) = load_message_schemas(&candidates);
        diagnostics.extend(schema_diagnostics);

        let mut registry = PluginRegistry::from_manifests(manifests)?;
        if let Some(persisted) = persisted {
            registry.apply_persisted_state(persisted);
        }
        diagnostics.shrink_to_fit();

        Ok(Self {
            registry,
            candidates,
            loaded: BTreeMap::new(),
            message_schemas,
            app_state,
            diagnostics,
        })
    }

    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    pub fn diagnostics(&self) -> &[PluginRuntimeDiagnostic] {
        self.diagnostics.as_slice()
    }

    pub fn loaded_plugin_count(&self) -> usize {
        self.loaded.len()
    }

    pub fn start_enabled_plugins(&mut self) {
        let plugin_ids = self
            .registry
            .list()
            .into_iter()
            .filter(|plugin| {
                matches!(
                    plugin.state,
                    PluginState::Enabled
                        | PluginState::Loaded
                        | PluginState::Initialized
                        | PluginState::Running
                )
            })
            .map(|plugin| plugin.id.clone())
            .collect::<Vec<_>>();

        for plugin_id in plugin_ids {
            self.start_plugin(plugin_id.as_str());
        }
    }

    pub fn start_plugin(&mut self, plugin_id: &str) {
        let Some(registered) = self.registry.get(plugin_id) else {
            self.fail_plugin(plugin_id, "plugin is not registered".to_string());
            return;
        };
        if matches!(
            registered.state,
            PluginState::Disabled | PluginState::Failed
        ) {
            return;
        }

        if self.loaded.contains_key(plugin_id) {
            let _ = self.registry.set_state(plugin_id, PluginState::Running);
            return;
        }

        let Some(candidate) = self.candidates.get(plugin_id) else {
            self.fail_plugin(plugin_id, "plugin candidate not found".to_string());
            return;
        };

        let mut plugin = match NativePluginLibrary::load(candidate) {
            Ok(plugin) => plugin,
            Err(error) => {
                self.fail_plugin(plugin_id, error.to_string());
                return;
            }
        };
        let _ = self.registry.set_state(plugin_id, PluginState::Loaded);

        let host_api = self.host_api();
        if let Err(error) = plugin.initialize_with_host_api(plugin_id, host_api) {
            self.fail_plugin(plugin_id, error.to_string());
            return;
        }
        let _ = self.registry.set_state(plugin_id, PluginState::Initialized);

        if let Err(error) = plugin.start() {
            self.fail_plugin(plugin_id, error.to_string());
            return;
        }
        let _ = self.registry.set_state(plugin_id, PluginState::Running);
        self.loaded.insert(plugin_id.to_string(), plugin);
    }

    pub fn drain_queued_lxmf_outbound_requests(&self) -> Vec<PluginLxmfOutboundRequest> {
        self.loaded
            .values()
            .flat_map(NativePluginLibrary::drain_queued_lxmf_outbound_requests)
            .collect()
    }

    pub fn dispatch_event_json(&mut self, event_json: &str) {
        let plugin_ids = self.loaded.keys().cloned().collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.dispatch_event_json_to(plugin_id.as_str(), event_json);
        }
    }

    pub fn dispatch_sanitized_event(&mut self, topic: &str, payload: JsonValue) {
        if !is_sanitized_runtime_topic(topic) {
            self.diagnostics.push(PluginRuntimeDiagnostic {
                plugin_id: None,
                path: None,
                message: format!("unsupported plug-in runtime event topic: {topic}"),
            });
            return;
        }
        let plugin_ids = self.loaded.keys().cloned().collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.dispatch_sanitized_event_to(plugin_id.as_str(), topic, payload.clone());
        }
    }

    pub fn dispatch_sanitized_event_to(
        &mut self,
        plugin_id: &str,
        topic: &str,
        payload: JsonValue,
    ) {
        if !is_sanitized_runtime_topic(topic) {
            self.diagnostics.push(PluginRuntimeDiagnostic {
                plugin_id: Some(plugin_id.to_string()),
                path: self
                    .candidates
                    .get(plugin_id)
                    .map(|candidate| candidate.install_dir.clone()),
                message: format!("unsupported plug-in runtime event topic: {topic}"),
            });
            return;
        }
        let Some(plugin) = self.loaded.get(plugin_id) else {
            return;
        };
        let delivered = plugin.deliver_event(topic, payload.clone());
        match delivered {
            Ok(false) => {}
            Ok(true) => {
                let event = json!({
                    "topic": topic,
                    "payload": payload,
                });
                self.dispatch_event_json_to(plugin_id, event.to_string().as_str());
            }
            Err(error) => self.fail_plugin(plugin_id, error.to_string()),
        }
    }

    pub fn dispatch_lxmf_message_received(&mut self, message: &PluginLxmfMessage) {
        let Some(plugin) = self.registry.get(message.plugin_id.as_str()) else {
            return;
        };
        if !(plugin.manifest.permissions.lxmf_receive && plugin.granted_permissions.lxmf_receive) {
            return;
        }
        let event = json!({
            "topic": "rem.plugin.lxmf.received",
            "payload": {
                "pluginId": message.plugin_id,
                "messageName": message.message_name,
                "wireType": message.wire_type,
                "payload": message.payload,
            }
        });
        self.dispatch_sanitized_event_to(
            message.plugin_id.as_str(),
            "rem.plugin.lxmf.received",
            event["payload"].clone(),
        );
    }

    fn dispatch_event_json_to(&mut self, plugin_id: &str, event_json: &str) {
        let Some(plugin) = self.loaded.get(plugin_id) else {
            return;
        };
        if let Err(error) = plugin.handle_event_json(event_json) {
            self.fail_plugin(plugin_id, error.to_string());
        }
    }

    pub fn stop_all(&mut self) {
        let loaded = std::mem::take(&mut self.loaded);
        for (plugin_id, plugin) in loaded {
            match plugin.stop() {
                Ok(()) => {
                    let _ = self
                        .registry
                        .set_state(plugin_id.as_str(), PluginState::Stopped);
                }
                Err(error) => {
                    self.fail_plugin(plugin_id.as_str(), error.to_string());
                }
            }
        }
    }

    fn fail_plugin(&mut self, plugin_id: &str, message: String) {
        let _ = self.registry.set_state(plugin_id, PluginState::Failed);
        let path = self
            .candidates
            .get(plugin_id)
            .map(|candidate| candidate.library_path.clone());
        self.diagnostics.push(PluginRuntimeDiagnostic {
            plugin_id: Some(plugin_id.to_string()),
            path,
            message,
        });
        self.loaded.remove(plugin_id);
    }

    fn host_api(&self) -> PluginHostApi {
        if let Some(app_state) = self.app_state.as_ref() {
            return PluginHostApi::new_with_message_schemas_and_app_state_store(
                self.registry.clone(),
                self.message_schemas.clone(),
                app_state.clone(),
            );
        }
        PluginHostApi::new_with_message_schemas(self.registry.clone(), self.message_schemas.clone())
    }
}

fn load_message_schemas(
    candidates: &BTreeMap<String, PluginLoadCandidate>,
) -> (PluginMessageSchemaMap, Vec<PluginRuntimeDiagnostic>) {
    let mut schemas = PluginMessageSchemaMap::new();
    let mut diagnostics = Vec::new();
    for candidate in candidates.values() {
        for message in &candidate.manifest.messages {
            let schema_path = candidate.install_dir.join(message.schema.as_str());
            let schema = fs_err::read_to_string(schema_path.as_path())
                .ok()
                .and_then(|source| serde_json::from_str(source.as_str()).ok());
            match schema {
                Some(schema) => {
                    schemas.insert(
                        (candidate.manifest.id.clone(), message.name.clone()),
                        schema,
                    );
                }
                None => diagnostics.push(PluginRuntimeDiagnostic {
                    plugin_id: Some(candidate.manifest.id.clone()),
                    path: Some(schema_path),
                    message: "plugin message schema could not be loaded".to_string(),
                }),
            }
        }
    }
    (schemas, diagnostics)
}

fn is_sanitized_runtime_topic(topic: &str) -> bool {
    SANITIZED_RUNTIME_TOPICS.contains(&topic)
}

fn loader_error_to_diagnostic(error: PluginLoaderError) -> PluginRuntimeDiagnostic {
    match error {
        PluginLoaderError::Io { path, message } => PluginRuntimeDiagnostic {
            plugin_id: None,
            path: Some(path),
            message,
        },
        PluginLoaderError::Manifest { path, source } => PluginRuntimeDiagnostic {
            plugin_id: None,
            path: Some(path),
            message: source.to_string(),
        },
        PluginLoaderError::MissingLibrary { path } => PluginRuntimeDiagnostic {
            plugin_id: None,
            path: Some(path),
            message: "missing native plugin library".to_string(),
        },
        PluginLoaderError::InvalidLibraryPath { path } => PluginRuntimeDiagnostic {
            plugin_id: None,
            path: Some(path),
            message: "native plugin library path escapes plugin directory".to_string(),
        },
    }
}

impl From<NativePluginLoadError> for PluginRuntimeDiagnostic {
    fn from(error: NativePluginLoadError) -> Self {
        Self {
            plugin_id: None,
            path: None,
            message: error.to_string(),
        }
    }
}
