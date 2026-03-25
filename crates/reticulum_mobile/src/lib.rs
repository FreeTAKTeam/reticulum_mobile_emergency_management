mod app_state;
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
    AnnounceRecord, AppSettingsRecord, ConversationRecord, EamProjectionRecord,
    EamSourceRecord, EamTeamSummaryRecord, EventProjectionRecord, HubMode, HubSettingsRecord,
    LegacyImportPayload, LogLevel, LxmfDeliveryMethod, LxmfDeliveryRepresentation,
    LxmfDeliveryStatus, LxmfDeliveryUpdate, LxmfFallbackStage, MessageDirection, MessageMethod,
    MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent, NodeStatus,
    OperationalSummary, PeerAvailabilityState, PeerChange, PeerManagementState, PeerRecord,
    PeerState, ProjectionInvalidation, ProjectionScope, SavedPeerRecord, SendLxmfRequest,
    SendMode, SendOutcome, SyncPhase, SyncStatus, TelemetryPositionRecord,
    TelemetrySettingsRecord,
};

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// Include UniFFI-generated scaffolding (built from `reticulum_mobile.udl`).
uniffi::include_scaffolding!("reticulum_mobile");
