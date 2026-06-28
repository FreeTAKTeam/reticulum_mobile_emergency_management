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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartRateMeasurement {
    pub bpm: u16,
    pub contact_detected: bool,
}

const REM_PLUGIN_STATUS_OK: i32 = 0;
const REM_PLUGIN_STATUS_ERROR: i32 = 1;
const LATEST_SAMPLE_KEY: *const c_char = c"latest_ble_heart_rate_sample".as_ptr();
const LXMF_RECEIVED_TOPIC: *const c_char = c"rem.plugin.lxmf.received".as_ptr();
const SAMPLE_HEART_RATE_BYTES: &[u8] = &[0b0000_0110, 82];
static SENT_HEART_RATE_SAMPLES: AtomicU64 = AtomicU64::new(0);
static RECEIVED_HEART_RATE_EVENTS: AtomicU64 = AtomicU64::new(0);

static METADATA_WITH_NUL: &[u8] = concat!(
    r#"{"id":"rem.plugin.example_ble_heart_rate","name":"Example BLE Heart Rate Plugin","version":"0.1.0","rem_api_version":">=1.0.0,<2.0.0","abi_major":1,"abi_minor":0}"#,
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
    if event.contains("\"messageName\":\"heart_rate_sample\"") {
        RECEIVED_HEART_RATE_EVENTS.fetch_add(1, Ordering::Relaxed);
    }
    REM_PLUGIN_STATUS_OK
}

#[must_use]
pub fn parse_heart_rate_measurement(bytes: &[u8]) -> Option<HeartRateMeasurement> {
    let flags = *bytes.first()?;
    let bpm_is_u16 = flags & 0b0000_0001 != 0;
    let contact_detected = flags & 0b0000_0100 != 0;
    let bpm = if bpm_is_u16 {
        u16::from_le_bytes([*bytes.get(1)?, *bytes.get(2)?])
    } else {
        u16::from(*bytes.get(1)?)
    };
    if !(1..=240).contains(&bpm) {
        return None;
    }
    Some(HeartRateMeasurement {
        bpm,
        contact_detected,
    })
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
    let measurement = parse_heart_rate_measurement(SAMPLE_HEART_RATE_BYTES).ok_or(())?;
    store_latest_sample(host, measurement)?;
    subscribe_lxmf_received(host)?;
    send_heart_rate_sample(host, measurement)
}

fn store_latest_sample(
    host: &RemPluginHostApi,
    measurement: HeartRateMeasurement,
) -> Result<(), ()> {
    let storage_set = host.storage_set.ok_or(())?;
    let sample_json = CString::new(format!(
        r#"{{"deviceId":"example-ble-hr","bpm":{},"sensorType":"ble_heart_rate","contactDetected":{}}}"#,
        measurement.bpm, measurement.contact_detected
    ))
    .map_err(|_| ())?;
    // SAFETY: The host callback table belongs to this init call; `LATEST_SAMPLE_KEY`
    // is static and `sample_json` lives for the duration of the call.
    let status = unsafe { storage_set(host.ctx, LATEST_SAMPLE_KEY, sample_json.as_ptr()) };
    if status == REM_PLUGIN_STATUS_OK {
        Ok(())
    } else {
        Err(())
    }
}

