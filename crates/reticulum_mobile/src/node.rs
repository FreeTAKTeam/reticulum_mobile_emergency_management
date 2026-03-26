use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel as cb;
use reticulum::destination::DestinationName;
use rmpv::Value as MsgPackValue;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::app_state::AppStateStore;
use crate::event_bus::EventBus;
use crate::logger::NodeLogger;
use crate::runtime::{load_or_create_identity, now_ms, run_node, Command};
use crate::types::{
    AnnounceRecord, AppSettingsRecord, ConversationRecord, EamProjectionRecord,
    EamSourceRecord, EamTeamSummaryRecord, EventProjectionRecord, LegacyImportPayload, LogLevel,
    MessageRecord, NodeConfig, NodeError, NodeEvent, NodeStatus, OperationalSummary,
    PeerManagementState, PeerRecord, ProjectionInvalidation, ProjectionScope, SavedPeerRecord,
    SendLxmfRequest, SendMode, SyncStatus, TelemetryPositionRecord,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const LXMF_FIELD_COMMANDS: i64 = 0x09;
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

struct NodeInner {
    app_state: AppStateStore,
    bus: EventBus,
    status: Arc<Mutex<NodeStatus>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    runtime: Option<Runtime>,
    cmd_tx: Option<mpsc::Sender<Command>>,
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
    let Some(lxmf_destination_hex) =
        peer.lxmf_destination_hex.as_deref().and_then(normalize_hex_32)
    else {
        return false;
    };
    app_destination_hex != lxmf_destination_hex
}

fn build_mission_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let mut targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
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
        let eligible_direct = peer.mission_ready;
        let eligible_relay = has_active_relay && peer.relay_eligible;
        if !eligible_direct && !eligible_relay {
            continue;
        }
        targets.push(MissionReplicationTarget {
            app_destination_hex,
        });
    }

    targets
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
        .collect::<HashSet<_>>();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
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
        let is_saved = saved_destinations.contains(app_destination_hex.as_str());
        let is_managed = matches!(peer.management_state, PeerManagementState::Managed {});
        if !is_saved && !is_managed {
            continue;
        }
        if peer.active_link && peer.mission_ready {
            direct_targets.push(MissionReplicationTarget {
                app_destination_hex,
            });
            continue;
        }
        if is_saved && has_active_relay && peer.mission_ready && peer.relay_eligible {
            relay_targets.push(MissionReplicationTarget {
                app_destination_hex,
            });
        }
    }

    direct_targets.extend(relay_targets);
    direct_targets
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

