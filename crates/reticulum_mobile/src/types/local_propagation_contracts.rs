#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalPropagationState {
    StoredLocally,
    Uploading,
    UploadedUnverified,
    RelayHolds,
    Delivered,
    Acknowledged,
    Expired,
    Evicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPropagationRecord {
    pub transient_id_hex: String,
    pub message_id_hex: String,
    pub destination_hex: String,
    pub state: LocalPropagationState,
    pub encrypted_bytes: u64,
    pub admitted_at_ms: u64,
    pub expires_at_ms: u64,
    pub may_exist_on_relay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPropagationConfig {
    pub capacity_bytes: u64,
    pub retention_seconds: u32,
    pub lora_poll_interval_seconds: u32,
    pub lora_transfer_limit_bytes: u32,
}
