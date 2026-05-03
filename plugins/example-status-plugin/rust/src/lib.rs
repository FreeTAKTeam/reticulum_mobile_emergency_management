#![allow(unsafe_code)]

use std::ffi::{CStr, c_char};
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
pub struct RemPluginHostApi {
    pub abi_major: u16,
    pub abi_minor: u16,
}

const REM_PLUGIN_STATUS_OK: i32 = 0;
static RECEIVED_STATUS_TEST_EVENTS: AtomicU64 = AtomicU64::new(0);

static METADATA_WITH_NUL: &[u8] = concat!(
    r#"{"id":"rem.plugin.example_status","name":"Example Status Plugin","version":"0.1.0","rem_api_version":">=1.0.0,<2.0.0","abi_major":1,"abi_minor":0}"#,
    "\0",
)
.as_bytes();

#[unsafe(no_mangle)]
pub extern "C" fn rem_plugin_metadata() -> *const c_char {
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
pub extern "C" fn rem_plugin_handle_event(event_json: *const c_char) -> i32 {
    let Some(event) = event_json_to_str(event_json) else {
        return REM_PLUGIN_STATUS_OK;
    };
    if event.contains("\"topic\":\"rem.plugin.lxmf.received\"")
        && event.contains("\"messageName\":\"status_test\"")
    {
        RECEIVED_STATUS_TEST_EVENTS.fetch_add(1, Ordering::Relaxed);
    }
    REM_PLUGIN_STATUS_OK
}

fn event_json_to_str(event_json: *const c_char) -> Option<String> {
    if event_json.is_null() {
        return None;
    }
    // SAFETY: REM passes a non-null, nul-terminated event JSON pointer for the
    // duration of this C ABI call. The example plug-in does not retain it.
    unsafe { CStr::from_ptr(event_json) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn handle_event_counts_status_test_receive_events() {
        RECEIVED_STATUS_TEST_EVENTS.store(0, Ordering::Relaxed);
        let event = CString::new(
            r#"{"topic":"rem.plugin.lxmf.received","payload":{"messageName":"status_test"}}"#,
        )
        .expect("event has no interior nul");

        assert_eq!(
            rem_plugin_handle_event(event.as_ptr()),
            REM_PLUGIN_STATUS_OK
        );
        assert_eq!(RECEIVED_STATUS_TEST_EVENTS.load(Ordering::Relaxed), 1);
    }
}
