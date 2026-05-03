use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::Mutex;

use libloading::Library;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{
    PluginHostApi, PluginHostError, PluginLoadCandidate, PluginLxmfOutboundRequest,
    RemPluginHostApi, RemPluginHostBuffer, RemPluginStatusCode, REM_PLUGIN_ABI_VERSION,
};
use crate::types::SendMode;

type MetadataFn = unsafe extern "C" fn() -> *const c_char;
type InitFn = unsafe extern "C" fn(*const RemPluginHostApi) -> i32;
type StatusFn = unsafe extern "C" fn() -> i32;
type HandleEventFn = unsafe extern "C" fn(*const c_char) -> i32;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativePluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub rem_api_version: String,
    pub abi_major: u16,
    pub abi_minor: u16,
}

#[derive(Debug, Error)]
pub enum NativePluginLoadError {
    #[error("failed to load native plugin library {path}: {message}")]
    LoadLibrary { path: PathBuf, message: String },
    #[error("missing native plugin symbol {symbol} in {path}: {message}")]
    MissingSymbol {
        path: PathBuf,
        symbol: String,
        message: String,
    },
    #[error("native plugin metadata pointer is null")]
    NullMetadata,
    #[error("native plugin metadata is not valid UTF-8")]
    InvalidMetadataUtf8,
    #[error("native plugin metadata is not valid JSON")]
    InvalidMetadataJson,
    #[error("native plugin metadata id {metadata_id} does not match manifest id {manifest_id}")]
    MetadataIdMismatch {
        manifest_id: String,
        metadata_id: String,
    },
    #[error("native plugin ABI {major}.{minor} is not supported")]
    UnsupportedAbi { major: u16, minor: u16 },
    #[error(
        "native plugin API version {metadata_version} does not match manifest {manifest_version}"
    )]
    ApiVersionMismatch {
        manifest_version: String,
        metadata_version: String,
    },
    #[error("native plugin call {entrypoint} failed with status {status:?}")]
    PluginCallFailed {
        entrypoint: &'static str,
        status: RemPluginStatusCode,
    },
    #[error("native plugin call {entrypoint} returned invalid status code {status}")]
    InvalidStatusCode {
        entrypoint: &'static str,
        status: i32,
    },
    #[error("native plugin event payload contains an interior nul byte")]
    InvalidEventPayload,
}

#[derive(Debug)]
pub struct NativePluginLibrary {
    _library: Library,
    metadata: NativePluginMetadata,
    init: InitFn,
    start: StatusFn,
    stop: StatusFn,
    handle_event: HandleEventFn,
    host_context: Option<Box<NativePluginHostContext>>,
}

#[derive(Debug)]
struct NativePluginHostContext {
    plugin_id: String,
    host_api: Mutex<PluginHostApi>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativePluginSendLxmfRequest {
    destination_hex: String,
    message_name: String,
    payload: JsonValue,
    body_utf8: String,
    title: Option<String>,
    #[serde(default = "default_send_mode")]
    send_mode: SendMode,
}

enum NativePluginCallbackError {
    Error,
    PermissionDenied,
}

impl NativePluginLibrary {
    pub fn load(candidate: &PluginLoadCandidate) -> Result<Self, NativePluginLoadError> {
        // SAFETY: Loading a dynamic library is inherently unsafe because library
        // constructors may run arbitrary code. The caller reaches this point only
        // after manifest path validation has kept the library inside the installed
        // plugin directory.
        let library = unsafe {
            Library::new(candidate.library_path.as_path()).map_err(|error| {
                NativePluginLoadError::LoadLibrary {
                    path: candidate.library_path.clone(),
                    message: error.to_string(),
                }
            })?
        };
        let metadata_symbol = load_symbol::<MetadataFn>(
            &library,
            candidate.library_path.clone(),
            candidate.manifest.entrypoints.metadata.as_str(),
        )?;
        let init = load_symbol::<InitFn>(
            &library,
            candidate.library_path.clone(),
            candidate.manifest.entrypoints.init.as_str(),
        )?;
        let start = load_symbol::<StatusFn>(
            &library,
            candidate.library_path.clone(),
            candidate.manifest.entrypoints.start.as_str(),
        )?;
        let stop = load_symbol::<StatusFn>(
            &library,
            candidate.library_path.clone(),
            candidate.manifest.entrypoints.stop.as_str(),
        )?;
        let handle_event = load_symbol::<HandleEventFn>(
            &library,
            candidate.library_path.clone(),
            candidate.manifest.entrypoints.handle_event.as_str(),
        )?;
        let metadata = read_metadata(metadata_symbol)?;
        validate_metadata(candidate, &metadata)?;

        Ok(Self {
            _library: library,
            metadata,
            init,
            start,
            stop,
            handle_event,
            host_context: None,
        })
    }

