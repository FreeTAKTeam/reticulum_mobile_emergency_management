#[cfg(target_os = "android")]
use std::ffi::c_void;
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
use crate::runtime::now_ms;
use crate::types::{
    AppSettingsRecord, ApplicationAckState, ChecklistCreateFromTemplateRequest,
    ChecklistCreateOnlineRequest, ChecklistDeleteRequest, ChecklistListActiveRequest,
    ChecklistRecord, ChecklistSettingsRecord, ChecklistTaskCellSetRequest,
    ChecklistTaskRowAddRequest, ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest,
    ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest,
    ChecklistTemplateRecord, ChecklistUpdatePatch, ChecklistUpdateRequest, ConversationRecord,
    DiscoveredPluginRecord, EamProjectionRecord, EventProjectionRecord, HubDirectoryPeerRecord,
    HubDirectorySnapshot, HubMode, HubSettingsRecord, InstalledPluginRecord, InterfaceStatusRecord,
    LegacyImportPayload, LogLevel, LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfDeliveryStatus,
    LxmfFallbackStage, MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig,
    NodeError, NodeEvent, NodeStatus, PeerChange, PeerRecord, PeerState, PluginCapabilityRecord,
    PluginLxmfSendRequest, PluginSensorSampleRequest, ProjectionScope, RnodeConnectionMode,
    RnodeSettingsRecord, RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot,
    RuntimeReadinessState, SavedPeerRecord, SendLxmfRequest, SendMode, SendOutcome, SosAlertRecord,
    SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosMessageKind, SosSettingsRecord,
    SosState, SosStatusRecord, SosTriggerSource, SyncPhase, TelemetryPositionRecord,
    TelemetrySettingsRecord, TransportDeliveryState,
};

const RESULT_OK: jint = 0;
const RESULT_ERR: jint = 1;

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: jni19::JavaVM, _reserved: *mut c_void) -> jint {
    match vm.get_env() {
        Ok(env) => match btleplug::platform::init(&env) {
            Ok(()) => log::info!("btleplug Android BLE backend initialized"),
            Err(error) => {
                log::error!("btleplug Android BLE backend initialization failed: {error}")
            }
        },
        Err(error) => log::error!("btleplug Android BLE backend missing JNI env: {error}"),
    }
    jni19::sys::JNI_VERSION_1_6
}

#[derive(Default)]
struct BridgeState {
    node: Option<Node>,
    subscription: Option<Arc<EventSubscription>>,
}

fn ensure_node(guard: &mut BridgeState) -> Result<&Node, NodeError> {
    if guard.node.is_none() {
        guard.node = Some(Node::new()?);
    }
    guard.node.as_ref().ok_or(NodeError::InternalError {})
}

fn ensure_node_with_storage<'a>(
    guard: &'a mut BridgeState,
    storage_dir: Option<&str>,
) -> Result<&'a Node, NodeError> {
    if guard.node.is_none() {
        guard.node = Some(Node::with_storage_dir(storage_dir)?);
    }
    guard.node.as_ref().ok_or(NodeError::InternalError {})
}

trait JniNodeFailure {
    fn node_failure() -> Self;
}

impl JniNodeFailure for jint {
    fn node_failure() -> Self {
        RESULT_ERR
    }
}

impl JniNodeFailure for jstring {
    fn node_failure() -> Self {
        ptr::null_mut()
    }
}

macro_rules! ensure_node_or_return {
    ($guard:expr) => {
        match ensure_node($guard) {
            Ok(node) => node,
            Err(error) => {
                set_last_node_error(error);
                return <_ as JniNodeFailure>::node_failure();
            }
        }
    };
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
    transport_node_enabled: Option<bool>,
    announce_interval_seconds: Option<u32>,
    stale_after_minutes: Option<u32>,
    announce_capabilities: Option<String>,
    hub_mode: Option<String>,
    hub_identity_hash: Option<String>,
    hub_api_base_url: Option<String>,
    hub_api_key: Option<String>,
    hub_refresh_interval_seconds: Option<u32>,
    rnode: Option<RnodeSettingsInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RnodeSettingsInput {
    enabled: Option<bool>,
    connection_mode: Option<String>,
    peripheral_id: Option<String>,
    display_name: Option<String>,
    region: Option<String>,
    profile: Option<String>,
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
struct PluginApprovalInput {
    plugin_id: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginPublisherInput {
    fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginEnabledInput {
    plugin_id: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginCapabilitiesInput {
    plugin_id: String,
    capabilities: PluginCapabilityRecord,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginRuntimeStateInput {
    plugin_id: String,
    state: String,
    diagnostic: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginHostRequestInput {
    protocol_version: u16,
    request_id: String,
    plugin_id: String,
    operation: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveredPluginsInput {
    #[serde(default)]
    items: Vec<DiscoveredPluginRecord>,
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
    #[serde(default = "default_true")]
    transport_node_enabled: bool,
    announce_interval_seconds: u32,
    telemetry: TelemetrySettingsInput,
    hub: HubSettingsInput,
    #[serde(default)]
    checklists: ChecklistSettingsInput,
    #[serde(default)]
    rnode: RnodeSettingsInput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistSettingsInput {
    default_task_due_step_minutes: Option<u32>,
}

fn default_true() -> bool {
    true
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
    identity_hex: Option<String>,
    lxmf_destination_hex: Option<String>,
    app_data: Option<String>,
    display_name: Option<String>,
    last_route_seen_at_ms: Option<u64>,
    last_hops: Option<u8>,
}
