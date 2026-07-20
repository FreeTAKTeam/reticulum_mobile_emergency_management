use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::announce_metadata::supports_mission_traffic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerAvailabilityState {
    Unseen,
    Discovered,
    Resolved,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageMethod {
    Direct,
    Opportunistic,
    Propagated,
    Resource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageState {
    Queued,
    PathRequested,
    LinkEstablishing,
    Sending,
    SentDirect,
    SentToPropagation,
    Delivered,
    Failed,
    TimedOut,
    Cancelled,
    Received,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportDeliveryState {
    Queued,
    Sending,
    SentDirect,
    SentToPropagation,
    TransportDelivered,
    Failed,
    TimedOut,
    Cancelled,
}

impl Default for TransportDeliveryState {
    fn default() -> Self {
        Self::Queued
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationAckState {
    NotRequired,
    Waiting,
    Accepted,
    Completed,
    Rejected,
    Failed,
}

impl Default for ApplicationAckState {
    fn default() -> Self {
        Self::NotRequired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Idle,
    PathRequested,
    LinkEstablishing,
    RequestSent,
    Receiving,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SendMode {
    #[default]
    Auto,
    DirectOnly,
    PropagationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnounceRecord {
    pub destination_hex: String,
    pub identity_hex: String,
    pub destination_kind: String,
    pub app_data: String,
    pub display_name: Option<String>,
    pub hops: u8,
    pub interface_hex: String,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub last_resolution_error: Option<String>,
    pub last_resolution_attempt_at_ms: Option<u64>,
    pub last_seen_at_ms: u64,
    pub announce_last_seen_at_ms: Option<u64>,
    pub lxmf_last_seen_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SavedPeerProfile {
    destination_hex: String,
    identity_hex: Option<String>,
    lxmf_destination_hex: Option<String>,
    app_data: Option<String>,
    display_name: Option<String>,
    last_route_seen_at_ms: Option<u64>,
}

pub(crate) struct SavedPeerProfileInput<'a> {
    pub destination_hex: &'a str,
    pub identity_hex: Option<&'a str>,
    pub lxmf_destination_hex: Option<&'a str>,
    pub app_data: Option<&'a str>,
    pub display_name: Option<&'a str>,
    pub last_route_seen_at_ms: Option<u64>,
}

pub(crate) struct MessageDeliveryUpdate<'a> {
    pub message_id_hex: &'a str,
    pub state: Option<MessageState>,
    pub transport_state: Option<TransportDeliveryState>,
    pub application_ack_state: Option<ApplicationAckState>,
    pub detail: Option<String>,
    pub last_wire_message_id_hex: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub conversation_id: String,
    pub peer_destination_hex: String,
    pub peer_display_name: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_at_ms: u64,
    pub unread_count: u32,
    pub last_message_state: Option<MessageState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub phase: SyncPhase,
    pub active_propagation_node_hex: Option<String>,
    pub requested_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub messages_received: u32,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub destination_hex: String,
    pub body_utf8: String,
    pub title: Option<String>,
    #[serde(default)]
    pub send_mode: SendMode,
    #[serde(default)]
    pub use_propagation_node: bool,
}

impl SendMessageRequest {
    pub fn effective_send_mode(&self) -> SendMode {
        if self.use_propagation_node {
            SendMode::PropagationOnly
        } else {
            self.send_mode
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOutboundMessage {
    pub request: SendMessageRequest,
    pub message_id_hex: String,
}

#[derive(Debug, Clone)]
pub struct MessagingStore {
    announce_records: HashMap<String, AnnounceRecord>,
    resolved_app_destination_by_identity: HashMap<String, String>,
    resolved_app_identity_by_destination: HashMap<String, String>,
    resolved_lxmf_by_identity: HashMap<String, String>,
    saved_destinations: HashSet<String>,
    saved_peer_profiles: HashMap<String, SavedPeerProfile>,
    active_link_destinations: HashSet<String>,
    last_resolution_errors: HashMap<String, String>,
    last_resolution_attempt_at_ms: HashMap<String, u64>,
    message_records: HashMap<String, MessageRecord>,
    message_order: Vec<String>,
    outbound_messages: HashMap<String, StoredOutboundMessage>,
    sync_status: SyncStatus,
    peer_stale_after_ms: u64,
}

const DEFAULT_PEER_STALE_AFTER_MINUTES: u32 = 30;
pub(crate) const DEFAULT_PEER_STALE_AFTER_MS: u64 =
    DEFAULT_PEER_STALE_AFTER_MINUTES as u64 * 60_000;

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            phase: SyncPhase::Idle,
            active_propagation_node_hex: None,
            requested_at_ms: None,
            completed_at_ms: None,
            messages_received: 0,
            detail: None,
        }
    }
}

impl Default for MessagingStore {
    fn default() -> Self {
        Self::new(DEFAULT_PEER_STALE_AFTER_MINUTES)
    }
}

include!("messaging_compat/announces.rs");
include!("messaging_compat/saved_peers.rs");
include!("messaging_compat/peer_projection.rs");
include!("messaging_compat/messages.rs");
include!("messaging_compat/sync.rs");
include!("messaging_compat/helpers.rs");

#[cfg(test)]
mod tests {
    include!("messaging_compat/tests/support.rs");
    include!("messaging_compat/tests/messages.rs");
    include!("messaging_compat/tests/capabilities.rs");
    include!("messaging_compat/tests/announces.rs");
    include!("messaging_compat/tests/saved_peers.rs");
    include!("messaging_compat/tests/peers.rs");
}
