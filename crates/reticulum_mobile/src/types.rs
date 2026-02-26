use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace {},
    Debug {},
    Info {},
    Warn {},
    Error {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubMode {
    Disabled {},
    RchLxmf {},
    RchHttp {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Connecting {},
    Connected {},
    Disconnected {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    SentDirect {},
    SentBroadcast {},
    DroppedMissingDestinationIdentity {},
    DroppedCiphertextTooLarge {},
    DroppedEncryptFailed {},
    DroppedNoRoute {},
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
    #[error("internal error")]
    InternalError {},
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub running: bool,
    pub name: String,
    pub identity_hex: String,
    pub app_destination_hex: String,
    pub lxmf_destination_hex: String,
}

#[derive(Debug, Clone)]
pub struct PeerChange {
    pub destination_hex: String,
    pub state: PeerState,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum NodeEvent {
    StatusChanged { status: NodeStatus },
    AnnounceReceived {
        destination_hex: String,
        app_data: String,
        hops: u8,
        interface_hex: String,
        received_at_ms: u64,
    },
    PeerChanged { change: PeerChange },
    PacketReceived { destination_hex: String, bytes: Vec<u8> },
    PacketSent {
        destination_hex: String,
        bytes: Vec<u8>,
        outcome: SendOutcome,
    },
    HubDirectoryUpdated {
        destinations: Vec<String>,
        received_at_ms: u64,
    },
    Log { level: LogLevel, message: String },
    Error { code: String, message: String },
}

