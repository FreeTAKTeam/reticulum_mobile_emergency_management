use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPermissions {
    #[serde(default)]
    pub storage_plugin: bool,
    #[serde(default)]
    pub storage_shared: bool,
    #[serde(default)]
    pub messages_read: bool,
    #[serde(default)]
    pub messages_write: bool,
    #[serde(default)]
    pub lxmf_send: bool,
    #[serde(default)]
    pub lxmf_receive: bool,
    #[serde(default)]
    pub notifications_raise: bool,
}