    pub fn metadata(&self) -> &NativePluginMetadata {
        &self.metadata
    }

    pub fn initialize(&self) -> Result<(), NativePluginLoadError> {
        let host_api = RemPluginHostApi::unsupported();
        // SAFETY: The function pointer was resolved from the loaded library with
        // the C ABI signature required by REM. The host API pointer is valid for
        // the duration of the call and the plugin must not retain it.
        let status = unsafe { (self.init)(&host_api) };
        status_to_result("init", status)
    }

    pub fn initialize_with_host_api(
        &mut self,
        plugin_id: impl Into<String>,
        host_api: PluginHostApi,
    ) -> Result<(), NativePluginLoadError> {
        self.host_context = Some(Box::new(NativePluginHostContext {
            plugin_id: plugin_id.into(),
            host_api: Mutex::new(host_api),
        }));
        let host_api = self
            .host_context
            .as_mut()
            .expect("host context exists")
            .rem_host_api();
        // SAFETY: The function pointer was resolved from the loaded library with
        // the C ABI signature required by REM. The callback table points at
        // NativePluginLibrary-owned context that remains valid until the plugin
        // library is dropped or reinitialized.
        let status = unsafe { (self.init)(&host_api) };
        match status_to_result("init", status) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.host_context = None;
                Err(error)
            }
        }
    }

    pub fn drain_queued_lxmf_outbound_requests(&self) -> Vec<PluginLxmfOutboundRequest> {
        let Some(context) = self.host_context.as_ref() else {
            return Vec::new();
        };
        context
            .host_api
            .lock()
            .map(|mut host_api| host_api.drain_queued_lxmf_outbound_requests())
            .unwrap_or_default()
    }

    pub fn deliver_event(&self, topic: &str, payload: JsonValue) -> Result<bool, PluginHostError> {
        let Some(context) = self.host_context.as_ref() else {
            return Ok(false);
        };
        context
            .host_api
            .lock()
            .map_err(|_| PluginHostError::PluginNotFound {
                plugin_id: context.plugin_id.clone(),
            })?
            .deliver_event_to_plugin(context.plugin_id.as_str(), topic, payload)
    }

    pub fn start(&self) -> Result<(), NativePluginLoadError> {
        // SAFETY: The function pointer was resolved from the loaded library with
        // the no-argument C ABI lifecycle signature.
        let status = unsafe { (self.start)() };
        status_to_result("start", status)
    }

    pub fn stop(&self) -> Result<(), NativePluginLoadError> {
        // SAFETY: The function pointer was resolved from the loaded library with
        // the no-argument C ABI lifecycle signature.
        let status = unsafe { (self.stop)() };
        status_to_result("stop", status)
    }

    pub fn handle_event_json(&self, event_json: &str) -> Result<(), NativePluginLoadError> {
        let event =
            CString::new(event_json).map_err(|_| NativePluginLoadError::InvalidEventPayload)?;
        // SAFETY: The function pointer was resolved from the loaded library with
        // the C ABI event signature, and the C string pointer remains valid for
        // the duration of the call.
        let status = unsafe { (self.handle_event)(event.as_ptr()) };
        status_to_result("handle_event", status)
    }
}

