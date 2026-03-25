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
    AppSettingsRecord, EamProjectionRecord, EventProjectionRecord, HubMode,
    HubSettingsRecord, LegacyImportPayload, LogLevel, LxmfDeliveryMethod,
    LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfFallbackStage, MessageDirection,
    MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent, NodeStatus,
    PeerAvailabilityState, PeerManagementState, PeerChange, PeerRecord, PeerState,
    ProjectionScope, SavedPeerRecord, SendLxmfRequest, SendMode, SendOutcome, SyncPhase,
    TelemetryPositionRecord, TelemetrySettingsRecord,
};

const RESULT_OK: jint = 0;
const RESULT_ERR: jint = 1;

#[derive(Default)]
struct BridgeState {
    node: Option<Node>,
    subscription: Option<Arc<EventSubscription>>,
}

fn ensure_node(guard: &mut BridgeState) -> &Node {
    guard.node.get_or_insert_with(Node::new)
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
    stale_after_minutes: Option<u32>,
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
    send_mode: Option<String>,
    #[serde(default)]
    use_propagation_node: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendLxmfInput {
    destination_hex: String,
    body_utf8: String,
    title: Option<String>,
    send_mode: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyImportInput {
    settings: Option<AppSettingsInput>,
    saved_peers: Option<Vec<SavedPeerInput>>,
    eams: Option<Vec<EamProjectionInput>>,
    events: Option<Vec<EventProjectionInput>>,
    messages: Option<Vec<MessageRecordInput>>,
    telemetry_positions: Option<Vec<TelemetryPositionInput>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsInput {
    display_name: String,
    auto_connect_saved: bool,
    announce_capabilities: String,
    tcp_clients: Vec<String>,
    broadcast: bool,
    announce_interval_seconds: u32,
    telemetry: TelemetrySettingsInput,
    hub: HubSettingsInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HubSettingsInput {
    mode: String,
    identity_hash: String,
    api_base_url: String,
    api_key: String,
    refresh_interval_seconds: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetrySettingsInput {
    enabled: bool,
    publish_interval_seconds: u32,
    accuracy_threshold_meters: Option<f64>,
    stale_after_minutes: u32,
    expire_after_minutes: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPeerInput {
    destination: String,
    label: Option<String>,
    saved_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EamSourceInput {
    rns_identity: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EamProjectionInput {
    callsign: String,
    group_name: String,
    security_status: String,
    capability_status: String,
    preparedness_status: String,
    medical_status: String,
    mobility_status: String,
    comms_status: String,
    notes: Option<String>,
    updated_at: u64,
    deleted_at: Option<u64>,
    eam_uid: Option<String>,
    team_member_uid: Option<String>,
    team_uid: Option<String>,
    reported_at: Option<String>,
    reported_by: Option<String>,
    overall_status: Option<String>,
    confidence: Option<f64>,
    ttl_seconds: Option<u64>,
    source: Option<EamSourceInput>,
    sync_state: Option<String>,
    sync_error: Option<String>,
    draft_created_at: Option<u64>,
    last_synced_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventProjectionInput {
    uid: String,
    command_id: String,
    source_identity: String,
    source_display_name: Option<String>,
    timestamp: String,
    command_type: String,
    mission_uid: String,
    content: String,
    callsign: String,
    server_time: Option<String>,
    client_time: Option<String>,
    keywords: Vec<String>,
    content_hashes: Vec<String>,
    updated_at: u64,
    deleted_at: Option<u64>,
    correlation_id: Option<String>,
    topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageRecordInput {
    message_id_hex: String,
    conversation_id: String,
    direction: String,
    destination_hex: String,
    source_hex: Option<String>,
    title: Option<String>,
    body_utf8: String,
    method: String,
    state: String,
    detail: Option<String>,
    sent_at: Option<u64>,
    received_at: Option<u64>,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryPositionInput {
    callsign: String,
    lat: f64,
    lon: f64,
    alt: Option<f64>,
    course: Option<f64>,
    speed: Option<f64>,
    accuracy: Option<f64>,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEamInput {
    callsign: String,
    deleted_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEventInput {
    uid: String,
    deleted_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPeersPayload {
    saved_peers: Vec<SavedPeerInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamUidInput {
    team_uid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallsignInput {
    callsign: String,
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
        stale_after_minutes: input.stale_after_minutes.unwrap_or(30).max(1),
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

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_initializeStorage(
    mut env: JNIEnv,
    _class: JClass,
    storage_dir: JString,
) -> jint {
    clear_last_error();
    let raw = match jstring_to_rust(&mut env, storage_dir) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return RESULT_ERR;
        }
    };

    let storage_dir = {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(guard) => guard,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return RESULT_ERR;
        }
    };

    let node = ensure_node(&mut guard);
    match node.initialize_storage(storage_dir.as_deref()) {
        Ok(()) => RESULT_OK,
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

fn parse_message_direction(value: &str) -> Result<MessageDirection, NodeError> {
    match value.trim() {
        "Inbound" => Ok(MessageDirection::Inbound {}),
        "Outbound" => Ok(MessageDirection::Outbound {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_message_method(value: &str) -> Result<MessageMethod, NodeError> {
    match value.trim() {
        "Direct" => Ok(MessageMethod::Direct {}),
        "Opportunistic" => Ok(MessageMethod::Opportunistic {}),
        "Propagated" => Ok(MessageMethod::Propagated {}),
        "Resource" => Ok(MessageMethod::Resource {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_message_state(value: &str) -> Result<MessageState, NodeError> {
    match value.trim() {
        "Queued" => Ok(MessageState::Queued {}),
        "PathRequested" => Ok(MessageState::PathRequested {}),
        "LinkEstablishing" => Ok(MessageState::LinkEstablishing {}),
        "Sending" => Ok(MessageState::Sending {}),
        "SentDirect" => Ok(MessageState::SentDirect {}),
        "SentToPropagation" => Ok(MessageState::SentToPropagation {}),
        "Delivered" => Ok(MessageState::Delivered {}),
        "Failed" => Ok(MessageState::Failed {}),
        "TimedOut" => Ok(MessageState::TimedOut {}),
        "Cancelled" => Ok(MessageState::Cancelled {}),
        "Received" => Ok(MessageState::Received {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn to_saved_peer_record(input: SavedPeerInput) -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: input.destination.trim().to_ascii_lowercase(),
        label: input.label.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        saved_at_ms: input.saved_at,
    }
}

fn to_app_settings_record(input: AppSettingsInput) -> AppSettingsRecord {
    AppSettingsRecord {
        display_name: input.display_name,
        auto_connect_saved: input.auto_connect_saved,
        announce_capabilities: input.announce_capabilities,
        tcp_clients: input.tcp_clients,
        broadcast: input.broadcast,
        announce_interval_seconds: input.announce_interval_seconds,
        telemetry: TelemetrySettingsRecord {
            enabled: input.telemetry.enabled,
            publish_interval_seconds: input.telemetry.publish_interval_seconds,
            accuracy_threshold_meters: input.telemetry.accuracy_threshold_meters,
            stale_after_minutes: input.telemetry.stale_after_minutes,
            expire_after_minutes: input.telemetry.expire_after_minutes,
        },
        hub: HubSettingsRecord {
            mode: parse_hub_mode(Some(input.hub.mode.as_str())),
            identity_hash: input.hub.identity_hash,
            api_base_url: input.hub.api_base_url,
            api_key: input.hub.api_key,
            refresh_interval_seconds: input.hub.refresh_interval_seconds,
        },
    }
}

fn to_eam_projection_record(input: EamProjectionInput) -> EamProjectionRecord {
    EamProjectionRecord {
        callsign: input.callsign,
        group_name: input.group_name,
        security_status: input.security_status,
        capability_status: input.capability_status,
        preparedness_status: input.preparedness_status,
        medical_status: input.medical_status,
        mobility_status: input.mobility_status,
        comms_status: input.comms_status,
        notes: input.notes,
        updated_at_ms: input.updated_at,
        deleted_at_ms: input.deleted_at,
        eam_uid: input.eam_uid,
        team_member_uid: input.team_member_uid,
        team_uid: input.team_uid,
        reported_at: input.reported_at,
        reported_by: input.reported_by,
        overall_status: input.overall_status,
        confidence: input.confidence,
        ttl_seconds: input.ttl_seconds,
        source: input.source.map(|source| crate::types::EamSourceRecord {
            rns_identity: source.rns_identity,
            display_name: source.display_name,
        }),
        sync_state: input.sync_state,
        sync_error: input.sync_error,
        draft_created_at_ms: input.draft_created_at,
        last_synced_at_ms: input.last_synced_at,
    }
}

fn to_event_projection_record(input: EventProjectionInput) -> EventProjectionRecord {
    EventProjectionRecord {
        uid: input.uid,
        command_id: input.command_id,
        source_identity: input.source_identity,
        source_display_name: input.source_display_name,
        timestamp: input.timestamp,
        command_type: input.command_type,
        mission_uid: input.mission_uid,
        content: input.content,
        callsign: input.callsign,
        server_time: input.server_time,
        client_time: input.client_time,
        keywords: input.keywords,
        content_hashes: input.content_hashes,
        updated_at_ms: input.updated_at,
        deleted_at_ms: input.deleted_at,
        correlation_id: input.correlation_id,
        topics: input.topics,
    }
}

fn to_message_record(input: MessageRecordInput) -> Result<MessageRecord, NodeError> {
    Ok(MessageRecord {
        message_id_hex: input.message_id_hex,
        conversation_id: input.conversation_id,
        direction: parse_message_direction(&input.direction)?,
        destination_hex: input.destination_hex,
        source_hex: input.source_hex,
        title: input.title,
        body_utf8: input.body_utf8,
        method: parse_message_method(&input.method)?,
        state: parse_message_state(&input.state)?,
        detail: input.detail,
        sent_at_ms: input.sent_at,
        received_at_ms: input.received_at,
        updated_at_ms: input.updated_at,
    })
}

fn to_telemetry_position_record(input: TelemetryPositionInput) -> TelemetryPositionRecord {
    TelemetryPositionRecord {
        callsign: input.callsign,
        lat: input.lat,
        lon: input.lon,
        alt: input.alt,
        course: input.course,
        speed: input.speed,
        accuracy: input.accuracy,
        updated_at_ms: input.updated_at,
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

fn peer_change_json(change: &PeerChange) -> serde_json::Value {
    json!({
        "destinationHex": change.destination_hex,
        "identityHex": change.identity_hex,
        "lxmfDestinationHex": change.lxmf_destination_hex,
        "displayName": change.display_name,
        "appData": change.app_data,
        "state": peer_state_to_str(change.state),
        "managementState": peer_management_state_to_str(change.management_state),
        "availabilityState": peer_availability_state_to_str(change.availability_state),
        "communicationReady": change.communication_ready,
        "missionReady": change.mission_ready,
        "relayEligible": change.relay_eligible,
        "stale": change.stale,
        "activeLink": change.active_link,
        "lastError": change.last_error,
        "lastResolutionError": change.last_resolution_error,
        "lastResolutionAttemptAtMs": change.last_resolution_attempt_at_ms,
        "lastReadyAtMs": change.last_ready_at_ms,
        "lastSeenAtMs": change.last_seen_at_ms,
        "announceLastSeenAtMs": change.announce_last_seen_at_ms,
        "lxmfLastSeenAtMs": change.lxmf_last_seen_at_ms
    })
}

fn peer_record_json(peer: &PeerRecord) -> serde_json::Value {
    json!({
        "destinationHex": peer.destination_hex,
        "identityHex": peer.identity_hex,
        "lxmfDestinationHex": peer.lxmf_destination_hex,
        "displayName": peer.display_name,
        "appData": peer.app_data,
        "state": peer_state_to_str(peer.state),
        "managementState": peer_management_state_to_str(peer.management_state),
        "availabilityState": peer_availability_state_to_str(peer.availability_state),
        "communicationReady": peer.communication_ready,
        "missionReady": peer.mission_ready,
        "relayEligible": peer.relay_eligible,
        "stale": peer.stale,
        "activeLink": peer.active_link,
        "lastResolutionError": peer.last_resolution_error,
        "lastResolutionAttemptAtMs": peer.last_resolution_attempt_at_ms,
        "lastReadyAtMs": peer.last_ready_at_ms,
        "lastSeenAtMs": peer.last_seen_at_ms,
        "announceLastSeenAtMs": peer.announce_last_seen_at_ms,
        "lxmfLastSeenAtMs": peer.lxmf_last_seen_at_ms
    })
}

fn hub_settings_json(settings: &HubSettingsRecord) -> serde_json::Value {
    json!({
        "mode": match settings.mode {
            HubMode::Disabled {} => "Disabled",
            HubMode::RchLxmf {} => "RchLxmf",
            HubMode::RchHttp {} => "RchHttp",
        },
        "identityHash": settings.identity_hash,
        "apiBaseUrl": settings.api_base_url,
        "apiKey": settings.api_key,
        "refreshIntervalSeconds": settings.refresh_interval_seconds
    })
}

fn telemetry_settings_json(settings: &TelemetrySettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "publishIntervalSeconds": settings.publish_interval_seconds,
        "accuracyThresholdMeters": settings.accuracy_threshold_meters,
        "staleAfterMinutes": settings.stale_after_minutes,
        "expireAfterMinutes": settings.expire_after_minutes
    })
}

fn app_settings_json(settings: &AppSettingsRecord) -> serde_json::Value {
    json!({
        "displayName": settings.display_name,
        "autoConnectSaved": settings.auto_connect_saved,
        "announceCapabilities": settings.announce_capabilities,
        "tcpClients": settings.tcp_clients,
        "broadcast": settings.broadcast,
        "announceIntervalSeconds": settings.announce_interval_seconds,
        "telemetry": telemetry_settings_json(&settings.telemetry),
        "hub": hub_settings_json(&settings.hub)
    })
}

fn saved_peer_json(peer: &SavedPeerRecord) -> serde_json::Value {
    json!({
        "destination": peer.destination_hex,
        "label": peer.label,
        "savedAt": peer.saved_at_ms
    })
}

fn eam_projection_json(record: &EamProjectionRecord) -> serde_json::Value {
    json!({
        "callsign": record.callsign,
        "groupName": record.group_name,
        "securityStatus": record.security_status,
        "capabilityStatus": record.capability_status,
        "preparednessStatus": record.preparedness_status,
        "medicalStatus": record.medical_status,
        "mobilityStatus": record.mobility_status,
        "commsStatus": record.comms_status,
        "notes": record.notes,
        "updatedAt": record.updated_at_ms,
        "deletedAt": record.deleted_at_ms,
        "eamUid": record.eam_uid,
        "teamMemberUid": record.team_member_uid,
        "teamUid": record.team_uid,
        "reportedAt": record.reported_at,
        "reportedBy": record.reported_by,
        "overallStatus": record.overall_status,
        "confidence": record.confidence,
        "ttlSeconds": record.ttl_seconds,
        "source": record.source.as_ref().map(|source| json!({
            "rns_identity": source.rns_identity,
            "display_name": source.display_name
        })),
        "syncState": record.sync_state,
        "syncError": record.sync_error,
        "draftCreatedAt": record.draft_created_at_ms,
        "lastSyncedAt": record.last_synced_at_ms
    })
}

fn event_projection_json(record: &EventProjectionRecord) -> serde_json::Value {
    json!({
        "command_id": record.command_id,
        "source": {
            "rns_identity": record.source_identity,
            "display_name": record.source_display_name
        },
        "timestamp": record.timestamp,
        "command_type": record.command_type,
        "args": {
            "entry_uid": record.uid,
            "mission_uid": record.mission_uid,
            "content": record.content,
            "callsign": record.callsign,
            "server_time": record.server_time,
            "client_time": record.client_time,
            "keywords": record.keywords,
            "content_hashes": record.content_hashes,
            "source_identity": record.source_identity,
            "source_display_name": record.source_display_name
        },
        "correlation_id": record.correlation_id,
        "topics": record.topics,
        "deleted_at": record.deleted_at_ms,
        "updatedAt": record.updated_at_ms
    })
}

fn telemetry_position_json(record: &TelemetryPositionRecord) -> serde_json::Value {
    json!({
        "callsign": record.callsign,
        "lat": record.lat,
        "lon": record.lon,
        "alt": record.alt,
        "course": record.course,
        "speed": record.speed,
        "accuracy": record.accuracy,
        "updatedAt": record.updated_at_ms
    })
}

fn eam_team_summary_json(summary: &crate::types::EamTeamSummaryRecord) -> serde_json::Value {
    json!({
        "teamUid": summary.team_uid,
        "total": summary.total,
        "activeTotal": summary.active_total,
        "deletedTotal": summary.deleted_total,
        "overallStatus": summary.overall_status,
        "greenTotal": summary.green_total,
        "yellowTotal": summary.yellow_total,
        "redTotal": summary.red_total,
        "updatedAt": summary.updated_at_ms
    })
}

fn operational_summary_json(summary: &crate::types::OperationalSummary) -> serde_json::Value {
    json!({
        "running": summary.running,
        "peerCountTotal": summary.peer_count_total,
        "peerCountCommunicationReady": summary.peer_count_communication_ready,
        "peerCountMissionReady": summary.peer_count_mission_ready,
        "peerCountRelayEligible": summary.peer_count_relay_eligible,
        "savedPeerCount": summary.saved_peer_count,
        "conversationCount": summary.conversation_count,
        "messageCount": summary.message_count,
        "eamCount": summary.eam_count,
        "eventCount": summary.event_count,
        "telemetryCount": summary.telemetry_count,
        "activePropagationNodeHex": summary.active_propagation_node_hex,
        "updatedAtMs": summary.updated_at_ms
    })
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
        LxmfDeliveryStatus::SentToPropagation {} => "SentToPropagation",
        LxmfDeliveryStatus::Acknowledged {} => "Acknowledged",
        LxmfDeliveryStatus::Failed {} => "Failed",
        LxmfDeliveryStatus::TimedOut {} => "TimedOut",
    }
}

fn send_mode_from_input(send_mode: Option<&str>, use_propagation_node: bool) -> SendMode {
    if use_propagation_node {
        return SendMode::PropagationOnly {};
    }
    match send_mode.unwrap_or("").trim() {
        "DirectOnly" => SendMode::DirectOnly {},
        "PropagationOnly" => SendMode::PropagationOnly {},
        _ => SendMode::Auto {},
    }
}

fn send_mode_to_str(mode: SendMode) -> &'static str {
    match mode {
        SendMode::Auto {} => "Auto",
        SendMode::DirectOnly {} => "DirectOnly",
        SendMode::PropagationOnly {} => "PropagationOnly",
    }
}

fn lxmf_delivery_method_to_str(method: LxmfDeliveryMethod) -> &'static str {
    match method {
        LxmfDeliveryMethod::Direct {} => "Direct",
        LxmfDeliveryMethod::Opportunistic {} => "Opportunistic",
        LxmfDeliveryMethod::Propagated {} => "Propagated",
    }
}

fn lxmf_delivery_representation_to_str(representation: LxmfDeliveryRepresentation) -> &'static str {
    match representation {
        LxmfDeliveryRepresentation::Packet {} => "Packet",
        LxmfDeliveryRepresentation::Resource {} => "Resource",
    }
}

fn lxmf_fallback_stage_to_str(stage: LxmfFallbackStage) -> &'static str {
    match stage {
        LxmfFallbackStage::AfterDirectRetryBudget {} => "AfterDirectRetryBudget",
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

fn projection_scope_to_str(scope: ProjectionScope) -> &'static str {
    match scope {
        ProjectionScope::AppSettings {} => "AppSettings",
        ProjectionScope::SavedPeers {} => "SavedPeers",
        ProjectionScope::OperationalSummary {} => "OperationalSummary",
        ProjectionScope::Peers {} => "Peers",
        ProjectionScope::SyncStatus {} => "SyncStatus",
        ProjectionScope::HubRegistration {} => "HubRegistration",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
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
                "change": peer_change_json(&change)
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
                "method": lxmf_delivery_method_to_str(update.method),
                "representation": lxmf_delivery_representation_to_str(update.representation),
                "relayDestinationHex": update.relay_destination_hex,
                "fallbackStage": update.fallback_stage.map(lxmf_fallback_stage_to_str),
                "detail": update.detail,
                "sentAtMs": update.sent_at_ms,
                "updatedAtMs": update.updated_at_ms
            }),
        ),
        NodeEvent::PeerResolved { peer } => (
            "peerResolved",
            peer_record_json(&peer),
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
        NodeEvent::ProjectionInvalidated { invalidation } => (
            "projectionInvalidated",
            json!({
                "scope": projection_scope_to_str(invalidation.scope),
                "key": invalidation.key,
                "revision": invalidation.revision,
                "updatedAtMs": invalidation.updated_at_ms,
                "reason": invalidation.reason
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

    let subscription = {
        let node = ensure_node(&mut guard);
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

    let subscription = {
        let node = ensure_node(&mut guard);
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
        send_mode_from_input(payload.send_mode.as_deref(), payload.use_propagation_node),
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
        send_mode: send_mode_from_input(payload.send_mode.as_deref(), payload.use_propagation_node),
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(peer_record_json).collect::<Vec<_>>()
            }),
        ),
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
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
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

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_legacyImportCompletedJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.legacy_import_completed() {
        Ok(completed) => ok_json_result(&mut env, &json!({ "completed": completed })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importLegacyStateJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: LegacyImportInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid legacy import payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    let messages = match payload
        .messages
        .unwrap_or_default()
        .into_iter()
        .map(to_message_record)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(err) => {
            set_last_node_error(err);
            return RESULT_ERR;
        }
    };
    let legacy = LegacyImportPayload {
        settings: payload.settings.map(to_app_settings_record),
        saved_peers: payload
            .saved_peers
            .unwrap_or_default()
            .into_iter()
            .map(to_saved_peer_record)
            .collect(),
        eams: payload
            .eams
            .unwrap_or_default()
            .into_iter()
            .map(to_eam_projection_record)
            .collect(),
        events: payload
            .events
            .unwrap_or_default()
            .into_iter()
            .map(to_event_projection_record)
            .collect(),
        messages,
        telemetry_positions: payload
            .telemetry_positions
            .unwrap_or_default()
            .into_iter()
            .map(to_telemetry_position_record)
            .collect(),
    };
    match node.import_legacy_state(legacy) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getAppSettingsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_app_settings() {
        Ok(Some(settings)) => ok_json_result(&mut env, &app_settings_json(&settings)),
        Ok(None) => ok_json_result(&mut env, &json!({ "settings": null })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setAppSettingsJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: AppSettingsInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid settings payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_app_settings(to_app_settings_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSavedPeersJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_saved_peers() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(saved_peer_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSavedPeersJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SavedPeersPayload = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid saved peers payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    let peers = payload.saved_peers.into_iter().map(to_saved_peer_record).collect();
    match node.set_saved_peers(peers) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getOperationalSummaryJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_operational_summary() {
        Ok(summary) => ok_json_result(&mut env, &operational_summary_json(&summary)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getEamsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_eams() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items.iter().map(eam_projection_json).collect::<Vec<_>>() })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_upsertEamJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: EamProjectionInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid eam payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.upsert_eam(to_eam_projection_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteEamJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: DeleteEamInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid eam delete payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_eam(payload.callsign, payload.deleted_at_ms.unwrap_or_else(crate::runtime::now_ms)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getEamTeamSummaryJson(
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
    let payload: TeamUidInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid eam team summary payload: {e}"));
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_eam_team_summary(payload.team_uid) {
        Ok(Some(summary)) => ok_json_result(&mut env, &eam_team_summary_json(&summary)),
        Ok(None) => ok_json_result(&mut env, &json!({ "summary": null })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getEventsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_events() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items.iter().map(event_projection_json).collect::<Vec<_>>() })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_upsertEventJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: EventProjectionInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid event payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.upsert_event(to_event_projection_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteEventJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: DeleteEventInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid event delete payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_event(payload.uid, payload.deleted_at_ms.unwrap_or_else(crate::runtime::now_ms)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getTelemetryPositionsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node(&mut guard);
    match node.get_telemetry_positions() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items.iter().map(telemetry_position_json).collect::<Vec<_>>() })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_recordLocalTelemetryFixJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: TelemetryPositionInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid telemetry payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.record_local_telemetry_fix(to_telemetry_position_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteLocalTelemetryJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: CallsignInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid telemetry delete payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_local_telemetry(payload.callsign) {
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
