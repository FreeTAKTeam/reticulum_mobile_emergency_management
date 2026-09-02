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

impl Default for HubMode {
    fn default() -> Self {
        Self::Autonomous {}
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
    Delivered {},
    Acknowledged {},
    Failed {},
    TimedOut {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportDeliveryState {
    Queued {},
    Sending {},
    SentDirect {},
    SentToPropagation {},
    TransportDelivered {},
    Failed {},
    TimedOut {},
    Cancelled {},
}

impl Default for TransportDeliveryState {
    fn default() -> Self {
        Self::Queued {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationAckState {
    NotRequired {},
    Waiting {},
    Accepted {},
    Completed {},
    Rejected {},
    Failed {},
}

impl Default for ApplicationAckState {
    fn default() -> Self {
        Self::NotRequired {}
    }
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

pub const DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES: u32 = 30;

fn default_true() -> bool {
    true
}

fn default_checklist_task_due_step_minutes() -> u32 {
    DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistSettingsRecord {
    #[serde(default = "default_checklist_task_due_step_minutes")]
    pub default_task_due_step_minutes: u32,
}

impl Default for ChecklistSettingsRecord {
    fn default() -> Self {
        Self {
            default_task_due_step_minutes: DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES,
        }
    }
}
