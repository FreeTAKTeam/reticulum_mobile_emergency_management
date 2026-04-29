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

impl PluginPermissions {
    pub fn intersection(&self, declared: &Self) -> Self {
        Self {
            storage_plugin: self.storage_plugin && declared.storage_plugin,
            storage_shared: self.storage_shared && declared.storage_shared,
            messages_read: self.messages_read && declared.messages_read,
            messages_write: self.messages_write && declared.messages_write,
            lxmf_send: self.lxmf_send && declared.lxmf_send,
            lxmf_receive: self.lxmf_receive && declared.lxmf_receive,
            notifications_raise: self.notifications_raise && declared.notifications_raise,
        }
    }
}
