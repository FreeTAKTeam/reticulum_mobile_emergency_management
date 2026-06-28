use std::collections::{btree_map::Entry, BTreeMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{PluginManifest, PluginPermissions};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    Discovered,
    Disabled,
    Enabled,
    Loaded,
    Initialized,
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPlugin {
    pub id: String,
    pub manifest: PluginManifest,
    pub state: PluginState,
    pub granted_permissions: PluginPermissions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedPluginRegistry {
    pub plugins: BTreeMap<String, PersistedPluginState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedPluginState {
    pub state: PluginState,
    pub granted_permissions: PluginPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginRegistryError {
    #[error("duplicate plugin id: {plugin_id}")]
    DuplicatePluginId { plugin_id: String },
    #[error("plugin not found: {plugin_id}")]
    PluginNotFound { plugin_id: String },
    #[error("plugin registry I/O failed: {message}")]
    Io { message: String },
    #[error("plugin registry JSON is invalid")]
    InvalidJson,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, RegisteredPlugin>,
}

impl PluginRegistry {
    pub fn from_manifests(manifests: Vec<PluginManifest>) -> Result<Self, PluginRegistryError> {
        let mut registry = Self::default();
        for manifest in manifests {
            let id = manifest.id.clone();
            match registry.plugins.entry(id.clone()) {
                Entry::Vacant(slot) => {
                    slot.insert(RegisteredPlugin {
                        id,
                        manifest,
                        state: PluginState::Disabled,
                        granted_permissions: PluginPermissions::default(),
                    });
                }
                Entry::Occupied(_) => {
                    return Err(PluginRegistryError::DuplicatePluginId { plugin_id: id });
                }
            }
        }
        Ok(registry)
    }

    pub fn list(&self) -> Vec<&RegisteredPlugin> {
        self.plugins.values().collect()
    }

    pub fn get(&self, plugin_id: &str) -> Option<&RegisteredPlugin> {
        self.plugins.get(plugin_id)
    }

    pub fn enable(&mut self, plugin_id: &str) -> Result<(), PluginRegistryError> {
        self.set_state(plugin_id, PluginState::Enabled)
    }

    pub fn disable(&mut self, plugin_id: &str) -> Result<(), PluginRegistryError> {
        self.set_state(plugin_id, PluginState::Disabled)
    }

    pub fn grant_permissions(
        &mut self,
        plugin_id: &str,
        update: impl FnOnce(&mut PluginPermissions),
    ) -> Result<(), PluginRegistryError> {
        let plugin =
            self.plugins
                .get_mut(plugin_id)
                .ok_or_else(|| PluginRegistryError::PluginNotFound {
                    plugin_id: plugin_id.to_string(),
                })?;
        let mut requested = plugin.granted_permissions.clone();
        update(&mut requested);
        plugin.granted_permissions = requested.intersection(&plugin.manifest.permissions);
        Ok(())
    }

    pub fn to_persisted_state(&self) -> PersistedPluginRegistry {
        PersistedPluginRegistry {
            plugins: self
                .plugins
                .iter()
                .map(|(id, plugin)| {
                    (
                        id.clone(),
                        PersistedPluginState {
                            state: plugin.state,
                            granted_permissions: plugin.granted_permissions.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn apply_persisted_state(&mut self, persisted: &PersistedPluginRegistry) {
        for (plugin_id, persisted_plugin) in &persisted.plugins {
            let Some(plugin) = self.plugins.get_mut(plugin_id) else {
                continue;
            };
            plugin.state = persisted_plugin.state;
            plugin.granted_permissions = persisted_plugin
                .granted_permissions
                .intersection(&plugin.manifest.permissions);
        }
    }

    pub(crate) fn set_state(
        &mut self,
        plugin_id: &str,
        state: PluginState,
    ) -> Result<(), PluginRegistryError> {
        let plugin =
            self.plugins
                .get_mut(plugin_id)
                .ok_or_else(|| PluginRegistryError::PluginNotFound {
                    plugin_id: plugin_id.to_string(),
                })?;
        plugin.state = state;
        Ok(())
    }
}

impl PersistedPluginRegistry {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, PluginRegistryError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let source = fs_err::read_to_string(path).map_err(registry_io_error)?;
        serde_json::from_str(source.as_str()).map_err(|_| PluginRegistryError::InvalidJson)
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), PluginRegistryError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent).map_err(registry_io_error)?;
        }
        let source =
            serde_json::to_string_pretty(self).map_err(|_| PluginRegistryError::InvalidJson)?;
        fs_err::write(path, source).map_err(registry_io_error)?;
        Ok(())
    }
}

fn registry_io_error(error: std::io::Error) -> PluginRegistryError {
    PluginRegistryError::Io {
        message: error.to_string(),
    }
}