impl NativePluginHostContext {
    fn rem_host_api(&mut self) -> RemPluginHostApi {
        RemPluginHostApi {
            abi_major: REM_PLUGIN_ABI_VERSION.major,
            abi_minor: REM_PLUGIN_ABI_VERSION.minor,
            ctx: self as *mut Self as *mut c_void,
            storage_get: Some(host_storage_get),
            storage_set: Some(host_storage_set),
            subscribe: Some(host_subscribe),
            publish_event: Some(host_publish_event),
            send_lxmf: Some(host_send_lxmf),
            raise_notification: Some(host_raise_notification),
            free_buffer: Some(host_free_buffer),
        }
    }
}

unsafe extern "C" fn host_storage_get(
    ctx: *mut c_void,
    key: *const c_char,
    out: *mut RemPluginHostBuffer,
) -> i32 {
    callback_status(storage_get_impl(ctx, key, out))
}

fn storage_get_impl(
    ctx: *mut c_void,
    key: *const c_char,
    out: *mut RemPluginHostBuffer,
) -> Result<(), NativePluginCallbackError> {
    if out.is_null() {
        return Err(NativePluginCallbackError::Error);
    }
    // SAFETY: The callback table passes the host-owned context pointer that was
    // created by `NativePluginHostContext::rem_host_api`.
    let context = unsafe { context_from_ptr(ctx)? };
    let key = read_c_string(key)?;
    let value = context
        .host_api
        .lock()
        .map_err(|_| NativePluginCallbackError::Error)?
        .get_plugin_storage(context.plugin_id.as_str(), key.as_str())
        .map_err(NativePluginCallbackError::from)?;
    let buffer = match value {
        Some(value) => host_buffer_from_json(&value)?,
        None => RemPluginHostBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
        },
    };
    // SAFETY: `out` was checked for null and points to caller-provided writable
    // storage for the duration of this callback.
    unsafe {
        *out = buffer;
    }
    Ok(())
}

unsafe extern "C" fn host_storage_set(
    ctx: *mut c_void,
    key: *const c_char,
    value_json: *const c_char,
) -> i32 {
    callback_status(storage_set_impl(ctx, key, value_json))
}

fn storage_set_impl(
    ctx: *mut c_void,
    key: *const c_char,
    value_json: *const c_char,
) -> Result<(), NativePluginCallbackError> {
    // SAFETY: The callback table passes the host-owned context pointer that was
    // created by `NativePluginHostContext::rem_host_api`.
    let context = unsafe { context_from_ptr(ctx)? };
    let key = read_c_string(key)?;
    let value = read_json(value_json)?;
    context
        .host_api
        .lock()
        .map_err(|_| NativePluginCallbackError::Error)?
        .set_plugin_storage(context.plugin_id.as_str(), key.as_str(), value)
        .map_err(NativePluginCallbackError::from)
}

unsafe extern "C" fn host_subscribe(ctx: *mut c_void, topic: *const c_char) -> i32 {
    callback_status(subscribe_impl(ctx, topic))
}

fn subscribe_impl(ctx: *mut c_void, topic: *const c_char) -> Result<(), NativePluginCallbackError> {
    // SAFETY: The callback table passes the host-owned context pointer that was
    // created by `NativePluginHostContext::rem_host_api`.
    let context = unsafe { context_from_ptr(ctx)? };
    let topic = read_c_string(topic)?;
    context
        .host_api
        .lock()
        .map_err(|_| NativePluginCallbackError::Error)?
        .subscribe(context.plugin_id.as_str(), topic.as_str())
        .map_err(NativePluginCallbackError::from)
}

unsafe extern "C" fn host_publish_event(
    ctx: *mut c_void,
    topic: *const c_char,
    payload_json: *const c_char,
) -> i32 {
    callback_status(publish_event_impl(ctx, topic, payload_json))
}

