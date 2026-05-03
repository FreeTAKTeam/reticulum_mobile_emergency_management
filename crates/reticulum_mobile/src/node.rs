use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel as cb;
use reticulum::destination::DestinationName;
use rmpv::Value as MsgPackValue;
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::sync::mpsc;

use crate::app_state::{canonicalize_chat_message, AppStateStore, ConversationPeerResolver};
use crate::event_bus::EventBus;
use crate::logger::NodeLogger;
use crate::lxmf_fields::FIELD_COMMANDS;
use crate::messaging_compat as sdkmsg;
use crate::plugins::{
    NativePluginRuntime, PersistedPluginRegistry, PluginCatalog, PluginCatalogReport,
    PluginHostApi, PluginHostError, PluginInstaller, PluginLoadCandidate, PluginLoader,
    PluginLxmfMessage, PluginLxmfOutboundRequest, PluginLxmfSendRequest, PluginMessageSchemaMap,
    PluginPermissions, PluginRegistry, PluginRuntimeDiagnostic, PluginState,
};
use crate::runtime::{load_or_create_identity, now_ms, run_node, Command};
use crate::sos::{
    active_status, compose_sos_body, countdown_status, default_sos_settings, idle_status,
    new_incident_id, normalize_sos_settings, set_pin, verify_pin,
};
use crate::sos_detector::SosTriggerDetector;
use crate::sos_fields::{build_sos_fields, SosCommand};
use crate::types::{
    AnnounceRecord, AppSettingsRecord, ChecklistCreateFromTemplateRequest,
    ChecklistCreateOnlineRequest, ChecklistListActiveRequest, ChecklistRecord,
    ChecklistTaskCellSetRequest, ChecklistTaskRecord, ChecklistTaskRowAddRequest,
    ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest, ChecklistTaskStatusSetRequest,
    ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest, ChecklistTemplateRecord,
    ChecklistUpdateRequest, ConversationRecord, EamProjectionRecord, EamSourceRecord,
    EamTeamSummaryRecord, EventProjectionRecord, HubDirectorySnapshot, HubMode,
    LegacyImportPayload, LogLevel, MessageDirection, MessageMethod, MessageRecord, MessageState,
    NodeConfig, NodeError, NodeEvent, NodeStatus, OperationalSummary, PeerRecord, PeerState,
    ProjectionInvalidation, ProjectionScope, SavedPeerRecord, SendLxmfRequest, SendMode,
    SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosMessageKind,
    SosSettingsRecord, SosState, SosStatusRecord, SosTriggerSource, SyncStatus,
    TelemetryPositionRecord,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const SEND_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_QUEUE_CAPACITY: usize = 256;
fn dispatch_command(tx: &mpsc::Sender<Command>, command: Command) -> Result<(), NodeError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return tx.try_send(command).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => NodeError::Timeout {},
            mpsc::error::TrySendError::Closed(_) => NodeError::NotRunning {},
        });
    }

    tx.blocking_send(command)
        .map_err(|_| NodeError::NotRunning {})
}

fn plugin_host_error_to_node_error(error: PluginHostError) -> NodeError {
    match error {
        PluginHostError::PermissionDenied { .. }
        | PluginHostError::PluginNotFound { .. }
        | PluginHostError::Storage(NodeError::InvalidConfig {})
        | PluginHostError::LxmfMessage(_) => NodeError::InvalidConfig {},
        PluginHostError::Storage(error) => error,
    }
}

fn load_plugin_message_schemas(
    candidates: &[PluginLoadCandidate],
    plugin_id_filter: Option<&str>,
) -> Result<PluginMessageSchemaMap, NodeError> {
    let mut schemas: PluginMessageSchemaMap = BTreeMap::new();
    for candidate in candidates {
        if plugin_id_filter.is_some_and(|plugin_id| plugin_id != candidate.manifest.id.as_str()) {
            continue;
        }
        for message in &candidate.manifest.messages {
            let schema_path = candidate.install_dir.join(message.schema.as_str());
            let schema_source =
                fs_err::read_to_string(schema_path.as_path()).map_err(|_| NodeError::IoError {})?;
            let schema = serde_json::from_str(schema_source.as_str())
                .map_err(|_| NodeError::InvalidConfig {})?;
            schemas.insert(
                (candidate.manifest.id.clone(), message.name.clone()),
                schema,
            );
        }
    }
    Ok(schemas)
}

fn plugin_runtime_state_allows_host_call(state: PluginState) -> bool {
    matches!(
        state,
        PluginState::Enabled
            | PluginState::Loaded
            | PluginState::Initialized
            | PluginState::Running
    )
}

fn build_node_runtime() -> Result<Runtime, NodeError> {
    RuntimeBuilder::new_multi_thread()
        .enable_io()
        .enable_time()
        .worker_threads(2)
        .thread_name("rem-node")
        .build()
        .map_err(|_| NodeError::InternalError {})
}

fn latest_sos_telemetry(
    telemetry_store: &Arc<Mutex<Option<SosDeviceTelemetryRecord>>>,
) -> Option<SosDeviceTelemetryRecord> {
    telemetry_store
        .lock()
        .ok()
        .and_then(|telemetry| telemetry.clone())
}

struct NodeInner {
    app_state: AppStateStore,
    bus: EventBus,
    status: Arc<Mutex<NodeStatus>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    hub_directory_snapshot: Arc<Mutex<Option<HubDirectorySnapshot>>>,
    sos_device_telemetry: Arc<Mutex<Option<SosDeviceTelemetryRecord>>>,
    sos_detector: Arc<Mutex<SosTriggerDetector>>,
    active_config: Option<NodeConfigFingerprint>,
    plugin_android_abi: Option<String>,
    plugin_runtime: Option<NativePluginRuntime>,
    runtime: Option<Runtime>,
    cmd_tx: Option<mpsc::Sender<Command>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeConfigFingerprint {
    name: String,
    storage_dir: Option<String>,
    tcp_clients: Vec<String>,
    broadcast: bool,
    announce_interval_seconds: u32,
    stale_after_minutes: u32,
    announce_capabilities: String,
    hub_mode: crate::types::HubMode,
    hub_identity_hash: Option<String>,
    hub_api_base_url: Option<String>,
    hub_api_key: Option<String>,
    hub_refresh_interval_seconds: u32,
}

impl NodeConfigFingerprint {
    fn from_config(config: &NodeConfig) -> Result<Self, NodeError> {
        let name = config.name.trim();
        if name.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }

        Ok(Self {
            name: name.to_string(),
            storage_dir: config.storage_dir.clone(),
            tcp_clients: config.tcp_clients.clone(),
            broadcast: config.broadcast,
            announce_interval_seconds: config.announce_interval_seconds,
            stale_after_minutes: config.stale_after_minutes,
            announce_capabilities: config.announce_capabilities.clone(),
            hub_mode: config.hub_mode,
            hub_identity_hash: config.hub_identity_hash.clone(),
            hub_api_base_url: config.hub_api_base_url.clone(),
            hub_api_key: config.hub_api_key.clone(),
            hub_refresh_interval_seconds: config.hub_refresh_interval_seconds,
        })
    }
}

fn start_enabled_native_plugins(
    app_state: &AppStateStore,
    bus: &EventBus,
    android_abi: Option<&str>,
) -> Option<NativePluginRuntime> {
    let android_abi = android_abi
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let install_root = app_state.storage_dir().join("plugins");
    let persisted =
        match PersistedPluginRegistry::load_from_path(install_root.join("registry.json")) {
            Ok(persisted) => persisted,
            Err(error) => {
                bus.emit(NodeEvent::Error {
                    code: "PluginRuntimeError".to_string(),
                    message: format!("failed to load plug-in registry: {error}"),
                });
                return None;
            }
        };
    let mut plugin_runtime = match NativePluginRuntime::discover_with_app_state_store(
        install_root,
        android_abi,
        Some(&persisted),
        app_state.clone(),
    ) {
        Ok(plugin_runtime) => plugin_runtime,
        Err(error) => {
            bus.emit(NodeEvent::Error {
                code: "PluginRuntimeError".to_string(),
                message: format!("failed to discover native plug-ins: {error}"),
            });
            return None;
        }
    };
    plugin_runtime.start_enabled_plugins();
    emit_plugin_runtime_diagnostics(bus, plugin_runtime.diagnostics());
    Some(plugin_runtime)
}

fn emit_plugin_runtime_diagnostics(bus: &EventBus, diagnostics: &[PluginRuntimeDiagnostic]) {
    for diagnostic in diagnostics {
        let plugin_id = diagnostic.plugin_id.as_deref().unwrap_or("unknown");
        bus.emit(NodeEvent::Error {
            code: "PluginRuntimeDiagnostic".to_string(),
            message: format!("plug-in {plugin_id}: {}", diagnostic.message),
        });
    }
}

fn restart_enabled_native_plugin_runtime(inner: &mut NodeInner) {
    if let Some(mut plugin_runtime) = inner.plugin_runtime.take() {
        plugin_runtime.stop_all();
        emit_plugin_runtime_diagnostics(&inner.bus, plugin_runtime.diagnostics());
    }
    if inner.runtime.is_none() {
        return;
    }
    inner.plugin_runtime = start_enabled_native_plugins(
        &inner.app_state,
        &inner.bus,
        inner.plugin_android_abi.as_deref(),
    );
}

fn create_app_state_store(storage_dir: Option<&str>) -> AppStateStore {
    match AppStateStore::new(storage_dir) {
        Ok(store) => store,
        Err(_) => {
            let fallback = std::env::temp_dir()
                .join("reticulum_mobile_app_state")
                .to_string_lossy()
                .to_string();
            match AppStateStore::new(Some(&fallback)) {
                Ok(store) => store,
                Err(_) => panic!("failed to initialize app state store"),
            }
        }
    }
}

fn emit_projection_invalidation(bus: &EventBus, invalidation: ProjectionInvalidation) {
    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn conversation_peer_resolver(peers: &[PeerRecord]) -> ConversationPeerResolver {
    let mut resolver = ConversationPeerResolver::default();
    for peer in peers {
        let destination_hex = match trimmed_non_empty(Some(peer.destination_hex.as_str())) {
            Some(value) => value,
            None => continue,
        };
        let lxmf_destination_hex = trimmed_non_empty(peer.lxmf_destination_hex.as_deref());
        let identity_hex = trimmed_non_empty(peer.identity_hex.as_deref());
        let canonical_id = identity_hex
            .clone()
            .or_else(|| lxmf_destination_hex.clone())
            .unwrap_or_else(|| destination_hex.clone());
        let peer_destination_hex = lxmf_destination_hex
            .clone()
            .unwrap_or_else(|| destination_hex.clone());
        let mut aliases = vec![destination_hex];
        if let Some(lxmf_destination_hex) = lxmf_destination_hex {
            aliases.push(lxmf_destination_hex);
        }
        if let Some(identity_hex) = identity_hex {
            aliases.push(identity_hex);
        }
        resolver.insert(
            aliases,
            canonical_id,
            peer_destination_hex,
            peer.display_name.clone(),
        );
    }
    resolver
}

const DEFAULT_R3AKT_TEAM_COLOR: &str = "YELLOW";
const TEAM_UID_YELLOW: &str = "d6b6e188b910d6bdd24d04b7a7ec5444";
const TEAM_UID_RED: &str = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9";
const TEAM_UID_BLUE: &str = "43341e5c822d99857fa6e8641f2ca9c0";
const TEAM_UID_ORANGE: &str = "a83eb640e4c4884be14831e3d7ef5ae0";
const TEAM_UID_MAGENTA: &str = "7ac50a910f42b06cd9cb68dad3def681";
const TEAM_UID_MAROON: &str = "372824ef4f15881291455562f7570233";
const TEAM_UID_PURPLE: &str = "4bf2a1d2217c8668942658137f2a6824";
const TEAM_UID_DARK_BLUE: &str = "cbb35fc9a8f5a91d7bd2b5e5b644edcd";
const TEAM_UID_CYAN: &str = "d4cd5030b68df059ec6beabe416dd6a6";
const TEAM_UID_TEAL: &str = "4d7a7a974beec395bf83491604768499";
const TEAM_UID_GREEN: &str = "612a32262163b73a80eca944c2158546";
const TEAM_UID_DARK_GREEN: &str = "341653613d4c76d56bee99c1f38177b1";
const TEAM_UID_BROWN: &str = "4efe72ac30f5b85142fdcab6d96c7631";

#[derive(Debug, Clone)]
struct MissionReplicationTarget {
    app_destination_hex: String,
    send_mode: SendMode,
}

fn effective_hub_mode(
    configured_mode: HubMode,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> HubMode {
    match configured_mode {
        HubMode::Autonomous {} => HubMode::Autonomous {},
        HubMode::Connected {} => HubMode::Connected {},
        HubMode::SemiAutonomous {} => {
            if hub_directory_snapshot.is_some_and(|snapshot| snapshot.effective_connected_mode) {
                HubMode::Connected {}
            } else {
                HubMode::SemiAutonomous {}
            }
        }
    }
}

fn has_capability_token(app_data: Option<&str>, capability: &str) -> bool {
    let requested = capability.trim();
    if requested.is_empty() {
        return false;
    }

    app_data.is_some_and(|value| {
        value
            .split([';', ','])
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .filter(|token| !token.to_ascii_lowercase().starts_with("name="))
            .any(|token| token.eq_ignore_ascii_case(requested))
    })
}

fn telemetry_destinations_from_peers(
    peers: &[PeerRecord],
    self_destination_hex: Option<&str>,
) -> Vec<String> {
    let mut destinations = Vec::new();
    let mut seen = HashSet::<String>::new();
    for peer in peers {
        let Some(destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex == Some(destination_hex.as_str()) {
            continue;
        }
        if !peer.active_link || !has_capability_token(peer.app_data.as_deref(), "telemetry") {
            continue;
        }
        if seen.insert(destination_hex.clone()) {
            destinations.push(destination_hex);
        }
    }
    destinations
}

fn telemetry_destinations_from_hub_snapshot(
    snapshot: &HubDirectorySnapshot,
    self_destination_hex: Option<&str>,
) -> Vec<String> {
    let mut destinations = Vec::new();
    let mut seen = HashSet::<String>::new();
    for item in &snapshot.items {
        let Some(destination_hex) = normalize_hex_32(item.destination_hash.as_str()) else {
            continue;
        };
        if self_destination_hex == Some(destination_hex.as_str()) {
            continue;
        }
        if !item
            .announce_capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("telemetry"))
        {
            continue;
        }
        if seen.insert(destination_hex.clone()) {
            destinations.push(destination_hex);
        }
    }
    destinations
}

fn build_runtime_telemetry_destinations(
    status: &NodeStatus,
    peers: &[PeerRecord],
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<String>, NodeError> {
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let Some(config) = active_config else {
        return Ok(telemetry_destinations_from_peers(
            peers,
            self_destination_hex.as_deref(),
        ));
    };

    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Autonomous {} => Ok(telemetry_destinations_from_peers(
            peers,
            self_destination_hex.as_deref(),
        )),
        HubMode::Connected {} => Ok(vec![configured_hub_destination(config)?]),
        HubMode::SemiAutonomous {} => {
            if config
                .hub_identity_hash
                .as_deref()
                .and_then(normalize_hex_32)
                .is_some()
            {
                if let Some(snapshot) = hub_directory_snapshot {
                    return Ok(telemetry_destinations_from_hub_snapshot(
                        snapshot,
                        self_destination_hex.as_deref(),
                    ));
                }
            }
            Ok(telemetry_destinations_from_peers(
                peers,
                self_destination_hex.as_deref(),
            ))
        }
    }
}

fn configured_hub_destination(config: &NodeConfigFingerprint) -> Result<String, NodeError> {
    config
        .hub_identity_hash
        .as_deref()
        .and_then(normalize_hex_32)
        .ok_or(NodeError::InvalidConfig {})
}

fn routed_destination_hex(
    requested_destination_hex: String,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<String, NodeError> {
    let Some(config) = active_config else {
        return Ok(requested_destination_hex);
    };
    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Connected {} => configured_hub_destination(config),
        HubMode::Autonomous {} | HubMode::SemiAutonomous {} => Ok(requested_destination_hex),
    }
}

fn normalize_hex_32(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() == 32 && normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(normalized)
    } else {
        None
    }
}

fn is_blank(value: Option<&str>) -> bool {
    value.is_none_or(|entry| entry.trim().is_empty())
}

fn normalize_team_color(value: &str) -> &'static str {
    match value.trim().to_ascii_uppercase().as_str() {
        "RED" => "RED",
        "BLUE" => "BLUE",
        "ORANGE" => "ORANGE",
        "MAGENTA" => "MAGENTA",
        "MAROON" => "MAROON",
        "PURPLE" => "PURPLE",
        "DARK_BLUE" => "DARK_BLUE",
        "CYAN" => "CYAN",
        "TEAL" => "TEAL",
        "GREEN" => "GREEN",
        "DARK_GREEN" => "DARK_GREEN",
        "BROWN" => "BROWN",
        _ => DEFAULT_R3AKT_TEAM_COLOR,
    }
}

fn team_uid_for_color(color: &str) -> &'static str {
    match normalize_team_color(color) {
        "RED" => TEAM_UID_RED,
        "BLUE" => TEAM_UID_BLUE,
        "ORANGE" => TEAM_UID_ORANGE,
        "MAGENTA" => TEAM_UID_MAGENTA,
        "MAROON" => TEAM_UID_MAROON,
        "PURPLE" => TEAM_UID_PURPLE,
        "DARK_BLUE" => TEAM_UID_DARK_BLUE,
        "CYAN" => TEAM_UID_CYAN,
        "TEAL" => TEAM_UID_TEAL,
        "GREEN" => TEAM_UID_GREEN,
        "DARK_GREEN" => TEAM_UID_DARK_GREEN,
        "BROWN" => TEAM_UID_BROWN,
        _ => TEAM_UID_YELLOW,
    }
}

fn populate_eam_defaults(status: &NodeStatus, record: &EamProjectionRecord) -> EamProjectionRecord {
    let mut normalized = record.clone();
    let team_color = normalize_team_color(normalized.group_name.as_str());
    normalized.group_name = team_color.to_string();
    if is_blank(normalized.team_member_uid.as_deref()) {
        let app_hash = status.app_destination_hex.trim();
        if !app_hash.is_empty() {
            normalized.team_member_uid = Some(app_hash.to_string());
        }
    }
    if is_blank(normalized.team_uid.as_deref()) {
        normalized.team_uid = Some(team_uid_for_color(team_color).to_string());
    }
    if is_blank(normalized.reported_by.as_deref()) && !status.name.trim().is_empty() {
        normalized.reported_by = Some(status.name.trim().to_string());
    }
    if normalized.source.is_none() && !status.identity_hex.trim().is_empty() {
        normalized.source = Some(EamSourceRecord {
            rns_identity: status.identity_hex.clone(),
            display_name: (!status.name.trim().is_empty()).then(|| status.name.trim().to_string()),
        });
    }
    if normalized.overall_status.is_none() {
        normalized.overall_status = derive_eam_overall_status(&normalized);
    }
    normalized
}

fn has_known_lxmf_route(peer: &PeerRecord) -> bool {
    let Some(app_destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
        return false;
    };
    let Some(lxmf_destination_hex) = peer
        .lxmf_destination_hex
        .as_deref()
        .and_then(normalize_hex_32)
    else {
        return false;
    };
    app_destination_hex != lxmf_destination_hex
}

fn peer_is_directly_reachable(peer: &PeerRecord) -> bool {
    peer.active_link || matches!(peer.state, PeerState::Connected {})
}

fn build_mission_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let saved_destinations = saved_peers
        .iter()
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    let saved_destination_set = saved_destinations.iter().cloned().collect::<HashSet<_>>();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
    let mut direct_destination_set = HashSet::<String>::new();
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();

    for peer in peers {
        let Some(app_destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen_app_destinations.insert(app_destination_hex.clone()) {
            continue;
        }
        if !saved_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        let direct_ready = has_known_lxmf_route(peer) && peer_is_directly_reachable(peer);
        if direct_ready {
            direct_destination_set.insert(app_destination_hex.clone());
            direct_targets.push(MissionReplicationTarget {
                app_destination_hex,
                send_mode: SendMode::Auto {},
            });
        }
    }

    for app_destination_hex in saved_destinations {
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if direct_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        relay_targets.push(MissionReplicationTarget {
            app_destination_hex,
            send_mode: if has_active_relay {
                SendMode::PropagationOnly {}
            } else {
                SendMode::Auto {}
            },
        });
    }

    direct_targets.extend(relay_targets);
    direct_targets
}

fn build_event_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let saved_destinations = saved_peers
        .iter()
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    let saved_destination_set = saved_destinations.iter().cloned().collect::<HashSet<_>>();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
    let mut direct_destination_set = HashSet::<String>::new();
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();

    for peer in peers {
        let Some(app_destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen_app_destinations.insert(app_destination_hex.clone()) {
            continue;
        }
        if !has_known_lxmf_route(peer) {
            continue;
        }
        if !saved_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        let direct_ready = has_known_lxmf_route(peer) && peer_is_directly_reachable(peer);
        if direct_ready {
            direct_destination_set.insert(app_destination_hex.clone());
            direct_targets.push(MissionReplicationTarget {
                app_destination_hex,
                send_mode: SendMode::Auto {},
            });
        }
    }

    for app_destination_hex in saved_destinations {
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if direct_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        relay_targets.push(MissionReplicationTarget {
            app_destination_hex,
            send_mode: if has_active_relay {
                SendMode::PropagationOnly {}
            } else {
                SendMode::Auto {}
            },
        });
    }

    direct_targets.extend(relay_targets);
    direct_targets
}

fn build_transient_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    directory_destinations: &[String],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let mut targets = Vec::new();
    let mut seen = HashSet::<String>::new();
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();

    for destination_hash in directory_destinations {
        let Some(app_destination_hex) = normalize_hex_32(destination_hash.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen.insert(app_destination_hex.clone()) {
            continue;
        }

        let matched_peer = peers.iter().find(|peer| {
            normalize_hex_32(peer.destination_hex.as_str()).as_deref()
                == Some(app_destination_hex.as_str())
        });
        let send_mode = if matched_peer.is_some_and(peer_is_directly_reachable) || !has_active_relay
        {
            SendMode::Auto {}
        } else {
            SendMode::PropagationOnly {}
        };
        targets.push(MissionReplicationTarget {
            app_destination_hex,
            send_mode,
        });
    }

    targets
}

