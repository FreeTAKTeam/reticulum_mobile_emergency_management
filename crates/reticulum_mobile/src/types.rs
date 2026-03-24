use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LogLevel {
    Trace {},
    Debug {},
    Info {},
    Warn {},
    Error {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
pub enum PeerManagementState {
    Unmanaged {},
    Managed {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PeerAvailabilityState {
    Unseen {},
    Discovered {},
    Resolved {},
    Ready {},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MessageMethod {
    Direct {},
    Opportunistic {},
    Propagated {},
    Resource {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MessageDirection {
    Inbound {},
    Outbound {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub management_state: PeerManagementState,
    pub availability_state: PeerAvailabilityState,
    pub communication_ready: bool,
    pub stale: bool,
    pub active_link: bool,
    pub last_error: Option<String>,
    pub last_resolution_error: Option<String>,
    pub last_resolution_attempt_at_ms: Option<u64>,
    pub last_ready_at_ms: Option<u64>,
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
    pub management_state: PeerManagementState,
    pub availability_state: PeerAvailabilityState,
    pub communication_ready: bool,
    pub stale: bool,
    pub active_link: bool,
    pub last_resolution_error: Option<String>,
    pub last_resolution_attempt_at_ms: Option<u64>,
    pub last_ready_at_ms: Option<u64>,
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

#[derive(Debug, Clone, Serialize)]
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
    pub use_propagation_node: bool,
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
    Log {
        level: LogLevel,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}
