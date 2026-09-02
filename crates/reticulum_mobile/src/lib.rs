#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]
// UniFFI 0.28 emits a separated metadata doc comment in generated scaffolding.
#![allow(clippy::empty_line_after_doc_comments)]
// JNI tests share the include-based bridge module with the exported functions they exercise.
#![allow(clippy::items_after_test_module)]
// Generated UniFFI and JNI exports require unsafe attributes such as `no_mangle`.
#![allow(unsafe_code)]

#[cfg(target_os = "android")]
#[deny(unsafe_code)]
mod android_rnode_backend;

#[deny(unsafe_code)]
mod announce_metadata;
#[deny(unsafe_code)]
mod app_state;
#[deny(unsafe_code)]
mod delivery_policy;
#[deny(unsafe_code)]
mod error_context;
#[deny(unsafe_code)]
mod event_bus;
// JNI requires stable, unmangled export names. The bridge contains no unsafe blocks.
#[allow(unsafe_code)]
mod jni_bridge;
#[deny(unsafe_code)]
mod logger;
#[deny(unsafe_code)]
mod lxmf_fields;
#[deny(unsafe_code)]
mod messaging_compat;
#[deny(unsafe_code)]
mod mission_commands;
#[deny(unsafe_code)]
mod mission_sync;
#[deny(unsafe_code)]
mod msgpack_values;
#[deny(unsafe_code)]
mod node;
#[deny(unsafe_code)]
mod numeric;
#[deny(unsafe_code)]
mod plugin_runtime;
#[deny(unsafe_code)]
mod runtime;
#[deny(unsafe_code)]
mod sdk_bridge;
#[deny(unsafe_code)]
mod sos;
#[deny(unsafe_code)]
mod sos_detector;
#[deny(unsafe_code)]
mod sos_fields;
#[deny(unsafe_code)]
mod types;

pub use node::{EventSubscription, Node};
pub use types::{
    AnnounceClass, AnnounceRecord, AppSettingsRecord, ApplicationAckState, BlockNetworkSettings,
    BlockOnboardingDraft, BlockOnboardingImportRequest, BlockOnboardingImportResult,
    BlockOnboardingInspection, BlockPeerTierRecord, BlockRadioSettings, ChecklistCellRecord,
    ChecklistColumnRecord, ChecklistColumnType, ChecklistCreateFromTemplateRequest,
    ChecklistCreateOnlineRequest, ChecklistDeleteRequest, ChecklistFeedPublicationRecord,
    ChecklistListActiveRequest, ChecklistMode, ChecklistOriginType, ChecklistRecord,
    ChecklistSettingsRecord, ChecklistStatusCounts, ChecklistSyncState, ChecklistSystemColumnKey,
    ChecklistTaskCellSetRequest, ChecklistTaskRecord, ChecklistTaskRowAddRequest,
    ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus,
    ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistTemplateListRequest,
    ChecklistTemplateRecord, ChecklistUpdatePatch, ChecklistUpdateRequest, ChecklistUserTaskStatus,
    CircleTier, CommunitySettingsRecord, CommunityStatusProjectionRecord, ConversationRecord,
    DiscoveredPluginRecord, EamProjectionRecord, EamReadinessMessageRecord,
    EamReadinessStatusMetricRecord, EamReadinessSummaryRecord, EamSourceRecord,
    EamTeamSummaryRecord, EventProjectionRecord, HouseholdStatus, HubCallerMembershipRecord,
    HubDirectoryPeerRecord, HubDirectorySnapshot, HubMode, HubSettingsRecord, HubTeamMemberRecord,
    HubTeamRecord, InstalledPluginRecord, InterfaceStatusRecord, LegacyImportPayload,
    LocalTeamRecord, LogLevel, LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfDeliveryStatus,
    LxmfDeliveryUpdate, LxmfFallbackStage, MessageDirection, MessageMethod, MessageRecord,
    MessageState, NodeConfig, NodeError, NodeEvent, NodeStatus, OperationalNotice,
    OperationalSummary, OutboundTrafficClass, PeerChange, PeerRecord, PeerState,
    PluginCapabilityRecord, PluginEventRecord, PluginLxmfSendRequest,
    PluginMessageDescriptorRecord, PluginSensorRecord, PluginSensorSampleRequest,
    PowerPolicyRecord, PowerStateRecord, PreferredMapLayer, ProjectionInvalidation,
    ProjectionScope, RnodeSettingsRecord, RuntimeInterfaceReadinessRecord,
    RuntimeReadinessSnapshot, RuntimeReadinessState, SavedPeerRecord, SendLxmfRequest, SendMode,
    SendOutcome, SignedBlockOnboardingEnvelope, SosAlertRecord, SosAudioRecord,
    SosDeviceTelemetryRecord, SosLocationRecord, SosMessageKind, SosSettingsRecord, SosState,
    SosStatusRecord, SosTriggerSource, SyncPhase, SyncStatus, TeamAliasRecord, TeamSettingsRecord,
    TelemetryPositionRecord, TelemetrySettingsRecord, TransportDeliveryState,
    TrustedPluginPublisherRecord,
};

#[must_use]
pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// UniFFI generates the required unmangled C ABI exports from the UDL.
uniffi::include_scaffolding!("reticulum_mobile");
