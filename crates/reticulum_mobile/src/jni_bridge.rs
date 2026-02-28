use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use jni::objects::{JClass, JString};
use jni::sys::{jint, jstring};
use jni::JNIEnv;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::node::{EventSubscription, Node};
use crate::types::{
    HubMode, LogLevel, NodeConfig, NodeError, NodeEvent, NodeStatus, PeerState, SendOutcome,
};

const RESULT_OK: jint = 0;
const RESULT_ERR: jint = 1;

#[derive(Default)]
struct BridgeState {
    node: Option<Node>,
    subscription: Option<Arc<EventSubscription>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LastError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeConfigInput {
    name: Option<String>,
    storage_dir: Option<String>,
    tcp_clients: Option<Vec<String>>,
    broadcast: Option<bool>,
    announce_interval_seconds: Option<u32>,
    announce_capabilities: Option<String>,
    hub_mode: Option<String>,
    hub_identity_hash: Option<String>,
    hub_api_base_url: Option<String>,
    hub_api_key: Option<String>,
    hub_refresh_interval_seconds: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendInput {
    destination_hex: String,
    bytes_base64: String,
}

fn bridge_state() -> &'static Mutex<BridgeState> {
    static STATE: OnceLock<Mutex<BridgeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BridgeState::default()))
}

fn last_error() -> &'static Mutex<Option<LastError>> {
    static LAST_ERROR: OnceLock<Mutex<Option<LastError>>> = OnceLock::new();
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

fn set_last_error(code: impl Into<String>, message: impl Into<String>) {
    if let Ok(mut guard) = last_error().lock() {
        *guard = Some(LastError {
            code: code.into(),
            message: message.into(),
        });
    }
}

fn clear_last_error() {
    if let Ok(mut guard) = last_error().lock() {
        *guard = None;
    }
}

fn set_last_node_error(err: NodeError) {
    let code = node_error_code(&err).to_string();
    let message = err.to_string();
    set_last_error(code, message);
}

fn node_error_code(err: &NodeError) -> &'static str {
    match err {
        NodeError::InvalidConfig {} => "InvalidConfig",
        NodeError::IoError {} => "IoError",
        NodeError::NetworkError {} => "NetworkError",
        NodeError::ReticulumError {} => "ReticulumError",
        NodeError::AlreadyRunning {} => "AlreadyRunning",
        NodeError::NotRunning {} => "NotRunning",
        NodeError::Timeout {} => "Timeout",
        NodeError::InternalError {} => "InternalError",
    }
}

fn jstring_to_rust(env: &mut JNIEnv, value: JString) -> Result<String, String> {
    env.get_string(&value)
        .map_err(|e| format!("jni string conversion failed: {e}"))
        .map(|s| s.into())
}

fn make_jstring_or_null(env: &mut JNIEnv, value: String) -> jstring {
    match env.new_string(value) {
        Ok(output) => output.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn parse_hub_mode(value: Option<&str>) -> HubMode {
    match value.unwrap_or("Disabled").trim().to_ascii_lowercase().as_str() {
        "rchlxmf" | "rch_lxmf" => HubMode::RchLxmf {},
        "rchhttp" | "rch_http" => HubMode::RchHttp {},
        _ => HubMode::Disabled {},
    }
}

fn parse_log_level(value: Option<&str>) -> LogLevel {
    match value.unwrap_or("Info").trim().to_ascii_lowercase().as_str() {
        "trace" => LogLevel::Trace {},
        "debug" => LogLevel::Debug {},
        "warn" => LogLevel::Warn {},
        "error" => LogLevel::Error {},
        _ => LogLevel::Info {},
    }
}

fn parse_node_config(input: NodeConfigInput) -> NodeConfig {
    NodeConfig {
        name: input
            .name
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "emergency-ops-mobile".to_string()),
        storage_dir: input.storage_dir.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        tcp_clients: input
            .tcp_clients
            .unwrap_or_default()
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect(),
        broadcast: input.broadcast.unwrap_or(true),
        announce_interval_seconds: input.announce_interval_seconds.unwrap_or(1800).max(1),
        announce_capabilities: input
            .announce_capabilities
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "R3AKT,EMergencyMessages".to_string()),
        hub_mode: parse_hub_mode(input.hub_mode.as_deref()),
        hub_identity_hash: input.hub_identity_hash.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_api_base_url: input.hub_api_base_url.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_api_key: input.hub_api_key.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_refresh_interval_seconds: input.hub_refresh_interval_seconds.unwrap_or(3600).max(1),
    }
}

