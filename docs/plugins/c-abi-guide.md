# C ABI Plug-In Guide

REM Android native plug-ins use a stable C ABI boundary. The host loads the
dynamic library, checks metadata, then calls lifecycle entrypoints.

## Required Exports

```rust
#[no_mangle]
pub extern "C" fn rem_plugin_metadata() -> *const c_char;

#[no_mangle]
pub extern "C" fn rem_plugin_init(host: *const RemPluginHostApi) -> i32;

#[no_mangle]
pub extern "C" fn rem_plugin_start() -> i32;

#[no_mangle]
pub extern "C" fn rem_plugin_stop() -> i32;

#[no_mangle]
pub extern "C" fn rem_plugin_handle_event(event_json: *const c_char) -> i32;
```

Return `0` for success. Non-zero status marks the operation failed.

## Host Callback Table

`rem_plugin_init` receives a v1 callback table:

```rust
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
```

The host owns plug-in identity through `ctx`; plug-ins must not pass their own
ID to bypass permission checks. Do not pass Rust `String`, `Vec`, references,
traits, or closures across the ABI boundary.

## Buffer Ownership

Host-allocated buffers returned by `storage_get` must be released with
`free_buffer`. Borrowed input pointers are valid only during the callback.

## Structured Calls

Structured callback payloads use UTF-8 JSON C strings. `send_lxmf` expects a
JSON object with destination, message name, payload, body, optional title, and
send mode.
