#![allow(unsafe_code)]

#[repr(C)]
pub struct RemPluginHostApi {
    pub abi_major: u16,
    pub abi_minor: u16,
}

const REM_PLUGIN_STATUS_OK: i32 = 0;

static METADATA_WITH_NUL: &[u8] = concat!(
    r#"{"id":"rem.plugin.example_status","name":"Example Status Plugin","version":"0.1.0","rem_api_version":">=1.0.0,<2.0.0","abi_major":1,"abi_minor":0}"#,
    "\0",
)
.as_bytes();

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_metadata() -> *const std::ffi::c_char {
    METADATA_WITH_NUL.as_ptr().cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_init(_host: *const RemPluginHostApi) -> i32 {
    REM_PLUGIN_STATUS_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_start() -> i32 {
    REM_PLUGIN_STATUS_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_stop() -> i32 {
    REM_PLUGIN_STATUS_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_handle_event(_event_json: *const std::ffi::c_char) -> i32 {
    REM_PLUGIN_STATUS_OK
}
