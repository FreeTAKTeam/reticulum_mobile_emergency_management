use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel as cb;
use flate2::{write::ZlibEncoder, Compression};
use reticulum::transport::destination::{
    DestinationName, SingleInputDestination, SingleOutputDestination,
};
use rmpv::Value as MsgPackValue;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::sync::{mpsc, watch};

use crate::announce_metadata::has_capability_token;
use crate::app_state::{
    canonicalize_chat_message, current_timestamp_rfc3339, AppStateStore, ConversationPeerResolver,
};
use crate::delivery_policy;
use crate::delivery_policy::normalize_hex_32;
use crate::event_bus::EventBus;
use crate::logger::NodeLogger;
use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_GROUP};
use crate::messaging_compat as sdkmsg;
use crate::mission_commands::{checklist_arg_wire_key, command_wire_value};
use crate::runtime::{load_or_create_identity, now_ms, run_node, Command};
use crate::sos::{
    active_status, compose_sos_body, countdown_status, default_sos_settings, idle_status,
    new_incident_id, normalize_sos_settings, set_pin, verify_pin,
};
use crate::sos_detector::SosTriggerDetector;
use crate::sos_fields::{build_sos_fields, SosCommand};
use crate::types::{
    canonical_team_color_for_uid, AnnounceRecord, AppSettingsRecord, ApplicationAckState,
    BlockNetworkSettings, BlockOnboardingDraft, BlockOnboardingImportRequest,
    BlockOnboardingImportResult, BlockOnboardingInspection, BlockRadioSettings,
    ChecklistCreateFromTemplateRequest, ChecklistCreateOnlineRequest, ChecklistDeleteRequest,
    ChecklistListActiveRequest, ChecklistRecord, ChecklistTaskCellSetRequest,
    ChecklistTaskRowAddRequest, ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest,
    ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest,
    ChecklistTemplateRecord, ChecklistUpdateRequest, CircleTier, CommunitySettingsRecord,
    CommunityStatusProjectionRecord, ConversationRecord, DiscoveredPluginRecord,
    EamProjectionRecord, EamReadinessSummaryRecord, EamSourceRecord, EamTeamSummaryRecord,
    EventProjectionRecord, HouseholdStatus, HubDirectorySnapshot, HubMode, InstalledPluginRecord,
    LegacyImportPayload, LogLevel, MessageDirection, MessageMethod, MessageRecord, MessageState,
    NodeConfig, NodeError, NodeEvent, NodeStatus, OperationalNotice, OperationalSummary,
    OutboundTrafficClass, PeerRecord, PluginCapabilityRecord, PluginEventRecord,
    PluginLxmfSendRequest, PluginSensorRecord, PluginSensorSampleRequest, PowerPolicyRecord,
    PowerStateRecord, PreferredMapLayer, ProjectionInvalidation, ProjectionScope,
    RuntimeReadinessSnapshot, SavedPeerRecord, SendLxmfRequest, SendMode,
    SignedBlockOnboardingEnvelope, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord,
    SosLocationRecord, SosMessageKind, SosSettingsRecord, SosState, SosStatusRecord,
    SosTriggerSource, SyncStatus, TeamSettingsRecord, TelemetryPositionRecord,
    TransportDeliveryState, HUB_DIRECTORY_SCHEMA_VERSION, YELLOW_TEAM_UID,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const DEFAULT_R3AKT_MISSION_UID: &str = "r3akt-default-mission";
const SEND_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const LXMF_SYNC_COMMAND_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const COMMAND_QUEUE_CAPACITY: usize = 256;
const PRIORITY_COMMAND_QUEUE_CAPACITY: usize = 1_024;
fn dispatch_command(tx: &mpsc::Sender<Command>, command: Command) -> Result<(), NodeError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return tx.try_send(command).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => NodeError::Timeout {},
            mpsc::error::TrySendError::Closed(_) => NodeError::NotRunning {},
        });
    }

    tx.blocking_send(command).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::NotRunning {}, error)
    })
}

