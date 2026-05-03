#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RemPluginHostBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

type StorageGetFn =
    unsafe extern "C" fn(*mut c_void, *const c_char, *mut RemPluginHostBuffer) -> i32;
type StorageSetFn = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> i32;
type SubscribeFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> i32;
type PublishEventFn = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> i32;
type SendLxmfFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> i32;
type RaiseNotificationFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> i32;
type FreeBufferFn = unsafe extern "C" fn(*mut c_void, RemPluginHostBuffer);

#[repr(C)]
pub struct RemPluginHostApi {
    pub abi_major: u16,
    pub abi_minor: u16,
    pub ctx: *mut c_void,
    pub storage_get: Option<StorageGetFn>,
    pub storage_set: Option<StorageSetFn>,
    pub subscribe: Option<SubscribeFn>,
    pub publish_event: Option<PublishEventFn>,
    pub send_lxmf: Option<SendLxmfFn>,
    pub raise_notification: Option<RaiseNotificationFn>,
    pub free_buffer: Option<FreeBufferFn>,
}

const REM_PLUGIN_STATUS_OK: i32 = 0;
const REM_PLUGIN_STATUS_ERROR: i32 = 1;
const COUNTER_KEY: *const c_char = c"status_send_count".as_ptr();
const LXMF_RECEIVED_TOPIC: *const c_char = c"rem.plugin.lxmf.received".as_ptr();
const STATUS_SEND_REQUEST: *const c_char = c"{\"destinationHex\":\"aabbccddeeff00112233445566778899\",\"messageName\":\"status_test\",\"payload\":{\"message\":\"Status test from example plug-in\"},\"bodyUtf8\":\"Status test from example plug-in\",\"title\":\"Status Test\",\"sendMode\":{\"PropagationOnly\":{}}}".as_ptr();
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
pub extern "C" fn rem_plugin_init(host: *const RemPluginHostApi) -> i32 {
    let Some(host) = host_api(host) else {
        return REM_PLUGIN_STATUS_ERROR;
    };
    if initialize_with_host(host).is_ok() {
        REM_PLUGIN_STATUS_OK
    } else {
        REM_PLUGIN_STATUS_ERROR
    }
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

fn host_api<'a>(host: *const RemPluginHostApi) -> Option<&'a RemPluginHostApi> {
    if host.is_null() {
        return None;
    }
    // SAFETY: REM calls `rem_plugin_init` with a non-null callback table pointer
    // that remains valid only for the duration of this call. The example uses it
    // immediately and does not retain it.
    Some(unsafe { &*host })
}

fn initialize_with_host(host: &RemPluginHostApi) -> Result<(), ()> {
    increment_counter(host)?;
    subscribe_lxmf_received(host)?;
    send_status_test(host)
}

fn increment_counter(host: &RemPluginHostApi) -> Result<(), ()> {
    let storage_get = host.storage_get.ok_or(())?;
    let storage_set = host.storage_set.ok_or(())?;
    let mut buffer = RemPluginHostBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    // SAFETY: The host callback table belongs to this init call; `COUNTER_KEY`
    // is a static nul-terminated C string and `buffer` is writable output.
    let status = unsafe { storage_get(host.ctx, COUNTER_KEY, &raw mut buffer) };
    if status != REM_PLUGIN_STATUS_OK {
        return Err(());
    }
    let next = counter_from_buffer(host, buffer).saturating_add(1);
    let json = CString::new(next.to_string()).map_err(|_| ())?;
    // SAFETY: The host callback table belongs to this init call; both C string
    // pointers are static and nul-terminated.
    let status = unsafe { storage_set(host.ctx, COUNTER_KEY, json.as_ptr()) };
    if status == REM_PLUGIN_STATUS_OK {
        Ok(())
    } else {
        Err(())
    }
}

fn counter_from_buffer(host: &RemPluginHostApi, buffer: RemPluginHostBuffer) -> u64 {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return 0;
    }
    // SAFETY: REM returned a host-owned buffer with `len` initialized bytes. The
    // example reads it immediately, then releases it through `free_buffer`.
    let value = unsafe { std::slice::from_raw_parts(buffer.ptr.cast_const(), buffer.len) };
    let counter = std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    if let Some(free_buffer) = host.free_buffer {
        // SAFETY: The buffer was allocated by REM for this host API table and is
        // released exactly once after the example has copied out the counter.
        unsafe {
            free_buffer(host.ctx, buffer);
        }
    }
    counter
}

