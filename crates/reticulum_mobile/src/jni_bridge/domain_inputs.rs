#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EamSourceInput {
    rns_identity: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EamProjectionInput {
    callsign: String,
    group_name: String,
    security_status: String,
    capability_status: String,
    preparedness_status: String,
    medical_status: String,
    mobility_status: String,
    comms_status: String,
    notes: Option<String>,
    updated_at: u64,
    deleted_at: Option<u64>,
    eam_uid: Option<String>,
    team_member_uid: Option<String>,
    team_uid: Option<String>,
    reported_at: Option<String>,
    reported_by: Option<String>,
    overall_status: Option<String>,
    confidence: Option<f64>,
    ttl_seconds: Option<u64>,
    source: Option<EamSourceInput>,
    sync_state: Option<String>,
    sync_error: Option<String>,
    draft_created_at: Option<u64>,
    last_synced_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventProjectionInput {
    uid: String,
    command_id: String,
    source_identity: String,
    source_display_name: Option<String>,
    timestamp: String,
    command_type: String,
    mission_uid: String,
    content: String,
    callsign: String,
    server_time: Option<String>,
    client_time: Option<String>,
    keywords: Vec<String>,
    content_hashes: Vec<String>,
    updated_at: u64,
    deleted_at: Option<u64>,
    correlation_id: Option<String>,
    topics: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageRecordInput {
    message_id_hex: String,
    conversation_id: String,
    direction: String,
    destination_hex: String,
    source_hex: Option<String>,
    requested_destination_hex: Option<String>,
    delivery_destination_hex: Option<String>,
    recipient_identity_hex: Option<String>,
    last_wire_message_id_hex: Option<String>,
    title: Option<String>,
    body_utf8: String,
    traffic_class: Option<String>,
    method: String,
    state: String,
    transport_state: Option<String>,
    application_ack_state: Option<String>,
    detail: Option<String>,
    sent_at: Option<u64>,
    received_at: Option<u64>,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryPositionInput {
    callsign: String,
    lat: f64,
    lon: f64,
    alt: Option<f64>,
    course: Option<f64>,
    speed: Option<f64>,
    accuracy: Option<f64>,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEamInput {
    callsign: String,
    deleted_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEventInput {
    uid: String,
    deleted_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistListInput {
    search: Option<String>,
    sort_by: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUidInput {
    checklist_uid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistDeleteInput {
    checklist_uid: String,
    #[serde(default)]
    delete_remote: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistCreateInput {
    checklist_uid: Option<String>,
    mission_uid: Option<String>,
    template_uid: String,
    name: String,
    description: String,
    start_time: String,
    created_by_team_member_rns_identity: Option<String>,
    created_by_team_member_display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTemplateImportInput {
    template_uid: Option<String>,
    name: String,
    description: Option<String>,
    csv_text: String,
    source_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUpdateInput {
    checklist_uid: String,
    patch: ChecklistUpdatePatchInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistUpdatePatchInput {
    mission_uid: Option<String>,
    template_uid: Option<String>,
    name: Option<String>,
    description: Option<String>,
    start_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskStatusInput {
    checklist_uid: String,
    task_uid: String,
    user_status: String,
    changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowAddInput {
    checklist_uid: String,
    task_uid: Option<String>,
    number: u32,
    due_relative_minutes: Option<u32>,
    legacy_value: Option<String>,
    changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowDeleteInput {
    checklist_uid: String,
    task_uid: String,
    changed_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskRowStyleInput {
    checklist_uid: String,
    task_uid: String,
    row_background_color: Option<String>,
    line_break_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChecklistTaskCellInput {
    checklist_uid: String,
    task_uid: String,
    column_uid: String,
    value: String,
    updated_by_team_member_rns_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPeersPayload {
    saved_peers: Vec<SavedPeerInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamUidInput {
    team_uid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallsignInput {
    callsign: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosSettingsInput {
    enabled: bool,
    message_template: String,
    #[serde(default)]
    cancel_message_template: String,
    countdown_seconds: u32,
    include_location: bool,
    trigger_shake: bool,
    trigger_tap_pattern: bool,
    trigger_power_button: bool,
    shake_sensitivity: f64,
    audio_recording: bool,
    audio_duration_seconds: u32,
    periodic_updates: bool,
    update_interval_seconds: u32,
    floating_button: bool,
    silent_auto_answer: bool,
    deactivation_pin_hash: Option<String>,
    deactivation_pin_salt: Option<String>,
    floating_button_x: f64,
    floating_button_y: f64,
    active_pill_x: f64,
    active_pill_y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosPinInput {
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosTriggerInput {
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosDeactivateInput {
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosTelemetryInput {
    lat: Option<f64>,
    lon: Option<f64>,
    alt: Option<f64>,
    speed: Option<f64>,
    course: Option<f64>,
    accuracy: Option<f64>,
    battery_percent: Option<f64>,
    battery_charging: Option<bool>,
    updated_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosAudioInput {
    audio_id: String,
    incident_id: String,
    source_hex: String,
    path: String,
    mime_type: String,
    duration_seconds: u32,
    created_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosAccelerometerInput {
    x: f64,
    y: f64,
    z: f64,
    at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SosScreenEventInput {
    at_ms: Option<u64>,
}
