mod announce_metadata;
mod app_state;
mod delivery_policy;
mod event_bus;
mod jni_bridge;
mod logger;
mod lxmf_fields;
mod messaging_compat;
mod mission_commands;
mod mission_sync;
mod msgpack_values;
mod node;
mod plugin_runtime;
mod runtime;
mod sdk_bridge;
mod sos;
mod sos_detector;
mod sos_fields;
mod types;

pub use node::{EventSubscription, Node};
pub use types::{
    AnnounceClass, AnnounceRecord, AppSettingsRecord, ApplicationAckState, ChecklistCellRecord,
    ChecklistColumnRecord, ChecklistColumnType, ChecklistCreateFromTemplateRequest,
    ChecklistCreateOnlineRequest, ChecklistDeleteRequest, ChecklistFeedPublicationRecord,
    ChecklistListActiveRequest, ChecklistMode, ChecklistOriginType, ChecklistRecord,
    ChecklistSettingsRecord, ChecklistStatusCounts, ChecklistSyncState, ChecklistSystemColumnKey,
    ChecklistTaskCellSetRequest, ChecklistTaskRecord, ChecklistTaskRowAddRequest,
    ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus,
    ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest,
    ChecklistTemplateRecord, ChecklistUpdatePatch, ChecklistUpdateRequest, ChecklistUserTaskStatus,
    ConversationRecord, DiscoveredPluginRecord, EamProjectionRecord, EamReadinessMessageRecord,
    EamReadinessStatusMetricRecord, EamReadinessSummaryRecord, EamSourceRecord,
    EamTeamSummaryRecord, EventProjectionRecord, HubDirectoryPeerRecord, HubDirectorySnapshot,
    HubMode, HubSettingsRecord, InstalledPluginRecord, InterfaceStatusRecord, LegacyImportPayload,
    LogLevel, LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfDeliveryStatus,
    LxmfDeliveryUpdate, LxmfFallbackStage, MessageDirection, MessageMethod, MessageRecord,
    MessageState, NodeConfig, NodeError, NodeEvent, NodeStatus, OperationalNotice,
    OperationalSummary, PeerChange, PeerRecord, PeerState, PluginCapabilityRecord,
    PluginEventRecord, PluginLxmfSendRequest, PluginMessageDescriptorRecord, PluginSensorRecord,
    PluginSensorSampleRequest, ProjectionInvalidation, ProjectionScope, RnodeSettingsRecord,
    RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot, RuntimeReadinessState,
    SavedPeerRecord, SendLxmfRequest, SendMode, SendOutcome, SosAlertRecord, SosAudioRecord,
    SosDeviceTelemetryRecord, SosLocationRecord, SosMessageKind, SosSettingsRecord, SosState,
    SosStatusRecord, SosTriggerSource, SyncPhase, SyncStatus, TelemetryPositionRecord,
    TelemetrySettingsRecord, TransportDeliveryState, TrustedPluginPublisherRecord,
};

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// Include UniFFI-generated scaffolding (built from `reticulum_mobile.udl`).
uniffi::include_scaffolding!("reticulum_mobile");