fn build_runtime_mission_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let Some(config) = active_config else {
        return Ok(build_mission_replication_targets(
            status,
            peers,
            saved_peers,
            active_propagation_node_hex,
        ));
    };

    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Autonomous {} => Ok(build_mission_replication_targets(
            status,
            peers,
            saved_peers,
            active_propagation_node_hex,
        )),
        HubMode::Connected {} => Ok(vec![MissionReplicationTarget {
            app_destination_hex: configured_hub_destination(config)?,
            send_mode: SendMode::Auto {},
        }]),
        HubMode::SemiAutonomous {} => {
            let Some(_hub_identity_hash) = config
                .hub_identity_hash
                .as_deref()
                .and_then(normalize_hex_32)
            else {
                return Ok(build_mission_replication_targets(
                    status,
                    peers,
                    saved_peers,
                    active_propagation_node_hex,
                ));
            };
            let Some(snapshot) = hub_directory_snapshot else {
                return Ok(build_mission_replication_targets(
                    status,
                    peers,
                    saved_peers,
                    active_propagation_node_hex,
                ));
            };
            Ok(build_transient_replication_targets(
                status,
                peers,
                &snapshot
                    .items
                    .iter()
                    .map(|item| item.destination_hash.clone())
                    .collect::<Vec<_>>(),
                active_propagation_node_hex,
            ))
        }
    }
}

fn build_runtime_event_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let Some(config) = active_config else {
        return Ok(build_event_replication_targets(
            status,
            peers,
            saved_peers,
            active_propagation_node_hex,
        ));
    };

    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Autonomous {} => Ok(build_event_replication_targets(
            status,
            peers,
            saved_peers,
            active_propagation_node_hex,
        )),
        HubMode::Connected {} => Ok(vec![MissionReplicationTarget {
            app_destination_hex: configured_hub_destination(config)?,
            send_mode: SendMode::Auto {},
        }]),
        HubMode::SemiAutonomous {} => {
            let Some(_hub_identity_hash) = config
                .hub_identity_hash
                .as_deref()
                .and_then(normalize_hex_32)
            else {
                return Ok(build_event_replication_targets(
                    status,
                    peers,
                    saved_peers,
                    active_propagation_node_hex,
                ));
            };
            let Some(snapshot) = hub_directory_snapshot else {
                return Ok(build_event_replication_targets(
                    status,
                    peers,
                    saved_peers,
                    active_propagation_node_hex,
                ));
            };
            Ok(build_transient_replication_targets(
                status,
                peers,
                &snapshot
                    .items
                    .iter()
                    .map(|item| item.destination_hash.clone())
                    .collect::<Vec<_>>(),
                active_propagation_node_hex,
            ))
        }
    }
}

fn eam_status_rank(value: &str) -> u8 {
    match value {
        "Green" => 1,
        "Yellow" => 2,
        "Red" => 3,
        _ => 0,
    }
}

fn derive_eam_overall_status(record: &EamProjectionRecord) -> Option<String> {
    let mut best_status: Option<&str> = None;
    for value in [
        record.security_status.as_str(),
        record.capability_status.as_str(),
        record.preparedness_status.as_str(),
        record.medical_status.as_str(),
        record.mobility_status.as_str(),
        record.comms_status.as_str(),
    ] {
        if eam_status_rank(value) >= eam_status_rank(best_status.unwrap_or_default()) {
            best_status = Some(value);
        }
    }
    best_status
        .filter(|value| !value.is_empty() && *value != "Unknown")
        .map(str::to_string)
}

fn msgpack_map(entries: Vec<(&str, MsgPackValue)>) -> MsgPackValue {
    MsgPackValue::Map(
        entries
            .into_iter()
            .map(|(key, value)| (MsgPackValue::from(key), value))
            .collect(),
    )
}

fn msgpack_string_array(values: &[String]) -> MsgPackValue {
    MsgPackValue::Array(
        values
            .iter()
            .map(|value| MsgPackValue::from(value.as_str()))
            .collect(),
    )
}

fn current_timestamp_rfc3339() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds_since_epoch = duration.as_secs() as i64;
    let nanos = duration.subsec_nanos();
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn json_value_to_msgpack(value: &JsonValue) -> Result<MsgPackValue, NodeError> {
    match value {
        JsonValue::Null => Ok(MsgPackValue::Nil),
        JsonValue::Bool(value) => Ok(MsgPackValue::Boolean(*value)),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_u64() {
                Ok(MsgPackValue::from(value))
            } else if let Some(value) = value.as_i64() {
                Ok(MsgPackValue::from(value))
            } else if let Some(value) = value.as_f64() {
                Ok(MsgPackValue::from(value))
            } else {
                Err(NodeError::InvalidConfig {})
            }
        }
        JsonValue::String(value) => Ok(MsgPackValue::from(value.as_str())),
        JsonValue::Array(values) => Ok(MsgPackValue::Array(
            values
                .iter()
                .map(json_value_to_msgpack)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        JsonValue::Object(entries) => Ok(MsgPackValue::Map(
            entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        MsgPackValue::from(key.as_str()),
                        json_value_to_msgpack(value)?,
                    ))
                })
                .collect::<Result<Vec<_>, NodeError>>()?,
        )),
    }
}

fn checklist_string_arg<'a>(args: &'a JsonMap<String, JsonValue>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(JsonValue::as_str).map(str::trim)
}

fn checklist_key_arg(args: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    checklist_string_arg(args, key)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sanitize_correlation_token(value: &str) -> String {
    let mut token = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while token.contains("--") {
        token = token.replace("--", "-");
    }
    token.trim_matches('-').to_string()
}

fn checklist_topics_from_args(args: &JsonMap<String, JsonValue>) -> Vec<String> {
    let mut topics = Vec::new();
    for key in ["mission_uid", "checklist_uid"] {
        if let Some(value) = checklist_key_arg(args, key) {
            if !topics.iter().any(|existing| existing == &value) {
                topics.push(value);
            }
        }
    }
    topics
}

fn checklist_subject_part(args: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    checklist_key_arg(args, key)
        .map(|value| sanitize_correlation_token(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn checklist_subject_token(command_type: &str, args: &JsonMap<String, JsonValue>) -> String {
    let checklist_uid = checklist_subject_part(args, "checklist_uid");
    let task_uid = checklist_subject_part(args, "task_uid");
    let column_uid = checklist_subject_part(args, "column_uid");
    if task_uid.is_some() || column_uid.is_some() {
        let parts = [
            checklist_uid.as_deref(),
            task_uid.as_deref(),
            column_uid.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if !parts.is_empty() {
            return parts.join("-");
        }
    }
    for key in ["checklist_uid", "mission_uid", "template_uid"] {
        if let Some(sanitized) = checklist_subject_part(args, key) {
            return sanitized;
        }
    }
    sanitize_correlation_token(command_type)
}

fn build_mission_command_fields(
    command_id: &str,
    correlation_id: &str,
    command_type: &str,
    args: Vec<(&str, MsgPackValue)>,
) -> Result<Vec<u8>, NodeError> {
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![msgpack_map(vec![
            ("command_id", MsgPackValue::from(command_id)),
            ("correlation_id", MsgPackValue::from(correlation_id)),
            ("command_type", MsgPackValue::from(command_type)),
            ("args", msgpack_map(args)),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
}

fn build_checklist_command_fields(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
) -> Result<Vec<u8>, NodeError> {
    let timestamp = current_timestamp_rfc3339();
    let send_ts_ms = now_ms();
    let subject = checklist_subject_token(command_type, args);
    let command_slug = sanitize_correlation_token(command_type);
    let destination_hint = &target.app_destination_hex[..target.app_destination_hex.len().min(8)];
    let correlation_id = format!("{command_slug}-{subject}-{destination_hint}-{send_ts_ms}");
    let command_id = command_id_override
        .map(str::to_string)
        .unwrap_or_else(|| format!("cmd-{correlation_id}"));
    let source_display_name = status.name.trim();
    let topics = checklist_topics_from_args(args)
        .into_iter()
        .map(MsgPackValue::from)
        .collect::<Vec<_>>();
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![msgpack_map(vec![
            ("command_id", MsgPackValue::from(command_id.as_str())),
            (
                "correlation_id",
                MsgPackValue::from(correlation_id.as_str()),
            ),
            ("command_type", MsgPackValue::from(command_type)),
            (
                "source",
                msgpack_map(vec![
                    (
                        "rns_identity",
                        MsgPackValue::from(status.identity_hex.as_str()),
                    ),
                    ("display_name", MsgPackValue::from(source_display_name)),
                ]),
            ),
            ("timestamp", MsgPackValue::from(timestamp.as_str())),
            ("topics", MsgPackValue::Array(topics)),
            (
                "args",
                json_value_to_msgpack(&JsonValue::Object(args.clone()))?,
            ),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
}

fn checklist_snapshot_msgpack_entry(
    snapshot_json: &str,
) -> Result<(&'static str, MsgPackValue), NodeError> {
    let snapshot_value = serde_json::from_str::<JsonValue>(snapshot_json)
        .map_err(|_| NodeError::InternalError {})?;
    Ok(("snapshot", json_value_to_msgpack(&snapshot_value)?))
}

fn checklist_snapshot_content_bytes(
    checklist_uid: &str,
    snapshot_json: &str,
) -> Result<Vec<u8>, NodeError> {
    let snapshot_entry = checklist_snapshot_msgpack_entry(snapshot_json)?;
    let content = msgpack_map(vec![
        ("type", MsgPackValue::from("rem.checklist.snapshot.v1")),
        ("checklist_uid", MsgPackValue::from(checklist_uid)),
        snapshot_entry,
    ]);
    rmp_serde::to_vec(&content).map_err(|_| NodeError::InternalError {})
}

fn build_checklist_replication_payload(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    build_checklist_replication_payload_with_command_id(status, target, command_type, args, None)
}

fn build_checklist_replication_payload_with_command_id(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let fields =
        build_checklist_command_fields(status, target, command_type, args, command_id_override)?;
    let body = format!(
        "Checklist {} {}",
        command_type,
        checklist_subject_token(command_type, args)
    )
    .into_bytes();
    Ok((body, fields))
}

fn build_checklist_replication_payload_with_snapshot(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
    snapshot_json: &str,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let fields =
        build_checklist_command_fields(status, target, command_type, args, command_id_override)?;
    let checklist_uid =
        checklist_key_arg(args, "checklist_uid").ok_or(NodeError::InvalidConfig {})?;
    let body = checklist_snapshot_content_bytes(checklist_uid.as_str(), snapshot_json)?;
    Ok((body, fields))
}

fn checklist_create_online_args_json(
    request: &ChecklistCreateOnlineRequest,
) -> Result<JsonMap<String, JsonValue>, NodeError> {
    let checklist_uid = request
        .checklist_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let name = request.name.trim();
    if name.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let template_uid = request.template_uid.trim();
    if template_uid.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let mission_uid = request
        .mission_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(NodeError::InvalidConfig {})?;
    let start_time = request.start_time.trim();
    if start_time.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let value = json!({
        "name": name,
        "template_uid": template_uid,
        "mission_uid": mission_uid,
        "description": request.description.trim(),
        "start_time": start_time,
    });
    match value {
        JsonValue::Object(mut map) => {
            if let Some(checklist_uid) = checklist_uid {
                map.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
            }
            Ok(map)
        }
        _ => Err(NodeError::InternalError {}),
    }
}

fn append_checklist_create_snapshot_args(
    args: &mut JsonMap<String, JsonValue>,
    checklist: &ChecklistRecord,
) -> Result<(), NodeError> {
    let JsonValue::Object(snapshot) =
        serde_json::to_value(checklist).map_err(|_| NodeError::InternalError {})?
    else {
        return Err(NodeError::InternalError {});
    };
    for key in [
        "columns",
        "participant_rns_identities",
        "created_at",
        "created_by_team_member_rns_identity",
    ] {
        if let Some(value) = snapshot.get(key) {
            args.insert(key.to_string(), value.clone());
        }
    }
    args.insert(
        "total_tasks".to_string(),
        JsonValue::from(checklist.expected_task_count.unwrap_or_else(|| {
            checklist
                .tasks
                .iter()
                .filter(|task| task.deleted_at.is_none())
                .count() as u32
        })),
    );
    Ok(())
}

fn checklist_task_row_add_args_from_task(
    checklist_uid: &str,
    task: &ChecklistTaskRecord,
    changed_by_identity: Option<&str>,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(task.task_uid.as_str()),
    );
    args.insert("number".to_string(), JsonValue::from(task.number));
    if let Some(due_relative_minutes) = task.due_relative_minutes {
        args.insert(
            "due_relative_minutes".to_string(),
            JsonValue::from(due_relative_minutes),
        );
    }
    if let Some(due_dtg) = task
        .due_dtg
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("due_dtg".to_string(), JsonValue::from(due_dtg));
    }
    if let Some(notes) = task
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("notes".to_string(), JsonValue::from(notes));
    }
    if let Some(legacy_value) = task.legacy_value.as_deref() {
        args.insert("legacy_value".to_string(), JsonValue::from(legacy_value));
    }
    if let Some(identity) = changed_by_identity
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "changed_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}

fn checklist_task_cell_args_from_cell(
    checklist_uid: &str,
    cell: &crate::types::ChecklistCellRecord,
    updated_by_identity: Option<&str>,
) -> Option<JsonMap<String, JsonValue>> {
    let value = cell.value.as_deref()?;
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(cell.task_uid.as_str()),
    );
    args.insert(
        "column_uid".to_string(),
        JsonValue::from(cell.column_uid.as_str()),
    );
    args.insert("value".to_string(), JsonValue::from(value));
    let identity = cell
        .updated_by_team_member_rns_identity
        .as_deref()
        .or(updated_by_identity)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(identity) = identity {
        args.insert(
            "updated_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    Some(args)
}

fn checklist_task_status_args_from_task(
    checklist_uid: &str,
    task: &ChecklistTaskRecord,
    changed_by_identity: Option<&str>,
) -> Option<JsonMap<String, JsonValue>> {
    if matches!(
        task.user_status,
        crate::types::ChecklistUserTaskStatus::Pending {}
    ) {
        return None;
    }
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(task.task_uid.as_str()),
    );
    args.insert(
        "user_status".to_string(),
        JsonValue::from(task.user_status.as_str()),
    );
    let identity = task
        .completed_by_team_member_rns_identity
        .as_deref()
        .or(changed_by_identity)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(identity) = identity {
        args.insert(
            "changed_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    Some(args)
}

fn checklist_task_row_style_args_from_task(
    checklist_uid: &str,
    task: &ChecklistTaskRecord,
    changed_by_identity: Option<&str>,
) -> Option<JsonMap<String, JsonValue>> {
    if task.row_background_color.is_none() && !task.line_break_enabled {
        return None;
    }
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(task.task_uid.as_str()),
    );
    if let Some(color) = task.row_background_color.as_deref() {
        args.insert("row_background_color".to_string(), JsonValue::from(color));
    }
    if task.line_break_enabled {
        args.insert("line_break_enabled".to_string(), JsonValue::from(true));
    }
    if let Some(identity) = changed_by_identity
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "changed_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    Some(args)
}

fn schedule_initial_checklist_task_payloads(
    scheduled_sends: &mut Vec<(String, Vec<u8>, Vec<u8>, SendMode)>,
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    checklist: &ChecklistRecord,
) {
    let changed_by = checklist
        .last_changed_by_team_member_rns_identity
        .as_deref()
        .or_else(|| Some(checklist.created_by_team_member_rns_identity.as_str()));
    for task in checklist
        .tasks
        .iter()
        .filter(|task| task.deleted_at.is_none())
    {
        let row_args =
            checklist_task_row_add_args_from_task(checklist.uid.as_str(), task, changed_by);
        if let Ok((body, fields)) =
            build_checklist_replication_payload(status, target, "checklist.task.row.add", &row_args)
        {
            scheduled_sends.push((
                target.app_destination_hex.clone(),
                body,
                fields,
                target.send_mode,
            ));
        }

        for cell in &task.cells {
            let Some(cell_args) =
                checklist_task_cell_args_from_cell(checklist.uid.as_str(), cell, changed_by)
            else {
                continue;
            };
            if let Ok((body, fields)) = build_checklist_replication_payload(
                status,
                target,
                "checklist.task.cell.set",
                &cell_args,
            ) {
                scheduled_sends.push((
                    target.app_destination_hex.clone(),
                    body,
                    fields,
                    target.send_mode,
                ));
            }
        }

        if let Some(status_args) =
            checklist_task_status_args_from_task(checklist.uid.as_str(), task, changed_by)
        {
            if let Ok((body, fields)) = build_checklist_replication_payload(
                status,
                target,
                "checklist.task.status.set",
                &status_args,
            ) {
                scheduled_sends.push((
                    target.app_destination_hex.clone(),
                    body,
                    fields,
                    target.send_mode,
                ));
            }
        }

        if let Some(style_args) =
            checklist_task_row_style_args_from_task(checklist.uid.as_str(), task, changed_by)
        {
            if let Ok((body, fields)) = build_checklist_replication_payload(
                status,
                target,
                "checklist.task.row.style.set",
                &style_args,
            ) {
                scheduled_sends.push((
                    target.app_destination_hex.clone(),
                    body,
                    fields,
                    target.send_mode,
                ));
            }
        }
    }
}

fn checklist_update_args_json(request: &ChecklistUpdateRequest) -> JsonMap<String, JsonValue> {
    let mut patch = JsonMap::new();
    if let Some(mission_uid) = request.patch.mission_uid.as_deref() {
        patch.insert(
            "mission_uid".to_string(),
            JsonValue::from(mission_uid.trim()),
        );
    }
    if let Some(template_uid) = request.patch.template_uid.as_deref() {
        patch.insert(
            "template_uid".to_string(),
            JsonValue::from(template_uid.trim()),
        );
    }
    if let Some(name) = request.patch.name.as_deref() {
        patch.insert("name".to_string(), JsonValue::from(name.trim()));
    }
    if let Some(description) = request.patch.description.as_deref() {
        patch.insert(
            "description".to_string(),
            JsonValue::from(description.trim()),
        );
    }
    if let Some(start_time) = request.patch.start_time.as_deref() {
        patch.insert("start_time".to_string(), JsonValue::from(start_time.trim()));
    }

    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert("patch".to_string(), JsonValue::Object(patch));
    args
}

fn checklist_uid_args_json(checklist_uid: &str) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(checklist_uid.trim()),
    );
    args
}

fn checklist_task_status_args_json(
    request: &ChecklistTaskStatusSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args.insert(
        "user_status".to_string(),
        JsonValue::from(request.user_status.as_str()),
    );
    if let Some(identity) = request
        .changed_by_team_member_rns_identity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "changed_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}

fn checklist_task_row_add_args_json(
    request: &ChecklistTaskRowAddRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    if let Some(task_uid) = request
        .task_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("task_uid".to_string(), JsonValue::from(task_uid));
    }
    args.insert("number".to_string(), JsonValue::from(request.number));
    if let Some(due_relative_minutes) = request.due_relative_minutes {
        args.insert(
            "due_relative_minutes".to_string(),
            JsonValue::from(due_relative_minutes),
        );
    }
    if let Some(legacy_value) = request.legacy_value.as_deref() {
        args.insert("legacy_value".to_string(), JsonValue::from(legacy_value));
    }
    args
}

fn checklist_task_row_delete_args_json(
    request: &ChecklistTaskRowDeleteRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args
}

fn checklist_task_row_style_args_json(
    request: &ChecklistTaskRowStyleSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    if let Some(color) = request.row_background_color.as_deref() {
        args.insert(
            "row_background_color".to_string(),
            JsonValue::from(color.trim()),
        );
    }
    if let Some(line_break_enabled) = request.line_break_enabled {
        args.insert(
            "line_break_enabled".to_string(),
            JsonValue::from(line_break_enabled),
        );
    }
    args
}

fn checklist_task_cell_args_json(
    request: &ChecklistTaskCellSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args.insert(
        "column_uid".to_string(),
        JsonValue::from(request.column_uid.trim()),
    );
    args.insert("value".to_string(), JsonValue::from(request.value.clone()));
    if let Some(identity) = request
        .updated_by_team_member_rns_identity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "updated_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}

fn build_eam_replication_payload(
    status: &NodeStatus,
    record: &EamProjectionRecord,
    target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let team_member_uid = record
        .team_member_uid
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(NodeError::InvalidConfig {})?;
    let team_uid = record
        .team_uid
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(NodeError::InvalidConfig {})?;
    if record.callsign.trim().is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let send_ts_ms = now_ms();
    let correlation_id = format!(
        "eam-upsert-{}-{}-{send_ts_ms}",
        record
            .eam_uid
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(record.callsign.as_str())
            .trim()
            .to_ascii_lowercase(),
        &target.app_destination_hex[..8],
    );
    let command_id = format!("cmd-{correlation_id}");
    let display_name = status.name.trim();
    let source_identity = status.identity_hex.as_str();
    let reported_by = record
        .reported_by
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| (!display_name.is_empty()).then_some(display_name));

    let overall_status = record
        .overall_status
        .clone()
        .or_else(|| derive_eam_overall_status(record))
        .unwrap_or_else(|| "Unknown".to_string());
    let body = format!("EAM {} {}", record.callsign.trim(), overall_status).into_bytes();

    let fields = build_mission_command_fields(
        command_id.as_str(),
        correlation_id.as_str(),
        "mission.registry.eam.upsert",
        vec![
            ("callsign", MsgPackValue::from(record.callsign.as_str())),
            ("team_member_uid", MsgPackValue::from(team_member_uid)),
            ("team_uid", MsgPackValue::from(team_uid)),
            (
                "security_status",
                MsgPackValue::from(record.security_status.as_str()),
            ),
            (
                "capability_status",
                MsgPackValue::from(record.capability_status.as_str()),
            ),
            (
                "preparedness_status",
                MsgPackValue::from(record.preparedness_status.as_str()),
            ),
            (
                "medical_status",
                MsgPackValue::from(record.medical_status.as_str()),
            ),
            (
                "mobility_status",
                MsgPackValue::from(record.mobility_status.as_str()),
            ),
            (
                "comms_status",
                MsgPackValue::from(record.comms_status.as_str()),
            ),
            (
                "source",
                msgpack_map(vec![
                    ("rns_identity", MsgPackValue::from(source_identity)),
                    (
                        "display_name",
                        MsgPackValue::from(reported_by.unwrap_or(display_name)),
                    ),
                ]),
            ),
        ]
        .into_iter()
        .chain(
            record
                .eam_uid
                .as_deref()
                .map(|value| ("eam_uid", MsgPackValue::from(value)))
                .into_iter(),
        )
        .chain(
            record
                .reported_by
                .as_deref()
                .map(|value| ("reported_by", MsgPackValue::from(value)))
                .into_iter(),
        )
        .chain(
            record
                .reported_at
                .as_deref()
                .map(|value| ("reported_at", MsgPackValue::from(value)))
                .into_iter(),
        )
        .chain(
            record
                .notes
                .as_deref()
                .map(|value| ("notes", MsgPackValue::from(value)))
                .into_iter(),
        )
        .chain(
            record
                .confidence
                .map(|value| ("confidence", MsgPackValue::from(value)))
                .into_iter(),
        )
        .chain(
            record
                .ttl_seconds
                .map(|value| ("ttl_seconds", MsgPackValue::from(value)))
                .into_iter(),
        )
        .collect(),
    )?;

    Ok((body, fields))
}

fn build_eam_delete_replication_payload(
    callsign: &str,
    deleted_at_ms: u64,
    target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let normalized_callsign = callsign.trim();
    if normalized_callsign.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let correlation_id = format!(
        "eam-delete-{}-{}-{deleted_at_ms}",
        normalized_callsign.to_ascii_lowercase(),
        &target.app_destination_hex[..8],
    );
    let command_id = format!("cmd-{correlation_id}");
    let body = format!("EAM deleted {normalized_callsign}").into_bytes();
    let fields = build_mission_command_fields(
        command_id.as_str(),
        correlation_id.as_str(),
        "mission.registry.eam.delete",
        vec![
            ("callsign", MsgPackValue::from(normalized_callsign)),
            ("deleted_at_ms", MsgPackValue::from(deleted_at_ms)),
        ],
    )?;

    Ok((body, fields))
}

fn build_event_replication_payload(
    status: &NodeStatus,
    record: &EventProjectionRecord,
    target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let uid = record.uid.trim();
    let command_id = record.command_id.trim();
    let mission_uid = record.mission_uid.trim();
    let content = record.content.trim();
    let callsign = record.callsign.trim();
    let timestamp = record.timestamp.trim();
    let command_type = record.command_type.trim();
    let source_identity = record.source_identity.trim();
    if uid.is_empty()
        || command_id.is_empty()
        || mission_uid.is_empty()
        || content.is_empty()
        || callsign.is_empty()
        || timestamp.is_empty()
        || command_type.is_empty()
        || source_identity.is_empty()
    {
        return Err(NodeError::InvalidConfig {});
    }

    let send_ts_ms = now_ms();
    let correlation_id = record
        .correlation_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "event-upsert-{uid}-{}-{send_ts_ms}",
                &target.app_destination_hex[..8]
            )
        });
    let display_name = record
        .source_display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let fallback = status.name.trim();
            if fallback.is_empty() {
                callsign
            } else {
                fallback
            }
        });
    let body = content.as_bytes().to_vec();

    let fields =
        MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from(command_id),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from(correlation_id.as_str()),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from(command_type),
                ),
                (
                    MsgPackValue::from("source"),
                    msgpack_map(vec![
                        ("rns_identity", MsgPackValue::from(source_identity)),
                        ("display_name", MsgPackValue::from(display_name)),
                    ]),
                ),
                (
                    MsgPackValue::from("timestamp"),
                    MsgPackValue::from(timestamp),
                ),
                (
                    MsgPackValue::from("args"),
                    msgpack_map(
                        vec![
                            ("entry_uid", MsgPackValue::from(uid)),
                            ("mission_uid", MsgPackValue::from(mission_uid)),
                            ("content", MsgPackValue::from(content)),
                            ("callsign", MsgPackValue::from(callsign)),
                            ("source_identity", MsgPackValue::from(source_identity)),
                            ("source_display_name", MsgPackValue::from(display_name)),
                        ]
                        .into_iter()
                        .chain(
                            record
                                .server_time
                                .as_deref()
                                .filter(|value| !value.trim().is_empty())
                                .map(|value| ("server_time", MsgPackValue::from(value)))
                                .into_iter(),
                        )
                        .chain(
                            record
                                .client_time
                                .as_deref()
                                .filter(|value| !value.trim().is_empty())
                                .map(|value| ("client_time", MsgPackValue::from(value)))
                                .into_iter(),
                        )
                        .chain((!record.keywords.is_empty()).then(|| {
                            ("keywords", msgpack_string_array(record.keywords.as_slice()))
                        }))
                        .chain((!record.content_hashes.is_empty()).then(|| {
                            (
                                "content_hashes",
                                msgpack_string_array(record.content_hashes.as_slice()),
                            )
                        }))
                        .collect(),
                    ),
                ),
                (
                    MsgPackValue::from("topics"),
                    msgpack_string_array(record.topics.as_slice()),
                ),
            ])]),
        )]);
    let fields_bytes = rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})?;

    Ok((body, fields_bytes))
}

