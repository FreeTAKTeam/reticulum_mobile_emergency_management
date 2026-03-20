mod announce_compat;
mod event_bus;
mod jni_bridge;
mod logger;
mod mission_sync;
mod messaging_compat;
mod node;
mod runtime;
mod sdk_bridge;
mod types;

pub use node::{EventSubscription, Node};
pub use types::{
    AnnounceRecord, ConversationRecord, HubMode, LogLevel, LxmfDeliveryStatus,
    LxmfDeliveryUpdate, MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig,
    NodeError, NodeEvent, NodeStatus, PeerChange, PeerRecord, PeerState, SendLxmfRequest,
    SendOutcome, SyncPhase, SyncStatus,
};

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// Include UniFFI-generated scaffolding (built from `reticulum_mobile.udl`).
uniffi::include_scaffolding!("reticulum_mobile");
