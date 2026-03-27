use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub title: Option<String>,
    pub body_utf8: String,
    pub method: MessageMethod,
    pub state: MessageState,
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
const REQUIRED_MISSION_CAPABILITIES: [&str; 2] = ["r3akt", "emergencymessages"];

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

impl MessagingStore {
    pub fn new(stale_after_minutes: u32) -> Self {
        Self {
            announce_records: HashMap::new(),
            resolved_app_destination_by_identity: HashMap::new(),
            resolved_app_identity_by_destination: HashMap::new(),
            resolved_lxmf_by_identity: HashMap::new(),
            saved_destinations: HashSet::new(),
            active_link_destinations: HashSet::new(),
            last_resolution_errors: HashMap::new(),
            last_resolution_attempt_at_ms: HashMap::new(),
            message_records: HashMap::new(),
            message_order: Vec::new(),
            outbound_messages: HashMap::new(),
            sync_status: SyncStatus::default(),
            peer_stale_after_ms: u64::from(stale_after_minutes.max(1)) * 60_000,
        }
    }

    fn update_app_destination_mapping(&mut self, destination_hex: String, identity_hex: String) {
        if let Some(previous_destination_hex) = self
            .resolved_app_destination_by_identity
            .insert(identity_hex.clone(), destination_hex.clone())
        {
            if previous_destination_hex != destination_hex {
                self.resolved_app_identity_by_destination
                    .remove(previous_destination_hex.as_str());
            }
        }
        self.resolved_app_identity_by_destination
            .insert(destination_hex, identity_hex);
    }

    fn replace_announce_for_identity(
        &mut self,
        destination_kind: &str,
        identity_hex: &str,
        destination_hex: &str,
    ) {
        let identity_hex = normalize_hex(identity_hex);
        if identity_hex.is_empty() {
            return;
        }

        let destination_hex = normalize_hex(destination_hex);
        let replaced_destinations = self
            .announce_records
            .iter()
            .filter_map(|(candidate_destination_hex, record)| {
                (record.destination_kind == destination_kind
                    && normalize_hex(record.identity_hex.as_str()) == identity_hex
                    && candidate_destination_hex != &destination_hex)
                    .then(|| candidate_destination_hex.clone())
            })
            .collect::<Vec<_>>();

        for replaced_destination_hex in replaced_destinations {
            self.announce_records
                .remove(replaced_destination_hex.as_str());
        }
    }

    pub fn conversation_id_for(destination_hex: &str) -> String {
        destination_hex.trim().to_ascii_lowercase()
    }

    pub fn record_announce(&mut self, mut record: AnnounceRecord) {
        let destination_hex = normalize_hex(record.destination_hex.as_str());
        let identity_hex = normalize_hex(record.identity_hex.as_str());
        let destination_kind = record.destination_kind.clone();
        let received_at_ms = record.received_at_ms;
        if let Some(existing) = self.announce_records.get(destination_hex.as_str()) {
            if existing.destination_kind == record.destination_kind
                && normalize_hex(existing.identity_hex.as_str()) == identity_hex
            {
                if record.app_data.trim().is_empty() && !existing.app_data.trim().is_empty() {
                    record.app_data = existing.app_data.clone();
                }
                if record.display_name.is_none() {
                    record.display_name = existing.display_name.clone();
                }
            }
        }
        if !destination_hex.is_empty() && !identity_hex.is_empty() {
            if record.destination_kind == "app" {
                self.update_app_destination_mapping(destination_hex.clone(), identity_hex.clone());
                self.replace_announce_for_identity(
                    "app",
                    identity_hex.as_str(),
                    destination_hex.as_str(),
                );
            } else if record.destination_kind == "lxmf_delivery" {
                if let Some(previous_destination_hex) = self
                    .resolved_lxmf_by_identity
                    .insert(identity_hex.clone(), destination_hex.clone())
                {
                    if previous_destination_hex != destination_hex {
                        self.announce_records
                            .remove(previous_destination_hex.as_str());
                    }
                }
                self.replace_announce_for_identity(
                    "lxmf_delivery",
                    identity_hex.as_str(),
                    destination_hex.as_str(),
                );
            }
        }
        self.announce_records.insert(destination_hex, record);

        let _ = destination_kind;
        let _ = received_at_ms;
    }