fn emit_sos_status(
    app_state: &AppStateStore,
    bus: &EventBus,
    status: &SosStatusRecord,
    reason: &str,
) -> Result<(), NodeError> {
    let invalidation = app_state.set_sos_status(status, reason)?;
    emit_projection_invalidation(bus, invalidation);
    bus.emit(NodeEvent::SosStatusChanged {
        status: status.clone(),
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_sos_fanout(
    app_state: AppStateStore,
    bus: EventBus,
    tx: mpsc::Sender<Command>,
    status: NodeStatus,
    settings: SosSettingsRecord,
    saved_peers: Vec<SavedPeerRecord>,
    telemetry: Option<SosDeviceTelemetryRecord>,
    incident_id: String,
    trigger_source: SosTriggerSource,
    kind: SosMessageKind,
) -> Option<SosStatusRecord> {
    let now = now_ms();
    let sending = SosStatusRecord {
        state: SosState::Sending {},
        incident_id: Some(incident_id.clone()),
        trigger_source: Some(trigger_source),
        countdown_deadline_ms: None,
        activated_at_ms: if matches!(kind, SosMessageKind::Cancelled {}) {
            None
        } else {
            Some(now)
        },
        last_sent_at_ms: None,
        last_update_at_ms: None,
        updated_at_ms: now,
    };
    if emit_sos_status(&app_state, &bus, &sending, "sos-sending").is_err() {
        return None;
    }

    if matches!(kind, SosMessageKind::Active {}) && settings.audio_recording {
        bus.emit(NodeEvent::SosAudioRecordingRequested {
            incident_id: incident_id.clone(),
            duration_seconds: settings.audio_duration_seconds,
        });
    }

    let body = compose_sos_body(&settings, kind, telemetry.as_ref());
    for peer in saved_peers {
        let destination_hex = peer.destination_hex.trim().to_ascii_lowercase();
        if destination_hex.is_empty() {
            continue;
        }
        let command = SosCommand {
            state: kind,
            incident_id: incident_id.clone(),
            trigger_source,
            sent_at_ms: now,
            audio_id: None,
        };
        let fields = match build_sos_fields(&command, telemetry.as_ref()) {
            Ok(fields) => fields,
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "InternalError".to_string(),
                    message: format!(
                        "sos field encode failed destination={destination_hex} reason={err}"
                    ),
                });
                continue;
            }
        };
        let message_id_hex = format!(
            "{}-{}-{}",
            incident_id,
            destination_hex.chars().take(8).collect::<String>(),
            now
        );
        let record = canonicalize_chat_message(&MessageRecord {
            message_id_hex: message_id_hex.clone(),
            conversation_id: sdkmsg::MessagingStore::conversation_id_for(destination_hex.as_str()),
            direction: MessageDirection::Outbound {},
            destination_hex: destination_hex.clone(),
            source_hex: Some(status.lxmf_destination_hex.clone()),
            title: Some("SOS Emergency".to_string()),
            body_utf8: body.clone(),
            method: MessageMethod::Direct {},
            state: MessageState::Queued {},
            detail: Some(format!("sos:{}", crate::sos::sos_kind_label(kind))),
            sent_at_ms: Some(now),
            received_at_ms: None,
            updated_at_ms: now,
        });
        if let Ok(invalidations) = app_state.upsert_message(&record) {
            for invalidation in invalidations {
                emit_projection_invalidation(&bus, invalidation);
            }
            bus.emit(NodeEvent::MessageUpdated { message: record });
        }

        let (resp_tx, _resp_rx) = cb::bounded(1);
        if let Err(err) = dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex: destination_hex.clone(),
                bytes: body.as_bytes().to_vec(),
                fields_bytes: Some(fields),
                send_mode: SendMode::Auto {},
                resp: resp_tx,
            },
        ) {
            bus.emit(NodeEvent::Error {
                code: "NotRunning".to_string(),
                message: format!(
                    "sos send enqueue failed destination={destination_hex} reason={err}"
                ),
            });
        }
    }

    let next = if matches!(kind, SosMessageKind::Cancelled {}) {
        idle_status()
    } else {
        active_status(incident_id, trigger_source, now)
    };
    if emit_sos_status(&app_state, &bus, &next, "sos-fanout-complete").is_err() {
        return None;
    }
    Some(next)
}

pub struct Node {
    inner: Mutex<NodeInner>,
}

impl Node {
    pub fn new() -> Self {
        Self::with_storage_dir(None)
    }

    pub(crate) fn with_storage_dir(storage_dir: Option<&str>) -> Self {
        NodeLogger::install();

        let initial = NodeStatus {
            running: false,
            name: "reticulum-mobile".to_string(),
            identity_hex: String::new(),
            app_destination_hex: String::new(),
            lxmf_destination_hex: String::new(),
        };

        Self {
            inner: Mutex::new(NodeInner {
                app_state: create_app_state_store(storage_dir),
                bus: EventBus::new(),
                status: Arc::new(Mutex::new(initial)),
                peers_snapshot: Arc::new(Mutex::new(Vec::new())),
                sync_status_snapshot: Arc::new(Mutex::new(SyncStatus {
                    phase: crate::types::SyncPhase::Idle {},
                    active_propagation_node_hex: None,
                    requested_at_ms: None,
                    completed_at_ms: None,
                    messages_received: 0,
                    detail: None,
                })),
                hub_directory_snapshot: Arc::new(Mutex::new(None)),
                sos_device_telemetry: Arc::new(Mutex::new(None)),
                sos_detector: Arc::new(Mutex::new(SosTriggerDetector::new())),
                active_config: None,
                plugin_android_abi: None,
                plugin_runtime: None,
                runtime: None,
                cmd_tx: None,
            }),
        }
    }

