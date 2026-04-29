use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemPluginAbiVersion {
    pub major: u16,
    pub minor: u16,
}

pub const REM_PLUGIN_ABI_VERSION: RemPluginAbiVersion = RemPluginAbiVersion { major: 1, minor: 0 };

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemPluginStatusCode {
    Ok = 0,
    Error = 1,
    PermissionDenied = 2,
    UnsupportedApi = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEntrypoints {
    pub metadata: String,
    pub init: String,
    pub start: String,
    pub stop: String,
    pub handle_event: String,
}

impl Default for PluginEntrypoints {
    fn default() -> Self {
        Self {
            metadata: "rem_plugin_metadata".to_string(),
            init: "rem_plugin_init".to_string(),
            start: "rem_plugin_start".to_string(),
            stop: "rem_plugin_stop".to_string(),
            handle_event: "rem_plugin_handle_event".to_string(),
        }
    }
}