fn status_to_json(status: NodeStatus) -> String {
    json!({
        "running": status.running,
        "name": status.name,
        "identityHex": status.identity_hex,
        "appDestinationHex": status.app_destination_hex,
        "lxmfDestinationHex": status.lxmf_destination_hex
    })
    .to_string()
}

fn peer_state_to_str(state: PeerState) -> &'static str {
    match state {
        PeerState::Connecting {} => "Connecting",
        PeerState::Connected {} => "Connected",
        PeerState::Disconnected {} => "Disconnected",
    }
}

fn send_outcome_to_str(outcome: SendOutcome) -> &'static str {
    match outcome {
        SendOutcome::SentDirect {} => "SentDirect",
        SendOutcome::SentBroadcast {} => "SentBroadcast",
        SendOutcome::DroppedMissingDestinationIdentity {} => "DroppedMissingDestinationIdentity",
        SendOutcome::DroppedCiphertextTooLarge {} => "DroppedCiphertextTooLarge",
        SendOutcome::DroppedEncryptFailed {} => "DroppedEncryptFailed",
        SendOutcome::DroppedNoRoute {} => "DroppedNoRoute",
    }
}

fn log_level_to_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace {} => "Trace",
        LogLevel::Debug {} => "Debug",
        LogLevel::Info {} => "Info",
        LogLevel::Warn {} => "Warn",
        LogLevel::Error {} => "Error",
    }
}

fn event_to_wire_json(event: NodeEvent) -> String {
    let (event_name, payload) = match event {
        NodeEvent::StatusChanged { status } => (
            "statusChanged",
            json!({
                "status": {
                    "running": status.running,
                    "name": status.name,
                    "identityHex": status.identity_hex,
                    "appDestinationHex": status.app_destination_hex,
                    "lxmfDestinationHex": status.lxmf_destination_hex
                }
            }),
        ),
        NodeEvent::AnnounceReceived {
            destination_hex,
            app_data,
            hops,
            interface_hex,
            received_at_ms,
        } => (
            "announceReceived",
            json!({
                "destinationHex": destination_hex,
                "appData": app_data,
                "hops": hops,
                "interfaceHex": interface_hex,
                "receivedAtMs": received_at_ms
            }),
        ),
        NodeEvent::PeerChanged { change } => (
            "peerChanged",
            json!({
                "change": {
                    "destinationHex": change.destination_hex,
                    "state": peer_state_to_str(change.state),
                    "lastError": change.last_error
                }
            }),
        ),
        NodeEvent::PacketReceived {
            destination_hex,
            bytes,
        } => (
            "packetReceived",
            json!({
                "destinationHex": destination_hex,
                "bytesBase64": BASE64_STANDARD.encode(bytes)
            }),
        ),
        NodeEvent::PacketSent {
            destination_hex,
            bytes,
            outcome,
        } => (
            "packetSent",
            json!({
                "destinationHex": destination_hex,
                "bytesBase64": BASE64_STANDARD.encode(bytes),
                "outcome": send_outcome_to_str(outcome)
            }),
        ),
        NodeEvent::HubDirectoryUpdated {
            destinations,
            received_at_ms,
        } => (
            "hubDirectoryUpdated",
            json!({
                "destinations": destinations,
                "receivedAtMs": received_at_ms
            }),
        ),
        NodeEvent::Log { level, message } => (
            "log",
            json!({
                "level": log_level_to_str(level),
                "message": message
            }),
        ),
        NodeEvent::Error { code, message } => (
            "error",
            json!({
                "code": code,
                "message": message
            }),
        ),
    };

    json!({
        "event": event_name,
        "payload": payload
    })
    .to_string()
}

fn ok_result() -> jint {
    clear_last_error();
    RESULT_OK
}

