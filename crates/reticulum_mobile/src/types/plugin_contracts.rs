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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilityRecord {
    #[serde(default)]
    pub events_publish: bool,
    #[serde(default)]
    pub sensors_publish: bool,
    #[serde(default)]
    pub lxmf_send: bool,
    #[serde(default)]
    pub lxmf_receive: bool,
    #[serde(default)]
    pub notifications_raise: bool,
    #[serde(default)]
    pub operational_read: bool,
}

impl PluginCapabilityRecord {
    pub fn is_subset_of(&self, declared: &Self) -> bool {
        (!self.events_publish || declared.events_publish)
            && (!self.sensors_publish || declared.sensors_publish)
            && (!self.lxmf_send || declared.lxmf_send)
            && (!self.lxmf_receive || declared.lxmf_receive)
            && (!self.notifications_raise || declared.notifications_raise)
            && (!self.operational_read || declared.operational_read)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMessageDescriptorRecord {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub send: bool,
    #[serde(default)]
    pub receive: bool,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredPluginRecord {
    pub plugin_id: String,
    pub display_name: String,
    pub version: String,
    pub api_major: u16,
    pub api_minor: u16,
    pub package_name: String,
    pub service_class_name: String,
    pub publisher_fingerprint: String,
    #[serde(default)]
    pub publisher_history: Vec<String>,
    #[serde(default)]
    pub android_permissions: Vec<String>,
    #[serde(default)]
    pub declared_capabilities: PluginCapabilityRecord,
    #[serde(default)]
    pub messages: Vec<PluginMessageDescriptorRecord>,
    pub configuration_entrypoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginRecord {
    #[serde(flatten)]
    pub discovered: DiscoveredPluginRecord,
    pub state: String,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub granted_capabilities: PluginCapabilityRecord,
    pub diagnostic: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedPluginPublisherRecord {
    pub fingerprint: String,
    pub display_name: String,
    pub approved_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSensorRecord {
    pub plugin_id: String,
    pub device_id: String,
    pub sensor_type: String,
    pub display_name: String,
    pub value: serde_json::Value,
    pub unit: Option<String>,
    pub operator_rns_identity: Option<String>,
    pub confidence: Option<f64>,
    pub connection_state: Option<String>,
    pub sample_at_ms: u64,
    pub stale_after_ms: u64,
    pub status: String,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSensorSampleRequest {
    pub device_id: String,
    pub sensor_type: String,
    pub display_name: String,
    pub value: serde_json::Value,
    pub unit: Option<String>,
    pub operator_rns_identity: Option<String>,
    pub confidence: Option<f64>,
    pub connection_state: Option<String>,
    pub timestamp_ms: u64,
    pub stale_after_ms: u64,
    #[serde(default = "default_plugin_sensor_origin")]
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEventRecord {
    pub plugin_id: String,
    pub event_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLxmfSendRequest {
    pub plugin_id: String,
    pub destination_hex: String,
    pub message_name: String,
    pub payload: serde_json::Value,
    pub body_utf8: String,
    pub title: Option<String>,
    pub send_mode: SendMode,
}

fn default_plugin_sensor_origin() -> String {
    "local".to_string()
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
    Plugins {},
    PluginSensors {},
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