fn build_node_runtime() -> Result<Runtime, NodeError> {
    RuntimeBuilder::new_multi_thread()
        .enable_io()
        .enable_time()
        .worker_threads(2)
        .thread_name("rem-node")
        .build()
        .map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })
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
    power_state: PowerStateRecord,
    power_saver_tx: watch::Sender<bool>,
    next_telemetry_publish_at_ms: Option<u64>,
    deferred_announce_capabilities: Option<String>,
    community_status_sent_in_saver: bool,
    active_config: Option<NodeConfigFingerprint>,
    runtime: Option<Runtime>,
    cmd_tx: Option<mpsc::Sender<Command>>,
    priority_cmd_tx: Option<mpsc::Sender<Command>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeConfigFingerprint {
    name: String,
    storage_dir: Option<String>,
    tcp_clients: Vec<String>,
    broadcast: bool,
    transport_node_enabled: bool,
    announce_interval_seconds: u32,
    stale_after_minutes: u32,
    announce_capabilities: String,
    hub_mode: crate::types::HubMode,
    hub_identity_hash: Option<String>,
    hub_api_base_url: Option<String>,
    hub_api_key: Option<String>,
    hub_refresh_interval_seconds: u32,
    rnode: crate::types::RnodeSettingsRecord,
}

impl NodeConfigFingerprint {
    fn from_config(config: &NodeConfig) -> Result<Self, NodeError> {
        let name = config.name.trim();
        if name.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        crate::types::RnodeConnectionMode::parse(Some(&config.rnode.connection_mode))?;

        Ok(Self {
            name: name.to_string(),
            storage_dir: config.storage_dir.clone(),
            tcp_clients: config.tcp_clients.clone(),
            broadcast: config.broadcast,
            transport_node_enabled: config.transport_node_enabled,
            announce_interval_seconds: config.announce_interval_seconds,
            stale_after_minutes: config.stale_after_minutes,
            announce_capabilities: config.announce_capabilities.clone(),
            hub_mode: config.hub_mode,
            hub_identity_hash: config.hub_identity_hash.clone(),
            hub_api_base_url: config.hub_api_base_url.clone(),
            hub_api_key: config.hub_api_key.clone(),
            hub_refresh_interval_seconds: config.hub_refresh_interval_seconds,
            rnode: config.rnode.clone(),
        })
    }

    fn to_config(&self) -> NodeConfig {
        NodeConfig {
            name: self.name.clone(),
            storage_dir: self.storage_dir.clone(),
            tcp_clients: self.tcp_clients.clone(),
            broadcast: self.broadcast,
            transport_node_enabled: self.transport_node_enabled,
            announce_interval_seconds: self.announce_interval_seconds,
            stale_after_minutes: self.stale_after_minutes,
            announce_capabilities: self.announce_capabilities.clone(),
            hub_mode: self.hub_mode,
            hub_identity_hash: self.hub_identity_hash.clone(),
            hub_api_base_url: self.hub_api_base_url.clone(),
            hub_api_key: self.hub_api_key.clone(),
            hub_refresh_interval_seconds: self.hub_refresh_interval_seconds,
            rnode: self.rnode.clone(),
        }
    }
}

fn create_app_state_store(storage_dir: Option<&str>) -> Result<AppStateStore, NodeError> {
    let fallback = std::env::temp_dir()
        .join("reticulum_mobile_app_state")
        .to_string_lossy()
        .to_string();
    create_app_state_store_with_fallback(storage_dir, fallback.as_str())
}

fn create_app_state_store_with_fallback(
    storage_dir: Option<&str>,
    fallback: &str,
) -> Result<AppStateStore, NodeError> {
    match AppStateStore::new(storage_dir) {
        Ok(store) => Ok(store),
        Err(_) => AppStateStore::new(Some(fallback)).map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        }),
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

