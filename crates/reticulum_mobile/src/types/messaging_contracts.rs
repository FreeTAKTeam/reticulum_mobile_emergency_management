#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    pub running: bool,
    pub name: String,
    pub identity_hex: String,
    pub app_destination_hex: String,
    pub lxmf_destination_hex: String,
    pub readiness: RuntimeReadinessSnapshot,
    pub interfaces: Vec<InterfaceStatusRecord>,
}

impl NodeStatus {
    pub fn refresh_readiness(&mut self) {
        self.readiness.refresh(self.running, &self.interfaces);
    }

    pub fn set_interface_readiness(
        &mut self,
        id: &str,
        state: RuntimeReadinessState,
        detail: String,
        last_error: Option<String>,
    ) {
        self.readiness
            .set_interface_state(id, state, detail, last_error, self.running);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterfaceStatusRecord {
    pub interface_hex: String,
    pub label: String,
    pub kind: String,
    pub state: String,
    pub last_error: Option<String>,
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub last_activity_ms: u64,
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
    pub transport_state: TransportDeliveryState,
    pub application_ack_state: ApplicationAckState,
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
    #[serde(default)]
    pub requested_destination_hex: Option<String>,
    #[serde(default)]
    pub delivery_destination_hex: Option<String>,
    #[serde(default)]
    pub recipient_identity_hex: Option<String>,
    #[serde(default)]
    pub last_wire_message_id_hex: Option<String>,
    pub title: Option<String>,
    pub body_utf8: String,
    pub method: MessageMethod,
    pub state: MessageState,
    #[serde(default)]
    pub transport_state: TransportDeliveryState,
    #[serde(default)]
    pub application_ack_state: ApplicationAckState,
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
    #[serde(default = "default_true")]
    pub transport_node_enabled: bool,
    pub announce_interval_seconds: u32,
    pub telemetry: TelemetrySettingsRecord,
    pub hub: HubSettingsRecord,
    #[serde(default)]
    pub checklists: ChecklistSettingsRecord,
    #[serde(default)]
    pub rnode: RnodeSettingsRecord,
}