fn publish_event_impl(
    ctx: *mut c_void,
    topic: *const c_char,
    payload_json: *const c_char,
) -> Result<(), NativePluginCallbackError> {
    // SAFETY: The callback table passes the host-owned context pointer that was
    // created by `NativePluginHostContext::rem_host_api`.
    let context = unsafe { context_from_ptr(ctx)? };
    let topic = read_c_string(topic)?;
    let payload = read_json(payload_json)?;
    context
        .host_api
        .lock()
        .map_err(|_| NativePluginCallbackError::Error)?
        .deliver_event(topic.as_str(), payload)
        .map_err(NativePluginCallbackError::from)
}

unsafe extern "C" fn host_send_lxmf(ctx: *mut c_void, request_json: *const c_char) -> i32 {
    callback_status(send_lxmf_impl(ctx, request_json))
}

fn send_lxmf_impl(
    ctx: *mut c_void,
    request_json: *const c_char,
) -> Result<(), NativePluginCallbackError> {
    // SAFETY: The callback table passes the host-owned context pointer that was
    // created by `NativePluginHostContext::rem_host_api`.
    let context = unsafe { context_from_ptr(ctx)? };
    let request_json = read_c_string(request_json)?;
    let request: NativePluginSendLxmfRequest = serde_json::from_str(request_json.as_str())
        .map_err(|_| NativePluginCallbackError::Error)?;
    context
        .host_api
        .lock()
        .map_err(|_| NativePluginCallbackError::Error)?
        .request_lxmf_send_to(
            context.plugin_id.as_str(),
            request.destination_hex.as_str(),
            request.message_name.as_str(),
            request.payload,
            request.body_utf8.as_str(),
            request.title,
            request.send_mode,
        )
        .map(|_| ())
        .map_err(NativePluginCallbackError::from)
}

unsafe extern "C" fn host_raise_notification(
    _ctx: *mut c_void,
    _notification_json: *const c_char,
) -> i32 {
    RemPluginStatusCode::UnsupportedApi as i32
}

unsafe extern "C" fn host_free_buffer(_ctx: *mut c_void, buffer: RemPluginHostBuffer) {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return;
    }
    // SAFETY: Host buffers are allocated from `Box<[u8]>` in
    // `host_buffer_from_json` and are returned with the exact pointer and length.
    // Reconstructing the same boxed slice releases the allocation once.
    unsafe {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            buffer.ptr, buffer.len,
        )));
    }
}

unsafe fn context_from_ptr(
    ctx: *mut c_void,
) -> Result<&'static NativePluginHostContext, NativePluginCallbackError> {
    if ctx.is_null() {
        return Err(NativePluginCallbackError::Error);
    }
    // SAFETY: The caller guarantees `ctx` is the host-owned pointer created from
    // a boxed NativePluginHostContext and that the box outlives this callback.
    Ok(unsafe { &*(ctx as *const NativePluginHostContext) })
}

fn read_c_string(ptr: *const c_char) -> Result<String, NativePluginCallbackError> {
    if ptr.is_null() {
        return Err(NativePluginCallbackError::Error);
    }
    // SAFETY: Callback string arguments must be valid NUL-terminated C strings
    // for the duration of the callback.
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(str::to_string)
        .map_err(|_| NativePluginCallbackError::Error)
}

fn read_json(ptr: *const c_char) -> Result<JsonValue, NativePluginCallbackError> {
    let value = read_c_string(ptr)?;
    serde_json::from_str(value.as_str()).map_err(|_| NativePluginCallbackError::Error)
}

fn host_buffer_from_json(
    value: &JsonValue,
) -> Result<RemPluginHostBuffer, NativePluginCallbackError> {
    let bytes = serde_json::to_vec(value).map_err(|_| NativePluginCallbackError::Error)?;
    let mut bytes = bytes.into_boxed_slice();
    let buffer = RemPluginHostBuffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    Ok(buffer)
}

