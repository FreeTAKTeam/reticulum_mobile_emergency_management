use std::ffi::c_void;
use std::os::raw::c_char;

use serde::{Deserialize, Serialize};

pub type RemPluginStorageGetFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    key: *const c_char,
    out: *mut RemPluginHostBuffer,
) -> i32;
pub type RemPluginStorageSetFn =
    unsafe extern "C" fn(ctx: *mut c_void, key: *const c_char, value_json: *const c_char) -> i32;
pub type RemPluginSubscribeFn = unsafe extern "C" fn(ctx: *mut c_void, topic: *const c_char) -> i32;
pub type RemPluginPublishEventFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    topic: *const c_char,
    payload_json: *const c_char,
) -> i32;
pub type RemPluginSendLxmfFn =
    unsafe extern "C" fn(ctx: *mut c_void, request_json: *const c_char) -> i32;
pub type RemPluginRaiseNotificationFn =
    unsafe extern "C" fn(ctx: *mut c_void, notification_json: *const c_char) -> i32;
pub type RemPluginFreeBufferFn =
    unsafe extern "C" fn(ctx: *mut c_void, buffer: RemPluginHostBuffer);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RemPluginHostBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RemPluginHostApi {
    pub abi_major: u16,
    pub abi_minor: u16,
    pub ctx: *mut c_void,
    pub storage_get: Option<RemPluginStorageGetFn>,
    pub storage_set: Option<RemPluginStorageSetFn>,
    pub subscribe: Option<RemPluginSubscribeFn>,
    pub publish_event: Option<RemPluginPublishEventFn>,
    pub send_lxmf: Option<RemPluginSendLxmfFn>,
    pub raise_notification: Option<RemPluginRaiseNotificationFn>,
    pub free_buffer: Option<RemPluginFreeBufferFn>,
}

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

impl RemPluginHostApi {
    pub fn unsupported() -> Self {
        Self {
            abi_major: REM_PLUGIN_ABI_VERSION.major,
            abi_minor: REM_PLUGIN_ABI_VERSION.minor,
            ctx: std::ptr::null_mut(),
            storage_get: Some(unsupported_storage_get),
            storage_set: Some(unsupported_storage_set),
            subscribe: Some(unsupported_subscribe),
            publish_event: Some(unsupported_publish_event),
            send_lxmf: Some(unsupported_send_lxmf),
            raise_notification: Some(unsupported_raise_notification),
            free_buffer: Some(unsupported_free_buffer),
        }
    }
}

unsafe extern "C" fn unsupported_storage_get(
    _ctx: *mut c_void,
    _key: *const c_char,
    _out: *mut RemPluginHostBuffer,
) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_storage_set(
    _ctx: *mut c_void,
    _key: *const c_char,
    _value_json: *const c_char,
) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_subscribe(_ctx: *mut c_void, _topic: *const c_char) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_publish_event(
    _ctx: *mut c_void,
    _topic: *const c_char,
    _payload_json: *const c_char,
) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_send_lxmf(_ctx: *mut c_void, _request_json: *const c_char) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_raise_notification(
    _ctx: *mut c_void,
    _notification_json: *const c_char,
) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn unsupported_free_buffer(_ctx: *mut c_void, _buffer: RemPluginHostBuffer) {}

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