    pub(crate) fn initialize_storage(&self, storage_dir: Option<&str>) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        if inner.runtime.is_some() {
            return Ok(());
        }
        inner.app_state = create_app_state_store(storage_dir);
        Ok(())
    }

    fn start_fresh(
        &self,
        config: NodeConfig,
        config_fingerprint: NodeConfigFingerprint,
    ) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        if inner.runtime.is_some() {
            return Err(NodeError::AlreadyRunning {});
        }

        let identity = load_or_create_identity(config.storage_dir.as_deref(), &config.name)?;

        let app_hash = reticulum::destination::SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .desc
        .address_hash;
        let lxmf_hash = reticulum::destination::SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .desc
        .address_hash;

        if let Ok(mut guard) = inner.status.lock() {
            *guard = NodeStatus {
                running: false,
                name: config.name.clone(),
                identity_hex: identity.address_hash().to_hex_string(),
                app_destination_hex: app_hash.to_hex_string(),
                lxmf_destination_hex: lxmf_hash.to_hex_string(),
            };
        }

        let prestart_state = {
            let legacy_import_completed = inner.app_state.legacy_import_completed()?;
            let app_settings = inner.app_state.get_app_settings()?;
            let saved_peers = inner.app_state.get_saved_peers()?;
            let eams = inner.app_state.get_eams()?;
            let events = inner.app_state.get_events()?;
            let messages = inner.app_state.list_messages(None)?;
            let telemetry_positions = inner.app_state.get_telemetry_positions()?;

            if legacy_import_completed
                || app_settings.is_some()
                || !saved_peers.is_empty()
                || !eams.is_empty()
                || !events.is_empty()
                || !messages.is_empty()
                || !telemetry_positions.is_empty()
            {
                Some(LegacyImportPayload {
                    settings: app_settings,
                    saved_peers,
                    eams,
                    events,
                    messages,
                    telemetry_positions,
                })
            } else {
                None
            }
        };

        inner.app_state = create_app_state_store(config.storage_dir.as_deref());
        if let Some(prestart_state) = prestart_state {
            inner.app_state.import_legacy_state(&prestart_state)?;
        }

        // Forward Rust logs to the UI event bus.
        NodeLogger::global().set_bus(Some(inner.bus.clone()));

        if let Ok(guard) = inner.status.lock() {
            inner.bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }

        let runtime = build_node_runtime()?;
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);

        let plugin_android_abi = inner.plugin_android_abi.clone();
        runtime.spawn(run_node(
            config,
            identity,
            inner.app_state.clone(),
            inner.status.clone(),
            inner.peers_snapshot.clone(),
            inner.sync_status_snapshot.clone(),
            inner.hub_directory_snapshot.clone(),
            inner.bus.clone(),
            cmd_rx,
        ));

        inner.runtime = Some(runtime);
        inner.cmd_tx = Some(cmd_tx);
        inner.active_config = Some(config_fingerprint);
        inner.plugin_runtime = start_enabled_native_plugins(
            &inner.app_state,
            &inner.bus,
            plugin_android_abi.as_deref(),
        );

        Ok(())
    }

    pub fn start(&self, config: NodeConfig) -> Result<(), NodeError> {
        let config_fingerprint = NodeConfigFingerprint::from_config(&config)?;
        let should_restart = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            match (&inner.runtime, &inner.active_config) {
                (Some(_), Some(active_config)) if active_config == &config_fingerprint => {
                    return Ok(());
                }
                (Some(_), _) => true,
                _ => false,
            }
        };

        if should_restart {
            self.stop()?;
        }

        self.start_fresh(config, config_fingerprint)
    }

    pub(crate) fn set_plugin_android_abi(
        &self,
        android_abi: Option<&str>,
    ) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.plugin_android_abi = android_abi
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        let (
            runtime,
            cmd_tx,
            bus,
            status,
            peers_snapshot,
            sync_status_snapshot,
            hub_directory_snapshot,
            plugin_runtime,
        ) = {
            let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.active_config = None;
            (
                inner.runtime.take(),
                inner.cmd_tx.take(),
                inner.bus.clone(),
                inner.status.clone(),
                inner.peers_snapshot.clone(),
                inner.sync_status_snapshot.clone(),
                inner.hub_directory_snapshot.clone(),
                inner.plugin_runtime.take(),
            )
        };

        if let Some(mut plugin_runtime) = plugin_runtime {
            plugin_runtime.stop_all();
            emit_plugin_runtime_diagnostics(&bus, plugin_runtime.diagnostics());
        }

        let Some(runtime) = runtime else {
            return Ok(());
        };

        if let Some(cmd_tx) = cmd_tx {
            let (tx, rx) = cb::bounded(1);
            let _ = dispatch_command(&cmd_tx, Command::Stop { resp: tx });
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }

        drop(runtime);
        NodeLogger::global().set_bus(None);

        if let Ok(mut guard) = status.lock() {
            guard.running = false;
            bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }
        if let Ok(mut guard) = peers_snapshot.lock() {
            guard.clear();
        }
        if let Ok(mut guard) = sync_status_snapshot.lock() {
            *guard = SyncStatus {
                phase: crate::types::SyncPhase::Idle {},
                active_propagation_node_hex: None,
                requested_at_ms: None,
                completed_at_ms: None,
                messages_received: 0,
                detail: None,
            };
        }
        if let Ok(mut guard) = hub_directory_snapshot.lock() {
            *guard = None;
        }

        Ok(())
    }

    pub fn restart(&self, config: NodeConfig) -> Result<(), NodeError> {
        self.stop()?;
        self.start(config)
    }

    pub fn get_status(&self) -> NodeStatus {
        let inner = self.inner.lock().ok();
        let Some(inner) = inner else {
            return NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
            };
        };

        inner
            .status
            .lock()
            .map(|v| v.clone())
            .unwrap_or(NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
            })
    }

    pub fn connect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ConnectPeer {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn disconnect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::DisconnectPeer {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_bytes(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
    ) -> Result<(), NodeError> {
        let (tx, active_config, hub_directory_snapshot) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
            )
        };
        let destination_hex = routed_destination_hex(
            destination_hex,
            active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?;

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                resp: resp_tx,
            },
        )
    }

    fn send_bytes_sync(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
    ) -> Result<(), NodeError> {
        let (tx, active_config, hub_directory_snapshot) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
            )
        };
        let destination_hex = routed_destination_hex(
            destination_hex,
            active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?;

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn broadcast_bytes(&self, bytes: Vec<u8>) -> Result<(), NodeError> {
        let (tx, active_config, hub_directory_snapshot) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
            )
        };
        if let Some(config) = active_config.as_ref() {
            match effective_hub_mode(config.hub_mode, hub_directory_snapshot.as_ref()) {
                HubMode::Connected {} => {
                    return self.send_bytes(
                        configured_hub_destination(config)?,
                        bytes,
                        None,
                        SendMode::Auto {},
                    );
                }
                HubMode::SemiAutonomous {} => {
                    if config
                        .hub_identity_hash
                        .as_deref()
                        .and_then(normalize_hex_32)
                        .is_some()
                    {
                        if let Some(snapshot) = hub_directory_snapshot.as_ref() {
                            for item in &snapshot.items {
                                self.send_bytes(
                                    item.destination_hash.clone(),
                                    bytes.clone(),
                                    None,
                                    SendMode::Auto {},
                                )?;
                            }
                            return Ok(());
                        }
                    }
                }
                HubMode::Autonomous {} => {}
            }
        }

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::BroadcastBytes {
                bytes,
                resp: resp_tx,
            },
        )
    }

    pub fn announce_now(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        dispatch_command(&tx, Command::AnnounceNow {})
    }

    pub fn request_peer_identity(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RequestPeerIdentity {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_lxmf(&self, request: SendLxmfRequest) -> Result<String, NodeError> {
        let (tx, active_config, hub_directory_snapshot) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
            )
        };
        let request = SendLxmfRequest {
            destination_hex: routed_destination_hex(
                request.destination_hex,
                active_config.as_ref(),
                hub_directory_snapshot.as_ref(),
            )?,
            ..request
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SendLxmf {
                request,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_plugin_lxmf_outbound(
        &self,
        request: PluginLxmfOutboundRequest,
    ) -> Result<(), NodeError> {
        self.send_bytes_sync(
            request.destination_hex,
            request.body_utf8.into_bytes(),
            Some(request.fields_bytes),
            request.send_mode,
        )
    }

    pub fn send_plugin_lxmf(
        &self,
        android_abi: &str,
        request: PluginLxmfSendRequest,
    ) -> Result<(), NodeError> {
        let outbound = self.build_plugin_lxmf_outbound_request(android_abi, request)?;
        self.send_plugin_lxmf_outbound(outbound)
    }

    pub(crate) fn build_plugin_lxmf_outbound_request(
        &self,
        android_abi: &str,
        request: PluginLxmfSendRequest,
    ) -> Result<PluginLxmfOutboundRequest, NodeError> {
        let (registry, message_schemas) =
            self.plugin_context_for_android_abi(android_abi, Some(request.plugin_id.as_str()))?;
        let plugin = registry
            .get(request.plugin_id.as_str())
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin_runtime_state_allows_host_call(plugin.state) {
            return Err(NodeError::InvalidConfig {});
        }
        let app_state = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.app_state.clone()
        };
        let mut host_api = PluginHostApi::new_with_message_schemas_and_app_state_store(
            registry,
            message_schemas,
            app_state,
        );
        host_api
            .request_lxmf_send_to(
                request.plugin_id.as_str(),
                request.destination_hex.as_str(),
                request.message_name.as_str(),
                request.payload,
                request.body_utf8.as_str(),
                request.title,
                request.send_mode,
            )
            .map_err(plugin_host_error_to_node_error)
    }

    pub fn receive_plugin_lxmf_fields(
        &self,
        android_abi: &str,
        fields_bytes: &[u8],
    ) -> Result<Option<PluginLxmfMessage>, NodeError> {
        let Some(plugin_id) = PluginLxmfMessage::try_plugin_id_from_fields_bytes(fields_bytes)
            .map_err(PluginHostError::from)
            .map_err(plugin_host_error_to_node_error)?
        else {
            return Ok(None);
        };
        let (registry, message_schemas) =
            self.plugin_context_for_android_abi(android_abi, Some(plugin_id.as_str()))?;
        let plugin = registry
            .get(plugin_id.as_str())
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin_runtime_state_allows_host_call(plugin.state) {
            return Err(NodeError::InvalidConfig {});
        }
        let app_state = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.app_state.clone()
        };
        let mut host_api = PluginHostApi::new_with_message_schemas_and_app_state_store(
            registry,
            message_schemas,
            app_state,
        );
        let message = host_api
            .receive_lxmf_fields(fields_bytes)
            .map_err(plugin_host_error_to_node_error)?;
        if let Some(message) = message.as_ref() {
            self.dispatch_plugin_lxmf_message_received(message)?;
        }
        Ok(message)
    }

    fn dispatch_plugin_lxmf_message_received(
        &self,
        message: &PluginLxmfMessage,
    ) -> Result<(), NodeError> {
        let diagnostics = {
            let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let Some(plugin_runtime) = inner.plugin_runtime.as_mut() else {
                return Ok(());
            };
            let previous_diagnostic_count = plugin_runtime.diagnostics().len();
            plugin_runtime.dispatch_lxmf_message_received(message);
            plugin_runtime.diagnostics()[previous_diagnostic_count..].to_vec()
        };
        if !diagnostics.is_empty() {
            let bus = self
                .inner
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .bus
                .clone();
            emit_plugin_runtime_diagnostics(&bus, diagnostics.as_slice());
        }
        Ok(())
    }

    fn plugin_context_for_android_abi(
        &self,
        android_abi: &str,
        schema_plugin_id: Option<&str>,
    ) -> Result<(PluginRegistry, PluginMessageSchemaMap), NodeError> {
        let install_root = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.app_state.storage_dir().join("plugins")
        };
        let registry_path = install_root.join("registry.json");
        let discovery = PluginLoader::new(install_root.as_path())
            .discover_installed_plugins(android_abi)
            .map_err(|_| NodeError::IoError {})?;
        let message_schemas =
            load_plugin_message_schemas(discovery.candidates.as_slice(), schema_plugin_id)?;
        let mut registry = PluginRegistry::from_manifests(
            discovery
                .candidates
                .into_iter()
                .map(|candidate| candidate.manifest)
                .collect(),
        )
        .map_err(|_| NodeError::InvalidConfig {})?;
        let persisted = PersistedPluginRegistry::load_from_path(registry_path.as_path())
            .map_err(|_| NodeError::IoError {})?;
        registry.apply_persisted_state(&persisted);
        Ok((registry, message_schemas))
    }

    pub fn retry_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RetryLxmf {
                message_id_hex,
                resp: resp_tx,
            },
        )
    }

    pub fn cancel_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::CancelLxmf {
                message_id_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_active_propagation_node(
        &self,
        destination_hex: Option<String>,
    ) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SetActivePropagationNode {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn request_lxmf_sync(&self, limit: Option<u32>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RequestLxmfSync {
                limit,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_announces(&self) -> Result<Vec<AnnounceRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::ListAnnounces { resp: resp_tx })?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_plugins(&self, android_abi: &str) -> Result<PluginCatalogReport, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let install_root = inner.app_state.storage_dir().join("plugins");
        let persisted = PersistedPluginRegistry::load_from_path(install_root.join("registry.json"))
            .map_err(|_| NodeError::IoError {})?;
        PluginCatalog::new(install_root)
            .list_installed_plugins_with_state(android_abi, Some(&persisted))
            .map_err(|_| NodeError::IoError {})
    }

    pub fn install_plugin_package_dir(
        &self,
        android_abi: &str,
        package_path: &str,
    ) -> Result<PluginCatalogReport, NodeError> {
        let (storage_root, install_root) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let storage_root = inner.app_state.storage_dir();
            let install_root = storage_root.join("plugins");
            (storage_root, install_root)
        };
        let staged_root = storage_root.join("plugin-packages");
        fs_err::create_dir_all(staged_root.as_path()).map_err(|_| NodeError::IoError {})?;
        let staged_root =
            fs_err::canonicalize(staged_root.as_path()).map_err(|_| NodeError::IoError {})?;
        let package_path =
            fs_err::canonicalize(Path::new(package_path)).map_err(|_| NodeError::IoError {})?;
        if !package_path.starts_with(staged_root.as_path()) {
            return Err(NodeError::InvalidConfig {});
        }

        let installer = PluginInstaller::new(install_root);
        if package_path.is_dir() {
            installer
                .install_from_package_dir(package_path.as_path(), android_abi)
                .map_err(|_| NodeError::InvalidConfig {})?;
        } else if package_path.is_file() {
            installer
                .install_from_archive(package_path.as_path(), android_abi)
                .map_err(|_| NodeError::InvalidConfig {})?;
        } else {
            return Err(NodeError::IoError {});
        }
        self.list_plugins(android_abi)
    }

    pub fn set_plugin_enabled(
        &self,
        android_abi: &str,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<(), NodeError> {
        self.update_persisted_plugin_registry(android_abi, |registry| {
            if enabled {
                registry.enable(plugin_id)
            } else {
                registry.disable(plugin_id)
            }
        })?;
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        restart_enabled_native_plugin_runtime(&mut inner);
        Ok(())
    }

    pub fn grant_plugin_permissions(
        &self,
        android_abi: &str,
        plugin_id: &str,
        granted_permissions: PluginPermissions,
    ) -> Result<(), NodeError> {
        self.update_persisted_plugin_registry(android_abi, |registry| {
            registry.grant_permissions(plugin_id, |permissions| {
                *permissions = granted_permissions;
            })
        })
    }

    fn update_persisted_plugin_registry(
        &self,
        android_abi: &str,
        update: impl FnOnce(&mut PluginRegistry) -> Result<(), crate::plugins::PluginRegistryError>,
    ) -> Result<(), NodeError> {
        let install_root = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.app_state.storage_dir().join("plugins")
        };
        let registry_path = install_root.join("registry.json");
        let discovery = PluginLoader::new(install_root.as_path())
            .discover_installed_plugins(android_abi)
            .map_err(|_| NodeError::IoError {})?;
        let persisted = PersistedPluginRegistry::load_from_path(registry_path.as_path())
            .map_err(|_| NodeError::IoError {})?;
        let manifests = discovery
            .candidates
            .into_iter()
            .map(|candidate| candidate.manifest)
            .collect();
        let mut registry =
            PluginRegistry::from_manifests(manifests).map_err(|_| NodeError::InvalidConfig {})?;
        registry.apply_persisted_state(&persisted);
        update(&mut registry).map_err(|_| NodeError::InvalidConfig {})?;
        registry
            .to_persisted_state()
            .save_to_path(registry_path.as_path())
            .map_err(|_| NodeError::IoError {})
    }

    pub fn list_peers(&self) -> Result<Vec<PeerRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .peers_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| NodeError::InternalError {})
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let resolver = conversation_peer_resolver(&peers);
        inner.app_state.list_conversations_resolved(&resolver)
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<String>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let resolver = conversation_peer_resolver(&peers);
        inner
            .app_state
            .list_messages_resolved(conversation_id.as_deref(), &resolver)
    }

    pub fn delete_conversation(&self, conversation_id: String) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let resolver = conversation_peer_resolver(&peers);
        for invalidation in inner
            .app_state
            .delete_conversation_resolved(conversation_id.as_str(), &resolver)?
        {
            emit_projection_invalidation(&inner.bus, invalidation);
        }
        Ok(())
    }

    pub fn get_lxmf_sync_status(&self) -> Result<SyncStatus, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .sync_status_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| NodeError::InternalError {})
    }

    pub fn list_telemetry_destinations(&self) -> Result<Vec<String>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let status = inner
            .status
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let hub_directory_snapshot = inner
            .hub_directory_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        build_runtime_telemetry_destinations(
            &status,
            peers.as_slice(),
            inner.active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )
    }

    pub fn set_announce_capabilities(&self, capability_string: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SetAnnounceCapabilities {
                capability_string,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_log_level(&self, level: LogLevel) {
        NodeLogger::global().set_level(level);
        if let Ok(inner) = self.inner.lock() {
            if let Some(tx) = inner.cmd_tx.clone() {
                let _ = tx.try_send(Command::SetLogLevel { level });
            }
        }
    }

    pub fn legacy_import_completed(&self) -> Result<bool, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.legacy_import_completed()
    }

    pub fn import_legacy_state(&self, payload: LegacyImportPayload) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidations = inner.app_state.import_legacy_state(&payload)?;
        for invalidation in invalidations {
            emit_projection_invalidation(&inner.bus, invalidation);
        }
        Ok(())
    }

    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_app_settings()
    }

    pub fn set_app_settings(&self, settings: AppSettingsRecord) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.set_app_settings(&settings)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("settings-updated".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn get_saved_peers(&self) -> Result<Vec<SavedPeerRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_saved_peers()
    }

    pub fn set_saved_peers(&self, peers: Vec<SavedPeerRecord>) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.set_saved_peers(&peers)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("saved-peers-updated".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn list_active_checklists(
        &self,
        request: Option<ChecklistListActiveRequest>,
    ) -> Result<Vec<ChecklistRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let mut items = inner.app_state.get_active_checklists()?;
        if let Some(request) = request {
            if let Some(search) = request
                .search
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let needle = search.to_ascii_lowercase();
                items.retain(|item| {
                    [
                        Some(item.uid.as_str()),
                        Some(item.name.as_str()),
                        Some(item.description.as_str()),
                        item.mission_uid.as_deref(),
                        item.template_uid.as_deref(),
                        item.template_name.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|value| value.to_ascii_lowercase().contains(needle.as_str()))
                });
            }
            match request.sort_by.as_deref().map(str::trim) {
                Some("name_asc") => items.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                        .then_with(|| left.uid.cmp(&right.uid))
                }),
                Some("name_desc") => items.sort_by(|left, right| {
                    right
                        .name
                        .to_ascii_lowercase()
                        .cmp(&left.name.to_ascii_lowercase())
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
                Some("updated_at_asc") | Some("created_at_asc") => items.sort_by(|left, right| {
                    left.updated_at
                        .cmp(&right.updated_at)
                        .then_with(|| left.created_at.cmp(&right.created_at))
                        .then_with(|| left.uid.cmp(&right.uid))
                }),
                Some("created_at_desc") => items.sort_by(|left, right| {
                    right
                        .created_at
                        .cmp(&left.created_at)
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
                _ => items.sort_by(|left, right| {
                    right
                        .updated_at
                        .cmp(&left.updated_at)
                        .then_with(|| right.created_at.cmp(&left.created_at))
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
            }
        }
        Ok(items)
    }

    pub fn get_checklist(
        &self,
        checklist_uid: String,
    ) -> Result<Option<ChecklistRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_checklist(checklist_uid.trim())
    }

    pub fn list_checklist_templates(
        &self,
        request: Option<ChecklistTemplateListRequest>,
    ) -> Result<Vec<ChecklistTemplateRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let mut items = inner.app_state.list_checklist_templates()?;
        if let Some(request) = request {
            if let Some(search) = request
                .search
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let needle = search.to_ascii_lowercase();
                items.retain(|record| {
                    record.uid.to_ascii_lowercase().contains(needle.as_str())
                        || record.name.to_ascii_lowercase().contains(needle.as_str())
                        || record
                            .description
                            .to_ascii_lowercase()
                            .contains(needle.as_str())
                });
            }
        }
        Ok(items)
    }

    pub fn get_checklist_template(
        &self,
        template_uid: String,
    ) -> Result<Option<ChecklistTemplateRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_checklist_template(template_uid.trim())
    }

    pub fn import_checklist_template_csv(
        &self,
        request: ChecklistTemplateImportCsvRequest,
    ) -> Result<ChecklistTemplateRecord, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.import_checklist_template_csv(&request)
    }

    pub fn create_checklist_from_template(
        &self,
        request: ChecklistCreateFromTemplateRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .checklist_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.checklist_uid = Some(format!("chk-{}", now_ms()));
            }
            if request
                .created_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.created_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let checklist_uid = request
                .checklist_uid
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(NodeError::InvalidConfig {})?
                .to_string();
            let create_command_id = format!("cmd-{checklist_uid}");
            let invalidations = inner.app_state.create_checklist_from_template(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let create_request = ChecklistCreateOnlineRequest {
                    checklist_uid: Some(checklist_uid.clone()),
                    mission_uid: request.mission_uid.clone(),
                    template_uid: request.template_uid.clone(),
                    name: request.name.clone(),
                    description: request.description.clone(),
                    start_time: request.start_time.clone(),
                    created_by_team_member_rns_identity: request
                        .created_by_team_member_rns_identity
                        .clone(),
                    created_by_team_member_display_name: request
                        .created_by_team_member_display_name
                        .clone(),
                };
                let mut snapshot = inner
                    .app_state
                    .get_checklist_any(checklist_uid.as_str())?
                    .ok_or(NodeError::InternalError {})?;
                snapshot.uploaded_at = Some(current_timestamp_rfc3339());
                snapshot.last_changed_by_team_member_rns_identity =
                    request.created_by_team_member_rns_identity.clone();
                snapshot.sync_state = crate::types::ChecklistSyncState::Synced {};
                let invalidations = inner
                    .app_state
                    .upsert_checklist(&snapshot, "checklist-uploaded")?;
                for invalidation in invalidations {
                    emit_projection_invalidation(&inner.bus, invalidation);
                }
                let mut create_args = checklist_create_online_args_json(&create_request)?;
                append_checklist_create_snapshot_args(&mut create_args, &snapshot)?;
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_checklist_replication_payload_with_command_id(
                        &status,
                        &target,
                        "checklist.create.online",
                        &create_args,
                        Some(create_command_id.as_str()),
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.create.online", err
                            ),
                        }),
                    }
                    schedule_initial_checklist_task_payloads(
                        &mut scheduled_sends,
                        &status,
                        &target,
                        &snapshot,
                    );
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.create.online/checklist.task.*", err
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn create_online_checklist(
        &self,
        request: ChecklistCreateOnlineRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .checklist_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.checklist_uid = Some(format!("chk-{}", now_ms()));
            }
            if request
                .created_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.created_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let checklist_uid = request
                .checklist_uid
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(NodeError::InvalidConfig {})?
                .to_string();
            let mut args = checklist_create_online_args_json(&request)?;
            let command_id = format!("cmd-{checklist_uid}");
            let invalidations = inner.app_state.create_online_checklist(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }
            let checklist = inner
                .app_state
                .get_checklist_any(checklist_uid.as_str())?
                .ok_or(NodeError::InternalError {})?;
            append_checklist_create_snapshot_args(&mut args, &checklist)?;

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_checklist_replication_payload_with_command_id(
                        &status,
                        &target,
                        "checklist.create.online",
                        &args,
                        Some(command_id.as_str()),
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.create.online", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.create.online", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn upload_checklist(&self, checklist_uid: String) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let normalized_uid = checklist_uid.trim();
        if normalized_uid.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut checklist = inner
                .app_state
                .get_checklist_any(normalized_uid)?
                .ok_or(NodeError::InvalidConfig {})?;
            if checklist.deleted_at.is_some() {
                return Err(NodeError::InvalidConfig {});
            }
            checklist.uploaded_at = Some(current_timestamp_rfc3339());
            checklist.last_changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            checklist.sync_state = crate::types::ChecklistSyncState::Synced {};
            let invalidations = inner
                .app_state
                .upsert_checklist(&checklist, "checklist-uploaded")?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let args = checklist_uid_args_json(normalized_uid);
                let snapshot_json =
                    serde_json::to_string(&checklist).map_err(|_| NodeError::InternalError {})?;
                let upload_command_id = format!("cmd-{normalized_uid}-upload");
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_checklist_replication_payload_with_snapshot(
                        &status,
                        &target,
                        "checklist.upload",
                        &args,
                        Some(upload_command_id.as_str()),
                        snapshot_json.as_str(),
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.upload", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.upload", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn update_checklist(&self, request: ChecklistUpdateRequest) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.update_checklist(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_update_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.update",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.update", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.update", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_checklist(&self, checklist_uid: String) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let normalized_uid = checklist_uid.trim().to_string();
            let invalidations = inner.app_state.delete_checklist_with_actor(
                normalized_uid.as_str(),
                Some(status.identity_hex.as_str()),
            )?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_uid_args_json(normalized_uid.as_str());
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.delete",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.delete", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.delete", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn join_checklist(&self, checklist_uid: String) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let normalized_uid = checklist_uid.trim().to_string();
            let mut checklist = inner
                .app_state
                .get_checklist_any(normalized_uid.as_str())?
                .ok_or(NodeError::InvalidConfig {})?;
            if checklist.deleted_at.is_some() {
                return Err(NodeError::InvalidConfig {});
            }
            if !status.identity_hex.trim().is_empty()
                && !checklist
                    .participant_rns_identities
                    .iter()
                    .any(|value| value == &status.identity_hex)
            {
                checklist
                    .participant_rns_identities
                    .push(status.identity_hex.clone());
                checklist.updated_at = Some(current_timestamp_rfc3339());
                checklist.last_changed_by_team_member_rns_identity =
                    Some(status.identity_hex.clone());
                let invalidations = inner
                    .app_state
                    .upsert_checklist(&checklist, "checklist-joined")?;
                for invalidation in invalidations {
                    emit_projection_invalidation(&inner.bus, invalidation);
                }
            }
            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_uid_args_json(normalized_uid.as_str());
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.join",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.join", err
                            ),
                        }),
                    }
                }
            }
            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.join", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn set_checklist_task_status(
        &self,
        request: ChecklistTaskStatusSetRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.set_checklist_task_status(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_task_status_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.status.set",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.task.status.set", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.status.set", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn add_checklist_task_row(
        &self,
        request: ChecklistTaskRowAddRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .task_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.task_uid = Some(format!(
                    "{}-task-{}-{}",
                    request.checklist_uid.trim(),
                    request.number,
                    now_ms()
                ));
            }
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.add_checklist_task_row(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_task_row_add_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.row.add",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.task.row.add", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.row.add", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_checklist_task_row(
        &self,
        request: ChecklistTaskRowDeleteRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.delete_checklist_task_row(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_task_row_delete_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.row.delete",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.task.row.delete", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.row.delete", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn set_checklist_task_row_style(
        &self,
        request: ChecklistTaskRowStyleSetRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.set_checklist_task_row_style(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_task_row_style_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.row.style.set",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.task.row.style.set", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.row.style.set", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn set_checklist_task_cell(
        &self,
        request: ChecklistTaskCellSetRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let mut request = request;
            if request
                .updated_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.updated_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.set_checklist_task_cell(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let args = checklist_task_cell_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.cell.set",
                        &args,
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.task.cell.set", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.cell.set", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn get_eams(&self) -> Result<Vec<EamProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eams()
    }

    pub fn upsert_eam(&self, record: EamProjectionRecord) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let normalized_record = populate_eam_defaults(&status, &record);
            let invalidation = inner.app_state.upsert_eam(&normalized_record)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("eam-upserted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_eam_replication_payload(&status, &normalized_record, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((
                                target.app_destination_hex.clone(),
                                body,
                                fields,
                                target.send_mode,
                            ));
                        }
                        Err(err) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "InvalidConfig".to_string(),
                                message: format!(
                                    "eam replication skipped destination={} callsign={} reason={}",
                                    target.app_destination_hex, normalized_record.callsign, err
                                ),
                            });
                        }
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "eam replication enqueue failed destination={} callsign={} reason={}",
                        destination_hex, record.callsign, err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_eam(&self, callsign: String, deleted_at_ms: u64) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let invalidation = inner.app_state.delete_eam(&callsign, deleted_at_ms)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("eam-deleted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_eam_delete_replication_payload(&callsign, deleted_at_ms, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((
                                target.app_destination_hex.clone(),
                                body,
                                fields,
                                target.send_mode,
                            ));
                        }
                        Err(err) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "InvalidConfig".to_string(),
                                message: format!(
                                    "eam delete replication skipped destination={} callsign={} reason={}",
                                    target.app_destination_hex, callsign, err
                                ),
                            });
                        }
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes_sync(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "eam delete replication enqueue failed destination={} callsign={} reason={}",
                        destination_hex, callsign, err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn get_eam_team_summary(
        &self,
        team_uid: String,
    ) -> Result<Option<EamTeamSummaryRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eam_team_summary(&team_uid)
    }

    pub fn get_events(&self) -> Result<Vec<EventProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_events()
    }

    pub fn upsert_event(&self, record: EventProjectionRecord) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let invalidation = inner.app_state.upsert_event(&record)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("event-upserted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_runtime_event_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in replication_targets {
                    match build_event_replication_payload(&status, &record, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((
                                target.app_destination_hex.clone(),
                                body,
                                fields,
                                target.send_mode,
                            ));
                        }
                        Err(err) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "InvalidConfig".to_string(),
                                message: format!(
                                    "event replication skipped destination={} uid={} reason={}",
                                    target.app_destination_hex, record.uid, err
                                ),
                            });
                        }
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "event replication enqueue failed destination={} uid={} reason={}",
                        destination_hex, record.uid, err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_event(&self, uid: String, deleted_at_ms: u64) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.delete_event(&uid, deleted_at_ms)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("event-deleted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn get_telemetry_positions(&self) -> Result<Vec<TelemetryPositionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_telemetry_positions()
    }

    pub fn record_local_telemetry_fix(
        &self,
        position: TelemetryPositionRecord,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.record_local_telemetry_fix(&position)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("telemetry-upserted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn delete_local_telemetry(&self, callsign: String) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.delete_local_telemetry(&callsign)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("telemetry-deleted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn get_sos_settings(&self) -> Result<SosSettingsRecord, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        Ok(inner
            .app_state
            .get_sos_settings()?
            .map(normalize_sos_settings)
            .unwrap_or_else(default_sos_settings))
    }

    pub fn set_sos_settings(&self, settings: SosSettingsRecord) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let normalized = normalize_sos_settings(settings);
        let invalidation = inner.app_state.set_sos_settings(&normalized)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_sos_pin(&self, pin: Option<String>) -> Result<(), NodeError> {
        let mut settings = self.get_sos_settings()?;
        set_pin(&mut settings, pin.as_deref().unwrap_or_default())?;
        self.set_sos_settings(settings)
    }

    pub fn get_sos_status(&self) -> Result<SosStatusRecord, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        Ok(inner
            .app_state
            .get_sos_status()?
            .unwrap_or_else(idle_status))
    }

    pub fn list_sos_alerts(&self) -> Result<Vec<SosAlertRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.list_sos_alerts()
    }

    pub fn list_sos_locations(&self) -> Result<Vec<SosLocationRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.list_sos_locations()
    }

    pub fn list_sos_audio(&self) -> Result<Vec<SosAudioRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.list_sos_audio()
    }

    pub fn submit_sos_device_telemetry(
        &self,
        telemetry: SosDeviceTelemetryRecord,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        *inner
            .sos_device_telemetry
            .lock()
            .map_err(|_| NodeError::InternalError {})? = Some(telemetry);
        Ok(())
    }

    pub fn submit_sos_accelerometer_sample(
        &self,
        x: f64,
        y: f64,
        z: f64,
        at_ms: u64,
    ) -> Result<Option<SosStatusRecord>, NodeError> {
        let (settings, detector) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            (settings, inner.sos_detector.clone())
        };
        let trigger = detector
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .accelerometer_sample(&settings, x, y, z, at_ms);
        match trigger {
            Some(source) => self.trigger_sos(source).map(Some),
            None => Ok(None),
        }
    }

    pub fn submit_sos_screen_event(
        &self,
        at_ms: u64,
    ) -> Result<Option<SosStatusRecord>, NodeError> {
        let (settings, detector) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            (settings, inner.sos_detector.clone())
        };
        let trigger = detector
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .screen_event(&settings, at_ms);
        match trigger {
            Some(source) => self.trigger_sos(source).map(Some),
            None => Ok(None),
        }
    }

    pub fn trigger_sos(&self, source: SosTriggerSource) -> Result<SosStatusRecord, NodeError> {
        let (app_state, bus, tx, status, settings, saved_peers, telemetry_store) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            if !settings.enabled {
                return Err(NodeError::InvalidConfig {});
            }
            let current = inner
                .app_state
                .get_sos_status()?
                .unwrap_or_else(idle_status);
            if !matches!(current.state, SosState::Idle {}) {
                return Ok(current);
            }
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let values = (
                inner.app_state.clone(),
                inner.bus.clone(),
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                status,
                settings,
                inner.app_state.get_saved_peers()?,
                inner.sos_device_telemetry.clone(),
            );
            values
        };

        let incident_id = new_incident_id(status.identity_hex.as_str());
        let countdown = settings.countdown_seconds;
        if countdown > 0 {
            let deadline = now_ms().saturating_add(u64::from(countdown) * 1000);
            let countdown_record = countdown_status(incident_id.clone(), source, deadline);
            emit_sos_status(&app_state, &bus, &countdown_record, "sos-countdown")?;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(u64::from(countdown)));
                let telemetry = latest_sos_telemetry(&telemetry_store);
                run_sos_fanout(
                    app_state,
                    bus,
                    tx,
                    status,
                    settings,
                    saved_peers,
                    telemetry,
                    incident_id,
                    source,
                    SosMessageKind::Active {},
                );
            });
            return Ok(countdown_record);
        }

        let telemetry = latest_sos_telemetry(&telemetry_store);
        let active = run_sos_fanout(
            app_state,
            bus,
            tx,
            status,
            settings,
            saved_peers,
            telemetry,
            incident_id,
            source,
            SosMessageKind::Active {},
        )
        .unwrap_or_else(idle_status);
        Ok(active)
    }

    pub fn deactivate_sos(&self, pin: Option<String>) -> Result<SosStatusRecord, NodeError> {
        let (app_state, bus, tx, status, settings, saved_peers, telemetry, current) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            if !verify_pin(&settings, pin.as_deref()) {
                return Err(NodeError::InvalidConfig {});
            }
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let telemetry = inner
                .sos_device_telemetry
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let current = inner
                .app_state
                .get_sos_status()?
                .unwrap_or_else(idle_status);
            let values = (
                inner.app_state.clone(),
                inner.bus.clone(),
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                status,
                settings,
                inner.app_state.get_saved_peers()?,
                telemetry,
                current,
            );
            values
        };
        let incident_id = current
            .incident_id
            .clone()
            .unwrap_or_else(|| new_incident_id(status.identity_hex.as_str()));
        run_sos_fanout(
            app_state.clone(),
            bus.clone(),
            tx,
            status,
            settings,
            saved_peers,
            telemetry,
            incident_id,
            SosTriggerSource::Manual {},
            SosMessageKind::Cancelled {},
        );
        let idle = idle_status();
        emit_sos_status(&app_state, &bus, &idle, "sos-deactivated")?;
        Ok(idle)
    }

    pub fn get_operational_summary(&self) -> Result<OperationalSummary, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let sync = inner
            .sync_status_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let status = inner
            .status
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let persisted_messages = inner.app_state.list_messages(None)?;
        let conversation_count = persisted_messages
            .iter()
            .map(|message| message.conversation_id.clone())
            .collect::<std::collections::HashSet<String>>()
            .len() as u32;
        Ok(OperationalSummary {
            running: status.running,
            peer_count_total: peers.len() as u32,
            saved_peer_count: inner.app_state.get_saved_peers()?.len() as u32,
            connected_peer_count: peers.iter().filter(|peer| peer.active_link).count() as u32,
            conversation_count,
            message_count: persisted_messages.len() as u32,
            eam_count: inner.app_state.get_eams()?.len() as u32,
            event_count: inner.app_state.get_events()?.len() as u32,
            telemetry_count: inner.app_state.get_telemetry_positions()?.len() as u32,
            active_propagation_node_hex: sync.active_propagation_node_hex,
            updated_at_ms: crate::runtime::now_ms(),
        })
    }

    pub fn subscribe_events(&self) -> Arc<EventSubscription> {
        let rx = self
            .inner
            .lock()
            .map(|inner| inner.bus.subscribe())
            .unwrap_or_else(|_| {
                let (_tx, rx) = cb::unbounded();
                rx
            });
        Arc::new(EventSubscription::new(rx))
    }

    pub fn refresh_hub_directory(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::RefreshHubDirectory { resp: resp_tx })?;
        resp_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(Err(NodeError::Timeout {}))
    }
}

