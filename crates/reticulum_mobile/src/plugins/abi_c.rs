use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;

use libloading::Library;
use serde::Deserialize;
use thiserror::Error;

use super::{PluginLoadCandidate, RemPluginHostApi, RemPluginStatusCode, REM_PLUGIN_ABI_VERSION};

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
