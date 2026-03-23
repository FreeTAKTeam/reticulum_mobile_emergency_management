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
    HubMode, LogLevel, LxmfDeliveryStatus, MessageDirection, MessageMethod, MessageState,
    NodeConfig, NodeError, NodeEvent, NodeStatus, PeerAvailabilityState, PeerManagementState,
    PeerState, SendLxmfRequest, SendOutcome, SyncPhase,
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
    fields_base64: Option<String>,
    #[serde(default)]
    use_propagation_node: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendLxmfInput {
    destination_hex: String,
    body_utf8: String,
    title: Option<String>,
    #[serde(default)]
    use_propagation_node: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageIdInput {
    message_id_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionalDestinationInput {
    destination_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncRequestInput {
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageListInput {
    conversation_id: Option<String>,
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
        NodeError::LxmfWireEncodeError {} => "LxmfWireEncodeError",
        NodeError::LxmfMessageIdParseError {} => "LxmfMessageIdParseError",
        NodeError::LxmfPacketTooLarge {} => "LxmfPacketTooLarge",
        NodeError::LxmfPacketBuildError {} => "LxmfPacketBuildError",
        NodeError::EventStreamClosed {} => "EventStreamClosed",
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
    match value
        .unwrap_or("Disabled")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
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

fn peer_management_state_to_str(state: PeerManagementState) -> &'static str {
    match state {
        PeerManagementState::Unmanaged {} => "Unmanaged",
        PeerManagementState::Managed {} => "Managed",
    }
}

fn peer_availability_state_to_str(state: PeerAvailabilityState) -> &'static str {
    match state {
        PeerAvailabilityState::Unseen {} => "Unseen",
        PeerAvailabilityState::Discovered {} => "Discovered",
        PeerAvailabilityState::Resolved {} => "Resolved",
        PeerAvailabilityState::Ready {} => "Ready",
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

fn lxmf_delivery_status_to_str(status: LxmfDeliveryStatus) -> &'static str {
    match status {
        LxmfDeliveryStatus::Sent {} => "Sent",
        LxmfDeliveryStatus::Acknowledged {} => "Acknowledged",
        LxmfDeliveryStatus::Failed {} => "Failed",
        LxmfDeliveryStatus::TimedOut {} => "TimedOut",
    }
}

fn message_method_to_str(method: MessageMethod) -> &'static str {
    match method {
        MessageMethod::Direct {} => "Direct",
        MessageMethod::Opportunistic {} => "Opportunistic",
        MessageMethod::Propagated {} => "Propagated",
        MessageMethod::Resource {} => "Resource",
    }
}

fn message_state_to_str(state: MessageState) -> &'static str {
    match state {
        MessageState::Queued {} => "Queued",
        MessageState::PathRequested {} => "PathRequested",
        MessageState::LinkEstablishing {} => "LinkEstablishing",
        MessageState::Sending {} => "Sending",
        MessageState::SentDirect {} => "SentDirect",
        MessageState::SentToPropagation {} => "SentToPropagation",
        MessageState::Delivered {} => "Delivered",
        MessageState::Failed {} => "Failed",
        MessageState::TimedOut {} => "TimedOut",
        MessageState::Cancelled {} => "Cancelled",
        MessageState::Received {} => "Received",
    }
}

fn message_direction_to_str(direction: MessageDirection) -> &'static str {
    match direction {
        MessageDirection::Inbound {} => "Inbound",
        MessageDirection::Outbound {} => "Outbound",
    }
}

fn sync_phase_to_str(phase: SyncPhase) -> &'static str {
    match phase {
        SyncPhase::Idle {} => "Idle",
        SyncPhase::PathRequested {} => "PathRequested",
        SyncPhase::LinkEstablishing {} => "LinkEstablishing",
        SyncPhase::RequestSent {} => "RequestSent",
        SyncPhase::Receiving {} => "Receiving",
        SyncPhase::Complete {} => "Complete",
        SyncPhase::Failed {} => "Failed",
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
            identity_hex,
            destination_kind,
            app_data,
            hops,
            interface_hex,
            received_at_ms,
        } => (
            "announceReceived",
            json!({
                "destinationHex": destination_hex,
                "identityHex": identity_hex,
                "destinationKind": destination_kind,
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
                    "identityHex": change.identity_hex,
                    "lxmfDestinationHex": change.lxmf_destination_hex,
                    "displayName": change.display_name,
                    "appData": change.app_data,
                    "state": peer_state_to_str(change.state),
                    "managementState": peer_management_state_to_str(change.management_state),
                    "availabilityState": peer_availability_state_to_str(change.availability_state),
                    "activeLink": change.active_link,
                    "lastError": change.last_error,
                    "lastResolutionError": change.last_resolution_error,
                    "lastResolutionAttemptAtMs": change.last_resolution_attempt_at_ms,
                    "lastReadyAtMs": change.last_ready_at_ms,
                    "lastSeenAtMs": change.last_seen_at_ms,
                    "announceLastSeenAtMs": change.announce_last_seen_at_ms,
                    "lxmfLastSeenAtMs": change.lxmf_last_seen_at_ms
                }
            }),
        ),
        NodeEvent::PacketReceived {
            destination_hex,
            source_hex,
            bytes,
            fields_bytes,
        } => (
            "packetReceived",
            json!({
                "destinationHex": destination_hex,
                "sourceHex": source_hex,
                "bytesBase64": BASE64_STANDARD.encode(bytes),
                "fieldsBase64": fields_bytes.map(|bytes| BASE64_STANDARD.encode(bytes))
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
        NodeEvent::LxmfDelivery { update } => (
            "lxmfDelivery",
            json!({
                "messageIdHex": update.message_id_hex,
                "destinationHex": update.destination_hex,
                "sourceHex": update.source_hex,
                "correlationId": update.correlation_id,
                "commandId": update.command_id,
                "commandType": update.command_type,
                "eventUid": update.event_uid,
                "missionUid": update.mission_uid,
                "status": lxmf_delivery_status_to_str(update.status),
                "detail": update.detail,
                "sentAtMs": update.sent_at_ms,
                "updatedAtMs": update.updated_at_ms
            }),
        ),
        NodeEvent::PeerResolved { peer } => (
            "peerResolved",
            json!({
                "destinationHex": peer.destination_hex,
                "identityHex": peer.identity_hex,
                "lxmfDestinationHex": peer.lxmf_destination_hex,
                "displayName": peer.display_name,
                "appData": peer.app_data,
                "state": peer_state_to_str(peer.state),
                "managementState": peer_management_state_to_str(peer.management_state),
                "availabilityState": peer_availability_state_to_str(peer.availability_state),
                "activeLink": peer.active_link,
                "lastResolutionError": peer.last_resolution_error,
                "lastResolutionAttemptAtMs": peer.last_resolution_attempt_at_ms,
                "lastReadyAtMs": peer.last_ready_at_ms,
                "lastSeenAtMs": peer.last_seen_at_ms,
                "announceLastSeenAtMs": peer.announce_last_seen_at_ms,
                "lxmfLastSeenAtMs": peer.lxmf_last_seen_at_ms
            }),
        ),
        NodeEvent::MessageReceived { message } => (
            "messageReceived",
            json!({
                "messageIdHex": message.message_id_hex,
                "conversationId": message.conversation_id,
                "direction": message_direction_to_str(message.direction),
                "destinationHex": message.destination_hex,
                "sourceHex": message.source_hex,
                "title": message.title,
                "bodyUtf8": message.body_utf8,
                "method": message_method_to_str(message.method),
                "state": message_state_to_str(message.state),
                "detail": message.detail,
                "sentAtMs": message.sent_at_ms,
                "receivedAtMs": message.received_at_ms,
                "updatedAtMs": message.updated_at_ms
            }),
        ),
        NodeEvent::MessageUpdated { message } => (
            "messageUpdated",
            json!({
                "messageIdHex": message.message_id_hex,
                "conversationId": message.conversation_id,
                "direction": message_direction_to_str(message.direction),
                "destinationHex": message.destination_hex,
                "sourceHex": message.source_hex,
                "title": message.title,
                "bodyUtf8": message.body_utf8,
                "method": message_method_to_str(message.method),
                "state": message_state_to_str(message.state),
                "detail": message.detail,
                "sentAtMs": message.sent_at_ms,
                "receivedAtMs": message.received_at_ms,
                "updatedAtMs": message.updated_at_ms
            }),
        ),
        NodeEvent::SyncUpdated { status } => (
            "syncUpdated",
            json!({
                "phase": sync_phase_to_str(status.phase),
                "activePropagationNodeHex": status.active_propagation_node_hex,
                "requestedAtMs": status.requested_at_ms,
                "completedAtMs": status.completed_at_ms,
                "messagesReceived": status.messages_received,
                "detail": status.detail
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

fn ok_json_result<T: Serialize>(env: &mut JNIEnv, value: &T) -> jstring {
    clear_last_error();
    match serde_json::to_string(value) {
        Ok(payload) => make_jstring_or_null(env, payload),
        Err(e) => {
            set_last_error("InternalError", format!("JSON serialization failed: {e}"));
            ptr::null_mut()
        }
    }
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_announceNow(
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
    match node.announce_now() {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_requestPeerIdentity(
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
    match node.request_peer_identity(destination) {
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
    let fields_bytes = match payload.fields_base64 {
        Some(encoded) => match BASE64_STANDARD.decode(encoded.as_bytes()) {
            Ok(value) => Some(value),
            Err(e) => {
                return err_result(
                    "InvalidConfig",
                    format!("invalid fields base64 payload: {e}"),
                )
            }
        },
        None => None,
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.send_bytes(
        payload.destination_hex,
        bytes,
        fields_bytes,
        payload.use_propagation_node,
    ) {
        Ok(_) => {
            log::debug!("jni sendJson result=ok");
            ok_result()
        }
        Err(err) => {
            log::error!(
                "jni sendJson result=err code={} message={}",
                node_error_code(&err),
                err
            );
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_sendLxmfJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: SendLxmfInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid lxmf payload: {e}"));
            return ptr::null_mut();
        }
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.send_lxmf(SendLxmfRequest {
        destination_hex: payload.destination_hex,
        body_utf8: payload.body_utf8,
        title: payload.title,
        use_propagation_node: payload.use_propagation_node,
    }) {
        Ok(message_id_hex) => ok_json_result(&mut env, &json!({ "messageIdHex": message_id_hex })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_retryLxmfJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: MessageIdInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid retry payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.retry_lxmf(payload.message_id_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_cancelLxmfJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: MessageIdInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid cancel payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.cancel_lxmf(payload.message_id_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setActivePropagationNodeJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: OptionalDestinationInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result("InvalidConfig", format!("invalid propagation node payload: {e}"))
        }
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.set_active_propagation_node(payload.destination_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_requestLxmfSyncJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SyncRequestInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid sync payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.request_lxmf_sync(payload.limit) {
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listAnnouncesJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.list_announces() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listPeersJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.list_peers() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listConversationsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.list_conversations() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listMessagesJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: MessageListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid message list payload: {e}"));
            return ptr::null_mut();
        }
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.list_messages(payload.conversation_id) {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getLxmfSyncStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.get_lxmf_sync_status() {
        Ok(status) => ok_json_result(&mut env, &status),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
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