pub struct EventSubscription {
    rx: cb::Receiver<NodeEvent>,
    closed: AtomicBool,
}

impl EventSubscription {
    fn new(rx: cb::Receiver<NodeEvent>) -> Self {
        Self {
            rx,
            closed: AtomicBool::new(false),
        }
    }

    pub fn next(&self, timeout_ms: u32) -> Option<NodeEvent> {
        if self.closed.load(Ordering::Relaxed) {
            return None;
        }

        if timeout_ms == 0 {
            return self.rx.try_recv().ok();
        }

        self.rx
            .recv_timeout(Duration::from_millis(timeout_ms as u64))
            .ok()
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mission_sync::parse_mission_sync_metadata;
    use crate::plugins::{PluginState, PLUGIN_LXMF_FIELD_KEY};
    use crate::types::{
        EamSourceRecord, HubSettingsRecord, MessageDirection, MessageMethod, MessageState,
        TelemetrySettingsRecord,
    };
    use crate::HubMode;
    use rmpv::Value as MsgPackValue;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, OnceLock};
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};

    static TEST_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

    const TEST_TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn plugin_android_abi_is_trimmed_runtime_context() {
        let node = Node::new();

        node.set_plugin_android_abi(Some(" arm64-v8a "))
            .expect("abi stores");

        let inner = node.inner.lock().expect("node lock");
        assert_eq!(inner.plugin_android_abi.as_deref(), Some("arm64-v8a"));
    }

    #[test]
    fn restarting_plugin_runtime_is_noop_before_node_start() {
        let node = Node::new();
        node.set_plugin_android_abi(Some("arm64-v8a"))
            .expect("abi stores");

        let mut inner = node.inner.lock().expect("node lock");
        restart_enabled_native_plugin_runtime(&mut inner);

        assert!(inner.plugin_runtime.is_none());
    }

    struct TcpRelayHandle {
        addr: SocketAddr,
        shutdown: Arc<Notify>,
        task: tokio::task::JoinHandle<()>,
    }

    impl TcpRelayHandle {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind relay listener");
            let addr = listener.local_addr().expect("relay local addr");
            let shutdown = Arc::new(Notify::new());
            let clients: Arc<AsyncMutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>> =
                Arc::new(AsyncMutex::new(HashMap::new()));
            let next_client_id = Arc::new(AtomicUsize::new(1));

            let task = {
                let shutdown = shutdown.clone();
                let clients = clients.clone();
                let next_client_id = next_client_id.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = shutdown.notified() => break,
                            accepted = listener.accept() => {
                                let Ok((stream, _peer)) = accepted else {
                                    break;
                                };
                                let client_id = next_client_id.fetch_add(1, AtomicOrdering::Relaxed);
                                let (mut read_half, mut write_half) = stream.into_split();
                                let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
                                clients.lock().await.insert(client_id, tx);

                                let writer_clients = clients.clone();
                                tokio::spawn(async move {
                                    while let Some(chunk) = rx.recv().await {
                                        if write_half.write_all(chunk.as_slice()).await.is_err() {
                                            break;
                                        }
                                    }
                                    writer_clients.lock().await.remove(&client_id);
                                });

                                let reader_clients = clients.clone();
                                tokio::spawn(async move {
                                    let mut buf = vec![0u8; 4096];
                                    loop {
                                        let read = match read_half.read(&mut buf).await {
                                            Ok(0) => break,
                                            Ok(n) => n,
                                            Err(_) => break,
                                        };
                                        let chunk = buf[..read].to_vec();
                                        let mut guard = reader_clients.lock().await;
                                        let mut dead_clients = Vec::new();
                                        for (peer_id, sender) in guard.iter() {
                                            if *peer_id == client_id {
                                                continue;
                                            }
                                            if sender.send(chunk.clone()).is_err() {
                                                dead_clients.push(*peer_id);
                                            }
                                        }
                                        for peer_id in dead_clients {
                                            guard.remove(&peer_id);
                                        }
                                    }
                                    reader_clients.lock().await.remove(&client_id);
                                });
                            }
                        }
                    }
                })
            };

            Self {
                addr,
                shutdown,
                task,
            }
        }

        fn address(&self) -> String {
            self.addr.to_string()
        }

        async fn shutdown(self) {
            self.shutdown.notify_waiters();
            let _ = self.task.await;
        }
    }

    fn test_lock() -> &'static AsyncMutex<()> {
        TEST_LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    fn wait_until_running(node: &Node) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if node.get_status().running {
                return;
            }
            if Instant::now() >= deadline {
                panic!("node did not report running in time");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    fn isolate_current_dir(name: &str) -> CurrentDirGuard {
        let previous = std::env::current_dir().expect("capture current dir");
        let dir = prepare_storage_dir(name);
        std::env::set_current_dir(&dir).expect("set current dir");
        CurrentDirGuard { previous }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "reticulum_mobile_e2e_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    fn prepare_storage_dir(name: &str) -> PathBuf {
        let dir = unique_test_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create storage dir");
        dir
    }

    fn write_test_plugin_file(package_dir: &Path, relative_path: &str, contents: &[u8]) {
        let path = package_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create package parent");
        }
        std::fs::write(path, contents).expect("write package file");
    }

    fn write_test_plugin_package(package_dir: &Path) {
        write_test_plugin_file(
            package_dir,
            "plugin.toml",
            br#"
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[permissions]
storage_plugin = true
lxmf_send = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send"]
schema = "schemas/status_test.schema.json"
"#,
        );
        write_test_plugin_file(
            package_dir,
            "logic/android/arm64-v8a/libexample_status_plugin.so",
            b"native",
        );
        write_test_plugin_file(
            package_dir,
            "ui/settings.schema.json",
            br#"{"type":"object"}"#,
        );
        write_test_plugin_file(
            package_dir,
            "schemas/status_test.schema.json",
            br#"{"type":"object","required":["status"],"properties":{"status":{"type":"string","minLength":1}},"additionalProperties":false}"#,
        );
    }

    fn write_test_plugin_receive_package(package_dir: &Path) {
        write_test_plugin_file(
            package_dir,
            "plugin.toml",
            br#"
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[permissions]
storage_plugin = true
lxmf_receive = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["receive"]
schema = "schemas/status_test.schema.json"
"#,
        );
        write_test_plugin_file(
            package_dir,
            "logic/android/arm64-v8a/libexample_status_plugin.so",
            b"native",
        );
        write_test_plugin_file(
            package_dir,
            "ui/settings.schema.json",
            br#"{"type":"object"}"#,
        );
        write_test_plugin_file(
            package_dir,
            "schemas/status_test.schema.json",
            br#"{"type":"object","required":["status"],"properties":{"status":{"type":"string","minLength":1}},"additionalProperties":false}"#,
        );
    }

    fn plugin_lxmf_fields_bytes() -> Vec<u8> {
        let fields = json!({
            PLUGIN_LXMF_FIELD_KEY: {
                "plugin_id": "rem.plugin.example_status",
                "message_name": "status_test",
                "wire_type": "plugin.rem.plugin.example_status.status_test",
                "payload": { "status": "ok" },
            }
        });
        rmp_serde::to_vec(&fields).expect("plugin fields encode")
    }

    fn write_test_plugin_archive(archive_path: &Path) {
        let archive_file = std::fs::File::create(archive_path).expect("create plugin archive");
        let mut archive = zip::ZipWriter::new(archive_file);
        let options = zip::write::SimpleFileOptions::default();
        for (relative_path, contents) in [
            (
                "plugin.toml",
                br#"
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[permissions]
storage_plugin = true
lxmf_send = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send"]
schema = "schemas/status_test.schema.json"
"#
                .as_slice(),
            ),
            (
                "logic/android/arm64-v8a/libexample_status_plugin.so",
                b"native".as_slice(),
            ),
            (
                "ui/settings.schema.json",
                br#"{"type":"object"}"#.as_slice(),
            ),
            (
                "schemas/status_test.schema.json",
                br#"{"type":"object"}"#.as_slice(),
            ),
        ] {
            archive
                .start_file(relative_path, options)
                .expect("start archive entry");
            std::io::Write::write_all(&mut archive, contents).expect("write archive entry");
        }
        archive.finish().expect("finish plugin archive");
    }

    #[test]
    fn install_plugin_package_dir_installs_from_app_private_staging() {
        let storage_dir = prepare_storage_dir("plugin_install_staged");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        let report = node
            .install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");

        assert!(report.errors.is_empty());
        assert_eq!(report.items.len(), 1);
        let plugin = &report.items[0];
        assert_eq!(plugin.id.as_str(), "rem.plugin.example_status");
        assert_eq!(plugin.state, PluginState::Disabled);
        assert!(storage_dir
            .join("plugins/rem.plugin.example_status/plugin.toml")
            .is_file());
    }

    #[test]
    fn install_plugin_package_dir_installs_archive_from_app_private_staging() {
        let storage_dir = prepare_storage_dir("plugin_install_staged_archive");
        let archive_path = storage_dir
            .join("plugin-packages")
            .join("example-status.remplugin");
        std::fs::create_dir_all(archive_path.parent().expect("archive parent"))
            .expect("create archive parent");
        write_test_plugin_archive(archive_path.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        let report = node
            .install_plugin_package_dir("arm64-v8a", archive_path.to_string_lossy().as_ref())
            .expect("staged archive installs");

        assert!(report.errors.is_empty());
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].id.as_str(), "rem.plugin.example_status");
        assert_eq!(report.items[0].state, PluginState::Disabled);
    }

    #[test]
    fn install_plugin_package_dir_rejects_package_outside_staging() {
        let storage_dir = prepare_storage_dir("plugin_install_rejects_outside");
        let package_dir = unique_test_dir("plugin_install_outside_package");
        std::fs::create_dir_all(package_dir.as_path()).expect("create outside package");
        write_test_plugin_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        let err = node
            .install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect_err("outside package is rejected");

        assert!(matches!(err, NodeError::InvalidConfig {}));
        assert!(!storage_dir
            .join("plugins/rem.plugin.example_status")
            .exists());
        let _ = std::fs::remove_dir_all(package_dir);
    }

    #[test]
    fn plugin_lxmf_dispatch_builds_declared_granted_outbound_request() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_dispatch_granted");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_send = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");

        let request = node
            .build_plugin_lxmf_outbound_request(
                "arm64-v8a",
                PluginLxmfSendRequest {
                    plugin_id: "rem.plugin.example_status".to_string(),
                    destination_hex: "aabbccddeeff00112233445566778899".to_string(),
                    message_name: "status_test".to_string(),
                    payload: json!({ "status": "ok" }),
                    body_utf8: "Status test from example plug-in".to_string(),
                    title: Some("Status Test".to_string()),
                    send_mode: SendMode::PropagationOnly {},
                },
            )
            .expect("outbound request builds");

        assert_eq!(request.plugin_id.as_str(), "rem.plugin.example_status");
        assert_eq!(request.message_name.as_str(), "status_test");
        assert_eq!(
            request.wire_type.as_str(),
            "plugin.rem.plugin.example_status.status_test"
        );
        assert!(matches!(request.send_mode, SendMode::PropagationOnly {}));
        assert!(!request.fields_bytes.is_empty());
    }

    #[test]
    fn plugin_lxmf_dispatch_rejects_payload_that_violates_message_schema() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_dispatch_bad_schema");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_send = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");

        let err = node
            .build_plugin_lxmf_outbound_request(
                "arm64-v8a",
                PluginLxmfSendRequest {
                    plugin_id: "rem.plugin.example_status".to_string(),
                    destination_hex: "aabbccddeeff00112233445566778899".to_string(),
                    message_name: "status_test".to_string(),
                    payload: json!({ "status": "" }),
                    body_utf8: "Status test from example plug-in".to_string(),
                    title: None,
                    send_mode: SendMode::Auto {},
                },
            )
            .expect_err("invalid payload is denied");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    #[test]
    fn plugin_lxmf_dispatch_does_not_load_unrelated_plugin_message_schema() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_dispatch_unrelated_bad_schema");
        let valid_package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_package(valid_package_dir.as_path());
        let bad_package_dir = storage_dir
            .join("plugin-packages")
            .join("bad-schema-package");
        write_test_plugin_file(
            bad_package_dir.as_path(),
            "plugin.toml",
            br#"
id = "rem.plugin.bad_schema"
name = "Bad Schema Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libbad_schema_plugin.so"

[permissions]
lxmf_send = true

[[messages]]
name = "bad_status"
version = "1.0.0"
direction = ["send"]
schema = "schemas/bad_status.schema.json"
"#,
        );
        write_test_plugin_file(
            bad_package_dir.as_path(),
            "logic/android/arm64-v8a/libbad_schema_plugin.so",
            b"native",
        );
        write_test_plugin_file(
            bad_package_dir.as_path(),
            "schemas/bad_status.schema.json",
            br#"{"type":"object","required":["badStatus"],"properties":{"badStatus":{"type":"string","minLength":1}},"additionalProperties":false}"#,
        );
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", valid_package_dir.to_string_lossy().as_ref())
            .expect("valid staged package installs");
        node.install_plugin_package_dir("arm64-v8a", bad_package_dir.to_string_lossy().as_ref())
            .expect("unrelated schema package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_send = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");

        let request = node
            .build_plugin_lxmf_outbound_request(
                "arm64-v8a",
                PluginLxmfSendRequest {
                    plugin_id: "rem.plugin.example_status".to_string(),
                    destination_hex: "aabbccddeeff00112233445566778899".to_string(),
                    message_name: "status_test".to_string(),
                    payload: json!({ "status": "ok" }),
                    body_utf8: "Status test from example plug-in".to_string(),
                    title: None,
                    send_mode: SendMode::Auto {},
                },
            )
            .expect("unrelated bad schema does not block valid plugin");

        assert_eq!(request.plugin_id.as_str(), "rem.plugin.example_status");
    }

    #[test]
    fn plugin_lxmf_dispatch_denies_disabled_plugin_even_with_grant() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_dispatch_disabled");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_send = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");

        let err = node
            .build_plugin_lxmf_outbound_request(
                "arm64-v8a",
                PluginLxmfSendRequest {
                    plugin_id: "rem.plugin.example_status".to_string(),
                    destination_hex: "aabbccddeeff00112233445566778899".to_string(),
                    message_name: "status_test".to_string(),
                    payload: json!({ "status": "ok" }),
                    body_utf8: "Status test from example plug-in".to_string(),
                    title: None,
                    send_mode: SendMode::Auto {},
                },
            )
            .expect_err("disabled plugin is denied");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    #[test]
    fn plugin_lxmf_receive_parses_declared_granted_enabled_message() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_receive_granted");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_receive_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_receive = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");

        let message = node
            .receive_plugin_lxmf_fields("arm64-v8a", plugin_lxmf_fields_bytes().as_slice())
            .expect("plugin fields parse")
            .expect("plugin message recognized");

        assert_eq!(message.plugin_id.as_str(), "rem.plugin.example_status");
        assert_eq!(message.message_name.as_str(), "status_test");
        assert_eq!(message.payload, json!({ "status": "ok" }));
    }

    #[test]
    fn plugin_lxmf_receive_rejects_payload_that_violates_message_schema() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_receive_bad_schema");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_receive_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_receive = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");
        let fields = json!({
            PLUGIN_LXMF_FIELD_KEY: {
                "plugin_id": "rem.plugin.example_status",
                "message_name": "status_test",
                "wire_type": "plugin.rem.plugin.example_status.status_test",
                "payload": { "unexpected": "ok" },
            }
        });
        let fields = rmp_serde::to_vec(&fields).expect("plugin fields encode");

        let err = node
            .receive_plugin_lxmf_fields("arm64-v8a", fields.as_slice())
            .expect_err("invalid payload is denied");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    #[test]
    fn plugin_lxmf_receive_ignores_non_plugin_fields() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_receive_non_plugin");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        let fields = rmp_serde::to_vec(&json!({ "other": true })).expect("fields encode");

        let message = node
            .receive_plugin_lxmf_fields("arm64-v8a", fields.as_slice())
            .expect("non-plugin fields parse");

        assert!(message.is_none());
    }

    #[test]
    fn plugin_lxmf_receive_denies_ungranted_plugin() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_receive_ungranted");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_receive_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        node.set_plugin_enabled("arm64-v8a", "rem.plugin.example_status", true)
            .expect("plugin enabled");

        let err = node
            .receive_plugin_lxmf_fields("arm64-v8a", plugin_lxmf_fields_bytes().as_slice())
            .expect_err("ungranted receive is denied");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    #[test]
    fn plugin_lxmf_receive_denies_disabled_plugin_even_with_grant() {
        let storage_dir = prepare_storage_dir("plugin_lxmf_receive_disabled");
        let package_dir = storage_dir
            .join("plugin-packages")
            .join("example-status-package");
        write_test_plugin_receive_package(package_dir.as_path());
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        node.install_plugin_package_dir("arm64-v8a", package_dir.to_string_lossy().as_ref())
            .expect("staged package installs");
        let mut grants = PluginPermissions::default();
        grants.lxmf_receive = true;
        node.grant_plugin_permissions("arm64-v8a", "rem.plugin.example_status", grants)
            .expect("permissions grant");

        let err = node
            .receive_plugin_lxmf_fields("arm64-v8a", plugin_lxmf_fields_bytes().as_slice())
            .expect_err("disabled receive is denied");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    fn build_config(name: &str, storage_dir: &Path, relay_addr: &str) -> NodeConfig {
        NodeConfig {
            name: name.to_string(),
            storage_dir: Some(storage_dir.to_string_lossy().to_string()),
            tcp_clients: vec![relay_addr.to_string()],
            broadcast: true,
            announce_interval_seconds: 1,
            stale_after_minutes: 30,
            announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            hub_mode: HubMode::Autonomous {},
            hub_identity_hash: None,
            hub_api_base_url: None,
            hub_api_key: None,
            hub_refresh_interval_seconds: 0,
        }
    }

    fn wait_for_event<F>(
        subscription: &Arc<EventSubscription>,
        timeout: Duration,
        mut predicate: F,
    ) -> Option<NodeEvent>
    where
        F: FnMut(&NodeEvent) -> bool,
    {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout_ms = remaining.as_millis().min(u32::MAX as u128).max(1) as u32;
            if let Some(event) = subscription.next(timeout_ms.min(250)) {
                if predicate(&event) {
                    return Some(event);
                }
            }
        }
    }

    fn msgpack_map(entries: Vec<(&str, MsgPackValue)>) -> MsgPackValue {
        MsgPackValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| (MsgPackValue::from(key), value))
                .collect(),
        )
    }

    fn mission_command_fields(
        command_id: &str,
        correlation_id: &str,
        command_type: &str,
        args: Vec<(&str, MsgPackValue)>,
    ) -> Vec<u8> {
        let fields = msgpack_map(vec![(
            "9",
            MsgPackValue::Array(vec![msgpack_map(vec![
                ("command_id", MsgPackValue::from(command_id)),
                ("correlation_id", MsgPackValue::from(correlation_id)),
                ("command_type", MsgPackValue::from(command_type)),
                ("args", msgpack_map(args)),
            ])]),
        )]);
        rmp_serde::to_vec(&fields).expect("msgpack command fields")
    }

    fn mission_event_fields(
        event_type: &str,
        event_uid: &str,
        payload: Vec<(&str, MsgPackValue)>,
    ) -> Vec<u8> {
        let fields = msgpack_map(vec![(
            "13",
            MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("event_type"),
                    MsgPackValue::from(event_type),
                ),
                (
                    MsgPackValue::from("event_id"),
                    MsgPackValue::from(event_uid),
                ),
                (MsgPackValue::from("payload"), msgpack_map(payload)),
            ]),
        )]);
        rmp_serde::to_vec(&fields).expect("msgpack event fields")
    }

    #[test]
    fn checklist_upload_snapshot_uses_msgpack_content_not_command_fields() {
        let status = NodeStatus {
            running: true,
            name: "Pixel".to_string(),
            identity_hex: "11111111111111111111111111111111".to_string(),
            app_destination_hex: "22222222222222222222222222222222".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let target = MissionReplicationTarget {
            app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            send_mode: SendMode::Auto {},
        };
        let snapshot_json =
            r#"{"uid":"chk-native","name":"Native","tasks":[{"task_uid":"task-1","number":1}]}"#;
        let args = checklist_uid_args_json("chk-native");

        let (body, fields) = build_checklist_replication_payload_with_snapshot(
            &status,
            &target,
            "checklist.upload",
            &args,
            Some("cmd-upload"),
            snapshot_json,
        )
        .expect("upload payload");
        let fields = rmp_serde::from_slice::<MsgPackValue>(fields.as_slice()).expect("fields");
        let MsgPackValue::Map(field_entries) = fields else {
            panic!("fields should be a map");
        };
        let commands = field_entries
            .iter()
            .find(|(key, _)| key.as_i64() == Some(FIELD_COMMANDS))
            .and_then(|(_, value)| value.as_array())
            .expect("commands");
        let command = commands[0].as_map().expect("command map");

        assert!(!command
            .iter()
            .any(|(key, _)| key.as_str() == Some("snapshot")));
        let content =
            rmp_serde::from_slice::<MsgPackValue>(body.as_slice()).expect("msgpack snapshot body");
        let MsgPackValue::Map(content_entries) = content else {
            panic!("snapshot body should be a map");
        };
        assert!(content_entries
            .iter()
            .any(|(key, value)| key.as_str() == Some("type")
                && value.as_str() == Some("rem.checklist.snapshot.v1")));
        assert!(content_entries
            .iter()
            .any(|(key, value)| key.as_str() == Some("snapshot")
                && matches!(value, MsgPackValue::Map(_))));
    }

    #[test]
    fn checklist_create_args_include_schema_but_not_full_task_snapshot() {
        let checklist = ChecklistRecord {
            uid: "chk-hydrate".to_string(),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: None,
            template_version: None,
            template_name: None,
            name: "Hydrate".to_string(),
            description: String::new(),
            start_time: None,
            mode: crate::types::ChecklistMode::Online {},
            sync_state: crate::types::ChecklistSyncState::Synced {},
            origin_type: crate::types::ChecklistOriginType::RchTemplate {},
            checklist_status: crate::types::ChecklistTaskStatus::Pending {},
            created_at: Some("2026-04-23T12:00:00Z".to_string()),
            created_by_team_member_rns_identity: "peer-a".to_string(),
            created_by_team_member_display_name: Some("Peer A".to_string()),
            updated_at: Some("2026-04-23T12:00:00Z".to_string()),
            last_changed_by_team_member_rns_identity: Some("peer-a".to_string()),
            deleted_at: None,
            uploaded_at: Some("2026-04-23T12:00:00Z".to_string()),
            participant_rns_identities: vec!["peer-a".to_string()],
            expected_task_count: Some(1),
            progress_percent: 0.0,
            counts: crate::types::ChecklistStatusCounts {
                pending_count: 1,
                late_count: 0,
                complete_count: 0,
            },
            columns: vec![crate::types::ChecklistColumnRecord {
                column_uid: "col-item".to_string(),
                column_name: "Item".to_string(),
                display_order: 0,
                column_type: crate::types::ChecklistColumnType::ShortString {},
                column_editable: true,
                background_color: None,
                text_color: None,
                is_removable: true,
                system_key: None,
            }],
            tasks: vec![crate::types::ChecklistTaskRecord {
                task_uid: "task-1".to_string(),
                number: 1,
                user_status: crate::types::ChecklistUserTaskStatus::Pending {},
                task_status: crate::types::ChecklistTaskStatus::Pending {},
                is_late: false,
                updated_at: Some("2026-04-23T12:00:00Z".to_string()),
                deleted_at: None,
                custom_status: None,
                due_relative_minutes: None,
                due_dtg: None,
                notes: None,
                row_background_color: None,
                line_break_enabled: false,
                completed_at: None,
                completed_by_team_member_rns_identity: None,
                legacy_value: Some("Water".to_string()),
                cells: vec![crate::types::ChecklistCellRecord {
                    cell_uid: "task-1:col-item".to_string(),
                    task_uid: "task-1".to_string(),
                    column_uid: "col-item".to_string(),
                    value: Some("Water".to_string()),
                    updated_at: None,
                    updated_by_team_member_rns_identity: None,
                }],
            }],
            feed_publications: Vec::new(),
        };
        let mut create_args = checklist_create_online_args_json(&ChecklistCreateOnlineRequest {
            checklist_uid: Some(checklist.uid.clone()),
            mission_uid: checklist.mission_uid.clone(),
            template_uid: "tmpl-hydrate".to_string(),
            name: checklist.name.clone(),
            description: checklist.description.clone(),
            start_time: "2026-04-23T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: Some(
                checklist.created_by_team_member_rns_identity.clone(),
            ),
            created_by_team_member_display_name: checklist
                .created_by_team_member_display_name
                .clone(),
        })
        .expect("create args");
        append_checklist_create_snapshot_args(&mut create_args, &checklist)
            .expect("append create snapshot");
        assert_eq!(
            create_args.get("checklist_uid").and_then(JsonValue::as_str),
            Some("chk-hydrate")
        );
        assert_eq!(
            create_args
                .get("columns")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            create_args.get("total_tasks").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert!(create_args.get("tasks").is_none());
        assert!(create_args.get("counts").is_none());
        assert!(create_args.get("progress_percent").is_none());
    }

    #[test]
    fn checklist_cell_subject_includes_task_and_column() {
        let task_one_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
            checklist_uid: "chk-hydrate".to_string(),
            task_uid: "task-1".to_string(),
            column_uid: "col-description".to_string(),
            value: "Reliable ignition source".to_string(),
            updated_by_team_member_rns_identity: None,
        });
        let task_two_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
            checklist_uid: "chk-hydrate".to_string(),
            task_uid: "task-2".to_string(),
            column_uid: "col-description".to_string(),
            value: "Hands-free lighting".to_string(),
            updated_by_team_member_rns_identity: None,
        });

        let task_one_subject = checklist_subject_token("checklist.task.cell.set", &task_one_args);
        let task_two_subject = checklist_subject_token("checklist.task.cell.set", &task_two_args);

        assert_eq!(task_one_subject, "chk-hydrate-task-1-col-description");
        assert_eq!(task_two_subject, "chk-hydrate-task-2-col-description");
        assert_ne!(task_one_subject, task_two_subject);
    }

    async fn start_node_pair(test_name: &str) -> (TcpRelayHandle, Node, Node) {
        let relay = TcpRelayHandle::start().await;

        let node_a_storage = prepare_storage_dir(&format!("{test_name}_a"));
        let node_b_storage = prepare_storage_dir(&format!("{test_name}_b"));

        let node_a = Node::new();
        node_a
            .start(build_config(
                &format!("{test_name}-a"),
                node_a_storage.as_path(),
                relay.address().as_str(),
            ))
            .expect("start node a");

        let node_b = Node::new();
        node_b
            .start(build_config(
                &format!("{test_name}-b"),
                node_b_storage.as_path(),
                relay.address().as_str(),
            ))
            .expect("start node b");

        node_a.announce_now().expect("announce node a");
        node_b.announce_now().expect("announce node b");
        tokio::time::sleep(Duration::from_millis(500)).await;

        let node_b_lxmf_destination_hex = node_b.get_status().lxmf_destination_hex;
        node_a
            .request_peer_identity(node_b_lxmf_destination_hex.clone())
            .expect("resolve node b");

        (relay, node_a, node_b)
    }

    async fn stop_node(node: Node) {
        let _ = tokio::task::spawn_blocking(move || node.stop()).await;
    }

    fn assert_packet_received(
        event: NodeEvent,
        expected_source_hex: &str,
        expected_body: &str,
        expected_fields: Option<&[u8]>,
    ) {
        match event {
            NodeEvent::MessageReceived { message } => {
                assert_eq!(message.source_hex.as_deref(), Some(expected_source_hex));
                assert_eq!(message.body_utf8, expected_body);
            }
            NodeEvent::PacketReceived {
                source_hex,
                bytes,
                fields_bytes,
                ..
            } => {
                assert_eq!(source_hex.as_deref(), Some(expected_source_hex));
                assert_eq!(bytes.as_slice(), expected_body.as_bytes());
                match (expected_fields, fields_bytes.as_deref()) {
                    (None, None) => {}
                    (Some(expected), Some(actual)) => {
                        assert_eq!(actual, expected);
                    }
                    (None, Some(_)) => panic!("unexpected mission fields"),
                    (Some(_), None) => panic!("expected mission fields"),
                }
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    fn build_app_settings() -> AppSettingsRecord {
        AppSettingsRecord {
            display_name: "Atlas-1".to_string(),
            auto_connect_saved: true,
            announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            tcp_clients: vec!["rns.beleth.net:4242".to_string()],
            broadcast: true,
            announce_interval_seconds: 1800,
            telemetry: TelemetrySettingsRecord {
                enabled: true,
                publish_interval_seconds: 15,
                accuracy_threshold_meters: Some(10.0),
                stale_after_minutes: 30,
                expire_after_minutes: 180,
            },
            hub: HubSettingsRecord {
                mode: HubMode::Autonomous {},
                identity_hash: String::new(),
                api_base_url: String::new(),
                api_key: String::new(),
                refresh_interval_seconds: 3600,
            },
            checklists: crate::types::ChecklistSettingsRecord::default(),
        }
    }

    fn build_saved_peer() -> SavedPeerRecord {
        SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("POCO".to_string()),
            saved_at_ms: 1_700_000_000_000,
        }
    }

    fn build_status_for_tests() -> NodeStatus {
        NodeStatus {
            running: true,
            name: "Atlas-1".to_string(),
            identity_hex: "99999999999999999999999999999999".to_string(),
            app_destination_hex: "12121212121212121212121212121212".to_string(),
            lxmf_destination_hex: "34343434343434343434343434343434".to_string(),
        }
    }

    fn build_config_fingerprint_for_tests(
        hub_mode: HubMode,
        hub_identity_hash: Option<&str>,
    ) -> NodeConfigFingerprint {
        NodeConfigFingerprint {
            name: "Atlas-1".to_string(),
            storage_dir: None,
            tcp_clients: Vec::new(),
            broadcast: true,
            announce_interval_seconds: 1800,
            stale_after_minutes: 30,
            announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            hub_mode,
            hub_identity_hash: hub_identity_hash.map(str::to_string),
            hub_api_base_url: None,
            hub_api_key: None,
            hub_refresh_interval_seconds: 3600,
        }
    }

    fn build_peer_record(
        destination_hex: &str,
        lxmf_destination_hex: &str,
        saved: bool,
        connected: bool,
        active_link: bool,
    ) -> PeerRecord {
        PeerRecord {
            destination_hex: destination_hex.to_string(),
            identity_hex: Some(format!("identity-{destination_hex}")),
            lxmf_destination_hex: Some(lxmf_destination_hex.to_string()),
            display_name: Some(format!("peer-{destination_hex}")),
            app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
            state: if connected {
                crate::types::PeerState::Connected {}
            } else {
                crate::types::PeerState::Disconnected {}
            },
            saved,
            stale: false,
            active_link,
            hub_derived: false,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: Some(now_ms()),
            last_seen_at_ms: now_ms(),
            announce_last_seen_at_ms: Some(now_ms()),
            lxmf_last_seen_at_ms: Some(now_ms()),
        }
    }

    #[test]
    fn effective_hub_mode_uses_server_connected_override() {
        let snapshot = HubDirectorySnapshot {
            effective_connected_mode: true,
            items: Vec::new(),
            received_at_ms: 123,
        };

        assert!(matches!(
            effective_hub_mode(HubMode::SemiAutonomous {}, Some(&snapshot)),
            HubMode::Connected {}
        ));
        assert!(matches!(
            effective_hub_mode(HubMode::SemiAutonomous {}, None),
            HubMode::SemiAutonomous {}
        ));
    }

    #[test]
    fn semi_autonomous_replication_targets_use_hub_directory_peers() {
        let status = build_status_for_tests();
        let config = build_config_fingerprint_for_tests(
            HubMode::SemiAutonomous {},
            Some("56565656565656565656565656565656"),
        );
        let snapshot = HubDirectorySnapshot {
            effective_connected_mode: false,
            items: vec![crate::types::HubDirectoryPeerRecord {
                identity: "78787878787878787878787878787878".to_string(),
                destination_hash: "abababababababababababababababab".to_string(),
                display_name: Some("Pixel".to_string()),
                announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
                client_type: Some("rem".to_string()),
                registered_mode: Some("semi_autonomous".to_string()),
                last_seen: Some("2026-04-02T12:43:28Z".to_string()),
                status: Some("active".to_string()),
            }],
            received_at_ms: 456,
        };

        let targets = build_runtime_mission_replication_targets(
            &status,
            &[],
            &[],
            None,
            Some(&config),
            Some(&snapshot),
        )
        .expect("semi-autonomous targets");

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "abababababababababababababababab"
        );
        assert!(matches!(targets[0].send_mode, SendMode::Auto {}));
    }

    #[test]
    fn connected_telemetry_destinations_route_only_to_hub() {
        let status = build_status_for_tests();
        let config = build_config_fingerprint_for_tests(
            HubMode::Connected {},
            Some("56565656565656565656565656565656"),
        );

        let destinations = build_runtime_telemetry_destinations(&status, &[], Some(&config), None)
            .expect("connected telemetry destinations");

        assert_eq!(
            destinations,
            vec!["56565656565656565656565656565656".to_string()]
        );
    }

    #[test]
    fn connected_telemetry_destinations_require_selected_hub() {
        let status = build_status_for_tests();
        let config = build_config_fingerprint_for_tests(HubMode::Connected {}, None);

        let err = build_runtime_telemetry_destinations(&status, &[], Some(&config), None)
            .expect_err("connected telemetry should require a hub");

        assert!(matches!(err, NodeError::InvalidConfig {}));
    }

    #[test]
    fn semi_autonomous_telemetry_destinations_use_hub_snapshot() {
        let status = build_status_for_tests();
        let config = build_config_fingerprint_for_tests(
            HubMode::SemiAutonomous {},
            Some("56565656565656565656565656565656"),
        );
        let peers = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        )];
        let snapshot = HubDirectorySnapshot {
            effective_connected_mode: false,
            items: vec![
                crate::types::HubDirectoryPeerRecord {
                    identity: "78787878787878787878787878787878".to_string(),
                    destination_hash: "abababababababababababababababab".to_string(),
                    display_name: Some("Pixel".to_string()),
                    announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
                    client_type: Some("rem".to_string()),
                    registered_mode: Some("semi_autonomous".to_string()),
                    last_seen: Some("2026-04-02T12:43:28Z".to_string()),
                    status: Some("active".to_string()),
                },
                crate::types::HubDirectoryPeerRecord {
                    identity: "89898989898989898989898989898989".to_string(),
                    destination_hash: "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd".to_string(),
                    display_name: Some("NoTelemetry".to_string()),
                    announce_capabilities: vec!["r3akt".to_string()],
                    client_type: Some("rem".to_string()),
                    registered_mode: Some("semi_autonomous".to_string()),
                    last_seen: Some("2026-04-02T12:43:28Z".to_string()),
                    status: Some("active".to_string()),
                },
            ],
            received_at_ms: 123,
        };

        let destinations = build_runtime_telemetry_destinations(
            &status,
            peers.as_slice(),
            Some(&config),
            Some(&snapshot),
        )
        .expect("semi-autonomous telemetry destinations");

        assert_eq!(
            destinations,
            vec!["abababababababababababababababab".to_string()]
        );
    }

    #[test]
    fn semi_autonomous_telemetry_destinations_fall_back_without_selected_hub() {
        let status = build_status_for_tests();
        let config = build_config_fingerprint_for_tests(HubMode::SemiAutonomous {}, None);
        let peers = vec![
            build_peer_record(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                true,
                true,
                true,
            ),
            build_peer_record(
                "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
                "efefefefefefefefefefefefefefefef",
                true,
                true,
                false,
            ),
        ];

        let destinations =
            build_runtime_telemetry_destinations(&status, peers.as_slice(), Some(&config), None)
                .expect("semi-autonomous fallback telemetry destinations");

        assert_eq!(
            destinations,
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
        );
    }

    fn build_eam() -> EamProjectionRecord {
        EamProjectionRecord {
            callsign: "POCO".to_string(),
            group_name: "Blue".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Yellow".to_string(),
            preparedness_status: "Green".to_string(),
            medical_status: "Green".to_string(),
            mobility_status: "Green".to_string(),
            comms_status: "Yellow".to_string(),
            notes: Some("pre-start eam".to_string()),
            updated_at_ms: 1_700_000_000_100,
            deleted_at_ms: None,
            eam_uid: Some("eam-1".to_string()),
            team_member_uid: Some("member-1".to_string()),
            team_uid: Some("team-1".to_string()),
            reported_at: Some("2026-03-25T00:00:00Z".to_string()),
            reported_by: Some("Atlas-1".to_string()),
            overall_status: Some("Yellow".to_string()),
            confidence: Some(0.9),
            ttl_seconds: Some(3600),
            source: Some(EamSourceRecord {
                rns_identity: "identity-1".to_string(),
                display_name: Some("Atlas-1".to_string()),
            }),
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(1_700_000_000_100),
            last_synced_at_ms: None,
        }
    }

    #[test]
    fn build_eam_replication_payload_emits_numeric_lxmf_command_field() {
        let status = NodeStatus {
            running: true,
            name: "Pixel".to_string(),
            identity_hex: "11111111111111111111111111111111".to_string(),
            app_destination_hex: "22222222222222222222222222222222".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let record = build_eam();
        let target = MissionReplicationTarget {
            app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            send_mode: SendMode::Auto {},
        };

        let (_, fields) =
            build_eam_replication_payload(&status, &record, &target).expect("eam fields");
        let metadata = parse_mission_sync_metadata(&fields).expect("mission metadata");

        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.eam.upsert")
        );
        assert_eq!(metadata.eam_uid.as_deref(), record.eam_uid.as_deref());
        assert_eq!(metadata.team_uid.as_deref(), record.team_uid.as_deref());
        assert_eq!(
            metadata.team_member_uid.as_deref(),
            record.team_member_uid.as_deref()
        );
    }

    #[test]
    fn populate_eam_defaults_uses_local_app_hash_and_team_color_hash() {
        let status = NodeStatus {
            running: true,
            name: "Pixel".to_string(),
            identity_hex: "11111111111111111111111111111111".to_string(),
            app_destination_hex: "22222222222222222222222222222222".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let mut record = build_eam();
        record.group_name = "blue".to_string();
        record.team_member_uid = None;
        record.team_uid = None;
        record.reported_by = None;
        record.source = None;
        record.overall_status = None;

        let normalized = populate_eam_defaults(&status, &record);

        assert_eq!(normalized.group_name, "BLUE");
        assert_eq!(
            normalized.team_member_uid.as_deref(),
            Some("22222222222222222222222222222222")
        );
        assert_eq!(normalized.team_uid.as_deref(), Some(TEAM_UID_BLUE));
        assert_eq!(normalized.reported_by.as_deref(), Some("Pixel"));
        assert_eq!(
            normalized
                .source
                .as_ref()
                .map(|source| source.rns_identity.as_str()),
            Some("11111111111111111111111111111111")
        );
        assert_eq!(normalized.overall_status.as_deref(), Some("Yellow"));
    }

    fn build_event() -> EventProjectionRecord {
        EventProjectionRecord {
            uid: "evt-1".to_string(),
            command_id: "cmd-1".to_string(),
            source_identity: "identity-1".to_string(),
            source_display_name: Some("Atlas-1".to_string()),
            timestamp: "2026-03-25T00:00:00Z".to_string(),
            command_type: "mission.registry.log_entry.upsert".to_string(),
            mission_uid: "mission-1".to_string(),
            content: "Economy Crash".to_string(),
            callsign: "Atlas-1".to_string(),
            server_time: None,
            client_time: None,
            keywords: vec!["economy".to_string()],
            content_hashes: vec!["hash-1".to_string()],
            updated_at_ms: 1_700_000_000_200,
            deleted_at_ms: None,
            correlation_id: Some("corr-1".to_string()),
            topics: vec!["mission-1".to_string()],
        }
    }

    fn build_message() -> MessageRecord {
        MessageRecord {
            message_id_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            conversation_id: "conversation-1".to_string(),
            direction: MessageDirection::Outbound {},
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            source_hex: Some("cccccccccccccccccccccccccccccccc".to_string()),
            title: Some("check-in".to_string()),
            body_utf8: "Hello world".to_string(),
            method: MessageMethod::Direct {},
            state: MessageState::Queued {},
            detail: None,
            sent_at_ms: Some(1_700_000_000_300),
            received_at_ms: None,
            updated_at_ms: 1_700_000_000_300,
        }
    }

    fn build_telemetry() -> TelemetryPositionRecord {
        TelemetryPositionRecord {
            callsign: "POCO".to_string(),
            lat: 44.6488,
            lon: -63.5752,
            alt: Some(12.0),
            course: Some(45.0),
            speed: Some(3.5),
            accuracy: Some(5.0),
            updated_at_ms: 1_700_000_000_400,
        }
    }

    #[test]
    fn latest_sos_telemetry_reads_updated_snapshot() {
        let store = Arc::new(Mutex::new(Some(SosDeviceTelemetryRecord {
            lat: None,
            lon: None,
            alt: None,
            speed: None,
            course: None,
            accuracy: None,
            battery_percent: Some(52.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_000_000,
        })));

        assert_eq!(
            latest_sos_telemetry(&store).and_then(|value| value.lat),
            None
        );

        *store.lock().expect("telemetry lock") = Some(SosDeviceTelemetryRecord {
            lat: Some(44.6488),
            lon: Some(-63.5752),
            alt: None,
            speed: None,
            course: None,
            accuracy: Some(8.0),
            battery_percent: Some(53.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_003_000,
        });

        let telemetry = latest_sos_telemetry(&store).expect("latest telemetry");
        assert_eq!(telemetry.lat, Some(44.6488));
        assert_eq!(telemetry.lon, Some(-63.5752));
        assert_eq!(telemetry.battery_percent, Some(53.0));
    }

    fn sample_app_settings() -> AppSettingsRecord {
        AppSettingsRecord {
            display_name: "Alpha".to_string(),
            auto_connect_saved: true,
            announce_capabilities: "mission,eam".to_string(),
            tcp_clients: vec!["tcp://127.0.0.1:4242".to_string()],
            broadcast: true,
            announce_interval_seconds: 30,
            telemetry: TelemetrySettingsRecord {
                enabled: true,
                publish_interval_seconds: 15,
                accuracy_threshold_meters: Some(8.5),
                stale_after_minutes: 5,
                expire_after_minutes: 30,
            },
            hub: HubSettingsRecord {
                mode: HubMode::Autonomous {},
                identity_hash: String::new(),
                api_base_url: String::new(),
                api_key: String::new(),
                refresh_interval_seconds: 0,
            },
            checklists: crate::types::ChecklistSettingsRecord::default(),
        }
    }

    fn sample_saved_peer() -> SavedPeerRecord {
        SavedPeerRecord {
            destination_hex: "A1B2C3D4".to_string(),
            label: Some("Bravo".to_string()),
            saved_at_ms: 1,
        }
    }

    fn sample_eam() -> EamProjectionRecord {
        EamProjectionRecord {
            callsign: "ALPHA-1".to_string(),
            group_name: "Operations".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Ready".to_string(),
            preparedness_status: "Ready".to_string(),
            medical_status: "Ready".to_string(),
            mobility_status: "Ready".to_string(),
            comms_status: "Ready".to_string(),
            notes: Some("pre-start import".to_string()),
            updated_at_ms: 1,
            deleted_at_ms: None,
            eam_uid: Some("eam-1".to_string()),
            team_member_uid: Some("member-1".to_string()),
            team_uid: Some("team-1".to_string()),
            reported_at: None,
            reported_by: None,
            overall_status: Some("Green".to_string()),
            confidence: Some(1.0),
            ttl_seconds: Some(3600),
            source: None,
            sync_state: Some("Synced".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(1),
            last_synced_at_ms: Some(1),
        }
    }

    fn sample_event() -> EventProjectionRecord {
        EventProjectionRecord {
            uid: "event-1".to_string(),
            command_id: "command-1".to_string(),
            source_identity: "identity-1".to_string(),
            source_display_name: Some("Alpha".to_string()),
            timestamp: "2026-03-25T00:00:00Z".to_string(),
            command_type: "event".to_string(),
            mission_uid: "mission-1".to_string(),
            content: "status update".to_string(),
            callsign: "ALPHA-1".to_string(),
            server_time: None,
            client_time: None,
            keywords: vec!["status".to_string()],
            content_hashes: vec!["hash-1".to_string()],
            updated_at_ms: 1,
            deleted_at_ms: None,
            correlation_id: Some("corr-1".to_string()),
            topics: vec!["mission".to_string()],
        }
    }

    fn sample_message() -> MessageRecord {
        MessageRecord {
            message_id_hex: "msg-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            direction: MessageDirection::Outbound {},
            destination_hex: "DEST-1".to_string(),
            source_hex: None,
            title: Some("Hello".to_string()),
            body_utf8: "hello from pre-start".to_string(),
            method: MessageMethod::Direct {},
            state: MessageState::Queued {},
            detail: Some("queued".to_string()),
            sent_at_ms: Some(1),
            received_at_ms: None,
            updated_at_ms: 1,
        }
    }

    fn sample_position() -> TelemetryPositionRecord {
        TelemetryPositionRecord {
            callsign: "ALPHA-1".to_string(),
            lat: 44.0,
            lon: -63.0,
            alt: Some(12.0),
            course: Some(90.0),
            speed: Some(3.0),
            accuracy: Some(5.0),
            updated_at_ms: 1,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn app_state_queries_and_writes_work_before_start() {
        let _guard = test_lock().lock().await;
        let _cwd = isolate_current_dir("prestart_app_state");
        let node = Node::new();

        let settings = sample_app_settings();
        let peer = sample_saved_peer();
        let payload = LegacyImportPayload {
            settings: Some(settings.clone()),
            saved_peers: vec![peer.clone()],
            eams: vec![sample_eam()],
            events: vec![sample_event()],
            messages: vec![sample_message()],
            telemetry_positions: vec![sample_position()],
        };

        node.set_app_settings(settings.clone())
            .expect("set app settings before start");
        node.set_saved_peers(vec![peer.clone()])
            .expect("set saved peers before start");
        node.import_legacy_state(payload)
            .expect("import legacy state before start");

        let persisted_settings = node
            .get_app_settings()
            .expect("get app settings")
            .expect("settings present");
        assert_eq!(persisted_settings.display_name, settings.display_name);
        assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);

        let persisted_peers = node.get_saved_peers().expect("get saved peers");
        assert_eq!(persisted_peers.len(), 1);
        assert_eq!(persisted_peers[0].destination_hex, peer.destination_hex);
        assert_eq!(persisted_peers[0].label, peer.label);
        assert!(node
            .legacy_import_completed()
            .expect("legacy import status"));
        let eams = node.get_eams().expect("get eams");
        assert_eq!(eams.len(), 1);
        assert_eq!(eams[0].callsign, "ALPHA-1");
        assert_eq!(eams[0].team_uid.as_deref(), Some("team-1"));

        let events = node.get_events().expect("get events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "event-1");
        assert_eq!(events[0].mission_uid, "mission-1");

        let telemetry_positions = node
            .get_telemetry_positions()
            .expect("get telemetry positions");
        assert_eq!(telemetry_positions.len(), 1);
        assert_eq!(telemetry_positions[0].callsign, "ALPHA-1");

        let conversations = node.list_conversations().expect("list conversations");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, "dest-1");
        assert_eq!(conversations[0].peer_destination_hex, "dest-1");

        let messages = node
            .list_messages(Some("dest-1".to_string()))
            .expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id_hex, "msg-1");
        assert_eq!(messages[0].conversation_id, "dest-1");
        assert_eq!(messages[0].body_utf8, "hello from pre-start");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_only_commands_still_fail_before_start() {
        let _guard = test_lock().lock().await;
        let _cwd = isolate_current_dir("prestart_runtime_commands");
        let node = Node::new();

        assert!(matches!(
            node.connect_peer("ABCDEF".to_string()),
            Err(NodeError::NotRunning {})
        ));
        assert!(matches!(
            node.send_lxmf(SendLxmfRequest {
                destination_hex: "ABCDEF".to_string(),
                body_utf8: "hello".to_string(),
                title: Some("test".to_string()),
                send_mode: SendMode::Auto {},
            }),
            Err(NodeError::NotRunning {})
        ));
    }

    #[test]
    fn start_is_idempotent_for_equivalent_config() {
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        let _guard = rt.block_on(test_lock().lock());
        let _cwd = isolate_current_dir("start_idempotent");
        let relay = rt.block_on(TcpRelayHandle::start());
        let storage_dir = prepare_storage_dir("start_idempotent");
        let config = build_config(
            "start-idempotent",
            storage_dir.as_path(),
            relay.address().as_str(),
        );
        let node = Node::new();

        node.start(config.clone()).expect("initial start");
        node.start(config.clone())
            .expect("repeat start with same config");
        wait_until_running(&node);
        node.announce_now()
            .expect("runtime command stays available after idempotent start");

        let status = node.get_status();
        assert!(status.running);
        assert_eq!(status.name, config.name);

        node.stop().expect("stop idempotent node");
        rt.block_on(relay.shutdown());
    }

    #[test]
    fn start_restarts_when_config_changes_while_running() {
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        let _guard = rt.block_on(test_lock().lock());
        let _cwd = isolate_current_dir("start_restart_changed");
        let relay = rt.block_on(TcpRelayHandle::start());
        let storage_dir = prepare_storage_dir("start_restart_changed");
        let node = Node::new();
        let config = build_config(
            "start-restart",
            storage_dir.as_path(),
            relay.address().as_str(),
        );
        let mut changed_config = config.clone();
        changed_config.name = "start-restart-updated".to_string();
        changed_config.announce_interval_seconds = 2;

        node.start(config).expect("initial start");
        node.start(changed_config.clone())
            .expect("start with changed config while running");
        wait_until_running(&node);
        node.announce_now()
            .expect("runtime command stays available after config restart");

        let status = node.get_status();
        assert!(status.running);
        assert_eq!(status.name, changed_config.name);

        node.stop().expect("stop restarted node");
        rt.block_on(relay.shutdown());
    }

    #[test]
    fn restart_while_running_keeps_runtime_available() {
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        let _guard = rt.block_on(test_lock().lock());
        let _cwd = isolate_current_dir("restart_running");
        let relay = rt.block_on(TcpRelayHandle::start());
        let storage_dir = prepare_storage_dir("restart_running");
        let config = build_config(
            "restart-running",
            storage_dir.as_path(),
            relay.address().as_str(),
        );
        let node = Node::new();

        node.start(config.clone()).expect("initial start");
        node.restart(config).expect("restart while running");
        wait_until_running(&node);
        node.announce_now()
            .expect("runtime command stays available after restart");

        let status = node.get_status();
        assert!(status.running);

        node.stop().expect("stop restarted node");
        rt.block_on(relay.shutdown());
    }

    #[test]
    fn stop_clears_running_state_and_commands_fail_after_stop() {
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        let _guard = rt.block_on(test_lock().lock());
        let _cwd = isolate_current_dir("stop_after_idle");
        let relay = rt.block_on(TcpRelayHandle::start());
        let storage_dir = prepare_storage_dir("stop_after_idle");
        let config = build_config(
            "stop-after-idle",
            storage_dir.as_path(),
            relay.address().as_str(),
        );
        let node = Node::new();

        node.start(config).expect("initial start");
        wait_until_running(&node);
        std::thread::sleep(Duration::from_millis(100));

        node.stop().expect("first stop succeeds");
        node.stop().expect("second stop remains idempotent");

        let status = node.get_status();
        assert!(!status.running);
        assert!(matches!(node.announce_now(), Err(NodeError::NotRunning {})));
        assert!(matches!(
            node.request_peer_identity("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            Err(NodeError::NotRunning {})
        ));

        rt.block_on(relay.shutdown());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_chat_message_is_received_by_peer() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("chat").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "chat: hello from node a";
        let subscription = node_b.subscribe_events();
        let message_id = node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: body.to_string(),
                title: Some("chat".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("send chat message");
        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == body)
        })
        .expect("node b received chat message");

        assert_packet_received(event, &node_a_status.lxmf_destination_hex, body, None);
        assert!(!message_id.is_empty());
        let persisted_messages = node_b.list_messages(None).expect("persisted messages");
        assert!(
            persisted_messages
                .iter()
                .any(|message| message.body_utf8 == body
                    && message.conversation_id
                        == node_a_status.lxmf_destination_hex.to_ascii_lowercase()),
            "received LXMF chat should be persisted in the canonical peer thread"
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[test]
    fn pre_start_app_state_queries_use_initialized_storage() {
        let storage_dir = prepare_storage_dir("pre_start_app_state");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        let settings = build_app_settings();
        let saved_peer = build_saved_peer();
        let eam = build_eam();
        let event = build_event();
        let message = build_message();
        let telemetry = build_telemetry();

        assert!(!node
            .legacy_import_completed()
            .expect("legacy import completed before import"));

        node.import_legacy_state(LegacyImportPayload {
            settings: Some(settings.clone()),
            saved_peers: vec![saved_peer.clone()],
            eams: vec![eam.clone()],
            events: vec![event.clone()],
            messages: vec![message.clone()],
            telemetry_positions: vec![telemetry.clone()],
        })
        .expect("import legacy state");

        assert!(node
            .legacy_import_completed()
            .expect("legacy import completed after import"));
        let persisted_settings = node
            .get_app_settings()
            .expect("app settings")
            .expect("settings present");
        assert_eq!(persisted_settings.display_name, settings.display_name);
        assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);

        let persisted_saved_peers = node.get_saved_peers().expect("saved peers");
        assert_eq!(persisted_saved_peers.len(), 1);
        assert_eq!(
            persisted_saved_peers[0].destination_hex,
            saved_peer.destination_hex
        );
        assert_eq!(persisted_saved_peers[0].label, saved_peer.label);

        let persisted_eams = node.get_eams().expect("eams");
        assert_eq!(persisted_eams.len(), 1);
        assert_eq!(persisted_eams[0].callsign, eam.callsign);
        assert_eq!(persisted_eams[0].team_uid, eam.team_uid);

        let persisted_events = node.get_events().expect("events");
        assert_eq!(persisted_events.len(), 1);
        assert_eq!(persisted_events[0].uid, event.uid);
        assert_eq!(persisted_events[0].mission_uid, event.mission_uid);

        let persisted_messages = node.list_messages(None).expect("messages");
        assert_eq!(persisted_messages.len(), 1);
        assert_eq!(persisted_messages[0].message_id_hex, message.message_id_hex);
        assert_eq!(
            persisted_messages[0].conversation_id,
            message.destination_hex.to_ascii_lowercase()
        );

        let conversations = node.list_conversations().expect("conversations");
        assert_eq!(conversations.len(), 1);
        assert_eq!(
            conversations[0].conversation_id,
            message.destination_hex.to_ascii_lowercase()
        );
        let persisted_telemetry = node.get_telemetry_positions().expect("telemetry");
        assert_eq!(persisted_telemetry.len(), 1);
        assert_eq!(persisted_telemetry[0].callsign, telemetry.callsign);
    }

    #[test]
    fn start_reuses_pre_initialized_storage_directory() {
        let storage_dir = prepare_storage_dir("pre_start_storage_reuse");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        let settings = build_app_settings();

        node.set_app_settings(settings.clone())
            .expect("persist settings before start");
        node.initialize_storage(Some(storage_dir.to_string_lossy().as_ref()))
            .expect("reinitialize same storage dir");

        let persisted_settings = node
            .get_app_settings()
            .expect("settings after reinitialize")
            .expect("settings present after reinitialize");
        assert_eq!(persisted_settings.display_name, settings.display_name);
        assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);
    }

    #[test]
    fn runtime_commands_still_fail_before_start() {
        let storage_dir = prepare_storage_dir("runtime_not_running");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        assert!(matches!(
            node.connect_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            Err(NodeError::NotRunning {})
        ));
        assert!(matches!(
            node.request_peer_identity("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            Err(NodeError::NotRunning {})
        ));
        assert!(matches!(node.announce_now(), Err(NodeError::NotRunning {})));
        assert!(matches!(
            node.send_lxmf(SendLxmfRequest {
                destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                body_utf8: "hello".to_string(),
                title: None,
                send_mode: SendMode::Auto {},
            }),
            Err(NodeError::NotRunning {})
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_emergency_message_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("emergency").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "emergency: request medevac";
        let fields = mission_command_fields(
            "cmd-eam-123",
            "corr-eam-123",
            "mission.registry.eam.upsert",
            vec![
                ("eam_uid", MsgPackValue::from("eam-123")),
                ("team_member_uid", MsgPackValue::from("member-1")),
                ("team_uid", MsgPackValue::from("team-1")),
                ("mission_uid", MsgPackValue::from("mission-1")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                SendMode::Auto {},
            )
            .expect("send emergency packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received emergency packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_event_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("event").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "event: checkpoint reached";
        let fields = mission_event_fields(
            "mission.registry.log_entry.upserted",
            "event-123",
            vec![
                ("entry_uid", MsgPackValue::from("event-123")),
                ("mission_uid", MsgPackValue::from("mission-1")),
                ("content", MsgPackValue::from("Checkpoint reached")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                SendMode::Auto {},
            )
            .expect("send event packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received event packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("event metadata");
        assert_eq!(
            metadata.event_type.as_deref(),
            Some("mission.registry.log_entry.upserted")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("event-123"));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_telemetry_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("telemetry").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "telemetry: position sample";
        let fields = mission_command_fields(
            "cmd-telemetry-123",
            "corr-telemetry-123",
            "mission.registry.telemetry.upsert",
            vec![
                ("event_uid", MsgPackValue::from("telemetry-123")),
                ("team_member_uid", MsgPackValue::from("member-1")),
                ("team_uid", MsgPackValue::from("team-1")),
                ("mission_uid", MsgPackValue::from("mission-1")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                SendMode::Auto {},
            )
            .expect("send telemetry packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received telemetry packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("telemetry metadata");
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.telemetry.upsert")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("telemetry-123"));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_emergency_message_to_app_destination_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("emergency_app_destination").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "emergency: request medevac";
        let fields = mission_command_fields(
            "cmd-eam-app-123",
            "corr-eam-app-123",
            "mission.registry.eam.upsert",
            vec![
                ("eam_uid", MsgPackValue::from("eam-123")),
                ("team_member_uid", MsgPackValue::from("member-1")),
                ("team_uid", MsgPackValue::from("team-1")),
                ("mission_uid", MsgPackValue::from("mission-1")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.app_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                SendMode::Auto {},
            )
            .expect("send emergency packet via app destination");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received emergency packet via app destination");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_built_eam_replication_payload_is_persisted_by_receiver() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("eam_payload_projection").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let record = EamProjectionRecord {
            callsign: "Pixel".to_string(),
            group_name: "Blue".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Yellow".to_string(),
            preparedness_status: "Green".to_string(),
            medical_status: "Green".to_string(),
            mobility_status: "Green".to_string(),
            comms_status: "Yellow".to_string(),
            notes: Some("native eam replication".to_string()),
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            eam_uid: Some("eam-upsert-native".to_string()),
            team_member_uid: Some("member-1".to_string()),
            team_uid: Some("team-1".to_string()),
            reported_at: Some("2026-03-25T16:30:00Z".to_string()),
            reported_by: Some(node_a_status.name.clone()),
            overall_status: Some("Yellow".to_string()),
            confidence: Some(0.8),
            ttl_seconds: Some(3600),
            source: Some(EamSourceRecord {
                rns_identity: node_a_status.identity_hex.clone(),
                display_name: Some(node_a_status.name.clone()),
            }),
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(now_ms()),
            last_synced_at_ms: None,
        };
        let target = MissionReplicationTarget {
            app_destination_hex: node_b_status.app_destination_hex.clone(),
            send_mode: SendMode::Auto {},
        };
        let (body, fields) =
            build_eam_replication_payload(&node_a_status, &record, &target).expect("eam payload");

        node_a
            .send_bytes(
                node_b_status.app_destination_hex.clone(),
                body,
                Some(fields),
                SendMode::Auto {},
            )
            .expect("send eam replication payload");

        let received_deadline = Instant::now() + TEST_TIMEOUT;
        let received = loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| eam.callsign == record.callsign);
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < received_deadline,
                "node b never persisted direct eam replication payload"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(received.eam_uid.as_deref(), record.eam_uid.as_deref());
        assert_eq!(received.team_uid.as_deref(), record.team_uid.as_deref());
        assert_eq!(
            received.team_member_uid.as_deref(),
            record.team_member_uid.as_deref()
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_peer_establishes_active_link_without_message_send() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("connect_peer_link").await;

        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.saved && peer.active_link);
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never established an active link from connect_peer"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn upsert_eam_replicates_to_native_peer_projection() {
        const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("eam_projection").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let warm_link_subscription = node_b.subscribe_events();
        node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: "warm eam link".to_string(),
                title: Some("warmup".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("warm eam link");
        wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam link")
        })
        .expect("node b received eam warmup message");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| {
                    peer.saved
                        && peer.active_link
                        && peer.lxmf_destination_hex.as_deref()
                            == Some(node_b_status.lxmf_destination_hex.as_str())
                });
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never became mission-ready"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let replication_targets = build_mission_replication_targets(
            &node_a.get_status(),
            node_a.list_peers().expect("list peers").as_slice(),
            node_a.get_saved_peers().expect("saved peers").as_slice(),
            node_a
                .get_lxmf_sync_status()
                .expect("sync status")
                .active_propagation_node_hex
                .as_deref(),
        );
        assert_eq!(
            replication_targets.len(),
            1,
            "expected one eam replication target"
        );
        assert_eq!(
            replication_targets[0].app_destination_hex,
            node_b_status.app_destination_hex
        );

        let record = EamProjectionRecord {
            callsign: "Pixel".to_string(),
            group_name: "Blue".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Yellow".to_string(),
            preparedness_status: "Green".to_string(),
            medical_status: "Green".to_string(),
            mobility_status: "Green".to_string(),
            comms_status: "Yellow".to_string(),
            notes: Some("native eam replication".to_string()),
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            eam_uid: Some("eam-upsert-native".to_string()),
            team_member_uid: Some("member-1".to_string()),
            team_uid: Some("team-1".to_string()),
            reported_at: Some("2026-03-25T16:30:00Z".to_string()),
            reported_by: Some(node_a_status.name.clone()),
            overall_status: Some("Yellow".to_string()),
            confidence: Some(0.8),
            ttl_seconds: Some(3600),
            source: Some(EamSourceRecord {
                rns_identity: node_a_status.identity_hex.clone(),
                display_name: Some(node_a_status.name.clone()),
            }),
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(now_ms()),
            last_synced_at_ms: None,
        };

        node_a.upsert_eam(record.clone()).expect("upsert local eam");

        let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        let received = loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| eam.callsign == record.callsign);
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < received_deadline,
                "node b never persisted replicated eam"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(received.callsign, record.callsign);
        assert_eq!(received.team_uid.as_deref(), record.team_uid.as_deref());
        assert_eq!(
            received.team_member_uid.as_deref(),
            record.team_member_uid.as_deref()
        );
        assert_eq!(received.eam_uid.as_deref(), record.eam_uid.as_deref());
        assert_eq!(received.security_status, record.security_status);
        assert_eq!(received.capability_status, record.capability_status);
        assert_eq!(received.overall_status.as_deref(), Some("Yellow"));
        assert_eq!(
            received
                .source
                .as_ref()
                .map(|source| source.rns_identity.as_str()),
            Some(node_a_status.identity_hex.as_str())
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn upsert_eam_defaults_and_replicates_to_native_peer_projection() {
        const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("eam_defaults_projection").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let warm_link_subscription = node_b.subscribe_events();
        node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: "warm eam defaults link".to_string(),
                title: Some("warmup".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("warm eam defaults link");
        wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam defaults link")
        })
        .expect("node b received eam defaults warmup message");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.saved && peer.active_link);
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never became mission-ready"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let record = EamProjectionRecord {
            callsign: "Pixel".to_string(),
            group_name: "Blue".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Yellow".to_string(),
            preparedness_status: "Green".to_string(),
            medical_status: "Green".to_string(),
            mobility_status: "Green".to_string(),
            comms_status: "Yellow".to_string(),
            notes: Some("native eam default replication".to_string()),
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            eam_uid: Some("eam-upsert-defaults".to_string()),
            team_member_uid: None,
            team_uid: None,
            reported_at: Some("2026-03-25T16:45:00Z".to_string()),
            reported_by: None,
            overall_status: None,
            confidence: Some(0.8),
            ttl_seconds: Some(3600),
            source: None,
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(now_ms()),
            last_synced_at_ms: None,
        };

        node_a.upsert_eam(record.clone()).expect("upsert local eam");

        let local = node_a
            .get_eams()
            .expect("get local eams")
            .into_iter()
            .find(|eam| eam.callsign == record.callsign)
            .expect("local eam persisted");
        assert_eq!(
            local.team_member_uid.as_deref(),
            Some(node_a_status.app_destination_hex.as_str())
        );
        assert_eq!(local.team_uid.as_deref(), Some(TEAM_UID_BLUE));

        let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        let received = loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| eam.callsign == record.callsign);
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < received_deadline,
                "node b never persisted replicated eam with defaults"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(
            received.team_member_uid.as_deref(),
            Some(node_a_status.app_destination_hex.as_str())
        );
        assert_eq!(received.team_uid.as_deref(), Some(TEAM_UID_BLUE));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn delete_eam_replicates_to_native_peer_projection() {
        const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("eam_delete_projection").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let warm_link_subscription = node_b.subscribe_events();
        node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: "warm eam delete link".to_string(),
                title: Some("warmup".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("warm eam delete link");
        wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam delete link")
        })
        .expect("node b received eam delete warmup message");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.saved && peer.active_link);
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never became mission-ready"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let record = EamProjectionRecord {
            callsign: "Ciccio".to_string(),
            group_name: "Yellow".to_string(),
            security_status: "Red".to_string(),
            capability_status: "Yellow".to_string(),
            preparedness_status: "Red".to_string(),
            medical_status: "Unknown".to_string(),
            mobility_status: "Unknown".to_string(),
            comms_status: "Unknown".to_string(),
            notes: Some("native eam delete replication".to_string()),
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            eam_uid: Some("eam-delete-native".to_string()),
            team_member_uid: Some(node_a_status.app_destination_hex.clone()),
            team_uid: Some(TEAM_UID_YELLOW.to_string()),
            reported_at: Some("2026-03-27T14:00:00Z".to_string()),
            reported_by: Some(node_a_status.name.clone()),
            overall_status: Some("Red".to_string()),
            confidence: Some(0.9),
            ttl_seconds: Some(3600),
            source: Some(EamSourceRecord {
                rns_identity: node_a_status.identity_hex.clone(),
                display_name: Some(node_a_status.name.clone()),
            }),
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(now_ms()),
            last_synced_at_ms: None,
        };

        node_a.upsert_eam(record.clone()).expect("upsert local eam");

        let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| eam.callsign == record.callsign && eam.deleted_at_ms.is_none());
            if received.is_some() {
                break;
            }
            assert!(
                Instant::now() < received_deadline,
                "node b never persisted replicated eam before delete"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let deleted_at_ms = now_ms();
        node_a
            .delete_eam(record.callsign.clone(), deleted_at_ms)
            .expect("delete local eam");

        let delete_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        let deleted = loop {
            let deleted = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| {
                    eam.callsign == record.callsign && eam.deleted_at_ms == Some(deleted_at_ms)
                });
            if let Some(deleted) = deleted {
                break deleted;
            }
            assert!(
                Instant::now() < delete_deadline,
                "node b never persisted replicated eam delete"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(deleted.callsign, record.callsign);
        assert_eq!(deleted.deleted_at_ms, Some(deleted_at_ms));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn upsert_event_replicates_to_native_peer_projection() {
        const EVENT_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("event_projection").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let warm_link_subscription = node_b.subscribe_events();
        node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: "warm event link".to_string(),
                title: Some("warmup".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("warm event link");
        wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm event link")
        })
        .expect("node b received warmup message");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.saved && has_known_lxmf_route(&peer));
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never became mission-ready"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let replication_targets = build_event_replication_targets(
            &node_a.get_status(),
            node_a.list_peers().expect("list peers").as_slice(),
            node_a.get_saved_peers().expect("saved peers").as_slice(),
            node_a
                .get_lxmf_sync_status()
                .expect("sync status")
                .active_propagation_node_hex
                .as_deref(),
        );
        assert_eq!(
            replication_targets.len(),
            1,
            "expected one event replication target"
        );
        assert_eq!(
            replication_targets[0].app_destination_hex,
            node_b_status.app_destination_hex
        );

        let record = EventProjectionRecord {
            uid: "evt-upsert-native".to_string(),
            command_id: "cmd-evt-upsert-native".to_string(),
            source_identity: node_a_status.identity_hex.clone(),
            source_display_name: Some(node_a_status.name.clone()),
            timestamp: "2026-03-25T16:50:00Z".to_string(),
            command_type: "mission.registry.log_entry.upsert".to_string(),
            mission_uid: "r3akt-default-mission".to_string(),
            content: "Native replicated event".to_string(),
            callsign: node_a_status.name.clone(),
            server_time: Some("2026-03-25T16:50:00Z".to_string()),
            client_time: Some("2026-03-25T16:50:00Z".to_string()),
            keywords: vec!["r3akt:event-type:Incident".to_string()],
            content_hashes: vec![],
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            correlation_id: Some("corr-evt-upsert-native".to_string()),
            topics: vec!["r3akt-default-mission".to_string(), "Default".to_string()],
        };

        node_a
            .upsert_event(record.clone())
            .expect("upsert local event");

        let received_deadline = Instant::now() + EVENT_REPLICATION_TIMEOUT;
        let received = loop {
            let received = node_b
                .get_events()
                .expect("get events")
                .into_iter()
                .find(|event| event.uid == record.uid);
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < received_deadline,
                "node b never persisted replicated event"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(received.uid, record.uid);
        assert_eq!(received.command_type, "mission.registry.log_entry.upsert");
        assert_eq!(received.mission_uid, record.mission_uid);
        assert_eq!(received.content, record.content);
        assert_eq!(received.callsign, record.callsign);
        assert_eq!(received.source_identity, node_a_status.identity_hex);

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn repeated_eam_updates_with_same_callsign_replicate_latest_projection() {
        const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("eam_repeated_updates").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        node_a
            .set_saved_peers(vec![SavedPeerRecord {
                destination_hex: node_b_status.app_destination_hex.clone(),
                label: Some("peer-b".to_string()),
                saved_at_ms: now_ms(),
            }])
            .expect("save peer b");
        node_a
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let warm_link_subscription = node_b.subscribe_events();
        node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: "warm repeated eam link".to_string(),
                title: Some("warmup".to_string()),
                send_mode: SendMode::Auto {},
            })
            .expect("warm repeated eam link");
        wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm repeated eam link")
        })
        .expect("node b received repeated eam warmup message");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.saved && has_known_lxmf_route(&peer));
            if peer_ready {
                break;
            }
            assert!(
                Instant::now() < peer_ready_deadline,
                "peer b never became mission-ready"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let first_record = EamProjectionRecord {
            callsign: "Pippo".to_string(),
            group_name: "Yellow".to_string(),
            security_status: "Green".to_string(),
            capability_status: "Green".to_string(),
            preparedness_status: "Yellow".to_string(),
            medical_status: "Unknown".to_string(),
            mobility_status: "Unknown".to_string(),
            comms_status: "Unknown".to_string(),
            notes: Some("first native eam".to_string()),
            updated_at_ms: now_ms(),
            deleted_at_ms: None,
            eam_uid: Some("eam-repeated-native".to_string()),
            team_member_uid: Some(node_a_status.app_destination_hex.clone()),
            team_uid: Some(TEAM_UID_YELLOW.to_string()),
            reported_at: Some("2026-03-27T15:00:00Z".to_string()),
            reported_by: Some(node_a_status.name.clone()),
            overall_status: Some("Yellow".to_string()),
            confidence: Some(0.9),
            ttl_seconds: Some(3600),
            source: Some(EamSourceRecord {
                rns_identity: node_a_status.identity_hex.clone(),
                display_name: Some(node_a_status.name.clone()),
            }),
            sync_state: Some("draft".to_string()),
            sync_error: None,
            draft_created_at_ms: Some(now_ms()),
            last_synced_at_ms: None,
        };

        node_a
            .upsert_eam(first_record.clone())
            .expect("upsert initial eam");

        let first_received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        let first_received = loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| eam.callsign == first_record.callsign && eam.deleted_at_ms.is_none());
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < first_received_deadline,
                "node b never persisted initial eam update"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };
        assert_eq!(first_received.callsign, first_record.callsign);

        let mut second_record = first_record.clone();
        second_record.preparedness_status = "Red".to_string();
        second_record.notes = Some("second native eam".to_string());
        second_record.updated_at_ms = first_received.updated_at_ms.saturating_add(1);
        second_record.overall_status = Some("Red".to_string());

        node_a
            .upsert_eam(second_record.clone())
            .expect("upsert repeated eam");

        let second_received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
        let received = loop {
            let received = node_b
                .get_eams()
                .expect("get eams")
                .into_iter()
                .find(|eam| {
                    eam.callsign == second_record.callsign
                        && eam.preparedness_status == second_record.preparedness_status
                        && eam.notes == second_record.notes
                });
            if let Some(received) = received {
                break received;
            }
            assert!(
                Instant::now() < second_received_deadline,
                "node b never persisted repeated eam update"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        };

        assert_eq!(received.callsign, second_record.callsign);
        assert!(received.updated_at_ms >= second_record.updated_at_ms);
        assert_eq!(
            received.preparedness_status,
            second_record.preparedness_status
        );
        assert_eq!(received.notes, second_record.notes);

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[test]
    fn event_replication_targets_only_include_intentional_peers() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();
        let peers = vec![
            build_peer_record(
                saved_peer.destination_hex.as_str(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                true,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                false,
                true,
                true,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                false,
                false,
                false,
            ),
            build_peer_record(
                "99999999999999999999999999999999",
                "12121212121212121212121212121212",
                false,
                true,
                false,
            ),
        ];

        let targets = build_event_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn event_replication_targets_include_connected_peer_without_active_link() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("saved-connected".to_string()),
            saved_at_ms: now_ms(),
        };
        let peers = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            false,
        )];

        let targets =
            build_event_replication_targets(&status, peers.as_slice(), &[saved_peer], None);

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn event_replication_targets_include_saved_relay_fallback_without_discovered_peers() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = SavedPeerRecord {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            label: Some("saved-relay".to_string()),
            saved_at_ms: now_ms(),
        };
        let peers = vec![
            build_peer_record(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                false,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                true,
                false,
                false,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                false,
                false,
                false,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                false,
                true,
                true,
            ),
        ];

        let targets = build_event_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "cccccccccccccccccccccccccccccccc"
        );
        assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
    }

    #[test]
    fn eam_replication_targets_only_include_intentional_peers() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();
        let peers = vec![
            build_peer_record(
                saved_peer.destination_hex.as_str(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                true,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                false,
                true,
                true,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                false,
                false,
                false,
            ),
            build_peer_record(
                "99999999999999999999999999999999",
                "12121212121212121212121212121212",
                false,
                true,
                false,
            ),
        ];

        let targets = build_mission_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn eam_replication_targets_include_saved_relay_fallback_without_discovered_peers() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = SavedPeerRecord {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            label: Some("saved-relay".to_string()),
            saved_at_ms: now_ms(),
        };
        let peers = vec![
            build_peer_record(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                false,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                true,
                false,
                false,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                false,
                false,
                false,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                false,
                true,
                true,
            ),
        ];

        let targets = build_mission_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "cccccccccccccccccccccccccccccccc"
        );
        assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
    }

    #[test]
    fn eam_replication_targets_include_saved_connected_peer_without_active_link() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("saved-connected".to_string()),
            saved_at_ms: now_ms(),
        };
        let peers = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            false,
        )];

        let targets =
            build_mission_replication_targets(&status, peers.as_slice(), &[saved_peer], None);

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn eam_replication_targets_include_saved_direct_peer_without_lxmf_snapshot() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("saved-direct".to_string()),
            saved_at_ms: now_ms(),
        };
        let peer = PeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            identity_hex: Some("identity-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            lxmf_destination_hex: None,
            display_name: Some("peer-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
            state: crate::types::PeerState::Connected {},
            saved: true,
            stale: false,
            active_link: true,
            hub_derived: false,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: Some(now_ms()),
            last_seen_at_ms: now_ms(),
            announce_last_seen_at_ms: Some(now_ms()),
            lxmf_last_seen_at_ms: None,
        };

        let targets = build_mission_replication_targets(&status, &[peer], &[saved_peer], None);

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn eam_replication_targets_use_propagation_for_saved_peer_without_direct_reachability() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();
        let peers = vec![build_peer_record(
            saved_peer.destination_hex.as_str(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            false,
            false,
        )];

        let targets = build_mission_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
    }

    #[test]
    fn event_replication_targets_use_propagation_for_saved_peer_without_direct_reachability() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();
        let peers = vec![build_peer_record(
            saved_peer.destination_hex.as_str(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            false,
            false,
        )];

        let targets = build_event_replication_targets(
            &status,
            peers.as_slice(),
            &[saved_peer],
            Some("99999999999999999999999999999999"),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
    }

    #[test]
    fn eam_replication_targets_keep_saved_peer_target_without_active_relay() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();

        let targets = build_mission_replication_targets(&status, &[], &[saved_peer], None);

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn event_replication_targets_keep_saved_peer_target_without_active_relay() {
        let status = NodeStatus {
            running: true,
            name: "pixel".to_string(),
            identity_hex: "22222222222222222222222222222222".to_string(),
            app_destination_hex: "11111111111111111111111111111111".to_string(),
            lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        };
        let saved_peer = build_saved_peer();

        let targets = build_event_replication_targets(&status, &[], &[saved_peer], None);

        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].app_destination_hex,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }

    #[test]
    fn checklist_create_online_args_match_supported_contract() {
        let args = checklist_create_online_args_json(&ChecklistCreateOnlineRequest {
            checklist_uid: Some("chk-001".to_string()),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: "tmpl-evac-001".to_string(),
            name: "Mission Alpha Evac".to_string(),
            description: "Shared run for Alpha".to_string(),
            start_time: "2026-04-22T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: Some("abcd1234".to_string()),
            created_by_team_member_display_name: None,
        })
        .expect("build create args");

        assert_eq!(
            args.get("name").and_then(JsonValue::as_str),
            Some("Mission Alpha Evac")
        );
        assert_eq!(
            args.get("template_uid").and_then(JsonValue::as_str),
            Some("tmpl-evac-001")
        );
        assert_eq!(
            args.get("mission_uid").and_then(JsonValue::as_str),
            Some("mission-alpha")
        );
        assert_eq!(
            args.get("description").and_then(JsonValue::as_str),
            Some("Shared run for Alpha")
        );
        assert_eq!(
            args.get("start_time").and_then(JsonValue::as_str),
            Some("2026-04-22T12:00:00Z")
        );
        assert_eq!(
            args.get("checklist_uid").and_then(JsonValue::as_str),
            Some("chk-001")
        );
    }

    #[test]
    fn checklist_update_args_include_explicit_clears() {
        let args = checklist_update_args_json(&ChecklistUpdateRequest {
            checklist_uid: "chk-001".to_string(),
            patch: crate::types::ChecklistUpdatePatch {
                mission_uid: Some(String::new()),
                template_uid: Some(String::new()),
                name: Some("".to_string()),
                description: Some("".to_string()),
                start_time: Some(String::new()),
            },
            changed_by_team_member_rns_identity: None,
        });
        let patch = args
            .get("patch")
            .and_then(JsonValue::as_object)
            .expect("patch object");

        assert_eq!(
            patch.get("mission_uid").and_then(JsonValue::as_str),
            Some("")
        );
        assert_eq!(
            patch.get("template_uid").and_then(JsonValue::as_str),
            Some("")
        );
        assert_eq!(patch.get("name").and_then(JsonValue::as_str), Some(""));
        assert_eq!(
            patch.get("description").and_then(JsonValue::as_str),
            Some("")
        );
        assert_eq!(
            patch.get("start_time").and_then(JsonValue::as_str),
            Some("")
        );
    }

    #[test]
    fn checklist_task_payloads_preserve_whitespace_and_style_clears() {
        let row_add_args = checklist_task_row_add_args_json(&ChecklistTaskRowAddRequest {
            checklist_uid: "chk-001".to_string(),
            task_uid: Some("task-001".to_string()),
            number: 1,
            due_relative_minutes: None,
            legacy_value: Some("  Confirm rally point  ".to_string()),
            changed_by_team_member_rns_identity: None,
        });
        assert_eq!(
            row_add_args.get("legacy_value").and_then(JsonValue::as_str),
            Some("  Confirm rally point  ")
        );

        let detail_row_args = checklist_task_row_add_args_from_task(
            "chk-001",
            &ChecklistTaskRecord {
                task_uid: "task-detail".to_string(),
                number: 2,
                user_status: crate::types::ChecklistUserTaskStatus::Pending {},
                task_status: crate::types::ChecklistTaskStatus::Pending {},
                is_late: false,
                updated_at: Some("2026-04-24T12:00:00Z".to_string()),
                deleted_at: None,
                custom_status: None,
                due_relative_minutes: Some(30),
                due_dtg: Some("2026-04-24T12:30:00Z".to_string()),
                notes: Some("Bring printed route card".to_string()),
                row_background_color: None,
                line_break_enabled: false,
                completed_at: None,
                completed_by_team_member_rns_identity: None,
                legacy_value: Some("Confirm rally point".to_string()),
                cells: Vec::new(),
            },
            Some("peer-a"),
        );
        assert_eq!(
            detail_row_args.get("due_dtg").and_then(JsonValue::as_str),
            Some("2026-04-24T12:30:00Z")
        );
        assert_eq!(
            detail_row_args.get("notes").and_then(JsonValue::as_str),
            Some("Bring printed route card")
        );

        let style_args = checklist_task_row_style_args_json(&ChecklistTaskRowStyleSetRequest {
            checklist_uid: "chk-001".to_string(),
            task_uid: "task-001".to_string(),
            row_background_color: Some(String::new()),
            line_break_enabled: None,
            changed_by_team_member_rns_identity: None,
        });
        assert_eq!(
            style_args
                .get("row_background_color")
                .and_then(JsonValue::as_str),
            Some("")
        );

        let cell_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
            checklist_uid: "chk-001".to_string(),
            task_uid: "task-001".to_string(),
            column_uid: "col-task".to_string(),
            value: "  Move to alternate pickup  ".to_string(),
            updated_by_team_member_rns_identity: None,
        });
        assert_eq!(
            cell_args.get("value").and_then(JsonValue::as_str),
            Some("  Move to alternate pickup  ")
        );
    }

    #[test]
    fn create_online_checklist_rejects_invalid_payload_before_local_persist() {
        let storage_dir = prepare_storage_dir("checklist-create-prevalidate");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        {
            let inner = node.inner.lock().expect("node inner");
            let mut status = inner.status.lock().expect("status");
            status.identity_hex = "joiner-identity".to_string();
        }

        let result = node.create_online_checklist(ChecklistCreateOnlineRequest {
            checklist_uid: Some("chk-invalid".to_string()),
            mission_uid: None,
            template_uid: "tmpl-evac-001".to_string(),
            name: "Mission Alpha Evac".to_string(),
            description: "Shared run for Alpha".to_string(),
            start_time: "2026-04-22T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: None,
            created_by_team_member_display_name: None,
        });

        assert!(matches!(result, Err(NodeError::InvalidConfig {})));
        assert!(node
            .get_checklist("chk-invalid".to_string())
            .expect("query checklist")
            .is_none());
    }

    #[test]
    fn join_checklist_updates_local_participants_immediately() {
        let storage_dir = prepare_storage_dir("checklist-join-local");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));
        {
            let inner = node.inner.lock().expect("node inner");
            let mut status = inner.status.lock().expect("status");
            status.identity_hex = "joiner-identity".to_string();
        }

        node.create_online_checklist(ChecklistCreateOnlineRequest {
            checklist_uid: Some("chk-join".to_string()),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: "tmpl-evac-001".to_string(),
            name: "Mission Alpha Evac".to_string(),
            description: "Shared run for Alpha".to_string(),
            start_time: "2026-04-22T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: Some("creator-identity".to_string()),
            created_by_team_member_display_name: None,
        })
        .expect("create checklist");

        node.join_checklist("chk-join".to_string())
            .expect("join checklist");

        let checklist = node
            .get_checklist("chk-join".to_string())
            .expect("get checklist")
            .expect("checklist exists");
        assert!(checklist
            .participant_rns_identities
            .iter()
            .any(|value| value == "creator-identity"));
        assert!(checklist
            .participant_rns_identities
            .iter()
            .any(|value| value == "joiner-identity"));
        assert_eq!(
            checklist
                .last_changed_by_team_member_rns_identity
                .as_deref(),
            Some("joiner-identity")
        );
    }

    #[test]
    fn list_active_checklists_supports_created_at_desc() {
        let storage_dir = prepare_storage_dir("checklist-created-at-desc");
        let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()));

        node.create_online_checklist(ChecklistCreateOnlineRequest {
            checklist_uid: Some("chk-old".to_string()),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: "tmpl-evac-001".to_string(),
            name: "Older Checklist".to_string(),
            description: "Created first".to_string(),
            start_time: "2026-04-22T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: Some("creator-identity".to_string()),
            created_by_team_member_display_name: None,
        })
        .expect("create older checklist");
        node.create_online_checklist(ChecklistCreateOnlineRequest {
            checklist_uid: Some("chk-new".to_string()),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: "tmpl-evac-001".to_string(),
            name: "Newer Checklist".to_string(),
            description: "Created second".to_string(),
            start_time: "2026-04-22T12:05:00Z".to_string(),
            created_by_team_member_rns_identity: Some("creator-identity".to_string()),
            created_by_team_member_display_name: None,
        })
        .expect("create newer checklist");

        {
            let inner = node.inner.lock().expect("node inner");
            let mut older = inner
                .app_state
                .get_checklist_any("chk-old")
                .expect("load older checklist")
                .expect("older checklist present");
            older.created_at = Some("2026-04-22T12:00:00Z".to_string());
            older.updated_at = Some("2026-04-22T12:30:00Z".to_string());
            inner
                .app_state
                .upsert_checklist(&older, "test-created-at-desc-old")
                .expect("persist older checklist");

            let mut newer = inner
                .app_state
                .get_checklist_any("chk-new")
                .expect("load newer checklist")
                .expect("newer checklist present");
            newer.created_at = Some("2026-04-22T12:05:00Z".to_string());
            newer.updated_at = Some("2026-04-22T12:10:00Z".to_string());
            inner
                .app_state
                .upsert_checklist(&newer, "test-created-at-desc-new")
                .expect("persist newer checklist");
        }

        let items = node
            .list_active_checklists(Some(ChecklistListActiveRequest {
                search: None,
                sort_by: Some("created_at_desc".to_string()),
            }))
            .expect("list checklists");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].uid, "chk-new");
        assert_eq!(items[1].uid, "chk-old");
    }
}