fn send_heart_rate_sample(
    host: &RemPluginHostApi,
    measurement: HeartRateMeasurement,
) -> Result<(), ()> {
    let send_lxmf = host.send_lxmf.ok_or(())?;
    let request_json = CString::new(format!(
        r#"{{"destinationHex":"aabbccddeeff00112233445566778899","messageName":"heart_rate_sample","payload":{{"deviceId":"example-ble-hr","bpm":{},"sensorType":"ble_heart_rate","contactDetected":{}}},"bodyUtf8":"BLE heart-rate sample: {} bpm","title":"BLE Heart Rate Sample","sendMode":{{"PropagationOnly":{{}}}}}}"#,
        measurement.bpm, measurement.contact_detected, measurement.bpm
    ))
    .map_err(|_| ())?;
    // SAFETY: The host callback table belongs to this init call and the request
    // JSON is a nul-terminated C string that lives for the duration of the call.
    let status = unsafe { send_lxmf(host.ctx, request_json.as_ptr()) };
    if status == REM_PLUGIN_STATUS_OK {
        SENT_HEART_RATE_SAMPLES.fetch_add(1, Ordering::Relaxed);
        Ok(())
    } else {
        Err(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    static STORAGE_SET_CALLED: AtomicBool = AtomicBool::new(false);
    static SUBSCRIBE_CALLED: AtomicBool = AtomicBool::new(false);
    static SEND_LXMF_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn test_storage_set(
        _ctx: *mut c_void,
        key: *const c_char,
        value_json: *const c_char,
    ) -> i32 {
        let key = unsafe { CStr::from_ptr(key) }.to_string_lossy();
        let value_json = unsafe { CStr::from_ptr(value_json) }.to_string_lossy();
        assert_eq!(key.as_ref(), "latest_ble_heart_rate_sample");
        assert!(value_json.contains("\"bpm\":82"));
        assert!(value_json.contains("\"sensorType\":\"ble_heart_rate\""));
        STORAGE_SET_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

    unsafe extern "C" fn test_send_lxmf(_ctx: *mut c_void, request_json: *const c_char) -> i32 {
        let request_json = unsafe { CStr::from_ptr(request_json) }.to_string_lossy();
        assert!(request_json.contains("\"messageName\":\"heart_rate_sample\""));
        assert!(request_json.contains("\"bpm\":82"));
        assert!(request_json.contains("\"bodyUtf8\":\"BLE heart-rate sample: 82 bpm\""));
        SEND_LXMF_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

    unsafe extern "C" fn test_subscribe(_ctx: *mut c_void, topic: *const c_char) -> i32 {
        let topic = unsafe { CStr::from_ptr(topic) }.to_string_lossy();
        assert_eq!(topic.as_ref(), "rem.plugin.lxmf.received");
        SUBSCRIBE_CALLED.store(true, Ordering::Relaxed);
        REM_PLUGIN_STATUS_OK
    }

    #[test]
    fn parses_eight_bit_heart_rate_measurement() {
        assert_eq!(
            parse_heart_rate_measurement(&[0b0000_0110, 82]),
            Some(HeartRateMeasurement {
                bpm: 82,
                contact_detected: true
            })
        );
    }

    #[test]
    fn parses_sixteen_bit_heart_rate_measurement() {
        assert_eq!(
            parse_heart_rate_measurement(&[0b0000_0001, 0x96, 0x00]),
            Some(HeartRateMeasurement {
                bpm: 150,
                contact_detected: false
            })
        );
    }

    #[test]
    fn rejects_missing_or_implausible_measurements() {
        assert_eq!(parse_heart_rate_measurement(&[]), None);
        assert_eq!(parse_heart_rate_measurement(&[0]), None);
        assert_eq!(parse_heart_rate_measurement(&[0, 0]), None);
        assert_eq!(
            parse_heart_rate_measurement(&[0b0000_0001, 0xf1, 0x00]),
            None
        );
    }

    #[test]
    fn handle_event_counts_heart_rate_receive_events() {
        RECEIVED_HEART_RATE_EVENTS.store(0, Ordering::Relaxed);
        let event = CString::new(
            r#"{"topic":"rem.plugin.lxmf.received","payload":{"messageName":"heart_rate_sample"}}"#,
        )
        .expect("event has no interior nul");

        assert_eq!(
            rem_plugin_handle_event(event.as_ptr()),
            REM_PLUGIN_STATUS_OK
        );
        assert_eq!(RECEIVED_HEART_RATE_EVENTS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn init_stores_and_sends_sample_measurement() {
        STORAGE_SET_CALLED.store(false, Ordering::Relaxed);
        SUBSCRIBE_CALLED.store(false, Ordering::Relaxed);
        SEND_LXMF_CALLED.store(false, Ordering::Relaxed);
        SENT_HEART_RATE_SAMPLES.store(0, Ordering::Relaxed);
        let host = RemPluginHostApi {
            abi_major: 1,
            abi_minor: 0,
            ctx: std::ptr::null_mut(),
            storage_get: None,
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
        assert_eq!(SENT_HEART_RATE_SAMPLES.load(Ordering::Relaxed), 1);
    }
}
