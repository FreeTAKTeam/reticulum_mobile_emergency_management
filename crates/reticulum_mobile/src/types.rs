use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LogLevel {
    Trace {},
    Debug {},
    Info {},
    Warn {},
    Error {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HubMode {
    Disabled {},
    RchLxmf {},
    RchHttp {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PeerState {
    Connecting {},
    Connected {},
    Disconnected {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SendOutcome {
    SentDirect {},
    SentBroadcast {},
    DroppedMissingDestinationIdentity {},
    DroppedCiphertextTooLarge {},
    DroppedEncryptFailed {},
    DroppedNoRoute {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LxmfDeliveryStatus {
    Sent {},
    SentToPropagation {},
    Acknowledged {},
    Failed {},
    TimedOut {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SendMode {
    Auto {},
    DirectOnly {},
    PropagationOnly {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LxmfDeliveryMethod {
    Direct {},
    Opportunistic {},
    Propagated {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LxmfDeliveryRepresentation {
    Packet {},
    Resource {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LxmfFallbackStage {
    AfterDirectRetryBudget {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageMethod {
    Direct {},
    Opportunistic {},
    Propagated {},
    Resource {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageState {
    Queued {},
    PathRequested {},
    LinkEstablishing {},
    Sending {},
    SentDirect {},
    SentToPropagation {},
    Delivered {},
    Failed {},
    TimedOut {},
    Cancelled {},
    Received {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    Inbound {},
    Outbound {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Idle {},
    PathRequested {},
    LinkEstablishing {},
    RequestSent {},
    Receiving {},
    Complete {},
    Failed {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NodeError {
    #[error("invalid config")]
    InvalidConfig {},
    #[error("io error")]
    IoError {},
    #[error("network error")]
    NetworkError {},
    #[error("reticulum error")]
    ReticulumError {},
    #[error("already running")]
    AlreadyRunning {},
    #[error("not running")]
    NotRunning {},
    #[error("timeout")]
    Timeout {},
    #[error("lxmf wire encode failed")]
    LxmfWireEncodeError {},
    #[error("lxmf message id parse failed")]
    LxmfMessageIdParseError {},
    #[error("lxmf packet too large")]
    LxmfPacketTooLarge {},
    #[error("lxmf packet build failed")]
    LxmfPacketBuildError {},
    #[error("event stream closed")]
    EventStreamClosed {},
    #[error("internal error")]
    InternalError {},
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeConfig {
    pub name: String,
    pub storage_dir: Option<String>,
    pub tcp_clients: Vec<String>,
    pub broadcast: bool,
    pub announce_interval_seconds: u32,
    pub stale_after_minutes: u32,
    pub announce_capabilities: String,
    pub hub_mode: HubMode,
    pub hub_identity_hash: Option<String>,
    pub hub_api_base_url: Option<String>,
    pub hub_api_key: Option<String>,
    pub hub_refresh_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    pub running: bool,
    pub name: String,
    pub identity_hex: String,
    pub app_destination_hex: String,
    pub lxmf_destination_hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerChange {
    pub destination_hex: String,
    pub identity_hex: Option<String>,
    pub lxmf_destination_hex: Option<String>,
    pub display_name: Option<String>,
    pub app_data: Option<String>,
    pub state: PeerState,
    pub saved: bool,
    pub stale: bool,
    pub active_link: bool,
    pub last_error: Option<String>,
    pub last_resolution_error: Option<String>,
    pub last_resolution_attempt_at_ms: Option<u64>,
    pub last_seen_at_ms: u64,
    pub announce_last_seen_at_ms: Option<u64>,
    pub lxmf_last_seen_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxmfDeliveryUpdate {
    pub message_id_hex: String,
    pub destination_hex: String,
    pub source_hex: Option<String>,
    pub correlation_id: Option<String>,
    pub command_id: Option<String>,
    pub command_type: Option<String>,
    pub event_uid: Option<String>,
    pub mission_uid: Option<String>,
    pub status: LxmfDeliveryStatus,
    pub method: LxmfDeliveryMethod,
    pub representation: LxmfDeliveryRepresentation,
    pub relay_destination_hex: Option<String>,
    pub fallback_stage: Option<LxmfFallbackStage>,
    pub detail: Option<String>,
    pub sent_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnounceRecord {
    pub destination_hex: String,
    pub identity_hex: String,
    pub destination_kind: String,
    pub app_data: String,
    pub display_name: Option<String>,
    pub hops: u8,
    pub interface_hex: String,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerRecord {
    pub destination_hex: String,
    pub identity_hex: Option<String>,
    pub lxmf_destination_hex: Option<String>,
    pub display_name: Option<String>,
    pub app_data: Option<String>,
    pub state: PeerState,
    pub saved: bool,
    pub stale: bool,
    pub active_link: bool,
    pub last_resolution_error: Option<String>,
    pub last_resolution_attempt_at_ms: Option<u64>,
    pub last_seen_at_ms: u64,
    pub announce_last_seen_at_ms: Option<u64>,
    pub lxmf_last_seen_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationRecord {
    pub conversation_id: String,
    pub peer_destination_hex: String,
    pub peer_display_name: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_at_ms: u64,
    pub unread_count: u32,
    pub last_message_state: Option<MessageState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub message_id_hex: String,
    pub conversation_id: String,
    pub direction: MessageDirection,
    pub destination_hex: String,
    pub source_hex: Option<String>,
    pub title: Option<String>,
    pub body_utf8: String,
    pub method: MessageMethod,
    pub state: MessageState,
    pub detail: Option<String>,
    pub sent_at_ms: Option<u64>,
    pub received_at_ms: Option<u64>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub phase: SyncPhase,
    pub active_propagation_node_hex: Option<String>,
    pub requested_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub messages_received: u32,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendLxmfRequest {
    pub destination_hex: String,
    pub body_utf8: String,
    pub title: Option<String>,
    pub send_mode: SendMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSettingsRecord {
    pub mode: HubMode,
    pub identity_hash: String,
    pub api_base_url: String,
    pub api_key: String,
    pub refresh_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySettingsRecord {
    pub enabled: bool,
    pub publish_interval_seconds: u32,
    pub accuracy_threshold_meters: Option<f64>,
    pub stale_after_minutes: u32,
    pub expire_after_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsRecord {
    pub display_name: String,
    pub auto_connect_saved: bool,
    pub announce_capabilities: String,
    pub tcp_clients: Vec<String>,
    pub broadcast: bool,
    pub announce_interval_seconds: u32,
    pub telemetry: TelemetrySettingsRecord,
    pub hub: HubSettingsRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPeerRecord {
    pub destination_hex: String,
    pub label: Option<String>,
    pub saved_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamSourceRecord {
    pub rns_identity: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamProjectionRecord {
    pub callsign: String,
    pub group_name: String,
    pub security_status: String,
    pub capability_status: String,
    pub preparedness_status: String,
    pub medical_status: String,
    pub mobility_status: String,
    pub comms_status: String,
    pub notes: Option<String>,
    pub updated_at_ms: u64,
    pub deleted_at_ms: Option<u64>,
    pub eam_uid: Option<String>,
    pub team_member_uid: Option<String>,
    pub team_uid: Option<String>,
    pub reported_at: Option<String>,
    pub reported_by: Option<String>,
    pub overall_status: Option<String>,
    pub confidence: Option<f64>,
    pub ttl_seconds: Option<u64>,
    pub source: Option<EamSourceRecord>,
    pub sync_state: Option<String>,
    pub sync_error: Option<String>,
    pub draft_created_at_ms: Option<u64>,
    pub last_synced_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamTeamSummaryRecord {
    pub team_uid: String,
    pub total: u32,
    pub active_total: u32,
    pub deleted_total: u32,
    pub overall_status: Option<String>,
    pub green_total: u32,
    pub yellow_total: u32,
    pub red_total: u32,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProjectionRecord {
    pub uid: String,
    pub command_id: String,
    pub source_identity: String,
    pub source_display_name: Option<String>,
    pub timestamp: String,
    pub command_type: String,
    pub mission_uid: String,
    pub content: String,
    pub callsign: String,
    pub server_time: Option<String>,
    pub client_time: Option<String>,
    pub keywords: Vec<String>,
    pub content_hashes: Vec<String>,
    pub updated_at_ms: u64,
    pub deleted_at_ms: Option<u64>,
    pub correlation_id: Option<String>,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPositionRecord {
    pub callsign: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
    pub course: Option<f64>,
    pub speed: Option<f64>,
    pub accuracy: Option<f64>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyImportPayload {
    pub settings: Option<AppSettingsRecord>,
    pub saved_peers: Vec<SavedPeerRecord>,
    pub eams: Vec<EamProjectionRecord>,
    pub events: Vec<EventProjectionRecord>,
    pub messages: Vec<MessageRecord>,
    pub telemetry_positions: Vec<TelemetryPositionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionScope {
    AppSettings {},
    SavedPeers {},
    OperationalSummary {},
    Peers {},
    SyncStatus {},
    HubRegistration {},
    Eams {},
    Events {},
    Conversations {},
    Messages {},
    Telemetry {},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionInvalidation {
    pub scope: ProjectionScope,
    pub key: Option<String>,
    pub revision: u64,
    pub updated_at_ms: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationalSummary {
    pub running: bool,
    pub peer_count_total: u32,
    pub saved_peer_count: u32,
    pub connected_peer_count: u32,
    pub conversation_count: u32,
    pub message_count: u32,
    pub eam_count: u32,
    pub event_count: u32,
    pub telemetry_count: u32,
    pub active_propagation_node_hex: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone)]
pub enum NodeEvent {
    StatusChanged {
        status: NodeStatus,
    },
    AnnounceReceived {
        destination_hex: String,
        identity_hex: String,
        destination_kind: String,
        app_data: String,
        hops: u8,
        interface_hex: String,
        received_at_ms: u64,
    },
    PeerChanged {
        change: PeerChange,
    },
    PacketReceived {
        destination_hex: String,
        source_hex: Option<String>,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
    },
    PacketSent {
        destination_hex: String,
        bytes: Vec<u8>,
        outcome: SendOutcome,
    },
    LxmfDelivery {
        update: LxmfDeliveryUpdate,
    },
    PeerResolved {
        peer: PeerRecord,
    },
    MessageReceived {
        message: MessageRecord,
    },
    MessageUpdated {
        message: MessageRecord,
    },
    SyncUpdated {
        status: SyncStatus,
    },
    HubDirectoryUpdated {
        destinations: Vec<String>,
        received_at_ms: u64,
    },
    ProjectionInvalidated {
        invalidation: ProjectionInvalidation,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}