fn build_mission_command_fields(
    command_id: &str,
    correlation_id: &str,
    command_type: &str,
    args: Vec<(&str, MsgPackValue)>,
) -> Result<Vec<u8>, NodeError> {
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(LXMF_FIELD_COMMANDS),
        MsgPackValue::Array(vec![msgpack_map(vec![
            ("command_id", MsgPackValue::from(command_id)),
            ("correlation_id", MsgPackValue::from(correlation_id)),
            ("command_type", MsgPackValue::from(command_type)),
            ("args", msgpack_map(args)),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
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
            ("security_status", MsgPackValue::from(record.security_status.as_str())),
            (
                "capability_status",
                MsgPackValue::from(record.capability_status.as_str()),
            ),
            (
                "preparedness_status",
                MsgPackValue::from(record.preparedness_status.as_str()),
            ),
            ("medical_status", MsgPackValue::from(record.medical_status.as_str())),
            ("mobility_status", MsgPackValue::from(record.mobility_status.as_str())),
            ("comms_status", MsgPackValue::from(record.comms_status.as_str())),
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
        .unwrap_or_else(|| format!("event-upsert-{uid}-{}-{send_ts_ms}", &target.app_destination_hex[..8]));
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

    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(LXMF_FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (MsgPackValue::from("command_id"), MsgPackValue::from(command_id)),
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
            (MsgPackValue::from("timestamp"), MsgPackValue::from(timestamp)),
            (
                MsgPackValue::from("args"),
                msgpack_map(vec![
                    ("entry_uid", MsgPackValue::from(uid)),
                    ("mission_uid", MsgPackValue::from(mission_uid)),
                    ("content", MsgPackValue::from(content)),
                    ("callsign", MsgPackValue::from(callsign)),
                    (
                        "source_identity",
                        MsgPackValue::from(source_identity),
                    ),
                    (
                        "source_display_name",
                        MsgPackValue::from(display_name),
                    ),
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
                .collect()),
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

pub struct Node {
    inner: Mutex<NodeInner>,
}

impl Node {
    pub fn new() -> Self {
        Self::with_storage_dir(None)
    }

    fn with_storage_dir(storage_dir: Option<&str>) -> Self {
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

    pub fn start(&self, config: NodeConfig) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        if inner.runtime.is_some() {
            return Err(NodeError::AlreadyRunning {});
        }

        if config.name.trim().is_empty() {
            return Err(NodeError::InvalidConfig {});
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

        let runtime = Runtime::new().map_err(|_| NodeError::InternalError {})?;
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);

        runtime.spawn(run_node(
            config,
            identity,
            inner.app_state.clone(),
            inner.status.clone(),
            inner.peers_snapshot.clone(),
            inner.sync_status_snapshot.clone(),
            inner.bus.clone(),
            cmd_rx,
        ));

        inner.runtime = Some(runtime);
        inner.cmd_tx = Some(cmd_tx);

        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        let (runtime, cmd_tx, bus, status, peers_snapshot, sync_status_snapshot) = {
            let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            (
                inner.runtime.take(),
                inner.cmd_tx.take(),
                inner.bus.clone(),
                inner.status.clone(),
                inner.peers_snapshot.clone(),
                inner.sync_status_snapshot.clone(),
            )
        };

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
        dispatch_command(&tx, Command::ConnectPeer {
            destination_hex,
            resp: resp_tx,
        })
        ?;
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
        dispatch_command(&tx, Command::DisconnectPeer {
            destination_hex,
            resp: resp_tx,
        })
        ?;
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
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::SendBytes {
            destination_hex,
            bytes,
            fields_bytes,
            send_mode,
            resp: resp_tx,
        })
        
    }

    pub fn broadcast_bytes(&self, bytes: Vec<u8>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::BroadcastBytes {
            bytes,
            resp: resp_tx,
        })
        
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
        dispatch_command(&tx, Command::RequestPeerIdentity {
            destination_hex,
            resp: resp_tx,
        })
        ?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_lxmf(&self, request: SendLxmfRequest) -> Result<String, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::SendLxmf {
            request,
            resp: resp_tx,
        })
        ?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn retry_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::RetryLxmf {
            message_id_hex,
            resp: resp_tx,
        })
        
    }

    pub fn cancel_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::CancelLxmf {
            message_id_hex,
            resp: resp_tx,
        })
        ?;
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
        dispatch_command(&tx, Command::SetActivePropagationNode {
            destination_hex,
            resp: resp_tx,
        })
        ?;
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
        dispatch_command(&tx, Command::RequestLxmfSync {
            limit,
            resp: resp_tx,
        })
        ?;
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
        inner.app_state.list_conversations()
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<String>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .app_state
            .list_messages(conversation_id.as_deref())
    }

    pub fn get_lxmf_sync_status(&self) -> Result<SyncStatus, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .sync_status_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| NodeError::InternalError {})
    }

    pub fn set_announce_capabilities(&self, capability_string: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::SetAnnounceCapabilities {
            capability_string,
            resp: resp_tx,
        })
        ?;
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

    pub fn get_eams(&self) -> Result<Vec<EamProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eams()
    }

    pub fn upsert_eam(&self, record: EamProjectionRecord) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>)>::new();
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
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                );
                for target in replication_targets {
                    match build_eam_replication_payload(&status, &normalized_record, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((target.app_destination_hex.clone(), body, fields));
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

        for (destination_hex, body, fields_bytes) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), SendMode::Auto {})
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
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.delete_eam(&callsign, deleted_at_ms)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("eam-deleted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
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
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>)>::new();
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
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let replication_targets = build_event_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                );
                for target in replication_targets {
                    match build_event_replication_payload(&status, &record, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((target.app_destination_hex.clone(), body, fields));
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

        for (destination_hex, body, fields_bytes) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), SendMode::Auto {})
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
            peer_count_communication_ready: peers.iter().filter(|peer| peer.communication_ready).count() as u32,
            peer_count_mission_ready: peers.iter().filter(|peer| peer.mission_ready).count() as u32,
            peer_count_relay_eligible: peers.iter().filter(|peer| peer.relay_eligible).count() as u32,
            saved_peer_count: inner.app_state.get_saved_peers()?.len() as u32,
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

    fn build_config(name: &str, storage_dir: &Path, relay_addr: &str) -> NodeConfig {
        NodeConfig {
            name: name.to_string(),
            storage_dir: Some(storage_dir.to_string_lossy().to_string()),
            tcp_clients: vec![relay_addr.to_string()],
            broadcast: true,
            announce_interval_seconds: 1,
            stale_after_minutes: 30,
            announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            hub_mode: HubMode::Disabled {},
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
            let timeout_ms = remaining
                .as_millis()
                .min(u32::MAX as u128)
                .max(1) as u32;
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
                (MsgPackValue::from("event_type"), MsgPackValue::from(event_type)),
                (MsgPackValue::from("event_id"), MsgPackValue::from(event_uid)),
                (MsgPackValue::from("payload"), msgpack_map(payload)),
            ]),
        )]);
        rmp_serde::to_vec(&fields).expect("msgpack event fields")
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
                mode: HubMode::Disabled {},
                identity_hash: String::new(),
                api_base_url: String::new(),
                api_key: String::new(),
                refresh_interval_seconds: 3600,
            },
        }
    }

    fn build_saved_peer() -> SavedPeerRecord {
        SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("POCO".to_string()),
            saved_at_ms: 1_700_000_000_000,
        }
    }

    fn build_peer_record(
        destination_hex: &str,
        lxmf_destination_hex: &str,
        management_state: PeerManagementState,
        communication_ready: bool,
        mission_ready: bool,
        relay_eligible: bool,
        active_link: bool,
    ) -> PeerRecord {
        PeerRecord {
            destination_hex: destination_hex.to_string(),
            identity_hex: Some(format!("identity-{destination_hex}")),
            lxmf_destination_hex: Some(lxmf_destination_hex.to_string()),
            display_name: Some(format!("peer-{destination_hex}")),
            app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
            state: if communication_ready {
                crate::types::PeerState::Connected {}
            } else {
                crate::types::PeerState::Disconnected {}
            },
            management_state,
            availability_state: if communication_ready {
                crate::types::PeerAvailabilityState::Ready {}
            } else {
                crate::types::PeerAvailabilityState::Resolved {}
            },
            communication_ready,
            mission_ready,
            relay_eligible,
            stale: false,
            active_link,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: Some(now_ms()),
            last_ready_at_ms: communication_ready.then(now_ms),
            last_seen_at_ms: now_ms(),
            announce_last_seen_at_ms: Some(now_ms()),
            lxmf_last_seen_at_ms: Some(now_ms()),
        }
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
                mode: HubMode::Disabled {},
                identity_hash: String::new(),
                api_base_url: String::new(),
                api_key: String::new(),
                refresh_interval_seconds: 0,
            },
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
        assert!(node.legacy_import_completed().expect("legacy import status"));
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
        assert_eq!(conversations[0].conversation_id, "conversation-1");
        assert_eq!(conversations[0].peer_destination_hex, "DEST-1");

        let messages = node
            .list_messages(Some("conversation-1".to_string()))
            .expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id_hex, "msg-1");
        assert_eq!(messages[0].conversation_id, "conversation-1");
        assert_eq!(messages[0].body_utf8, "hello from pre-start");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_only_commands_still_fail_before_start() {
        let _guard = test_lock().lock().await;
        let _cwd = isolate_current_dir("prestart_runtime_commands");
        let node = Node::new();

        assert!(matches!(node.connect_peer("ABCDEF".to_string()), Err(NodeError::NotRunning {})));
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
        assert_eq!(persisted_saved_peers[0].destination_hex, saved_peer.destination_hex);
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
        assert_eq!(persisted_messages[0].conversation_id, message.conversation_id);

        let conversations = node.list_conversations().expect("conversations");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, message.conversation_id);
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
        assert!(matches!(
            node.announce_now(),
            Err(NodeError::NotRunning {})
        ));
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
        assert_eq!(received.team_member_uid.as_deref(), record.team_member_uid.as_deref());

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

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| {
                    peer.mission_ready
                        && peer.communication_ready
                        && peer.lxmf_destination_hex.as_deref()
                            == Some(node_b_status.lxmf_destination_hex.as_str())
                });
            if peer_ready {
                break;
            }
            assert!(Instant::now() < peer_ready_deadline, "peer b never became mission-ready");
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let replication_targets = build_mission_replication_targets(
            &node_a.get_status(),
            node_a.list_peers().expect("list peers").as_slice(),
            node_a
                .get_lxmf_sync_status()
                .expect("sync status")
                .active_propagation_node_hex
                .as_deref(),
        );
        assert_eq!(replication_targets.len(), 1, "expected one eam replication target");
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
        assert_eq!(received.team_member_uid.as_deref(), record.team_member_uid.as_deref());
        assert_eq!(received.eam_uid.as_deref(), record.eam_uid.as_deref());
        assert_eq!(received.security_status, record.security_status);
        assert_eq!(received.capability_status, record.capability_status);
        assert_eq!(received.overall_status.as_deref(), Some("Yellow"));
        assert_eq!(received.source.as_ref().map(|source| source.rns_identity.as_str()), Some(node_a_status.identity_hex.as_str()));

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
            .connect_peer(node_b_status.app_destination_hex.clone())
            .expect("connect peer b");

        let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let peer_ready = node_a
                .list_peers()
                .expect("list peers")
                .into_iter()
                .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
                .is_some_and(|peer| peer.mission_ready && peer.communication_ready);
            if peer_ready {
                break;
            }
            assert!(Instant::now() < peer_ready_deadline, "peer b never became mission-ready");
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
                .is_some_and(|peer| peer.mission_ready && peer.communication_ready);
            if peer_ready {
                break;
            }
            assert!(Instant::now() < peer_ready_deadline, "peer b never became mission-ready");
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
        assert_eq!(replication_targets.len(), 1, "expected one event replication target");
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
                PeerManagementState::Unmanaged {},
                true,
                true,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                PeerManagementState::Unmanaged {},
                true,
                true,
                true,
                true,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                PeerManagementState::Managed {},
                false,
                true,
                true,
                false,
            ),
            build_peer_record(
                "99999999999999999999999999999999",
                "12121212121212121212121212121212",
                PeerManagementState::Managed {},
                true,
                true,
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
        assert_eq!(targets[0].app_destination_hex, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
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
                PeerManagementState::Managed {},
                true,
                true,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                PeerManagementState::Managed {},
                true,
                true,
                true,
                false,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                PeerManagementState::Unmanaged {},
                false,
                true,
                true,
                false,
            ),
            build_peer_record(
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "ffffffffffffffffffffffffffffffff",
                PeerManagementState::Unmanaged {},
                true,
                true,
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

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].app_destination_hex, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(targets[1].app_destination_hex, "cccccccccccccccccccccccccccccccc");
    }
}