fn fields_with_active_team(
    fields_bytes: Option<Vec<u8>>,
    team_uid: &str,
) -> Result<Vec<u8>, NodeError> {
    let fields = fields_bytes
        .as_deref()
        .map(rmp_serde::from_slice::<MsgPackValue>)
        .transpose()
        .map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?
        .unwrap_or_else(|| MsgPackValue::Map(Vec::new()));
    let MsgPackValue::Map(mut entries) = fields else {
        return Err(NodeError::InternalError {});
    };
    entries.retain(|(key, _)| key.as_i64() != Some(FIELD_GROUP));
    entries.push((
        MsgPackValue::from(FIELD_GROUP),
        MsgPackValue::from(team_uid),
    ));
    rmp_serde::to_vec(&MsgPackValue::Map(entries)).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
    })
}

include!("node/local_team_routing.rs");
include!("node/replication_targets.rs");
include!("node/chat_routing.rs");
include!("node/replication_planning.rs");
include!("node/replication_routing.rs");
include!("node/wire_fields.rs");
include!("node/checklist_payloads.rs");
include!("node/checklist_task_args.rs");
include!("node/mission_payloads.rs");
include!("node/sos_fanout.rs");
include!("node/community.rs");
include!("node/outbound_policy.rs");
include!("node/block_onboarding.rs");

pub struct Node {
    inner: Mutex<NodeInner>,
}

include!("node/lifecycle.rs");
include!("node/status.rs");
include!("node/messaging.rs");
include!("node/messaging_policy.rs");
include!("node/legacy.rs");
include!("node/settings.rs");
include!("node/checklist_queries.rs");
include!("node/checklist_mutations.rs");
include!("node/checklist_task_status.rs");
include!("node/checklist_task_edits.rs");
include!("node/eam.rs");
include!("node/plugin_registry.rs");
include!("node/events_plugins_telemetry.rs");
include!("node/sos.rs");
include!("node/team.rs");
include!("node/event_subscription.rs");

#[cfg(test)]
mod tests {
    include!("node/tests/support.rs");
    include!("node/tests/checklist_contracts_checklist_create_online_args_match_support.rs");
    include!("node/tests/checklist_contracts_checklist_upload_snapshot_uses_compressed_.rs");
    include!("node/tests/checklist_contracts_create_online_checklist_rejects_invalid_pa.rs");
    include!("node/tests/checklist_routing.rs");
    include!("node/tests/checklist_runtime.rs");
    include!("node/tests/core.rs");
    include!("node/tests/eam_build_eam_replication_payload_emits_numeri.rs");
    include!("node/tests/eam_eam_replication_targets_include_saved_dire.rs");
    include!("node/tests/eam_repeated_eam_updates_with_same_callsign_re.rs");
    include!("node/tests/eam_upsert_eam_replicates_to_native_peer_proje.rs");
    include!("node/tests/events_delete_event_replicates_to_native_peer_pro.rs");
    include!("node/tests/events_event_replication_payload_uses_compact_mec.rs");
    include!("node/tests/events_event_route_priority_puts_low_hop_live_pee.rs");
    include!("node/tests/lifecycle.rs");
    include!("node/tests/messaging_delivery.rs");
    include!("node/tests/peers_routes.rs");
    include!("node/tests/peers_routes_capabilities.rs");
    include!("node/tests/chat_routing.rs");
    include!("node/tests/team_switch.rs");
    include!("node/tests/local_team_routing.rs");
    include!("node/tests/propagation.rs");
    include!("node/tests/sos_sos_targets_skip_unsaved_stale_stored_rout.rs");
    include!("node/tests/sos_send_outcomes.rs");
    include!("node/tests/sos_trigger_sos_rebroadcasts_existing_active_i.rs");
    include!("node/tests/telemetry_connected_telemetry_destinations_route_onl.rs");
    include!("node/tests/telemetry_telemetry_replication_payload_stays_under_.rs");
    include!("node/tests/community.rs");
    include!("node/tests/outbound_policy.rs");
    include!("node/tests/block_onboarding.rs");
}