fn subscribe_lxmf_received(host: &RemPluginHostApi) -> Result<(), ()> {
    let subscribe = host.subscribe.ok_or(())?;
    // SAFETY: The host callback table belongs to this init call and the topic is
    // a static nul-terminated C string.
    let status = unsafe { subscribe(host.ctx, LXMF_RECEIVED_TOPIC) };
    if status == REM_PLUGIN_STATUS_OK {
        Ok(())
    } else {
        Err(())
    }
}

fn send_status_test(host: &RemPluginHostApi) -> Result<(), ()> {
    let send_lxmf = host.send_lxmf.ok_or(())?;
    // SAFETY: The host callback table belongs to this init call and the request
    // JSON is a static nul-terminated C string.
    let status = unsafe { send_lxmf(host.ctx, STATUS_SEND_REQUEST) };
    if status == REM_PLUGIN_STATUS_OK {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    static STORAGE_SET_CALLED: AtomicBool = AtomicBool::new(false);
    static SUBSCRIBE_CALLED: AtomicBool = AtomicBool::new(false);
    static SEND_LXMF_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn test_storage_get(
        _ctx: *mut c_void,
        key: *const c_char,
        out: *mut RemPluginHostBuffer,
    ) -> i32 {
        let key = unsafe { CStr::from_ptr(key) }.to_string_lossy();
        assert_eq!(key.as_ref(), "status_send_count");
        unsafe {
            *out = RemPluginHostBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
            };
        }
        REM_PLUGIN_STATUS_OK
    }

    unsafe extern "C" fn test_storage_set(
        _ctx: *mut c_void,
        key: *const c_char,
        value_json: *const c_char,
    ) -> i32 {
        let key = unsafe { CStr::from_ptr(key) }.to_string_lossy();
        let value_json = unsafe { CStr::from_ptr(value_json) }.to_string_lossy();
        assert_eq!(key.as_ref(), "status_send_count");
        assert_eq!(value_json.as_ref(), "1");
        STORAGE_SET_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

    unsafe extern "C" fn test_subscribe(_ctx: *mut c_void, topic: *const c_char) -> i32 {
        let topic = unsafe { CStr::from_ptr(topic) }.to_string_lossy();
        assert_eq!(topic.as_ref(), "rem.plugin.lxmf.received");
        SUBSCRIBE_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

    unsafe extern "C" fn test_send_lxmf(_ctx: *mut c_void, request_json: *const c_char) -> i32 {
        let request_json = unsafe { CStr::from_ptr(request_json) }.to_string_lossy();
        assert!(request_json.contains("\"messageName\":\"status_test\""));
        assert!(request_json.contains("\"message\":\"Status test from example plug-in\""));
        SEND_LXMF_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

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

    #[test]
    fn init_uses_host_callbacks_for_storage_subscription_and_send() {
        STORAGE_SET_CALLED.store(false, Ordering::Relaxed);
        SUBSCRIBE_CALLED.store(false, Ordering::Relaxed);
        SEND_LXMF_CALLED.store(false, Ordering::Relaxed);
        let host = RemPluginHostApi {
            abi_major: 1,
            abi_minor: 0,
            ctx: std::ptr::null_mut(),
            storage_get: Some(test_storage_get),
            storage_set: Some(test_storage_set),
            subscribe: Some(test_subscribe),
            publish_event: None,
            send_lxmf: Some(test_send_lxmf),
            raise_notification: None,
            free_buffer: None,
        };

        assert_eq!(rem_plugin_init(&host), REM_PLUGIN_STATUS_OK);
        assert!(STORAGE_SET_CALLED.load(Ordering::Relaxed));
        assert!(SUBSCRIBE_CALLED.load(Ordering::Relaxed));
        assert!(SEND_LXMF_CALLED.load(Ordering::Relaxed));
    }
}
