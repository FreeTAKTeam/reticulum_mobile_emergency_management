use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use jni::objects::{JClass, JString};
use jni::sys::{jint, jstring};
use jni::JNIEnv;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::node::{EventSubscription, Node};
use crate::types::{
    AppSettingsRecord, ChecklistCreateFromTemplateRequest, ChecklistCreateOnlineRequest,
    ChecklistListActiveRequest, ChecklistRecord, ChecklistSettingsRecord,
    ChecklistTaskCellSetRequest, ChecklistTaskRowAddRequest, ChecklistTaskRowDeleteRequest,
    ChecklistTaskRowStyleSetRequest, ChecklistTaskStatusSetRequest,
    ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest, ChecklistTemplateRecord,
    ChecklistUpdatePatch, ChecklistUpdateRequest, ConversationRecord, EamProjectionRecord,
    EventProjectionRecord, HubDirectoryPeerRecord, HubDirectorySnapshot, HubMode,
    HubSettingsRecord, LegacyImportPayload, LogLevel, LxmfDeliveryMethod,
    LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfFallbackStage, MessageDirection,
    MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent, NodeStatus,
    PeerChange, PeerRecord, PeerState, ProjectionScope, SavedPeerRecord, SendLxmfRequest, SendMode,
    SendOutcome, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord,
    SosMessageKind, SosSettingsRecord, SosState, SosStatusRecord, SosTriggerSource, SyncPhase,
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
struct ConversationDeleteInput {
    conversation_id: String,
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
    #[serde(default)]
    checklists: ChecklistSettingsInput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistSettingsInput {
    default_task_due_step_minutes: Option<u32>,
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
struct ChecklistListInput {
    search: Option<String>,
    sort_by: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUidInput {
    checklist_uid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistCreateInput {
    checklist_uid: Option<String>,
    mission_uid: Option<String>,
    template_uid: String,
    name: String,
    description: String,
    start_time: String,
    created_by_team_member_rns_identity: Option<String>,
    created_by_team_member_display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTemplateImportInput {
    template_uid: Option<String>,
    name: String,
    description: Option<String>,
    csv_text: String,
    source_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUpdateInput {
    checklist_uid: String,
    patch: ChecklistUpdatePatchInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUpdatePatchInput {
    mission_uid: Option<String>,
    template_uid: Option<String>,
    name: Option<String>,
    description: Option<String>,
    start_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskStatusInput {
    checklist_uid: String,
    task_uid: String,
    user_status: String,
    changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowAddInput {
    checklist_uid: String,
    task_uid: Option<String>,
    number: u32,
    due_relative_minutes: Option<u32>,
    legacy_value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowDeleteInput {
    checklist_uid: String,
    task_uid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowStyleInput {
    checklist_uid: String,
    task_uid: String,
    row_background_color: Option<String>,
    line_break_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskCellInput {
    checklist_uid: String,
    task_uid: String,
    column_uid: String,
    value: String,
    updated_by_team_member_rns_identity: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosSettingsInput {
    enabled: bool,
    message_template: String,
    #[serde(default)]
    cancel_message_template: String,
    countdown_seconds: u32,
    include_location: bool,
    trigger_shake: bool,
    trigger_tap_pattern: bool,
    trigger_power_button: bool,
    shake_sensitivity: f64,
    audio_recording: bool,
    audio_duration_seconds: u32,
    periodic_updates: bool,
    update_interval_seconds: u32,
    floating_button: bool,
    silent_auto_answer: bool,
    deactivation_pin_hash: Option<String>,
    deactivation_pin_salt: Option<String>,
    floating_button_x: f64,
    floating_button_y: f64,
    active_pill_x: f64,
    active_pill_y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosPinInput {
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosTriggerInput {
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosDeactivateInput {
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosTelemetryInput {
    lat: Option<f64>,
    lon: Option<f64>,
    alt: Option<f64>,
    speed: Option<f64>,
    course: Option<f64>,
    accuracy: Option<f64>,
    battery_percent: Option<f64>,
    battery_charging: Option<bool>,
    updated_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosAccelerometerInput {
    x: f64,
    y: f64,
    z: f64,
    at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosScreenEventInput {
    at_ms: Option<u64>,
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
        .unwrap_or("Autonomous")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "connected" => HubMode::Connected {},
        "semiautonomous" | "semi_autonomous" | "semi-autonomous" | "rchlxmf" | "rch_lxmf"
        | "rchhttp" | "rch_http" => HubMode::SemiAutonomous {},
        _ => HubMode::Autonomous {},
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

fn parse_sos_trigger_source(value: Option<&str>) -> SosTriggerSource {
    match value
        .unwrap_or("Manual")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "floatingbutton" | "floating_button" | "floating-button" => {
            SosTriggerSource::FloatingButton {}
        }
        "shake" => SosTriggerSource::Shake {},
        "tappattern" | "tap_pattern" | "tap-pattern" => SosTriggerSource::TapPattern {},
        "powerbutton" | "power_button" | "power-button" => SosTriggerSource::PowerButton {},
        "restore" => SosTriggerSource::Restore {},
        "remote" => SosTriggerSource::Remote {},
        _ => SosTriggerSource::Manual {},
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

fn to_sos_settings_record(input: SosSettingsInput) -> SosSettingsRecord {
    SosSettingsRecord {
        enabled: input.enabled,
        message_template: input.message_template,
        cancel_message_template: input.cancel_message_template,
        countdown_seconds: input.countdown_seconds,
        include_location: input.include_location,
        trigger_shake: input.trigger_shake,
        trigger_tap_pattern: input.trigger_tap_pattern,
        trigger_power_button: input.trigger_power_button,
        shake_sensitivity: input.shake_sensitivity,
        audio_recording: input.audio_recording,
        audio_duration_seconds: input.audio_duration_seconds,
        periodic_updates: input.periodic_updates,
        update_interval_seconds: input.update_interval_seconds,
        floating_button: input.floating_button,
        silent_auto_answer: input.silent_auto_answer,
        deactivation_pin_hash: input.deactivation_pin_hash,
        deactivation_pin_salt: input.deactivation_pin_salt,
        floating_button_x: input.floating_button_x,
        floating_button_y: input.floating_button_y,
        active_pill_x: input.active_pill_x,
        active_pill_y: input.active_pill_y,
    }
}

fn to_sos_telemetry_record(input: SosTelemetryInput) -> SosDeviceTelemetryRecord {
    SosDeviceTelemetryRecord {
        lat: input.lat,
        lon: input.lon,
        alt: input.alt,
        speed: input.speed,
        course: input.course,
        accuracy: input.accuracy,
        battery_percent: input.battery_percent,
        battery_charging: input.battery_charging,
        updated_at_ms: input.updated_at_ms.unwrap_or_else(crate::runtime::now_ms),
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
        checklists: ChecklistSettingsRecord {
            default_task_due_step_minutes: input
                .checklists
                .default_task_due_step_minutes
                .unwrap_or(crate::types::DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES)
                .max(1),
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

fn to_checklist_create_request(input: ChecklistCreateInput) -> ChecklistCreateOnlineRequest {
    ChecklistCreateOnlineRequest {
        checklist_uid: input.checklist_uid,
        mission_uid: input.mission_uid,
        template_uid: input.template_uid,
        name: input.name,
        description: input.description,
        start_time: input.start_time,
        created_by_team_member_rns_identity: input.created_by_team_member_rns_identity,
        created_by_team_member_display_name: input.created_by_team_member_display_name,
    }
}

fn to_checklist_template_import_request(
    input: ChecklistTemplateImportInput,
) -> ChecklistTemplateImportCsvRequest {
    ChecklistTemplateImportCsvRequest {
        template_uid: input.template_uid,
        name: input.name,
        description: input.description,
        csv_text: input.csv_text,
        source_filename: input.source_filename,
    }
}

fn to_checklist_update_request(input: ChecklistUpdateInput) -> ChecklistUpdateRequest {
    ChecklistUpdateRequest {
        checklist_uid: input.checklist_uid,
        patch: ChecklistUpdatePatch {
            mission_uid: input.patch.mission_uid,
            template_uid: input.patch.template_uid,
            name: input.patch.name,
            description: input.patch.description,
            start_time: input.patch.start_time,
        },
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_status_request(
    input: ChecklistTaskStatusInput,
) -> Result<ChecklistTaskStatusSetRequest, NodeError> {
    let user_status = match input.user_status.trim() {
        "COMPLETE" => crate::types::ChecklistUserTaskStatus::Complete {},
        "PENDING" => crate::types::ChecklistUserTaskStatus::Pending {},
        _ => return Err(NodeError::InvalidConfig {}),
    };
    Ok(ChecklistTaskStatusSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        user_status,
        changed_by_team_member_rns_identity: input.changed_by_team_member_rns_identity,
    })
}

fn to_checklist_task_row_add_request(
    input: ChecklistTaskRowAddInput,
) -> ChecklistTaskRowAddRequest {
    ChecklistTaskRowAddRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        number: input.number,
        due_relative_minutes: input.due_relative_minutes,
        legacy_value: input.legacy_value,
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_row_delete_request(
    input: ChecklistTaskRowDeleteInput,
) -> ChecklistTaskRowDeleteRequest {
    ChecklistTaskRowDeleteRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_row_style_request(
    input: ChecklistTaskRowStyleInput,
) -> ChecklistTaskRowStyleSetRequest {
    ChecklistTaskRowStyleSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        row_background_color: input.row_background_color,
        line_break_enabled: input.line_break_enabled,
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_cell_request(input: ChecklistTaskCellInput) -> ChecklistTaskCellSetRequest {
    ChecklistTaskCellSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        column_uid: input.column_uid,
        value: input.value,
        updated_by_team_member_rns_identity: input.updated_by_team_member_rns_identity,
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

fn announce_class_to_str(class: crate::types::AnnounceClass) -> &'static str {
    match class {
        crate::types::AnnounceClass::PeerApp {} => "PeerApp",
        crate::types::AnnounceClass::RchHubServer {} => "RchHubServer",
        crate::types::AnnounceClass::PropagationNode {} => "PropagationNode",
        crate::types::AnnounceClass::LxmfDelivery {} => "LxmfDelivery",
        crate::types::AnnounceClass::Other {} => "Other",
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
        "saved": change.saved,
        "stale": change.stale,
        "activeLink": change.active_link,
        "lastError": change.last_error,
        "lastResolutionError": change.last_resolution_error,
        "lastResolutionAttemptAtMs": change.last_resolution_attempt_at_ms,
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
        "saved": peer.saved,
        "stale": peer.stale,
        "activeLink": peer.active_link,
        "hubDerived": peer.hub_derived,
        "lastResolutionError": peer.last_resolution_error,
        "lastResolutionAttemptAtMs": peer.last_resolution_attempt_at_ms,
        "lastSeenAtMs": peer.last_seen_at_ms,
        "announceLastSeenAtMs": peer.announce_last_seen_at_ms,
        "lxmfLastSeenAtMs": peer.lxmf_last_seen_at_ms
    })
}

fn hub_directory_peer_json(peer: &HubDirectoryPeerRecord) -> serde_json::Value {
    json!({
        "identity": peer.identity,
        "destinationHash": peer.destination_hash,
        "displayName": peer.display_name,
        "announceCapabilities": peer.announce_capabilities,
        "clientType": peer.client_type,
        "registeredMode": peer.registered_mode,
        "lastSeen": peer.last_seen,
        "status": peer.status
    })
}

fn hub_directory_snapshot_json(snapshot: &HubDirectorySnapshot) -> serde_json::Value {
    json!({
        "effectiveConnectedMode": snapshot.effective_connected_mode,
        "items": snapshot
            .items
            .iter()
            .map(hub_directory_peer_json)
            .collect::<Vec<_>>(),
        "receivedAtMs": snapshot.received_at_ms
    })
}

fn operational_notice_json(notice: &crate::types::OperationalNotice) -> serde_json::Value {
    json!({
        "level": log_level_to_str(notice.level),
        "message": notice.message,
        "atMs": notice.at_ms
    })
}

fn hub_settings_json(settings: &HubSettingsRecord) -> serde_json::Value {
    json!({
        "mode": settings.mode.as_str(),
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
        "hub": hub_settings_json(&settings.hub),
        "checklists": {
            "defaultTaskDueStepMinutes": settings.checklists.default_task_due_step_minutes
        }
    })
}

fn sos_state_to_str(state: SosState) -> &'static str {
    crate::sos::sos_status_label(state)
}

fn sos_trigger_to_str(source: SosTriggerSource) -> &'static str {
    crate::sos::sos_trigger_label(source)
}

fn sos_kind_to_str(kind: SosMessageKind) -> &'static str {
    crate::sos::sos_kind_label(kind)
}

fn sos_settings_json(settings: &SosSettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "messageTemplate": settings.message_template,
        "cancelMessageTemplate": settings.cancel_message_template,
        "countdownSeconds": settings.countdown_seconds,
        "includeLocation": settings.include_location,
        "triggerShake": settings.trigger_shake,
        "triggerTapPattern": settings.trigger_tap_pattern,
        "triggerPowerButton": settings.trigger_power_button,
        "shakeSensitivity": settings.shake_sensitivity,
        "audioRecording": settings.audio_recording,
        "audioDurationSeconds": settings.audio_duration_seconds,
        "periodicUpdates": settings.periodic_updates,
        "updateIntervalSeconds": settings.update_interval_seconds,
        "floatingButton": settings.floating_button,
        "silentAutoAnswer": settings.silent_auto_answer,
        "deactivationPinHash": settings.deactivation_pin_hash,
        "deactivationPinSalt": settings.deactivation_pin_salt,
        "floatingButtonX": settings.floating_button_x,
        "floatingButtonY": settings.floating_button_y,
        "activePillX": settings.active_pill_x,
        "activePillY": settings.active_pill_y
    })
}

fn sos_status_json(status: &SosStatusRecord) -> serde_json::Value {
    json!({
        "state": sos_state_to_str(status.state),
        "incidentId": status.incident_id,
        "triggerSource": status.trigger_source.map(sos_trigger_to_str),
        "countdownDeadlineMs": status.countdown_deadline_ms,
        "activatedAtMs": status.activated_at_ms,
        "lastSentAtMs": status.last_sent_at_ms,
        "lastUpdateAtMs": status.last_update_at_ms,
        "updatedAtMs": status.updated_at_ms
    })
}

fn sos_alert_json(alert: &SosAlertRecord) -> serde_json::Value {
    json!({
        "incidentId": alert.incident_id,
        "sourceHex": alert.source_hex,
        "conversationId": alert.conversation_id,
        "state": sos_kind_to_str(alert.state),
        "active": alert.active,
        "bodyUtf8": alert.body_utf8,
        "lat": alert.lat,
        "lon": alert.lon,
        "batteryPercent": alert.battery_percent,
        "audioId": alert.audio_id,
        "messageIdHex": alert.message_id_hex,
        "receivedAtMs": alert.received_at_ms,
        "updatedAtMs": alert.updated_at_ms
    })
}

fn sos_location_json(location: &SosLocationRecord) -> serde_json::Value {
    json!({
        "incidentId": location.incident_id,
        "sourceHex": location.source_hex,
        "lat": location.lat,
        "lon": location.lon,
        "alt": location.alt,
        "accuracy": location.accuracy,
        "batteryPercent": location.battery_percent,
        "recordedAtMs": location.recorded_at_ms
    })
}

fn sos_audio_json(audio: &SosAudioRecord) -> serde_json::Value {
    json!({
        "audioId": audio.audio_id,
        "incidentId": audio.incident_id,
        "sourceHex": audio.source_hex,
        "path": audio.path,
        "mimeType": audio.mime_type,
        "durationSeconds": audio.duration_seconds,
        "createdAtMs": audio.created_at_ms
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

fn checklist_column_json(column: &crate::types::ChecklistColumnRecord) -> serde_json::Value {
    json!({
        "columnUid": column.column_uid,
        "columnName": column.column_name,
        "columnType": column.column_type.as_str(),
        "columnEditable": column.column_editable,
        "backgroundColor": column.background_color,
        "textColor": column.text_color,
        "isRemovable": column.is_removable,
        "systemKey": column.system_key.map(|key| key.as_str()),
        "displayOrder": column.display_order
    })
}

fn checklist_cell_json(cell: &crate::types::ChecklistCellRecord) -> serde_json::Value {
    json!({
        "cellUid": cell.cell_uid,
        "taskUid": cell.task_uid,
        "columnUid": cell.column_uid,
        "value": cell.value,
        "updatedAt": cell.updated_at,
        "updatedByTeamMemberRnsIdentity": cell.updated_by_team_member_rns_identity
    })
}

fn checklist_task_json(task: &crate::types::ChecklistTaskRecord) -> serde_json::Value {
    json!({
        "taskUid": task.task_uid,
        "number": task.number,
        "userStatus": task.user_status.as_str(),
        "taskStatus": task.task_status.as_str(),
        "isLate": task.is_late,
        "updatedAt": task.updated_at,
        "deletedAt": task.deleted_at,
        "customStatus": task.custom_status,
        "dueRelativeMinutes": task.due_relative_minutes,
        "dueDtg": task.due_dtg,
        "notes": task.notes,
        "rowBackgroundColor": task.row_background_color,
        "lineBreakEnabled": task.line_break_enabled,
        "completedAt": task.completed_at,
        "completedByTeamMemberRnsIdentity": task.completed_by_team_member_rns_identity,
        "legacyValue": task.legacy_value,
        "cells": task.cells.iter().map(checklist_cell_json).collect::<Vec<_>>()
    })
}

fn checklist_feed_publication_json(
    publication: &crate::types::ChecklistFeedPublicationRecord,
) -> serde_json::Value {
    json!({
        "publicationUid": publication.publication_uid,
        "checklistUid": publication.checklist_uid,
        "missionFeedUid": publication.mission_feed_uid,
        "publishedAt": publication.published_at,
        "publishedByTeamMemberRnsIdentity": publication.published_by_team_member_rns_identity
    })
}

fn checklist_record_json(record: &ChecklistRecord) -> serde_json::Value {
    json!({
        "uid": record.uid,
        "missionUid": record.mission_uid,
        "templateUid": record.template_uid,
        "templateVersion": record.template_version,
        "templateName": record.template_name,
        "name": record.name,
        "description": record.description,
        "startTime": record.start_time,
        "mode": record.mode.as_str(),
        "syncState": record.sync_state.as_str(),
        "originType": record.origin_type.as_str(),
        "checklistStatus": record.checklist_status.as_str(),
        "createdAt": record.created_at,
        "createdByTeamMemberRnsIdentity": record.created_by_team_member_rns_identity,
        "createdByTeamMemberDisplayName": record.created_by_team_member_display_name,
        "updatedAt": record.updated_at,
        "lastChangedByTeamMemberRnsIdentity": record.last_changed_by_team_member_rns_identity,
        "deletedAt": record.deleted_at,
        "uploadedAt": record.uploaded_at,
        "participantRnsIdentities": record.participant_rns_identities,
        "expectedTaskCount": record.expected_task_count,
        "progressPercent": record.progress_percent,
        "counts": {
            "pendingCount": record.counts.pending_count,
            "lateCount": record.counts.late_count,
            "completeCount": record.counts.complete_count
        },
        "columns": record.columns.iter().map(checklist_column_json).collect::<Vec<_>>(),
        "tasks": record.tasks.iter().map(checklist_task_json).collect::<Vec<_>>(),
        "feedPublications": record
            .feed_publications
            .iter()
            .map(checklist_feed_publication_json)
            .collect::<Vec<_>>()
    })
}

fn checklist_template_json(record: &ChecklistTemplateRecord) -> serde_json::Value {
    json!({
        "uid": record.uid,
        "name": record.name,
        "description": record.description,
        "version": record.version,
        "originType": record.origin_type.as_str(),
        "createdAt": record.created_at,
        "updatedAt": record.updated_at,
        "sourceFilename": record.source_filename,
        "columns": record.columns.iter().map(checklist_column_json).collect::<Vec<_>>(),
        "tasks": record.tasks.iter().map(checklist_task_json).collect::<Vec<_>>()
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
        "savedPeerCount": summary.saved_peer_count,
        "connectedPeerCount": summary.connected_peer_count,
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
        ProjectionScope::Checklists {} => "Checklists",
        ProjectionScope::ChecklistDetail {} => "ChecklistDetail",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
        ProjectionScope::Sos {} => "Sos",
    }
}

fn message_record_json(message: &MessageRecord) -> Value {
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
    })
}

fn conversation_record_json(conversation: &ConversationRecord) -> Value {
    json!({
        "conversationId": conversation.conversation_id,
        "peerDestinationHex": conversation.peer_destination_hex,
        "peerDisplayName": conversation.peer_display_name,
        "lastMessagePreview": conversation.last_message_preview,
        "lastMessageAtMs": conversation.last_message_at_ms,
        "unreadCount": conversation.unread_count,
        "lastMessageState": conversation.last_message_state.map(message_state_to_str)
    })
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
            announce_class,
            app_data,
            display_name,
            hops,
            interface_hex,
            received_at_ms,
        } => (
            "announceReceived",
            json!({
                "destinationHex": destination_hex,
                "identityHex": identity_hex,
                "destinationKind": destination_kind,
                "announceClass": announce_class_to_str(announce_class),
                "appData": app_data,
                "displayName": display_name,
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
        NodeEvent::PeerResolved { peer } => ("peerResolved", peer_record_json(&peer)),
        NodeEvent::MessageReceived { message } => {
            ("messageReceived", message_record_json(&message))
        }
        NodeEvent::MessageUpdated { message } => ("messageUpdated", message_record_json(&message)),
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
        NodeEvent::HubDirectoryUpdated { snapshot } => (
            "hubDirectoryUpdated",
            hub_directory_snapshot_json(&snapshot),
        ),
        NodeEvent::OperationalNotice { notice } => {
            ("operationalNotice", operational_notice_json(&notice))
        }
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
        NodeEvent::SosStatusChanged { status } => ("sosStatusChanged", sos_status_json(&status)),
        NodeEvent::SosAlertChanged { alert } => ("sosAlertChanged", sos_alert_json(&alert)),
        NodeEvent::SosTelemetryRequested {} => ("sosTelemetryRequested", json!({})),
        NodeEvent::SosAudioRecordingRequested {
            incident_id,
            duration_seconds,
        } => (
            "sosAudioRecordingRequested",
            json!({
                "incidentId": incident_id,
                "durationSeconds": duration_seconds
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
            return err_result(
                "InvalidConfig",
                format!("invalid propagation node payload: {e}"),
            )
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(conversation_record_json).collect::<Vec<_>>()
            }),
        ),
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
            set_last_error(
                "InvalidConfig",
                format!("invalid message list payload: {e}"),
            );
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(message_record_json).collect::<Vec<_>>()
            }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteConversationJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return 1;
        }
    };
    let payload: ConversationDeleteInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid conversation delete payload: {e}"),
            );
            return 1;
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return 1;
        }
    };
    let node = ensure_node(&mut guard);
    match node.delete_conversation(payload.conversation_id) {
        Ok(()) => 0,
        Err(err) => {
            set_last_node_error(err);
            1
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listTelemetryDestinationsJson(
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
    match node.list_telemetry_destinations() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items
            }),
        ),
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
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid legacy import payload: {e}"),
            )
        }
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
    let peers = payload
        .saved_peers
        .into_iter()
        .map(to_saved_peer_record)
        .collect();
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistsJson(
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
    let payload: ChecklistListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist list payload: {e}"),
            );
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
    match node.list_active_checklists(Some(ChecklistListActiveRequest {
        search: payload.search,
        sort_by: payload.sort_by,
    })) {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(checklist_record_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistJson(
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
    let payload: ChecklistUidInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist get payload: {e}"),
            );
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
    match node.get_checklist(payload.checklist_uid) {
        Ok(Some(record)) => ok_json_result(&mut env, &checklist_record_json(&record)),
        Ok(None) => ok_json_result(&mut env, &json!({ "checklist": null })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistTemplatesJson(
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
    let payload: ChecklistListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist template list payload: {e}"),
            );
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
    match node.list_checklist_templates(Some(ChecklistTemplateListRequest {
        search: payload.search,
        sort_by: payload.sort_by,
    })) {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(checklist_template_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importChecklistTemplateCsvJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let err_result = |code: &str, message: String| {
        set_last_error(code, message);
        ptr::null_mut()
    };
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTemplateImportInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist template import payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned".to_string()),
    };
    let node = ensure_node(&mut guard);
    match node.import_checklist_template_csv(to_checklist_template_import_request(payload)) {
        Ok(template) => ok_json_result(&mut env, &checklist_template_json(&template)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createChecklistFromTemplateJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let err_result = |code: &str, message: String| {
        set_last_error(code, message);
        RESULT_ERR
    };
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistCreateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist create-from-template payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned".to_string()),
    };
    let node = ensure_node(&mut guard);
    match node.create_checklist_from_template(ChecklistCreateFromTemplateRequest {
        checklist_uid: payload.checklist_uid,
        mission_uid: payload.mission_uid,
        template_uid: payload.template_uid,
        name: payload.name,
        description: payload.description,
        start_time: payload.start_time,
        created_by_team_member_rns_identity: payload.created_by_team_member_rns_identity,
        created_by_team_member_display_name: payload.created_by_team_member_display_name,
    }) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createOnlineChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistCreateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist create payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.create_online_checklist(to_checklist_create_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_updateChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistUpdateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist update payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.update_checklist(to_checklist_update_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistUidInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_checklist(payload.checklist_uid) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_joinChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistUidInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist join payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.join_checklist(payload.checklist_uid) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_uploadChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistUidInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist upload payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.upload_checklist(payload.checklist_uid) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setChecklistTaskStatusJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTaskStatusInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist task status payload: {e}"),
            )
        }
    };
    let request = match to_checklist_task_status_request(payload) {
        Ok(v) => v,
        Err(err) => return err_result("InvalidConfig", err.to_string()),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_checklist_task_status(request) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_addChecklistTaskRowJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTaskRowAddInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist task row add payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.add_checklist_task_row(to_checklist_task_row_add_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteChecklistTaskRowJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTaskRowDeleteInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist task row delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_checklist_task_row(to_checklist_task_row_delete_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setChecklistTaskRowStyleJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTaskRowStyleInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist task row style payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_checklist_task_row_style(to_checklist_task_row_style_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setChecklistTaskCellJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTaskCellInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist task cell payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_checklist_task_cell(to_checklist_task_cell_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(eam_projection_json).collect::<Vec<_>>() }),
        ),
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
    match node.delete_eam(
        payload.callsign,
        payload.deleted_at_ms.unwrap_or_else(crate::runtime::now_ms),
    ) {
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
            set_last_error(
                "InvalidConfig",
                format!("invalid eam team summary payload: {e}"),
            );
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(event_projection_json).collect::<Vec<_>>() }),
        ),
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
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid event delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.delete_event(
        payload.uid,
        payload.deleted_at_ms.unwrap_or_else(crate::runtime::now_ms),
    ) {
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
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(telemetry_position_json).collect::<Vec<_>>() }),
        ),
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
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid telemetry delete payload: {e}"),
            )
        }
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
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSosSettingsJson(
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
    match node.get_sos_settings() {
        Ok(settings) => ok_json_result(&mut env, &sos_settings_json(&settings)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSosSettingsJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosSettingsInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid SOS settings payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_sos_settings(to_sos_settings_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSosPinJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosPinInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid SOS PIN payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.set_sos_pin(payload.pin) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSosStatusJson(
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
    match node.get_sos_status() {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_triggerSosJson(
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
    let payload: SosTriggerInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid SOS trigger payload: {e}"));
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
    match node.trigger_sos(parse_sos_trigger_source(payload.source.as_deref())) {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deactivateSosJson(
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
    let payload: SosDeactivateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid SOS deactivate payload: {e}"),
            );
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
    match node.deactivate_sos(payload.pin) {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosTelemetryJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosTelemetryInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid SOS telemetry payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node(&mut guard);
    match node.submit_sos_device_telemetry(to_sos_telemetry_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosAccelerometerJson(
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
    let payload: SosAccelerometerInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid SOS accelerometer payload: {e}"),
            );
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
    let at_ms = payload.at_ms.unwrap_or_else(crate::runtime::now_ms);
    match node.submit_sos_accelerometer_sample(payload.x, payload.y, payload.z, at_ms) {
        Ok(Some(status)) => ok_json_result(
            &mut env,
            &json!({ "triggered": true, "status": sos_status_json(&status) }),
        ),
        Ok(None) => ok_json_result(&mut env, &json!({ "triggered": false })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosScreenEventJson(
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
    let payload: SosScreenEventInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid SOS screen payload: {e}"));
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
    let at_ms = payload.at_ms.unwrap_or_else(crate::runtime::now_ms);
    match node.submit_sos_screen_event(at_ms) {
        Ok(Some(status)) => ok_json_result(
            &mut env,
            &json!({ "triggered": true, "status": sos_status_json(&status) }),
        ),
        Ok(None) => ok_json_result(&mut env, &json!({ "triggered": false })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosAlertsJson(
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
    match node.list_sos_alerts() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_alert_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosLocationsJson(
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
    match node.list_sos_locations() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_location_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosAudioJson(
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
    match node.list_sos_audio() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_audio_json).collect::<Vec<_>>() }),
        ),
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