fn err_result(code: impl Into<String>, message: impl Into<String>) -> jint {
    set_last_error(code, message);
    RESULT_ERR
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_start(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, config_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let input: NodeConfigInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid node config JSON: {e}")),
    };
    let config = parse_node_config(input);

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    if guard.node.is_none() {
        guard.node = Some(Node::new());
    }

    let subscription = {
        let node = match guard.node.as_ref() {
            Some(v) => v,
            None => return err_result("InternalError", "missing node"),
        };
        if let Err(err) = node.start(config) {
            set_last_node_error(err);
            return RESULT_ERR;
        }
        node.subscribe_events()
    };

    guard.subscription = Some(subscription);
    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_stop(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    if let Some(subscription) = guard.subscription.take() {
        subscription.close();
    }

    if let Some(node) = guard.node.as_ref() {
        if let Err(err) = node.stop() {
            set_last_node_error(err);
            return RESULT_ERR;
        }
    }

    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_restart(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, config_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let input: NodeConfigInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid node config JSON: {e}")),
    };
    let config = parse_node_config(input);

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    if guard.node.is_none() {
        guard.node = Some(Node::new());
    }

    let subscription = {
        let node = match guard.node.as_ref() {
            Some(v) => v,
            None => return err_result("InternalError", "missing node"),
        };
        if let Err(err) = node.restart(config) {
            set_last_node_error(err);
            return RESULT_ERR;
        }
        node.subscribe_events()
    };

    guard.subscription = Some(subscription);
    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let status = {
        let guard = match bridge_state().lock() {
            Ok(v) => v,
            Err(_) => {
                set_last_error("InternalError", "bridge lock poisoned");
                return ptr::null_mut();
            }
        };
        if let Some(node) = guard.node.as_ref() {
            node.get_status()
        } else {
            NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
            }
        }
    };

    make_jstring_or_null(&mut env, status_to_json(status))
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_connectPeer(
    mut env: JNIEnv,
    _class: JClass,
    destination_hex: JString,
) -> jint {
    let destination = match jstring_to_rust(&mut env, destination_hex) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.connect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_disconnectPeer(
    mut env: JNIEnv,
    _class: JClass,
    destination_hex: JString,
) -> jint {
    let destination = match jstring_to_rust(&mut env, destination_hex) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.disconnect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_sendJson(
    mut env: JNIEnv,
    _class: JClass,
    send_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, send_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SendInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid send payload: {e}")),
    };
    let bytes = match BASE64_STANDARD.decode(payload.bytes_base64.as_bytes()) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid base64 payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.send_bytes(payload.destination_hex, bytes) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_broadcastBase64(
    mut env: JNIEnv,
    _class: JClass,
    bytes_base64: JString,
) -> jint {
    let encoded = match jstring_to_rust(&mut env, bytes_base64) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let bytes = match BASE64_STANDARD.decode(encoded.as_bytes()) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid base64 payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.broadcast_bytes(bytes) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setAnnounceCapabilities(
    mut env: JNIEnv,
    _class: JClass,
    capability_string: JString,
) -> jint {
    let value = match jstring_to_rust(&mut env, capability_string) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.set_announce_capabilities(value) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setLogLevel(
    mut env: JNIEnv,
    _class: JClass,
    level_string: JString,
) -> jint {
    let value = match jstring_to_rust(&mut env, level_string) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    node.set_log_level(parse_log_level(Some(value.as_str())));
    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_refreshHubDirectory(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.refresh_hub_directory() {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_nextEventJson(
    mut env: JNIEnv,
    _class: JClass,
    timeout_ms: jint,
) -> jstring {
    let subscription = {
        let guard = match bridge_state().lock() {
            Ok(v) => v,
            Err(_) => {
                set_last_error("InternalError", "bridge lock poisoned");
                return ptr::null_mut();
            }
        };
        guard.subscription.clone()
    };

    let Some(subscription) = subscription else {
        return ptr::null_mut();
    };

    let timeout = if timeout_ms < 0 { 0 } else { timeout_ms as u32 };
    let Some(event) = subscription.next(timeout) else {
        return ptr::null_mut();
    };

    make_jstring_or_null(&mut env, event_to_wire_json(event))
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_takeLastErrorJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let value = {
        let mut guard = match last_error().lock() {
            Ok(v) => v,
            Err(_) => return ptr::null_mut(),
        };
        guard.take()
    };

    let Some(value) = value else {
        return ptr::null_mut();
    };

    match serde_json::to_string(&value) {
        Ok(payload) => make_jstring_or_null(&mut env, payload),
        Err(_) => ptr::null_mut(),
    }
}