fn callback_status(result: Result<(), NativePluginCallbackError>) -> i32 {
    match result {
        Ok(()) => RemPluginStatusCode::Ok as i32,
        Err(NativePluginCallbackError::PermissionDenied) => {
            RemPluginStatusCode::PermissionDenied as i32
        }
        Err(NativePluginCallbackError::Error) => RemPluginStatusCode::Error as i32,
    }
}

fn default_send_mode() -> SendMode {
    SendMode::Auto {}
}

impl From<PluginHostError> for NativePluginCallbackError {
    fn from(error: PluginHostError) -> Self {
        match error {
            PluginHostError::PermissionDenied { .. } => Self::PermissionDenied,
            PluginHostError::Storage(_)
            | PluginHostError::PluginNotFound { .. }
            | PluginHostError::LxmfMessage(_) => Self::Error,
        }
    }
}

fn load_symbol<T: Copy>(
    library: &Library,
    path: PathBuf,
    symbol: &str,
) -> Result<T, NativePluginLoadError> {
    // SAFETY: The requested symbol type is constrained to the REM C ABI function
    // pointer signatures. The copied function pointer remains valid while
    // NativePluginLibrary owns the Library handle.
    unsafe {
        library
            .get::<T>(symbol.as_bytes())
            .map(|symbol| *symbol)
            .map_err(|error| NativePluginLoadError::MissingSymbol {
                path,
                symbol: symbol.to_string(),
                message: error.to_string(),
            })
    }
}

fn read_metadata(
    metadata_symbol: MetadataFn,
) -> Result<NativePluginMetadata, NativePluginLoadError> {
    // SAFETY: The function pointer was resolved from the loaded library with the
    // C ABI metadata signature. The plugin contract requires a non-null pointer
    // to a NUL-terminated static JSON string.
    let ptr = unsafe { metadata_symbol() };
    if ptr.is_null() {
        return Err(NativePluginLoadError::NullMetadata);
    }
    // SAFETY: The plugin contract requires the returned pointer to reference a
    // valid NUL-terminated string for at least the loaded library lifetime.
    let source = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| NativePluginLoadError::InvalidMetadataUtf8)?;
    serde_json::from_str(source).map_err(|_| NativePluginLoadError::InvalidMetadataJson)
}

fn validate_metadata(
    candidate: &PluginLoadCandidate,
    metadata: &NativePluginMetadata,
) -> Result<(), NativePluginLoadError> {
    if metadata.id != candidate.manifest.id {
        return Err(NativePluginLoadError::MetadataIdMismatch {
            manifest_id: candidate.manifest.id.clone(),
            metadata_id: metadata.id.clone(),
        });
    }
    if metadata.abi_major != REM_PLUGIN_ABI_VERSION.major {
        return Err(NativePluginLoadError::UnsupportedAbi {
            major: metadata.abi_major,
            minor: metadata.abi_minor,
        });
    }
    if metadata.rem_api_version != candidate.manifest.rem_api_version {
        return Err(NativePluginLoadError::ApiVersionMismatch {
            manifest_version: candidate.manifest.rem_api_version.clone(),
            metadata_version: metadata.rem_api_version.clone(),
        });
    }
    Ok(())
}

fn status_to_result(entrypoint: &'static str, status: i32) -> Result<(), NativePluginLoadError> {
    let status = match status {
        0 => RemPluginStatusCode::Ok,
        1 => RemPluginStatusCode::Error,
        2 => RemPluginStatusCode::PermissionDenied,
        3 => RemPluginStatusCode::UnsupportedApi,
        _ => {
            return Err(NativePluginLoadError::InvalidStatusCode { entrypoint, status });
        }
    };
    if status == RemPluginStatusCode::Ok {
        return Ok(());
    }
    Err(NativePluginLoadError::PluginCallFailed { entrypoint, status })
}
