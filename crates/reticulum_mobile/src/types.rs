use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

macro_rules! string_enum {
    ($vis:vis enum $name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis enum $name {
            $(
                $variant {},
            )+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(
                        Self::$variant {} => $value,
                    )+
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str((*self).as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                match value.trim().to_ascii_uppercase().as_str() {
                    $(
                        $value => Ok(Self::$variant {}),
                    )+
                    other => Err(D::Error::custom(format!(
                        "unknown {}: {other}",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LogLevel {
    Trace {},
    Debug {},
    Info {},
    Warn {},
    Error {},
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationalNotice {
    pub level: LogLevel,
    pub message: String,
    pub at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubMode {
    Autonomous {},
    SemiAutonomous {},
    Connected {},
}

impl HubMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Autonomous {} => "Autonomous",
            Self::SemiAutonomous {} => "SemiAutonomous",
            Self::Connected {} => "Connected",
        }
    }
}

impl Serialize for HubMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*self).as_str())
    }
}

impl<'de> Deserialize<'de> for HubMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "autonomous" | "disabled" => Ok(Self::Autonomous {}),
            "semiautonomous" | "semi_autonomous" | "semi-autonomous" | "rchlxmf" | "rch_lxmf"
            | "rchhttp" | "rch_http" => Ok(Self::SemiAutonomous {}),
            "connected" => Ok(Self::Connected {}),
            other => Err(D::Error::custom(format!("unknown hub mode: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PeerState {
    Connecting {},
    Connected {},
    Disconnected {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AnnounceClass {
    PeerApp {},
    RchHubServer {},
    PropagationNode {},
    LxmfDelivery {},
    Other {},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SosState {
    Idle {},
    Countdown {},
    Sending {},
    Active {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SosTriggerSource {
    Manual {},
    FloatingButton {},
    Shake {},
    TapPattern {},
    PowerButton {},
    Restore {},
    Remote {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SosMessageKind {
    Active {},
    Update {},
    Cancelled {},
}

string_enum! {
    pub enum ChecklistMode {
        Online => "ONLINE",
        Offline => "OFFLINE"
    }
}

string_enum! {
    pub enum ChecklistSyncState {
        LocalOnly => "LOCAL_ONLY",
        UploadPending => "UPLOAD_PENDING",
        Synced => "SYNCED"
    }
}

string_enum! {
    pub enum ChecklistOriginType {
        RchTemplate => "RCH_TEMPLATE",
        BlankTemplate => "BLANK_TEMPLATE",
        CsvImport => "CSV_IMPORT",
        ExistingTemplateClone => "EXISTING_TEMPLATE_CLONE"
    }
}

string_enum! {
    pub enum ChecklistUserTaskStatus {
        Pending => "PENDING",
        Complete => "COMPLETE"
    }
}

string_enum! {
    pub enum ChecklistTaskStatus {
        Pending => "PENDING",
        Complete => "COMPLETE",
        CompleteLate => "COMPLETE_LATE",
        Late => "LATE"
    }
}

impl ChecklistTaskStatus {
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Complete {} | Self::CompleteLate {})
    }

    pub fn is_late(self) -> bool {
        matches!(self, Self::Late {} | Self::CompleteLate {})
    }
}

string_enum! {
    pub enum ChecklistColumnType {
        ShortString => "SHORT_STRING",
        LongString => "LONG_STRING",
        Integer => "INTEGER",
        ActualTime => "ACTUAL_TIME",
        RelativeTime => "RELATIVE_TIME"
    }
}

string_enum! {
    pub enum ChecklistSystemColumnKey {
        DueRelativeDtg => "DUE_RELATIVE_DTG"
    }
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
    pub announce_class: AnnounceClass,
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
    pub hub_derived: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubDirectoryPeerRecord {
    pub identity: String,
    pub destination_hash: String,
    pub display_name: Option<String>,
    pub announce_capabilities: Vec<String>,
    pub client_type: Option<String>,
    pub registered_mode: Option<String>,
    pub last_seen: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubDirectorySnapshot {
    pub effective_connected_mode: bool,
    pub items: Vec<HubDirectoryPeerRecord>,
    pub received_at_ms: u64,
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
pub struct SosSettingsRecord {
    pub enabled: bool,
    pub message_template: String,
    #[serde(default)]
    pub cancel_message_template: String,
    pub countdown_seconds: u32,
    pub include_location: bool,
    pub trigger_shake: bool,
    pub trigger_tap_pattern: bool,
    pub trigger_power_button: bool,
    pub shake_sensitivity: f64,
    pub audio_recording: bool,
    pub audio_duration_seconds: u32,
    pub periodic_updates: bool,
    pub update_interval_seconds: u32,
    pub floating_button: bool,
    pub silent_auto_answer: bool,
    pub deactivation_pin_hash: Option<String>,
    pub deactivation_pin_salt: Option<String>,
    pub floating_button_x: f64,
    pub floating_button_y: f64,
    pub active_pill_x: f64,
    pub active_pill_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosDeviceTelemetryRecord {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f64>,
    pub speed: Option<f64>,
    pub course: Option<f64>,
    pub accuracy: Option<f64>,
    pub battery_percent: Option<f64>,
    pub battery_charging: Option<bool>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosStatusRecord {
    pub state: SosState,
    pub incident_id: Option<String>,
    pub trigger_source: Option<SosTriggerSource>,
    pub countdown_deadline_ms: Option<u64>,
    pub activated_at_ms: Option<u64>,
    pub last_sent_at_ms: Option<u64>,
    pub last_update_at_ms: Option<u64>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosAlertRecord {
    pub incident_id: String,
    pub source_hex: String,
    pub conversation_id: String,
    pub state: SosMessageKind,
    pub active: bool,
    pub body_utf8: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub battery_percent: Option<f64>,
    pub audio_id: Option<String>,
    pub message_id_hex: Option<String>,
    pub received_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosLocationRecord {
    pub incident_id: String,
    pub source_hex: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
    pub accuracy: Option<f64>,
    pub battery_percent: Option<f64>,
    pub recorded_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosAudioRecord {
    pub audio_id: String,
    pub incident_id: String,
    pub source_hex: String,
    pub path: String,
    pub mime_type: String,
    pub duration_seconds: u32,
    pub created_at_ms: u64,
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
pub struct ChecklistStatusCounts {
    pub pending_count: u32,
    pub late_count: u32,
    pub complete_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistColumnRecord {
    pub column_uid: String,
    pub column_name: String,
    pub display_order: u32,
    pub column_type: ChecklistColumnType,
    pub column_editable: bool,
    pub background_color: Option<String>,
    pub text_color: Option<String>,
    pub is_removable: bool,
    pub system_key: Option<ChecklistSystemColumnKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistCellRecord {
    pub cell_uid: String,
    pub task_uid: String,
    pub column_uid: String,
    pub value: Option<String>,
    pub updated_at: Option<String>,
    pub updated_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRecord {
    pub task_uid: String,
    pub number: u32,
    pub user_status: ChecklistUserTaskStatus,
    pub task_status: ChecklistTaskStatus,
    pub is_late: bool,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    pub custom_status: Option<i32>,
    pub due_relative_minutes: Option<u32>,
    pub due_dtg: Option<String>,
    pub notes: Option<String>,
    pub row_background_color: Option<String>,
    pub line_break_enabled: bool,
    pub completed_at: Option<String>,
    pub completed_by_team_member_rns_identity: Option<String>,
    pub legacy_value: Option<String>,
    pub cells: Vec<ChecklistCellRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistFeedPublicationRecord {
    pub publication_uid: String,
    pub checklist_uid: String,
    pub mission_feed_uid: String,
    pub published_at: Option<String>,
    pub published_by_team_member_rns_identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistRecord {
    pub uid: String,
    pub mission_uid: Option<String>,
    pub template_uid: Option<String>,
    pub template_version: Option<u32>,
    pub template_name: Option<String>,
    pub name: String,
    pub description: String,
    pub start_time: Option<String>,
    pub mode: ChecklistMode,
    pub sync_state: ChecklistSyncState,
    pub origin_type: ChecklistOriginType,
    pub checklist_status: ChecklistTaskStatus,
    pub created_at: Option<String>,
    pub created_by_team_member_rns_identity: String,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    pub uploaded_at: Option<String>,
    pub participant_rns_identities: Vec<String>,
    pub progress_percent: f64,
    pub counts: ChecklistStatusCounts,
    pub columns: Vec<ChecklistColumnRecord>,
    pub tasks: Vec<ChecklistTaskRecord>,
    pub feed_publications: Vec<ChecklistFeedPublicationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTemplateRecord {
    pub uid: String,
    pub name: String,
    pub description: String,
    pub version: u32,
    pub origin_type: ChecklistOriginType,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub source_filename: Option<String>,
    pub columns: Vec<ChecklistColumnRecord>,
    pub tasks: Vec<ChecklistTaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTemplateListRequest {
    pub search: Option<String>,
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTemplateImportCsvRequest {
    pub template_uid: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub csv_text: String,
    pub source_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistCreateFromTemplateRequest {
    pub checklist_uid: Option<String>,
    pub mission_uid: Option<String>,
    pub template_uid: String,
    pub name: String,
    pub description: String,
    pub start_time: String,
    pub created_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistListActiveRequest {
    pub search: Option<String>,
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistCreateOnlineRequest {
    pub checklist_uid: Option<String>,
    pub mission_uid: Option<String>,
    pub template_uid: String,
    pub name: String,
    pub description: String,
    pub start_time: String,
    pub created_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistUpdatePatch {
    pub mission_uid: Option<String>,
    pub template_uid: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistUpdateRequest {
    pub checklist_uid: String,
    pub patch: ChecklistUpdatePatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskStatusSetRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub user_status: ChecklistUserTaskStatus,
    pub changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRowAddRequest {
    pub checklist_uid: String,
    pub task_uid: Option<String>,
    pub number: u32,
    pub due_relative_minutes: Option<u32>,
    pub legacy_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRowDeleteRequest {
    pub checklist_uid: String,
    pub task_uid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRowStyleSetRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub row_background_color: Option<String>,
    pub line_break_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskCellSetRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub column_uid: String,
    pub value: String,
    pub updated_by_team_member_rns_identity: Option<String>,
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
    Checklists {},
    ChecklistDetail {},
    Eams {},
    Events {},
    Conversations {},
    Messages {},
    Telemetry {},
    Sos {},
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
        announce_class: AnnounceClass,
        app_data: String,
        display_name: Option<String>,
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
        snapshot: HubDirectorySnapshot,
    },
    OperationalNotice {
        notice: OperationalNotice,
    },
    ProjectionInvalidated {
        invalidation: ProjectionInvalidation,
    },
    SosStatusChanged {
        status: SosStatusRecord,
    },
    SosAlertChanged {
        alert: SosAlertRecord,
    },
    SosTelemetryRequested {},
    SosAudioRecordingRequested {
        incident_id: String,
        duration_seconds: u32,
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

#[cfg(test)]
mod tests {
    use super::{
        ChecklistColumnType, ChecklistMode, ChecklistOriginType, ChecklistSystemColumnKey,
        ChecklistTaskStatus, ChecklistUserTaskStatus, HubMode,
    };

    #[test]
    fn hub_mode_deserialize_migrates_legacy_values() {
        assert!(matches!(
            serde_json::from_str::<HubMode>("\"Disabled\"").expect("disabled mode"),
            HubMode::Autonomous {}
        ));
        assert!(matches!(
            serde_json::from_str::<HubMode>("\"RchLxmf\"").expect("rch lxmf mode"),
            HubMode::SemiAutonomous {}
        ));
        assert!(matches!(
            serde_json::from_str::<HubMode>("\"RchHttp\"").expect("rch http mode"),
            HubMode::SemiAutonomous {}
        ));
        assert!(matches!(
            serde_json::from_str::<HubMode>("\"Connected\"").expect("connected mode"),
            HubMode::Connected {}
        ));
    }

    #[test]
    fn checklist_enums_serialize_as_contract_strings() {
        assert_eq!(
            serde_json::to_string(&ChecklistMode::Online {}).expect("serialize checklist mode"),
            "\"ONLINE\""
        );
        assert_eq!(
            serde_json::to_string(&ChecklistOriginType::RchTemplate {})
                .expect("serialize origin type"),
            "\"RCH_TEMPLATE\""
        );
        assert_eq!(
            serde_json::to_string(&ChecklistTaskStatus::CompleteLate {})
                .expect("serialize task status"),
            "\"COMPLETE_LATE\""
        );
        assert_eq!(
            serde_json::to_string(&ChecklistColumnType::RelativeTime {})
                .expect("serialize column type"),
            "\"RELATIVE_TIME\""
        );
        assert_eq!(
            serde_json::to_string(&ChecklistSystemColumnKey::DueRelativeDtg {})
                .expect("serialize system key"),
            "\"DUE_RELATIVE_DTG\""
        );
    }

    #[test]
    fn checklist_enums_deserialize_from_contract_strings() {
        assert!(matches!(
            serde_json::from_str::<ChecklistMode>("\"online\"").expect("deserialize mode"),
            ChecklistMode::Online {}
        ));
        assert!(matches!(
            serde_json::from_str::<ChecklistUserTaskStatus>("\"COMPLETE\"")
                .expect("deserialize user status"),
            ChecklistUserTaskStatus::Complete {}
        ));
        assert!(matches!(
            serde_json::from_str::<ChecklistTaskStatus>("\"late\"")
                .expect("deserialize task status"),
            ChecklistTaskStatus::Late {}
        ));
    }
}
