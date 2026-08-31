use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use jni::objects::{GlobalRef, JByteArray, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use rns_transport::iface::rnode_bearer::{RnodeBearerBackend, RnodeBearerInfo, RnodeBearerKind};
use serde::Deserialize;

const MANAGER_CLASS: &str = "network/reticulum/emergency/RNodeAndroidTransportManager";
static ANDROID_JNI: OnceLock<AndroidJniRuntime> = OnceLock::new();
static NEXT_GENERATION: AtomicI64 = AtomicI64::new(1);

struct AndroidJniRuntime {
    vm: JavaVM,
    manager_class: GlobalRef,
}

pub fn install_java_vm(vm: JavaVM) -> Result<(), String> {
    let manager_class = {
        let mut env = vm
            .get_env()
            .map_err(|error| format!("get Android JNI environment: {error}"))?;
        let local_class = env
            .find_class(MANAGER_CLASS)
            .map_err(|error| jni_error(&mut env, error))?;
        env.new_global_ref(local_class)
            .map_err(|error| jni_error(&mut env, error))?
    };
    ANDROID_JNI
        .set(AndroidJniRuntime { vm, manager_class })
        .map_err(|_| "Android JNI runtime is already installed".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidRnodeMode {
    Ble,
    BluetoothClassic,
}

impl AndroidRnodeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ble => "ble",
            Self::BluetoothClassic => "bluetooth_classic",
        }
    }
}

pub struct AndroidRnodeBackend {
    generation: i64,
    mode: AndroidRnodeMode,
    device_id: String,
    open_timeout: Duration,
    read_timeout: Duration,
    write_timeout: Duration,
    negotiated_mtu: Option<u16>,
    opened: bool,
    read_chunks: u64,
    read_bytes: u64,
    write_chunks: u64,
    write_bytes: u64,
}

impl AndroidRnodeBackend {
    #[must_use]
    pub fn new(mode: AndroidRnodeMode, device_id: impl Into<String>) -> Self {
        Self {
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            mode,
            device_id: device_id.into(),
            open_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_millis(250),
            write_timeout: Duration::from_secs(5),
            negotiated_mtu: None,
            opened: false,
            read_chunks: 0,
            read_bytes: 0,
            write_chunks: 0,
            write_bytes: 0,
        }
    }

    #[must_use]
    pub fn generation(&self) -> i64 {
        self.generation
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidOpenResult {
    generation: i64,
    kind: String,
    negotiated_mtu: Option<u16>,
}

impl RnodeBearerBackend for AndroidRnodeBackend {
    async fn open(&mut self) -> Result<RnodeBearerInfo, String> {
        let generation = self.generation;
        let mode = self.mode;
        let device_id = self.device_id.clone();
        let timeout = self.open_timeout;
        let result = tokio::task::spawn_blocking(move || {
            jni_open(generation, mode.as_str(), &device_id, timeout)
        })
        .await
        .map_err(|error| format!("join Android RNode open operation: {error}"))??;
        if result.generation != generation {
            return Err(format!(
                "Android RNode generation mismatch: expected {generation}, got {}",
                result.generation
            ));
        }
        let kind = match result.kind.as_str() {
            "ble" => RnodeBearerKind::Ble,
            "bluetooth_classic" => RnodeBearerKind::BluetoothClassic,
            value => {
                return Err(format!(
                    "Android returned unsupported RNode bearer kind: {value}"
                ))
            }
        };
        self.negotiated_mtu = result.negotiated_mtu;
        self.opened = true;
        log::info!(
            "Android RNode JNI opened generation={} mode={} negotiated_mtu={:?}",
            generation,
            mode.as_str(),
            result.negotiated_mtu
        );
        Ok(RnodeBearerInfo {
            kind,
            negotiated_mtu: result.negotiated_mtu,
        })
    }

    async fn read(&mut self) -> Result<Option<Vec<u8>>, String> {
        let generation = self.generation;
        let timeout = self.read_timeout;
        let result = tokio::task::spawn_blocking(move || jni_read(generation, timeout))
            .await
            .map_err(|error| format!("join Android RNode read operation: {error}"))??;
        if let Some(payload) = result.as_ref() {
            self.read_chunks = self.read_chunks.saturating_add(1);
            self.read_bytes = self
                .read_bytes
                .saturating_add(u64::try_from(payload.len()).unwrap_or(u64::MAX));
            log::debug!(
                "Android RNode JNI read generation={} chunk_bytes={} read_chunks={} read_bytes={}",
                generation,
                payload.len(),
                self.read_chunks,
                self.read_bytes
            );
        }
        Ok(result)
    }

    async fn write(&mut self, payload: Vec<u8>) -> Result<(), String> {
        let generation = self.generation;
        let timeout = self.write_timeout;
        let payload_len = payload.len();
        tokio::task::spawn_blocking(move || jni_write(generation, &payload, timeout))
            .await
            .map_err(|error| format!("join Android RNode write operation: {error}"))??;
        self.write_chunks = self.write_chunks.saturating_add(1);
        self.write_bytes = self
            .write_bytes
            .saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX));
        log::debug!(
            "Android RNode JNI wrote generation={} chunk_bytes={} write_chunks={} write_bytes={}",
            generation,
            payload_len,
            self.write_chunks,
            self.write_bytes
        );
        Ok(())
    }