    pub fn list_announces(&self) -> Vec<AnnounceRecord> {
        let mut records = self.announce_records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| right.received_at_ms.cmp(&left.received_at_ms));
        records
    }

    pub fn mark_peer_saved(&mut self, destination_hex: &str, saved: bool) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if saved {
            self.saved_destinations.insert(normalized.clone());
            self.last_resolution_errors.remove(&normalized);
        } else {
            self.saved_destinations.remove(&normalized);
            self.active_link_destinations.remove(&normalized);
            self.last_resolution_errors.remove(&normalized);
            self.last_resolution_attempt_at_ms.remove(&normalized);
        }
    }

    pub fn is_peer_saved(&self, destination_hex: &str) -> bool {
        let normalized = normalize_hex(destination_hex);
        !normalized.is_empty() && self.saved_destinations.contains(normalized.as_str())
    }

    pub fn record_resolution_attempt(&mut self, destination_hex: &str, attempted_at_ms: u64) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        self.last_resolution_attempt_at_ms
            .insert(normalized, attempted_at_ms);
    }

    pub fn record_resolution_result(
        &mut self,
        destination_hex: &str,
        identity_hex: &str,
        lxmf_destination_hex: &str,
        _resolved_at_ms: u64,
    ) {
        let normalized_destination = normalize_hex(destination_hex);
        let normalized_identity = normalize_hex(identity_hex);
        let normalized_lxmf_destination = normalize_hex(lxmf_destination_hex);
        if normalized_destination.is_empty() || normalized_identity.is_empty() {
            return;
        }
        self.update_app_destination_mapping(
            normalized_destination.clone(),
            normalized_identity.clone(),
        );
        if !normalized_lxmf_destination.is_empty() {
            self.resolved_lxmf_by_identity
                .insert(normalized_identity, normalized_lxmf_destination);
        }
        self.last_resolution_errors.remove(&normalized_destination);
    }

    pub fn record_resolution_error(&mut self, destination_hex: &str, error: Option<String>) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if let Some(error) = error.filter(|value| !value.trim().is_empty()) {
            self.last_resolution_errors.insert(normalized, error);
        } else {
            self.last_resolution_errors.remove(&normalized);
        }
    }

    pub fn set_peer_active_link(
        &mut self,
        destination_hex: &str,
        active: bool,
        changed_at_ms: u64,
    ) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if active {
            self.active_link_destinations.insert(normalized);
        } else {
            self.active_link_destinations.remove(&normalized);
        }

        let _ = changed_at_ms;
    }

    pub fn list_peers(&self) -> Vec<PeerRecord> {
        let now_ms = current_time_ms();
        let mut app_dest_by_identity = HashMap::<String, String>::new();
        let mut lxmf_dest_by_identity = HashMap::<String, String>::new();
        let mut app_records = HashMap::<String, AnnounceRecord>::new();
        let mut lxmf_records = HashMap::<String, AnnounceRecord>::new();

        for record in self.announce_records.values() {
            if record.destination_kind == "app" {
                app_dest_by_identity
                    .insert(record.identity_hex.clone(), record.destination_hex.clone());
                app_records.insert(record.destination_hex.clone(), record.clone());
            } else if record.destination_kind == "lxmf_delivery" {
                lxmf_dest_by_identity
                    .insert(record.identity_hex.clone(), record.destination_hex.clone());
                lxmf_records.insert(record.destination_hex.clone(), record.clone());
            }
        }

        for (identity_hex, destination_hex) in &self.resolved_app_destination_by_identity {
            app_dest_by_identity
                .entry(identity_hex.clone())
                .or_insert_with(|| destination_hex.clone());
        }

        for (identity_hex, lxmf_destination_hex) in &self.resolved_lxmf_by_identity {
            lxmf_dest_by_identity
                .entry(identity_hex.clone())
                .or_insert_with(|| lxmf_destination_hex.clone());
        }

        let mut candidate_destinations = HashSet::<String>::new();
        candidate_destinations.extend(app_records.keys().cloned());
        candidate_destinations.extend(self.saved_destinations.iter().cloned());
        candidate_destinations.extend(self.resolved_app_identity_by_destination.keys().cloned());

        let mut peers = Vec::<PeerRecord>::new();
        for destination_hex in candidate_destinations {
            let app_record = app_records.get(&destination_hex);
            let identity_hex = app_record
                .map(|record| record.identity_hex.clone())
                .or_else(|| {
                    self.resolved_app_identity_by_destination
                        .get(&destination_hex)
                        .cloned()
                });
            let lxmf_destination_hex = identity_hex
                .as_ref()
                .and_then(|identity| lxmf_dest_by_identity.get(identity).cloned());
            let lxmf_record = lxmf_destination_hex
                .as_ref()
                .and_then(|value| lxmf_records.get(value));
            let saved = self.saved_destinations.contains(destination_hex.as_str());
            let active_link = self
                .active_link_destinations
                .contains(destination_hex.as_str())
                || lxmf_destination_hex
                    .as_ref()
                    .is_some_and(|value| self.active_link_destinations.contains(value.as_str()));
            let peer_app_data = app_record.map(|record| record.app_data.as_str());
            let mission_capable = app_record.is_some() && supports_mission_traffic(peer_app_data);
            if !saved && !mission_capable {
                continue;
            }
            let latest_seen_at_ms = app_record
                .map(|record| record.received_at_ms)
                .unwrap_or(0)
                .max(lxmf_record.map(|record| record.received_at_ms).unwrap_or(0));
            let stale = peer_is_stale(
                saved,
                active_link,
                app_record.map(|record| record.received_at_ms),
                lxmf_record.map(|record| record.received_at_ms),
                now_ms,
                self.peer_stale_after_ms,
            );
            let availability_state = peer_availability_state(
                app_record.is_some(),
                identity_hex.as_ref(),
                lxmf_destination_hex.as_ref(),
                stale,
            );
            peers.push(PeerRecord {
                destination_hex: destination_hex.clone(),
                identity_hex,
                lxmf_destination_hex: lxmf_destination_hex.clone(),
                display_name: lxmf_record.and_then(|record| record.display_name.clone()),
                app_data: peer_app_data.map(ToOwned::to_owned),
                state: compatibility_peer_state(saved, availability_state, active_link),
                saved,
                stale,
                active_link,
                last_resolution_error: self.last_resolution_errors.get(&destination_hex).cloned(),
                last_resolution_attempt_at_ms: self
                    .last_resolution_attempt_at_ms
                    .get(&destination_hex)
                    .copied(),
                last_seen_at_ms: latest_seen_at_ms,
                announce_last_seen_at_ms: app_record.map(|record| record.received_at_ms),
                lxmf_last_seen_at_ms: lxmf_record.map(|record| record.received_at_ms),
            });
        }

        peers.sort_by(|left, right| right.last_seen_at_ms.cmp(&left.last_seen_at_ms));
        peers
    }

    pub fn peer_for_identity(&self, identity_hex: &str) -> Option<PeerRecord> {
        self.list_peers()
            .into_iter()
            .find(|peer| peer.identity_hex.as_deref() == Some(identity_hex))
    }

    pub fn app_destination_for_identity(&self, identity_hex: &str) -> Option<String> {
        let normalized = normalize_hex(identity_hex);
        if normalized.is_empty() {
            return None;
        }

        self.resolved_app_destination_by_identity
            .get(normalized.as_str())
            .cloned()
            .or_else(|| {
                self.resolved_app_identity_by_destination.iter().find_map(
                    |(destination_hex, resolved_identity_hex)| {
                        if normalize_hex(resolved_identity_hex.as_str()) == normalized {
                            Some(destination_hex.clone())
                        } else {
                            None
                        }
                    },
                )
            })
    }

    pub fn peer_by_destination(&self, destination_hex: &str) -> Option<PeerRecord> {
        let normalized = normalize_hex(destination_hex);
        self.list_peers()
            .into_iter()
            .find(|peer| peer.destination_hex == normalized)
    }

    pub fn peer_change_for_destination(&self, destination_hex: &str) -> Option<PeerChange> {
        self.peer_by_destination(destination_hex)
            .map(peer_change_from_record)
    }

    pub fn upsert_message(&mut self, message: MessageRecord) -> bool {
        let is_new = !self
            .message_records
            .contains_key(message.message_id_hex.as_str());
        self.message_records
            .insert(message.message_id_hex.clone(), message.clone());
        if is_new {
            self.message_order.push(message.message_id_hex);
        }
        is_new
    }

    pub fn update_message(
        &mut self,
        message_id_hex: &str,
        state: MessageState,
        detail: Option<String>,
        updated_at_ms: u64,
    ) -> Option<MessageRecord> {
        let record = self.message_records.get_mut(message_id_hex)?;
        record.state = state;
        record.detail = detail;
        record.updated_at_ms = updated_at_ms;
        Some(record.clone())
    }

    pub fn list_messages(&self, conversation_id: Option<&str>) -> Vec<MessageRecord> {
        let mut out = Vec::<MessageRecord>::new();
        for message_id_hex in &self.message_order {
            let Some(record) = self.message_records.get(message_id_hex).cloned() else {
                continue;
            };
            if conversation_id.is_some_and(|value| record.conversation_id != value) {
                continue;
            }
            out.push(record);
        }
        out.sort_by(|left, right| {
            let left_time = left
                .received_at_ms
                .or(left.sent_at_ms)
                .unwrap_or(left.updated_at_ms);
            let right_time = right
                .received_at_ms
                .or(right.sent_at_ms)
                .unwrap_or(right.updated_at_ms);
            left_time.cmp(&right_time)
        });
        out
    }

    pub fn list_conversations(&self) -> Vec<ConversationRecord> {
        let peers = self.list_peers();
        let mut peer_map = HashMap::<String, PeerRecord>::new();
        for peer in peers {
            peer_map.insert(peer.destination_hex.clone(), peer.clone());
            if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.clone() {
                peer_map.insert(lxmf_destination_hex, peer);
            }
        }

        let records = self.list_messages(None);
        let mut by_conversation = HashMap::<String, ConversationRecord>::new();
        for record in records {
            let entry = by_conversation
                .entry(record.conversation_id.clone())
                .or_insert_with(|| ConversationRecord {
                    conversation_id: record.conversation_id.clone(),
                    peer_destination_hex: record.destination_hex.clone(),
                    peer_display_name: peer_map
                        .get(&record.destination_hex)
                        .and_then(peer_display_name_for),
                    last_message_preview: None,
                    last_message_at_ms: 0,
                    unread_count: 0,
                    last_message_state: None,
                });

            let event_time = record
                .received_at_ms
                .or(record.sent_at_ms)
                .unwrap_or(record.updated_at_ms);
            if event_time >= entry.last_message_at_ms {
                entry.peer_destination_hex = record.destination_hex.clone();
                entry.peer_display_name = peer_map
                    .get(&record.destination_hex)
                    .and_then(peer_display_name_for);
                entry.last_message_preview = message_preview(record.body_utf8.as_str());
                entry.last_message_at_ms = event_time;
                entry.last_message_state = Some(record.state);
            }
            if matches!(record.direction, MessageDirection::Inbound) {
                entry.unread_count = entry.unread_count.saturating_add(1);
            }
        }

        let mut out = by_conversation.into_values().collect::<Vec<_>>();
        out.sort_by(|left, right| right.last_message_at_ms.cmp(&left.last_message_at_ms));
        out
    }

    pub fn store_outbound(&mut self, outbound: StoredOutboundMessage) {
        self.outbound_messages
            .insert(outbound.message_id_hex.clone(), outbound);
    }

    pub fn outbound(&self, message_id_hex: &str) -> Option<StoredOutboundMessage> {
        self.outbound_messages.get(message_id_hex).cloned()
    }

    pub fn set_active_propagation_node(&mut self, destination_hex: Option<String>) -> SyncStatus {
        self.sync_status.active_propagation_node_hex =
            destination_hex.map(|value| normalize_hex(value.as_str()));
        self.sync_status.clone()
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.sync_status.clone()
    }

    pub fn update_sync_status<F>(&mut self, apply: F) -> SyncStatus
    where
        F: FnOnce(&mut SyncStatus),
    {
        apply(&mut self.sync_status);
        self.sync_status.clone()
    }
}

