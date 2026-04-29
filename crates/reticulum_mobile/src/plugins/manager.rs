use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

use super::{
    NativePluginLibrary, NativePluginLoadError, PersistedPluginRegistry, PluginLoadCandidate,
    PluginLoader, PluginLoaderError, PluginRegistry, PluginRegistryError, PluginState,
};

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
    diagnostics: Vec<PluginRuntimeDiagnostic>,
}

impl NativePluginRuntime {
    pub fn discover(
        install_root: impl Into<PathBuf>,
        android_abi: &str,
        persisted: Option<&PersistedPluginRegistry>,
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

        let mut registry = PluginRegistry::from_manifests(manifests)?;
        if let Some(persisted) = persisted {
            registry.apply_persisted_state(persisted);
        }
        diagnostics.shrink_to_fit();

        Ok(Self {
            registry,
            candidates,
            loaded: BTreeMap::new(),
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

        let plugin = match NativePluginLibrary::load(candidate) {
            Ok(plugin) => plugin,
            Err(error) => {
                self.fail_plugin(plugin_id, error.to_string());
                return;
            }
        };
        let _ = self.registry.set_state(plugin_id, PluginState::Loaded);

        if let Err(error) = plugin.initialize() {
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

    pub fn dispatch_event_json(&mut self, event_json: &str) {
        let plugin_ids = self.loaded.keys().cloned().collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            let Some(plugin) = self.loaded.get(plugin_id.as_str()) else {
                continue;
            };
            if let Err(error) = plugin.handle_event_json(event_json) {
                self.fail_plugin(plugin_id.as_str(), error.to_string());
            }
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