    async fn close(&mut self) -> Result<(), String> {
        let generation = self.generation;
        self.opened = false;
        self.negotiated_mtu = None;
        tokio::task::spawn_blocking(move || jni_close(generation))
            .await
            .map_err(|error| format!("join Android RNode close operation: {error}"))?
    }
}

fn jni_open(
    generation: i64,
    mode: &str,
    device_id: &str,
    timeout: Duration,
) -> Result<AndroidOpenResult, String> {
    with_env(|env, manager_class| {
        let java_mode = env
            .new_string(mode)
            .map_err(|error| jni_error(env, error))?;
        let java_device = env
            .new_string(device_id)
            .map_err(|error| jni_error(env, error))?;
        let mode_object = JObject::from(java_mode);
        let device_object = JObject::from(java_device);
        let value = env
            .call_static_method(
                manager_class,
                "open",
                "(JLjava/lang/String;Ljava/lang/String;J)Ljava/lang/String;",
                &[
                    JValue::Long(generation),
                    JValue::Object(&mode_object),
                    JValue::Object(&device_object),
                    JValue::Long(duration_millis(timeout)),
                ],
            )
            .map_err(|error| jni_error(env, error))?;
        let object = value.l().map_err(|error| jni_error(env, error))?;
        if object.is_null() {
            return Err("Android RNode open returned no result".to_string());
        }
        let json: String = env
            .get_string(&JString::from(object))
            .map_err(|error| jni_error(env, error))?
            .into();
        serde_json::from_str(&json)
            .map_err(|error| format!("parse Android RNode open result: {error}"))
    })
}

fn jni_read(generation: i64, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
    with_env(|env, manager_class| {
        let value = env
            .call_static_method(
                manager_class,
                "read",
                "(JJ)[B",
                &[
                    JValue::Long(generation),
                    JValue::Long(duration_millis(timeout)),
                ],
            )
            .map_err(|error| jni_error(env, error))?;
        let object = value.l().map_err(|error| jni_error(env, error))?;
        if object.is_null() {
            return Ok(None);
        }
        env.convert_byte_array(JByteArray::from(object))
            .map(Some)
            .map_err(|error| jni_error(env, error))
    })
}

fn jni_write(generation: i64, payload: &[u8], timeout: Duration) -> Result<(), String> {
    with_env(|env, manager_class| {
        let bytes = env
            .byte_array_from_slice(payload)
            .map_err(|error| jni_error(env, error))?;
        let bytes_object = JObject::from(bytes);
        env.call_static_method(
            manager_class,
            "write",
            "(J[BJ)V",
            &[
                JValue::Long(generation),
                JValue::Object(&bytes_object),
                JValue::Long(duration_millis(timeout)),
            ],
        )
        .map_err(|error| jni_error(env, error))?;
        Ok(())
    })
}

fn jni_close(generation: i64) -> Result<(), String> {
    with_env(|env, manager_class| {
        env.call_static_method(manager_class, "close", "(J)V", &[JValue::Long(generation)])
            .map_err(|error| jni_error(env, error))?;
        Ok(())
    })
}

fn with_env<T>(
    operation: impl FnOnce(&mut JNIEnv<'_>, &GlobalRef) -> Result<T, String>,
) -> Result<T, String> {
    let runtime = ANDROID_JNI
        .get()
        .ok_or_else(|| "Android JNI runtime is not initialized".to_string())?;
    let mut env = runtime
        .vm
        .attach_current_thread()
        .map_err(|error| format!("attach Android RNode JNI thread: {error}"))?;
    operation(&mut env, &runtime.manager_class)
}

fn jni_error(env: &mut JNIEnv<'_>, error: jni::errors::Error) -> String {
    let fallback = format!("Android RNode JNI call failed: {error}");
    let Ok(true) = env.exception_check() else {
        return fallback;
    };
    let Ok(throwable) = env.exception_occurred() else {
        return fallback;
    };
    if env.exception_clear().is_err() {
        return fallback;
    }
    let Ok(value) = env.call_method(throwable, "toString", "()Ljava/lang/String;", &[]) else {
        return fallback;
    };
    let Ok(object) = value.l() else {
        return fallback;
    };
    let java_message = JString::from(object);
    let Ok(message) = env.get_string(&java_message) else {
        return fallback;
    };
    message.into()
}

fn duration_millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}