fn message_preview(body_utf8: &str) -> Option<String> {
    let trimmed = body_utf8.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(80).collect())
}

fn peer_display_name_for(peer: &PeerRecord) -> Option<String> {
    peer.display_name
        .clone()
        .or_else(|| peer.identity_hex.clone())
        .or_else(|| Some(peer.destination_hex.clone()))
}

fn normalize_hex(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_capability_tokens(app_data: &str) -> Vec<String> {
    app_data
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .filter(|token| !token.to_ascii_lowercase().starts_with("name="))
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn supports_mission_traffic(app_data: Option<&str>) -> bool {
    let tokens = app_data.map(parse_capability_tokens).unwrap_or_default();
    REQUIRED_MISSION_CAPABILITIES
        .iter()
        .all(|required| tokens.iter().any(|token| token == required))
}

fn peer_is_stale(
    saved: bool,
    active_link: bool,
    announce_last_seen_at_ms: Option<u64>,
    lxmf_last_seen_at_ms: Option<u64>,
    now_ms: u64,
    stale_after_ms: u64,
) -> bool {
    if !saved || active_link {
        return false;
    }

    let latest_known_activity = [
        announce_last_seen_at_ms.unwrap_or(0),
        lxmf_last_seen_at_ms.unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);

    latest_known_activity > 0 && now_ms.saturating_sub(latest_known_activity) > stale_after_ms
}

fn compatibility_peer_state(
    saved: bool,
    availability_state: PeerAvailabilityState,
    active_link: bool,
) -> PeerState {
    if active_link {
        return PeerState::Connected;
    }

    match availability_state {
        PeerAvailabilityState::Ready if saved => PeerState::Connected,
        _ if saved => PeerState::Connecting,
        _ => PeerState::Disconnected,
    }
}

fn peer_availability_state(
    has_app_announce: bool,
    identity_hex: Option<&String>,
    lxmf_destination_hex: Option<&String>,
    stale: bool,
) -> PeerAvailabilityState {
    if identity_hex.is_some() && lxmf_destination_hex.is_some() {
        if !has_app_announce {
            return PeerAvailabilityState::Resolved;
        }
        return if stale {
            PeerAvailabilityState::Resolved
        } else {
            PeerAvailabilityState::Ready
        };
    }
    if has_app_announce || identity_hex.is_some() {
        return PeerAvailabilityState::Discovered;
    }
    PeerAvailabilityState::Unseen
}

fn peer_change_from_record(record: PeerRecord) -> PeerChange {
    PeerChange {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: record.state,
        saved: record.saved,
        stale: record.stale,
        active_link: record.active_link,
        last_error: record.last_resolution_error.clone(),
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_projection_merges_app_and_lxmf_announces() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: None,
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Alice".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        store.mark_peer_saved("appdest", true);

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].destination_hex, "appdest");
        assert_eq!(peers[0].lxmf_destination_hex.as_deref(), Some("lxmfdest"));
        assert_eq!(peers[0].display_name.as_deref(), Some("Alice"));
        assert_eq!(peers[0].state, PeerState::Connected);
        assert!(peers[0].saved);
        assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(10));
        assert!(!peers[0].stale);
    }

    #[test]
    fn conversation_projection_uses_lxmf_destination_for_peer_lookup() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: None,
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Alice".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });
        store.upsert_message(MessageRecord {
            message_id_hex: "msg".into(),
            conversation_id: "lxmfdest".into(),
            direction: MessageDirection::Outbound,
            destination_hex: "lxmfdest".into(),
            source_hex: None,
            title: None,
            body_utf8: "hello".into(),
            method: MessageMethod::Direct,
            state: MessageState::Delivered,
            detail: None,
            sent_at_ms: Some(30),
            received_at_ms: None,
            updated_at_ms: now,
        });

        let conversations = store.list_conversations();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].peer_display_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn saved_peer_without_resolution_stays_connecting() {
        let mut store = MessagingStore::default();
        store.mark_peer_saved("appdest", true);

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].destination_hex, "appdest");
        assert!(peers[0].saved);
        assert_eq!(peers[0].state, PeerState::Connecting);
        assert_eq!(peers[0].last_seen_at_ms, 0);
        assert!(!peers[0].stale);
    }

    #[test]
    fn capability_relevant_unsaved_peer_appears_in_possible_peers() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].destination_hex, "appdest");
        assert!(!peers[0].saved);
        assert_eq!(peers[0].state, PeerState::Disconnected);
        assert!(!peers[0].stale);
    }

    #[test]
    fn capability_irrelevant_peer_is_excluded_from_possible_peers() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "chat".into(),
            display_name: Some("Ignored".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Chat Only".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        assert!(store.list_peers().is_empty());
    }

    #[test]
    fn latest_app_announce_replaces_stale_destination_for_identity() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest-old".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: Some("Old".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(30),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest-new".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: Some("New".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("New".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        assert_eq!(
            store.app_destination_for_identity("identity").as_deref(),
            Some("appdest-new")
        );

        let peer_destinations = store
            .list_peers()
            .into_iter()
            .map(|peer| peer.destination_hex)
            .collect::<Vec<_>>();
        assert_eq!(peer_destinations, vec!["appdest-new".to_string()]);
    }

    #[test]
    fn empty_app_announce_does_not_erase_mission_capabilities() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages,Telemetry".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(30),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(20),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: String::new(),
            display_name: None,
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(
            peers[0].app_data.as_deref(),
            Some("R3AKT,EMergencyMessages,Telemetry")
        );
        assert_eq!(peers[0].display_name.as_deref(), Some("Poco"));
        assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(10));
    }

    #[test]
    fn capability_relevant_saved_peer_becomes_stale_after_timeout() {
        let mut store = MessagingStore::new(1);
        let stale = current_time_ms().saturating_sub(70_000);
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: stale,
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: stale,
        });
        store.mark_peer_saved("appdest", true);

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert!(peers[0].saved);
        assert!(peers[0].stale);
    }

    #[test]
    fn lxmf_only_announce_does_not_create_peer_record() {
        let mut store = MessagingStore::default();
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: current_time_ms(),
        });

        assert!(store.list_peers().is_empty());
    }

    #[test]
    fn last_seen_comes_only_from_announce_and_lxmf_timestamps() {
        let mut store = MessagingStore::default();
        let now = current_time_ms();
        store.record_announce(AnnounceRecord {
            destination_hex: "appdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "app".into(),
            app_data: "R3AKT,EMergencyMessages".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(40),
        });
        store.record_announce(AnnounceRecord {
            destination_hex: "lxmfdest".into(),
            identity_hex: "identity".into(),
            destination_kind: "lxmf_delivery".into(),
            app_data: "chat".into(),
            display_name: Some("Poco".into()),
            hops: 1,
            interface_hex: "iface".into(),
            received_at_ms: now.saturating_sub(10),
        });

        let peers = store.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(10));
    }
}
