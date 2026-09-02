#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPeerRecord {
    pub destination_hex: String,
    pub label: Option<String>,
    pub saved_at_ms: u64,
    #[serde(default)]
    pub identity_hex: Option<String>,
    #[serde(default)]
    pub lxmf_destination_hex: Option<String>,
    #[serde(default)]
    pub app_data: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub last_route_seen_at_ms: Option<u64>,
    #[serde(default)]
    pub last_hops: Option<u8>,
    #[serde(default)]
    pub circle_tier: CircleTier,
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
pub struct EamReadinessStatusMetricRecord {
    pub field: String,
    pub label: String,
    pub score: u32,
    pub band: String,
    pub ring_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamReadinessMessageRecord {
    pub callsign: String,
    pub overall_score: u32,
    pub overall_band: String,
    pub overall_ring_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamReadinessSummaryRecord {
    pub active_total: u32,
    pub updated_at_ms: u64,
    pub status_metrics: Vec<EamReadinessStatusMetricRecord>,
    pub messages: Vec<EamReadinessMessageRecord>,
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
    #[serde(default)]
    pub created_by_team_member_display_name: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_changed_by_team_member_rns_identity: Option<String>,
    pub deleted_at: Option<String>,
    pub uploaded_at: Option<String>,
    pub participant_rns_identities: Vec<String>,
    #[serde(default)]
    pub expected_task_count: Option<u32>,
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
    pub created_by_team_member_display_name: Option<String>,
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
    pub created_by_team_member_display_name: Option<String>,
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
    pub changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistDeleteRequest {
    pub checklist_uid: String,
    pub delete_remote: bool,
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
    pub changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRowDeleteRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskRowStyleSetRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub row_background_color: Option<String>,
    pub line_break_enabled: Option<bool>,
    pub changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTaskCellSetRequest {
    pub checklist_uid: String,
    pub task_uid: String,
    pub column_uid: String,
    pub value: String,
    pub updated_by_team_member_rns_identity: Option<String>,
}
