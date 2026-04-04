mod announce_compat;
mod app_state;
mod event_bus;
mod jni_bridge;
mod logger;
mod messaging_compat;
mod mission_sync;
mod node;
mod runtime;
mod sdk_bridge;
mod types;

pub use node::{EventSubscription, Node};
pub use types::{
    AnnounceRecord, AppSettingsRecord, ConversationRecord, EamProjectionRecord, EamSourceRecord,
    EamTeamSummaryRecord, EventProjectionRecord, HubDirectoryPeerRecord, HubDirectorySnapshot,
    HubMode, HubSettingsRecord, LegacyImportPayload, LogLevel, LxmfDeliveryMethod,
    LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfDeliveryUpdate, LxmfFallbackStage,
    MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent,
    NodeStatus, OperationalSummary, PeerChange, PeerRecord, PeerState, ProjectionInvalidation,
    ProjectionScope, SavedPeerRecord, SendLxmfRequest, SendMode, SendOutcome, SyncPhase,
    SyncStatus, TelemetryPositionRecord, TelemetrySettingsRecord,
};

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// Include UniFFI-generated scaffolding (built from `reticulum_mobile.udl`).
uniffi::include_scaffolding!("reticulum_mobile");
