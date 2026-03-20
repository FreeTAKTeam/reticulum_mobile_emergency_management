use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel as cb;
use fs_err as fs;
#[cfg(feature = "legacy-lxmf-runtime")]
use log::error;
use log::{debug, info};
use lxmf::announce::{
    display_name_from_delivery_app_data, encode_delivery_display_name_app_data,
};
use lxmf::message::Message as LxmfMessage;
use lxmf::message::WireMessage as LxmfWireMessage;
use lxmf_sdk::messaging as sdkmsg;
use rand_core::OsRng;
use regex::Regex;
use reticulum::destination::link::{LinkEvent, LinkStatus};
use reticulum::destination::{DestinationDesc, DestinationName, SingleOutputDestination};
use reticulum::hash::AddressHash;
use reticulum::identity::PrivateIdentity;
use reticulum::iface::tcp_client::TcpClient;
#[cfg(feature = "legacy-lxmf-runtime")]
use reticulum::packet::LXMF_MAX_PAYLOAD;
use reticulum::packet::{Packet, PacketDataBuffer, PropagationType};
use reticulum::resource::ResourceEventKind;
use reticulum::transport::{
    DeliveryReceipt, ReceiptHandler, SendPacketOutcome as RnsSendOutcome, Transport,
    TransportConfig,
};
use rmpv::Value as MsgPackValue;
use tokio::sync::{mpsc, Mutex as TokioMutex};

use crate::event_bus::EventBus;
use crate::sdk_bridge::{RuntimeLxmfSdk, SdkTransportState};
use crate::types::{
    AnnounceRecord, ConversationRecord, HubMode, LxmfDeliveryStatus, LxmfDeliveryUpdate,
    MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError,
    NodeEvent, NodeStatus, PeerChange, PeerRecord, PeerState, SendLxmfRequest, SendOutcome,
    SyncPhase, SyncStatus,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const LXMF_FIELD_COMMANDS: i64 = 0x09;
const LXMF_FIELD_RESULTS: i64 = 0x0A;
const LXMF_FIELD_EVENT: i64 = 0x0D;

const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);
const DEFAULT_LXMF_ACK_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn lxmf_private_identity(
    identity: &PrivateIdentity,
) -> Result<lxmf::identity::PrivateIdentity, NodeError> {
    lxmf::identity::PrivateIdentity::from_private_key_bytes(&identity.to_private_key_bytes())
        .map_err(|_| NodeError::InternalError {})
}

fn normalize_hex_32(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.len() != 32 {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn parse_address_hash(hex_32: &str) -> Result<AddressHash, NodeError> {
    let normalized = normalize_hex_32(hex_32).ok_or(NodeError::InvalidConfig {})?;
    AddressHash::new_from_hex_string(&normalized).map_err(|_| NodeError::InvalidConfig {})
}

fn address_hash_to_hex(hash: &AddressHash) -> String {
    hash.to_hex_string()
}

fn announce_destination_kind(desc: &DestinationDesc) -> &'static str {
    let app_name = DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1);
    let lxmf_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);

    let app_hash = SingleOutputDestination::new(desc.identity.clone(), app_name.clone())
        .desc
        .address_hash;
    if desc.address_hash == app_hash || desc.name.hash == app_name.hash {
        "app"
    } else {
        let lxmf_hash = SingleOutputDestination::new(desc.identity.clone(), lxmf_name.clone())
            .desc
            .address_hash;
        if desc.address_hash == lxmf_hash || desc.name.hash == lxmf_name.hash {
            "lxmf_delivery"
        } else {
            "other"
        }
    }
}

fn join_url(base: &str, path: &str) -> Result<String, NodeError> {
    let base = base.trim();
    if base.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    Ok(format!("{base}/{path}"))
}

fn extract_hex_destinations(text: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[^0-9a-f])([0-9a-f]{32})(?:$|[^0-9a-f])").expect("regex")
    });

    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for caps in re.captures_iter(text) {
        let Some(m) = caps.get(1) else {
            continue;
        };
        let value = m.as_str().to_ascii_lowercase();
        if seen.insert(value.clone()) {
            out.push(value);
        }
    }
    out
}

fn send_outcome_to_udl(outcome: RnsSendOutcome) -> SendOutcome {
    match outcome {
        RnsSendOutcome::SentDirect => SendOutcome::SentDirect {},
        RnsSendOutcome::SentBroadcast => SendOutcome::SentBroadcast {},
        RnsSendOutcome::DroppedMissingDestinationIdentity => {
            SendOutcome::DroppedMissingDestinationIdentity {}
        }
        RnsSendOutcome::DroppedCiphertextTooLarge => SendOutcome::DroppedCiphertextTooLarge {},
        RnsSendOutcome::DroppedEncryptFailed => SendOutcome::DroppedEncryptFailed {},
        RnsSendOutcome::DroppedNoRoute => SendOutcome::DroppedNoRoute {},
    }
}

fn from_sdk_peer_state(state: sdkmsg::PeerState) -> PeerState {
    match state {
        sdkmsg::PeerState::Connecting => PeerState::Connecting {},
        sdkmsg::PeerState::Connected => PeerState::Connected {},
        sdkmsg::PeerState::Disconnected => PeerState::Disconnected {},
    }
}

fn to_sdk_message_method(method: MessageMethod) -> sdkmsg::MessageMethod {
    match method {
        MessageMethod::Direct {} => sdkmsg::MessageMethod::Direct,
        MessageMethod::Opportunistic {} => sdkmsg::MessageMethod::Opportunistic,
        MessageMethod::Propagated {} => sdkmsg::MessageMethod::Propagated,
        MessageMethod::Resource {} => sdkmsg::MessageMethod::Resource,
    }
}

fn from_sdk_message_method(method: sdkmsg::MessageMethod) -> MessageMethod {
    match method {
        sdkmsg::MessageMethod::Direct => MessageMethod::Direct {},
        sdkmsg::MessageMethod::Opportunistic => MessageMethod::Opportunistic {},
        sdkmsg::MessageMethod::Propagated => MessageMethod::Propagated {},
        sdkmsg::MessageMethod::Resource => MessageMethod::Resource {},
    }
}

fn to_sdk_message_state(state: MessageState) -> sdkmsg::MessageState {
    match state {
        MessageState::Queued {} => sdkmsg::MessageState::Queued,
        MessageState::PathRequested {} => sdkmsg::MessageState::PathRequested,
        MessageState::LinkEstablishing {} => sdkmsg::MessageState::LinkEstablishing,
        MessageState::Sending {} => sdkmsg::MessageState::Sending,
        MessageState::SentDirect {} => sdkmsg::MessageState::SentDirect,
        MessageState::SentToPropagation {} => sdkmsg::MessageState::SentToPropagation,
        MessageState::Delivered {} => sdkmsg::MessageState::Delivered,
        MessageState::Failed {} => sdkmsg::MessageState::Failed,
        MessageState::TimedOut {} => sdkmsg::MessageState::TimedOut,
        MessageState::Cancelled {} => sdkmsg::MessageState::Cancelled,
        MessageState::Received {} => sdkmsg::MessageState::Received,
    }
}

fn from_sdk_message_state(state: sdkmsg::MessageState) -> MessageState {
    match state {
        sdkmsg::MessageState::Queued => MessageState::Queued {},
        sdkmsg::MessageState::PathRequested => MessageState::PathRequested {},
        sdkmsg::MessageState::LinkEstablishing => MessageState::LinkEstablishing {},
        sdkmsg::MessageState::Sending => MessageState::Sending {},
        sdkmsg::MessageState::SentDirect => MessageState::SentDirect {},
        sdkmsg::MessageState::SentToPropagation => MessageState::SentToPropagation {},
        sdkmsg::MessageState::Delivered => MessageState::Delivered {},
        sdkmsg::MessageState::Failed => MessageState::Failed {},
        sdkmsg::MessageState::TimedOut => MessageState::TimedOut {},
        sdkmsg::MessageState::Cancelled => MessageState::Cancelled {},
        sdkmsg::MessageState::Received => MessageState::Received {},
    }
}

fn to_sdk_message_direction(direction: MessageDirection) -> sdkmsg::MessageDirection {
    match direction {
        MessageDirection::Inbound {} => sdkmsg::MessageDirection::Inbound,
        MessageDirection::Outbound {} => sdkmsg::MessageDirection::Outbound,
    }
}

fn from_sdk_message_direction(direction: sdkmsg::MessageDirection) -> MessageDirection {
    match direction {
        sdkmsg::MessageDirection::Inbound => MessageDirection::Inbound {},
        sdkmsg::MessageDirection::Outbound => MessageDirection::Outbound {},
    }
}

fn from_sdk_sync_phase(phase: sdkmsg::SyncPhase) -> SyncPhase {
    match phase {
        sdkmsg::SyncPhase::Idle => SyncPhase::Idle {},
        sdkmsg::SyncPhase::PathRequested => SyncPhase::PathRequested {},
        sdkmsg::SyncPhase::LinkEstablishing => SyncPhase::LinkEstablishing {},
        sdkmsg::SyncPhase::RequestSent => SyncPhase::RequestSent {},
        sdkmsg::SyncPhase::Receiving => SyncPhase::Receiving {},
        sdkmsg::SyncPhase::Complete => SyncPhase::Complete {},
        sdkmsg::SyncPhase::Failed => SyncPhase::Failed {},
    }
}

fn to_sdk_announce_record(record: AnnounceRecord) -> sdkmsg::AnnounceRecord {
    sdkmsg::AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        app_data: record.app_data,
        display_name: record.display_name,
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn from_sdk_announce_record(record: sdkmsg::AnnounceRecord) -> AnnounceRecord {
    AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        app_data: record.app_data,
        display_name: record.display_name,
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn to_sdk_message_record(record: MessageRecord) -> sdkmsg::MessageRecord {
    sdkmsg::MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: to_sdk_message_direction(record.direction),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: to_sdk_message_method(record.method),
        state: to_sdk_message_state(record.state),
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn from_sdk_message_record(record: sdkmsg::MessageRecord) -> MessageRecord {
    MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: from_sdk_message_direction(record.direction),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: from_sdk_message_method(record.method),
        state: from_sdk_message_state(record.state),
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn from_sdk_peer_record(record: sdkmsg::PeerRecord) -> PeerRecord {
    PeerRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: from_sdk_peer_state(record.state),
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}

fn from_sdk_conversation_record(record: sdkmsg::ConversationRecord) -> ConversationRecord {
    ConversationRecord {
        conversation_id: record.conversation_id,
        peer_destination_hex: record.peer_destination_hex,
        peer_display_name: record.peer_display_name,
        last_message_preview: record.last_message_preview,
        last_message_at_ms: record.last_message_at_ms,
        unread_count: record.unread_count,
        last_message_state: record.last_message_state.map(from_sdk_message_state),
    }
}

fn from_sdk_sync_status(status: sdkmsg::SyncStatus) -> SyncStatus {
    SyncStatus {
        phase: from_sdk_sync_phase(status.phase),
        active_propagation_node_hex: status.active_propagation_node_hex,
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail,
    }
}

fn to_sdk_send_request(request: &SendLxmfRequest) -> sdkmsg::SendMessageRequest {
    sdkmsg::SendMessageRequest {
        destination_hex: request.destination_hex.clone(),
        body_utf8: request.body_utf8.clone(),
        title: request.title.clone(),
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MissionSyncMetadata {
    pub(crate) correlation_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) command_type: Option<String>,
    pub(crate) result_status: Option<String>,
    pub(crate) event_type: Option<String>,
    pub(crate) event_uid: Option<String>,
    pub(crate) mission_uid: Option<String>,
}

impl MissionSyncMetadata {
    pub(crate) fn tracking_key(&self) -> Option<&str> {
        self.correlation_id
            .as_deref()
            .or(self.command_id.as_deref())
    }

    pub(crate) fn primary_kind(&self) -> &'static str {
        if self.command_type.is_some() {
            "command"
        } else if self.result_status.is_some() {
            "result"
        } else if self.event_type.is_some() {
            "event"
        } else {
            "message"
        }
    }

    pub(crate) fn primary_name(&self) -> Option<&str> {
        self.command_type
            .as_deref()
            .or(self.event_type.as_deref())
            .or(self.result_status.as_deref())
    }

    pub(crate) fn is_event_related(&self) -> bool {
        self.command_type
            .as_deref()
            .is_some_and(is_event_mission_name)
            || self
                .event_type
                .as_deref()
                .is_some_and(is_event_mission_name)
            || self.event_uid.is_some()
            || self.mission_uid.is_some()
    }
}

#[derive(Debug, Clone)]
struct PendingLxmfDelivery {
    message_id_hex: String,
    destination_hex: String,
    correlation_id: Option<String>,
    command_id: Option<String>,
    command_type: Option<String>,
    event_uid: Option<String>,
    mission_uid: Option<String>,
    sent_at_ms: u64,
}

#[derive(Debug, Clone)]
struct PendingLxmfAcknowledgement {
    source_hex: String,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
struct RegisteredPendingLxmfDelivery {
    pending: PendingLxmfDelivery,
    buffered_ack: Option<PendingLxmfAcknowledgement>,
}

#[derive(Debug, Clone)]
pub(crate) struct LxmfSendReport {
    pub(crate) outcome: RnsSendOutcome,
    pub(crate) message_id_hex: String,
    pub(crate) resolved_destination_hex: String,
    pub(crate) metadata: Option<MissionSyncMetadata>,
    pub(crate) track_delivery_timeout: bool,
    pub(crate) used_resource: bool,
    pub(crate) receipt_hash_hex: Option<String>,
}

struct RuntimeReceiptBridge {
    receipt_message_ids: Arc<Mutex<HashMap<String, String>>>,
    tx: mpsc::UnboundedSender<String>,
}

impl ReceiptHandler for RuntimeReceiptBridge {
    fn on_receipt(&self, receipt: &DeliveryReceipt) {
        let packet_hash_hex = hex::encode(receipt.message_id);
        let Some(message_id_hex) = self
            .receipt_message_ids
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&packet_hash_hex))
        else {
            return;
        };
        let _ = self.tx.send(message_id_hex);
    }
}

fn is_event_mission_name(name: &str) -> bool {
    matches!(
        name,
        "mission.registry.mission.upsert"
            | "mission.registry.mission.upserted"
            | "mission.registry.log_entry.upsert"
            | "mission.registry.log_entry.upserted"
            | "mission.registry.log_entry.list"
            | "mission.registry.log_entry.listed"
    )
}

fn msgpack_get_indexed<'a>(value: &'a MsgPackValue, key: i64) -> Option<&'a MsgPackValue> {
    let entries = match value {
        MsgPackValue::Map(entries) => entries,
        _ => return None,
    };
    let key_string = key.to_string();

    for (entry_key, entry_value) in entries {
        match entry_key {
            MsgPackValue::Integer(value) if value.as_i64() == Some(key) => {
                return Some(entry_value)
            }
            MsgPackValue::String(value) if value.as_str() == Some(key_string.as_str()) => {
                return Some(entry_value)
            }
            _ => {}
        }
    }
    None
}

fn msgpack_get_named<'a>(value: &'a MsgPackValue, keys: &[&str]) -> Option<&'a MsgPackValue> {
    let entries = match value {
        MsgPackValue::Map(entries) => entries,
        _ => return None,
    };

    for wanted in keys {
        for (entry_key, entry_value) in entries {
            if matches!(entry_key, MsgPackValue::String(actual) if actual.as_str() == Some(*wanted))
            {
                return Some(entry_value);
            }
        }
    }
    None
}

fn msgpack_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().map(ToOwned::to_owned),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone()).ok(),
        _ => None,
    }
}

fn parse_mission_sync_metadata(fields_bytes: &[u8]) -> Option<MissionSyncMetadata> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let mut metadata = MissionSyncMetadata::default();

    if let Some(commands) = msgpack_get_indexed(&fields, LXMF_FIELD_COMMANDS) {
        if let MsgPackValue::Array(entries) = commands {
            if let Some(first) = entries.first() {
                metadata.command_id =
                    msgpack_get_named(first, &["command_id"]).and_then(msgpack_string);
                metadata.correlation_id =
                    msgpack_get_named(first, &["correlation_id"]).and_then(msgpack_string);
                metadata.command_type =
                    msgpack_get_named(first, &["command_type"]).and_then(msgpack_string);
                if let Some(args) = msgpack_get_named(first, &["args"]) {
                    metadata.event_uid = msgpack_get_named(args, &["entry_uid", "entryUid", "uid"])
                        .and_then(msgpack_string);
                    metadata.mission_uid =
                        msgpack_get_named(args, &["mission_uid", "missionUid", "uid"])
                            .and_then(msgpack_string);
                }
            }
        }
    }

    if let Some(result) = msgpack_get_indexed(&fields, LXMF_FIELD_RESULTS) {
        metadata.command_id = metadata
            .command_id
            .or_else(|| msgpack_get_named(result, &["command_id"]).and_then(msgpack_string));
        metadata.correlation_id = metadata
            .correlation_id
            .or_else(|| msgpack_get_named(result, &["correlation_id"]).and_then(msgpack_string));
        metadata.result_status = msgpack_get_named(result, &["status"]).and_then(msgpack_string);
    }

    if let Some(event) = msgpack_get_indexed(&fields, LXMF_FIELD_EVENT) {
        metadata.event_type = msgpack_get_named(event, &["event_type"]).and_then(msgpack_string);
        metadata.event_uid = metadata
            .event_uid
            .or_else(|| msgpack_get_named(event, &["event_id"]).and_then(msgpack_string));
        if let Some(payload) = msgpack_get_named(event, &["payload"]) {
            metadata.event_uid = metadata.event_uid.or_else(|| {
                msgpack_get_named(payload, &["entry_uid", "entryUid", "uid"])
                    .and_then(msgpack_string)
            });
            metadata.mission_uid = metadata.mission_uid.or_else(|| {
                msgpack_get_named(payload, &["mission_uid", "missionUid", "uid"])
                    .and_then(msgpack_string)
            });
        }
    }

    if metadata.command_id.is_none()
        && metadata.correlation_id.is_none()
        && metadata.command_type.is_none()
        && metadata.result_status.is_none()
        && metadata.event_type.is_none()
        && metadata.event_uid.is_none()
        && metadata.mission_uid.is_none()
    {
        return None;
    }

    Some(metadata)
}

fn emit_lxmf_delivery(
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    status: LxmfDeliveryStatus,
    detail: Option<String>,
) {
    let now = now_ms();
    bus.emit(NodeEvent::LxmfDelivery {
        update: LxmfDeliveryUpdate {
            message_id_hex: pending.message_id_hex.clone(),
            destination_hex: pending.destination_hex.clone(),
            source_hex: None,
            correlation_id: pending.correlation_id.clone(),
            command_id: pending.command_id.clone(),
            command_type: pending.command_type.clone(),
            event_uid: pending.event_uid.clone(),
            mission_uid: pending.mission_uid.clone(),
            status,
            detail,
            sent_at_ms: pending.sent_at_ms,
            updated_at_ms: now,
        },
    });
}

fn emit_lxmf_delivery_with_source(
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    source_hex: Option<String>,
    status: LxmfDeliveryStatus,
    detail: Option<String>,
) {
    let now = now_ms();
    bus.emit(NodeEvent::LxmfDelivery {
        update: LxmfDeliveryUpdate {
            message_id_hex: pending.message_id_hex.clone(),
            destination_hex: pending.destination_hex.clone(),
            source_hex,
            correlation_id: pending.correlation_id.clone(),
            command_id: pending.command_id.clone(),
            command_type: pending.command_type.clone(),
            event_uid: pending.event_uid.clone(),
            mission_uid: pending.mission_uid.clone(),
            status,
            detail,
            sent_at_ms: pending.sent_at_ms,
            updated_at_ms: now,
        },
    });
}

fn create_transport_data_packet(destination: AddressHash, bytes: &[u8]) -> Packet {
    let mut packet = Packet::default();
    packet.header.propagation_type = PropagationType::Transport;
    packet.destination = destination;
    packet.data = PacketDataBuffer::new_from_slice(bytes);
    packet
}

async fn send_transport_packet_with_path_retry(
    transport: &Arc<Transport>,
    destination: AddressHash,
    bytes: &[u8],
) -> RnsSendOutcome {
    const MAX_ATTEMPTS: usize = 6;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    let mut last_outcome = RnsSendOutcome::DroppedNoRoute;

    for _ in 0..MAX_ATTEMPTS {
        let packet = create_transport_data_packet(destination, bytes);
        let outcome = transport.send_packet_with_outcome(packet).await;
        if matches!(
            outcome,
            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
        ) {
            return outcome;
        }

        last_outcome = outcome;
        if matches!(
            outcome,
            RnsSendOutcome::DroppedNoRoute | RnsSendOutcome::DroppedMissingDestinationIdentity
        ) {
            transport.request_path(&destination, None, None).await;
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }
        break;
    }

    last_outcome
}

fn conversation_id_for(destination_hex: &str) -> String {
    sdkmsg::MessagingStore::conversation_id_for(destination_hex)
}

async fn connected_destination_hexes(state: &NodeRuntimeState) -> Vec<String> {
    state.connected_peers
        .lock()
        .await
        .iter()
        .map(address_hash_to_hex)
        .collect::<Vec<_>>()
}

async fn snapshot_peer_records(state: &NodeRuntimeState) -> Vec<PeerRecord> {
    let connected = connected_destination_hexes(state).await;
    state
        .messaging
        .lock()
        .await
        .list_peers(connected.iter().map(String::as_str))
        .into_iter()
        .map(from_sdk_peer_record)
        .collect()
}

async fn emit_peer_resolved(state: &NodeRuntimeState, bus: &EventBus, identity_hex: &str) {
    let connected = connected_destination_hexes(state).await;
    if let Some(peer) = state
        .messaging
        .lock()
        .await
        .peer_for_identity(identity_hex, connected.iter().map(String::as_str))
        .map(from_sdk_peer_record)
    {
        bus.emit(NodeEvent::PeerResolved { peer });
    }
}

async fn upsert_message_record(
    state: &NodeRuntimeState,
    bus: &EventBus,
    message: MessageRecord,
    emit_received: bool,
) {
    state.messaging.lock().await.upsert_message(to_sdk_message_record(message.clone()));

    if emit_received {
        bus.emit(NodeEvent::MessageReceived {
            message: message.clone(),
        });
    }
    bus.emit(NodeEvent::MessageUpdated { message });
}

async fn message_records_snapshot(
    state: &NodeRuntimeState,
    conversation_id: Option<&str>,
) -> Vec<MessageRecord> {
    state
        .messaging
        .lock()
        .await
        .list_messages(conversation_id)
        .into_iter()
        .map(from_sdk_message_record)
        .collect()
}

async fn conversation_records_snapshot(state: &NodeRuntimeState) -> Vec<ConversationRecord> {
    let connected = connected_destination_hexes(state).await;
    state
        .messaging
        .lock()
        .await
        .list_conversations(connected.iter().map(String::as_str))
        .into_iter()
        .map(from_sdk_conversation_record)
        .collect()
}

pub enum Command {
    Stop {
        resp: cb::Sender<Result<(), NodeError>>,
    },
    AnnounceNow {
        resp: cb::Sender<Result<(), NodeError>>,
    },
    ConnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    DisconnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendBytes {
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    BroadcastBytes {
        bytes: Vec<u8>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    RequestPeerIdentity {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendLxmf {
        request: SendLxmfRequest,
        resp: cb::Sender<Result<String, NodeError>>,
    },
    RetryLxmf {
        message_id_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    CancelLxmf {
        message_id_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetActivePropagationNode {
        destination_hex: Option<String>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    RequestLxmfSync {
        limit: Option<u32>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    ListAnnounces {
        resp: cb::Sender<Result<Vec<AnnounceRecord>, NodeError>>,
    },
    ListPeers {
        resp: cb::Sender<Result<Vec<PeerRecord>, NodeError>>,
    },
    ListConversations {
        resp: cb::Sender<Result<Vec<ConversationRecord>, NodeError>>,
    },
    ListMessages {
        conversation_id: Option<String>,
        resp: cb::Sender<Result<Vec<MessageRecord>, NodeError>>,
    },
    GetLxmfSyncStatus {
        resp: cb::Sender<Result<SyncStatus, NodeError>>,
    },
    SetAnnounceCapabilities {
        capability_string: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetLogLevel {
        level: crate::types::LogLevel,
    },
    RefreshHubDirectory {
        resp: cb::Sender<Result<(), NodeError>>,
    },
}

#[derive(Clone)]
struct NodeRuntimeState {
    identity: PrivateIdentity,
    transport: Arc<Transport>,
    lxmf_destination: Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
    connected_peers: Arc<TokioMutex<HashSet<AddressHash>>>,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    out_links:
        Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>>,
    pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>>,
    pending_lxmf_acknowledgements:
        Arc<TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>>,
    messaging: Arc<TokioMutex<sdkmsg::MessagingStore>>,
    sdk: Arc<RuntimeLxmfSdk>,
}

async fn ensure_destination_desc(
    state: &NodeRuntimeState,
    dest: AddressHash,
    expected_name: Option<DestinationName>,
) -> Result<DestinationDesc, NodeError> {
    if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
        return Ok(desc);
    }

    state.transport.request_path(&dest, None, None).await;

    let deadline = tokio::time::Instant::now() + DEFAULT_IDENTITY_WAIT_TIMEOUT;
    loop {
        if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
            return Ok(desc);
        }

        if let Some(identity) = state.transport.destination_identity(&dest).await {
            let name = expected_name.unwrap_or_else(|| {
                DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1)
            });
            return Ok(DestinationDesc {
                identity,
                address_hash: dest,
                name,
            });
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(feature = "legacy-lxmf-runtime")]
async fn resolve_lxmf_destination_desc(
    state: &NodeRuntimeState,
    destination: AddressHash,
) -> Result<DestinationDesc, NodeError> {
    let desc = ensure_destination_desc(state, destination, None).await?;
    let lxmf_destination = SingleOutputDestination::new(
        desc.identity,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    );
    Ok(lxmf_destination.desc)
}

#[cfg(feature = "legacy-lxmf-runtime")]
async fn ensure_lxmf_output_link(
    state: &NodeRuntimeState,
    desc: DestinationDesc,
) -> Result<Arc<TokioMutex<reticulum::destination::link::Link>>, NodeError> {
    const MAX_ATTEMPTS: usize = 3;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    for attempt in 0..MAX_ATTEMPTS {
        let link = {
            let mut links = state.out_links.lock().await;
            if let Some(existing) = links.get(&desc.address_hash).cloned() {
                existing
            } else {
                let created = state.transport.link(desc).await;
                links.insert(desc.address_hash, created.clone());
                created
            }
        };

        match wait_for_link_active(&state.transport, &link).await {
            Ok(()) => return Ok(link),
            Err(err) => {
                let stale = state.out_links.lock().await.remove(&desc.address_hash);
                if let Some(stale) = stale {
                    stale.lock().await.close();
                }
                if attempt + 1 == MAX_ATTEMPTS {
                    return Err(err);
                }
                info!(
                    "[lxmf][events] link activation retry destination={} attempt={} reason={}",
                    address_hash_to_hex(&desc.address_hash),
                    attempt + 1,
                    err,
                );
                state
                    .transport
                    .request_path(&desc.address_hash, None, None)
                    .await;
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(NodeError::Timeout {})
}

#[cfg(feature = "legacy-lxmf-runtime")]
async fn send_lxmf_message(
    state: &NodeRuntimeState,
    destination: AddressHash,
    content: &[u8],
    fields_bytes: Option<Vec<u8>>,
) -> Result<LxmfSendReport, NodeError> {
    let remote_desc = resolve_lxmf_destination_desc(state, destination).await?;

    let mut source = [0u8; 16];
    source.copy_from_slice(
        state
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .as_slice(),
    );

    let mut target = [0u8; 16];
    target.copy_from_slice(remote_desc.address_hash.as_slice());

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(target);
    message.set_content_from_bytes(content);
    message.fields = match fields_bytes.as_ref() {
        Some(bytes) => Some(
            rmp_serde::from_slice::<MsgPackValue>(bytes)
                .map_err(|_| NodeError::InvalidConfig {})?,
        ),
        None => None,
    };

    let signer = lxmf_private_identity(&state.identity)?;
    let wire = message
        .to_wire(Some(&signer))
        .map_err(|_| NodeError::LxmfWireEncodeError {})?;
    debug!(
        "[lxmf][debug] send_lxmf_message wire ready requested_destination={} resolved_destination={} content_bytes={} fields_bytes={} wire_bytes={} max_wire_bytes={}",
        address_hash_to_hex(&destination),
        address_hash_to_hex(&remote_desc.address_hash),
        content.len(),
        fields_bytes.as_ref().map(Vec::len).unwrap_or(0),
        wire.len(),
        LXMF_MAX_PAYLOAD,
    );
    if wire.len() > LXMF_MAX_PAYLOAD {
        error!(
            "[lxmf][events] packet too large requested_destination={} resolved_destination={} content_bytes={} fields_bytes={} wire_bytes={} max_wire_bytes={}",
            address_hash_to_hex(&destination),
            address_hash_to_hex(&remote_desc.address_hash),
            content.len(),
            fields_bytes.as_ref().map(Vec::len).unwrap_or(0),
            wire.len(),
            LXMF_MAX_PAYLOAD,
        );
        return Err(NodeError::LxmfPacketTooLarge {});
    }
    let message_id_hex = LxmfWireMessage::unpack(&wire)
        .map(|wire| hex::encode(wire.message_id()))
        .map_err(|_| NodeError::LxmfMessageIdParseError {})?;
    let metadata = fields_bytes
        .as_deref()
        .and_then(parse_mission_sync_metadata);

    if let Some(metadata) = metadata
        .as_ref()
        .filter(|metadata| metadata.is_event_related())
    {
        info!(
            "[lxmf][events] attempting send requested_destination={} resolved_destination={} kind={} name={} message_id={} event_uid={} mission_uid={} correlation={}",
            address_hash_to_hex(&destination),
            address_hash_to_hex(&remote_desc.address_hash),
            metadata.primary_kind(),
            metadata.primary_name().unwrap_or("-"),
            message_id_hex,
            metadata.event_uid.as_deref().unwrap_or("-"),
            metadata.mission_uid.as_deref().unwrap_or("-"),
            metadata.correlation_id.as_deref().unwrap_or("-"),
        );
    }

    let link = ensure_lxmf_output_link(state, remote_desc).await?;
    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|_| NodeError::LxmfPacketBuildError {})?;
    let receipt_hash_hex = hex::encode(packet.hash().to_bytes());
    let outcome = state.transport.send_packet_with_outcome(packet).await;

    Ok(LxmfSendReport {
        outcome,
        message_id_hex,
        resolved_destination_hex: address_hash_to_hex(&remote_desc.address_hash),
        metadata,
        track_delivery_timeout: true,
        used_resource: false,
        receipt_hash_hex: Some(receipt_hash_hex),
    })
}

async fn register_pending_lxmf_delivery(
    state: &NodeRuntimeState,
    report: &LxmfSendReport,
) -> Option<RegisteredPendingLxmfDelivery> {
    if !report.track_delivery_timeout {
        return None;
    }
    let metadata = report.metadata.as_ref()?;
    let tracking_key = metadata.tracking_key()?.to_string();
    let pending = PendingLxmfDelivery {
        message_id_hex: report.message_id_hex.clone(),
        destination_hex: report.resolved_destination_hex.clone(),
        correlation_id: metadata.correlation_id.clone(),
        command_id: metadata.command_id.clone(),
        command_type: metadata.command_type.clone(),
        event_uid: metadata.event_uid.clone(),
        mission_uid: metadata.mission_uid.clone(),
        sent_at_ms: now_ms(),
    };

    state
        .pending_lxmf_deliveries
        .lock()
        .await
        .insert(tracking_key.clone(), pending.clone());
    let buffered_ack = state
        .pending_lxmf_acknowledgements
        .lock()
        .await
        .remove(&tracking_key);
    Some(RegisteredPendingLxmfDelivery {
        pending,
        buffered_ack,
    })
}

async fn emit_received_payload(
    state: &NodeRuntimeState,
    bus: &EventBus,
    sdk: &RuntimeLxmfSdk,
    destination_hex: String,
    payload: Vec<u8>,
    fallback_fields_bytes: Option<Vec<u8>>,
) {
    if let Ok(message) = LxmfMessage::from_wire(payload.as_slice()) {
        let source_hex = message.source_hash.map(hex::encode);
        let body_utf8 = String::from_utf8_lossy(message.content.as_slice()).to_string();
        let title = if message.title.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(message.title.as_slice()).to_string())
        };
        let fields_bytes = message
            .fields
            .and_then(|value| rmp_serde::to_vec(&value).ok());
        let metadata = fields_bytes
            .as_deref()
            .and_then(parse_mission_sync_metadata);
        if let Some(metadata) = metadata.as_ref() {
            if metadata.is_event_related() {
                info!(
                    "[lxmf][events] received kind={} name={} source={} destination={} event_uid={} mission_uid={} correlation={}",
                    metadata.primary_kind(),
                    metadata.primary_name().unwrap_or("-"),
                    source_hex.as_deref().unwrap_or("-"),
                    destination_hex,
                    metadata.event_uid.as_deref().unwrap_or("-"),
                    metadata.mission_uid.as_deref().unwrap_or("-"),
                    metadata.correlation_id.as_deref().unwrap_or("-"),
                );
            }
            ack_pending_lxmf_delivery(state, bus, source_hex.as_deref(), &metadata).await;
        }
        if !metadata.as_ref().is_some_and(MissionSyncMetadata::is_event_related) {
            let peer_hex = source_hex.clone().unwrap_or_else(|| destination_hex.clone());
            let message_id_hex = LxmfWireMessage::unpack(payload.as_slice())
                .map(|wire| hex::encode(wire.message_id()))
                .unwrap_or_else(|_| hex::encode(destination_hex.as_bytes()));
            let record = MessageRecord {
                message_id_hex,
                conversation_id: conversation_id_for(peer_hex.as_str()),
                direction: MessageDirection::Inbound {},
                destination_hex: peer_hex.clone(),
                source_hex: source_hex.clone(),
                title,
                body_utf8,
                method: MessageMethod::Direct {},
                state: MessageState::Received {},
                detail: None,
                sent_at_ms: None,
                received_at_ms: Some(now_ms()),
                updated_at_ms: now_ms(),
            };
            upsert_message_record(state, bus, record, true).await;
        }
        sdk.record_packet_received(
            &destination_hex,
            source_hex.as_deref(),
            message.content.as_slice(),
            fields_bytes.as_deref(),
        );
        bus.emit(NodeEvent::PacketReceived {
            destination_hex,
            source_hex,
            bytes: message.content,
            fields_bytes,
        });
        return;
    }

    sdk.record_packet_received(
        &destination_hex,
        None,
        payload.as_slice(),
        fallback_fields_bytes.as_deref(),
    );
    bus.emit(NodeEvent::PacketReceived {
        destination_hex,
        source_hex: None,
        bytes: payload,
        fields_bytes: fallback_fields_bytes,
    });
}

async fn ack_pending_lxmf_delivery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    metadata: &MissionSyncMetadata,
) {
    let Some(source_hex) = source_hex else {
        return;
    };

    let detail = metadata
        .result_status
        .clone()
        .or_else(|| metadata.event_type.clone())
        .or_else(|| metadata.command_type.clone());
    let mut guard = state.pending_lxmf_deliveries.lock().await;
    let mut matched: Option<PendingLxmfDelivery> = None;

    for key in [
        metadata.correlation_id.as_deref(),
        metadata.command_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(candidate) = guard.remove(key) {
            matched = Some(candidate);
            break;
        }
    }

    drop(guard);

    let Some(pending) = matched else {
        if let Some(tracking_key) = metadata.tracking_key().map(ToOwned::to_owned) {
            state
                .pending_lxmf_acknowledgements
                .lock()
                .await
                .insert(
                    tracking_key.clone(),
                    PendingLxmfAcknowledgement {
                        source_hex: source_hex.to_string(),
                        detail: detail.clone(),
                    },
                );
            info!(
                "[lxmf][events] buffered acknowledgement source={} command={} correlation={} detail={}",
                source_hex,
                metadata.command_type.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
                detail.as_deref().unwrap_or("-"),
            );
        }
        return;
    };
    if pending.destination_hex != source_hex {
        if let Some(tracking_key) = pending
            .correlation_id
            .as_deref()
            .or(pending.command_id.as_deref())
            .map(ToOwned::to_owned)
        {
            state
                .pending_lxmf_deliveries
                .lock()
                .await
                .insert(tracking_key, pending);
        }
        return;
    }

    state.sdk.record_delivery_acknowledged(
        &pending.message_id_hex,
        &pending.destination_hex,
        Some(source_hex),
        pending.correlation_id.as_deref(),
        pending.command_id.as_deref(),
        pending.command_type.as_deref(),
        pending.event_uid.as_deref(),
        pending.mission_uid.as_deref(),
        detail.as_deref(),
    );
    emit_lxmf_delivery_with_source(
        bus,
        &pending,
        Some(source_hex.to_string()),
        LxmfDeliveryStatus::Acknowledged {},
        detail.clone(),
    );
    info!(
        "[lxmf][events] acknowledged message_id={} destination={} command={} correlation={} detail={}",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
        detail.as_deref().unwrap_or("-"),
    );
}

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<reticulum::destination::link::Link>>,
) -> Result<(), NodeError> {
    if link.lock().await.status() == LinkStatus::Active {
        return Ok(());
    }

    let link_id = *link.lock().await.id();
    let mut events = transport.out_link_events();
    let deadline = tokio::time::Instant::now() + DEFAULT_LINK_CONNECT_TIMEOUT;

    loop {
        if link.lock().await.status() == LinkStatus::Active {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        match tokio::time::timeout(Duration::from_millis(250), events.recv()).await {
            Ok(Ok(event)) => {
                if event.id == link_id && matches!(event.event, LinkEvent::Activated) {
                    return Ok(());
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        }
    }
}

async fn refresh_hub_directory_http(config: &NodeConfig) -> Result<Vec<String>, NodeError> {
    let base = config
        .hub_api_base_url
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let url = join_url(base, "/Client")?;

    let mut req = reqwest::Client::new().get(url);
    if let Some(key) = config
        .hub_api_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        req = req
            .header("X-API-Key", key)
            .header("Authorization", format!("Bearer {}", key));
    }

    let body = req
        .send()
        .await
        .map_err(|_| NodeError::NetworkError {})?
        .text()
        .await
        .map_err(|_| NodeError::NetworkError {})?;

    Ok(extract_hex_destinations(&body))
}

async fn refresh_hub_directory_lxmf(
    config: &NodeConfig,
    state: &NodeRuntimeState,
) -> Result<Vec<String>, NodeError> {
    let hub_hex = config
        .hub_identity_hash
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let hub = parse_address_hash(hub_hex)?;

    let hub_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    let hub_desc = ensure_destination_desc(state, hub, Some(hub_name)).await?;

    let link = {
        let mut links = state.out_links.lock().await;
        if let Some(existing) = links.get(&hub).cloned() {
            existing
        } else {
            let created = state.transport.link(hub_desc).await;
            links.insert(hub, created.clone());
            created
        }
    };

    wait_for_link_active(&state.transport, &link).await?;

    let mut source = [0u8; 16];
    source.copy_from_slice(
        state
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .as_slice(),
    );
    let mut destination = [0u8; 16];
    destination.copy_from_slice(hub.as_slice());

    let content = r#"\\\{"Command":"ListClients"}"#;

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(destination);
    message.set_title_from_string("ListClients");
    message.set_content_from_string(content);

    let signer = lxmf_private_identity(&state.identity)?;
    let wire = message
        .to_wire(Some(&signer))
        .map_err(|_| NodeError::InternalError {})?;

    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|_| NodeError::InternalError {})?;
    let outcome = state.transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(NodeError::NetworkError {});
    }

    let mut rx = state.transport.received_data_events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        let received = match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(event)) => event,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        };

        if received.destination != hub {
            continue;
        }

        let Ok(reply) = LxmfMessage::from_wire(received.data.as_slice()) else {
            continue;
        };

        let mut text = String::new();
        if !reply.title.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.title));
            text.push('\n');
        }
        if !reply.content.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.content));
            text.push('\n');
        }
        if let Some(fields) = &reply.fields {
            text.push_str(&format!("{fields:?}"));
        }

        let destinations = extract_hex_destinations(&text);
        if !destinations.is_empty() {
            return Ok(destinations);
        }
    }
}

pub async fn run_node(
    config: NodeConfig,
    identity: PrivateIdentity,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
) {
    let mut transport_cfg = TransportConfig::new(config.name.clone(), &identity, config.broadcast);
    transport_cfg.set_retransmit(false);

    if let Some(dir) = config
        .storage_dir
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let mut path = PathBuf::from(dir);
        path.push("ratchets.dat");
        transport_cfg.set_ratchet_store_path(path);
    }

    let mut transport = Transport::new(transport_cfg);
    let receipt_message_ids = Arc::new(Mutex::new(HashMap::<String, String>::new()));
    let (receipt_tx, mut receipt_rx) = mpsc::unbounded_channel::<String>();
    transport
        .set_receipt_handler(Box::new(RuntimeReceiptBridge {
            receipt_message_ids: receipt_message_ids.clone(),
            tx: receipt_tx,
        }))
        .await;

    for endpoint in &config.tcp_clients {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            continue;
        }
        transport
            .iface_manager()
            .lock()
            .await
            .spawn(TcpClient::new(endpoint), TcpClient::spawn);
    }

    let app_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .await;
    let lxmf_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .await;

    let transport = Arc::new(transport);

    let announce_capabilities = Arc::new(TokioMutex::new(config.announce_capabilities.clone()));
    let known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let out_links: Arc<
        TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>,
    > = Arc::new(TokioMutex::new(HashMap::new()));
    let connected_peers: Arc<TokioMutex<HashSet<AddressHash>>> =
        Arc::new(TokioMutex::new(HashSet::new()));
    let pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let pending_lxmf_acknowledgements: Arc<
        TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>,
    > = Arc::new(TokioMutex::new(HashMap::new()));
    let messaging = Arc::new(TokioMutex::new(sdkmsg::MessagingStore::default()));
    let sdk = Arc::new(RuntimeLxmfSdk::new(
        identity.address_hash().to_hex_string(),
        SdkTransportState {
            identity: identity.clone(),
            transport: transport.clone(),
            lxmf_destination: lxmf_destination.clone(),
            known_destinations: known_destinations.clone(),
            out_links: out_links.clone(),
        },
    ));

    let state = NodeRuntimeState {
        identity: identity.clone(),
        transport: transport.clone(),
        lxmf_destination: lxmf_destination.clone(),
        connected_peers: connected_peers.clone(),
        known_destinations: known_destinations.clone(),
        out_links: out_links.clone(),
        pending_lxmf_deliveries: pending_lxmf_deliveries.clone(),
        pending_lxmf_acknowledgements: pending_lxmf_acknowledgements.clone(),
        messaging: messaging.clone(),
        sdk: sdk.clone(),
    };

    if let Err(err) = sdk.start().await {
        bus.emit(NodeEvent::Error {
            code: "sdk_start_failed".to_string(),
            message: err.to_string(),
        });
    }

    if let Ok(mut guard) = status.lock() {
        guard.running = true;
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }

    // Transport delivery receipts.
    {
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            while let Some(message_id_hex) = receipt_rx.recv().await {
                let maybe_record = state
                    .messaging
                    .lock()
                    .await
                    .update_message(
                        message_id_hex.as_str(),
                        sdkmsg::MessageState::Delivered,
                        Some("transport receipt".to_string()),
                        now_ms(),
                    )
                    .map(from_sdk_message_record);

                if let Some(record) = maybe_record {
                    sdk.record_delivery_acknowledged(
                        &record.message_id_hex,
                        &record.destination_hex,
                        record.source_hex.as_deref(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        record.detail.as_deref(),
                    );
                    bus.emit(NodeEvent::MessageUpdated {
                        message: record.clone(),
                    });
                }
            }
        });
    }

    // Announces.
    {
        let transport = transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = lxmf_destination.clone();
        let announce_capabilities = announce_capabilities.clone();
        let lxmf_display_name_app_data = encode_delivery_display_name_app_data(&config.name);
        let interval_secs = config.announce_interval_seconds.max(1);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                let caps = announce_capabilities.lock().await.clone();
                transport
                    .send_announce(&app_destination, Some(caps.as_bytes()))
                    .await;
                transport
                    .send_announce(&lxmf_destination, lxmf_display_name_app_data.as_deref())
                    .await;
            }
        });
    }

    // Announce receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let sdk = sdk.clone();
        let known_destinations = known_destinations.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.recv_announces().await;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let desc = event.destination.lock().await.desc;
                        known_destinations
                            .lock()
                            .await
                            .insert(desc.address_hash, desc);
                        let destination_hex = address_hash_to_hex(&desc.address_hash);
                        let identity_hex = desc.identity.address_hash.to_hex_string();
                        let destination_kind = announce_destination_kind(&desc).to_string();
                        let app_data_bytes = event.app_data.as_slice().to_vec();
                        let app_data = String::from_utf8(app_data_bytes.clone())
                            .unwrap_or_else(|_| hex::encode(app_data_bytes.as_slice()));
                        let display_name = if destination_kind == "lxmf_delivery" {
                            display_name_from_delivery_app_data(app_data_bytes.as_slice())
                        } else {
                            None
                        };
                        let interface_hex = hex::encode(event.interface);
                        state.messaging.lock().await.record_announce(to_sdk_announce_record(
                            AnnounceRecord {
                                destination_hex: destination_hex.clone(),
                                identity_hex: identity_hex.clone(),
                                destination_kind: destination_kind.clone(),
                                app_data: app_data.clone(),
                                display_name: display_name.clone(),
                                hops: event.hops,
                                interface_hex: interface_hex.clone(),
                                received_at_ms: now_ms(),
                            },
                        ));
                        sdk.record_announce_received(
                            &destination_hex,
                            &identity_hex,
                            &destination_kind,
                            &app_data,
                            event.hops,
                            &interface_hex,
                        );
                        bus.emit(NodeEvent::AnnounceReceived {
                            destination_hex,
                            identity_hex: identity_hex.clone(),
                            destination_kind,
                            app_data,
                            hops: event.hops,
                            interface_hex,
                            received_at_ms: now_ms(),
                        });
                        emit_peer_resolved(&state, &bus, &identity_hex).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Data receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            let mut rx = transport.received_data_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.destination);
                        emit_received_payload(
                            &state,
                            &bus,
                            &sdk,
                            destination_hex,
                            event.data.as_slice().to_vec(),
                            None,
                        )
                        .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Resource receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            let mut rx = transport.resource_events();
            loop {
                match rx.recv().await {
                    Ok(event) => match event.kind {
                        ResourceEventKind::Complete(complete) => {
                            let destination_hex = if let Some(link) =
                                transport.find_in_link(&event.link_id).await
                            {
                                address_hash_to_hex(&link.lock().await.destination().address_hash)
                            } else if let Some(link) = transport.find_out_link(&event.link_id).await
                            {
                                address_hash_to_hex(&link.lock().await.destination().address_hash)
                            } else {
                                address_hash_to_hex(&event.link_id)
                            };
                            info!(
                                "[lxmf][events] resource complete link_id={} destination={} bytes={} metadata_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                destination_hex,
                                complete.data.len(),
                                complete.metadata.as_ref().map(Vec::len).unwrap_or(0),
                            );
                            emit_received_payload(
                                &state,
                                &bus,
                                &sdk,
                                destination_hex,
                                complete.data,
                                complete.metadata,
                            )
                            .await;
                        }
                        ResourceEventKind::Progress(progress) => {
                            debug!(
                                "[lxmf][debug] resource progress link_id={} received_bytes={} total_bytes={} received_parts={} total_parts={}",
                                address_hash_to_hex(&event.link_id),
                                progress.received_bytes,
                                progress.total_bytes,
                                progress.received_parts,
                                progress.total_parts,
                            );
                        }
                        ResourceEventKind::OutboundComplete => {
                            info!(
                                "[lxmf][events] resource outbound complete link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Pending LXMF acknowledgement timeout watcher.
    {
        let bus = bus.clone();
        let sdk = sdk.clone();
        let pending_lxmf_deliveries = pending_lxmf_deliveries.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = now_ms();
                let mut expired = Vec::<PendingLxmfDelivery>::new();
                {
                    let mut guard = pending_lxmf_deliveries.lock().await;
                    let expired_keys = guard
                        .iter()
                        .filter_map(|(key, pending)| {
                            (now.saturating_sub(pending.sent_at_ms)
                                >= DEFAULT_LXMF_ACK_TIMEOUT.as_millis() as u64)
                                .then(|| key.clone())
                        })
                        .collect::<Vec<_>>();
                    for key in expired_keys {
                        if let Some(pending) = guard.remove(&key) {
                            expired.push(pending);
                        }
                    }
                }
                for pending in expired {
                    sdk.record_delivery_timed_out(
                        &pending.message_id_hex,
                        &pending.destination_hex,
                        pending.correlation_id.as_deref(),
                        pending.command_id.as_deref(),
                        pending.command_type.as_deref(),
                        pending.event_uid.as_deref(),
                        pending.mission_uid.as_deref(),
                        Some("ack timeout"),
                    );
                    emit_lxmf_delivery(
                        &bus,
                        &pending,
                        LxmfDeliveryStatus::TimedOut {},
                        Some("ack timeout".to_string()),
                    );
                    info!(
                        "[lxmf][events] timed out message_id={} destination={} command={} correlation={}",
                        pending.message_id_hex,
                        pending.destination_hex,
                        pending.command_type.as_deref().unwrap_or("-"),
                        pending.correlation_id.as_deref().unwrap_or("-"),
                    );
                }
            }
        });
    }

    // Link events.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let sdk = sdk.clone();
        let connected_peers = connected_peers.clone();
        tokio::spawn(async move {
            let mut rx = transport.out_link_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.address_hash);
                        match event.event {
                            LinkEvent::Activated => {
                                connected_peers.lock().await.insert(event.address_hash);
                                sdk.record_peer_changed(
                                    &destination_hex,
                                    PeerState::Connected {},
                                    None,
                                );
                                bus.emit(NodeEvent::PeerChanged {
                                    change: PeerChange {
                                        destination_hex,
                                        state: PeerState::Connected {},
                                        last_error: None,
                                    },
                                })
                            }
                            LinkEvent::Closed => {
                                connected_peers.lock().await.remove(&event.address_hash);
                                sdk.record_peer_changed(
                                    &destination_hex,
                                    PeerState::Disconnected {},
                                    None,
                                );
                                bus.emit(NodeEvent::PeerChanged {
                                    change: PeerChange {
                                        destination_hex,
                                        state: PeerState::Disconnected {},
                                        last_error: None,
                                    },
                                })
                            }
                            LinkEvent::Data(_) => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Optional periodic hub refresh.
    if !matches!(config.hub_mode, HubMode::Disabled {}) && config.hub_refresh_interval_seconds > 0 {
        let bus = bus.clone();
        let config = config.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        let interval_secs = config.hub_refresh_interval_seconds;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                let destinations = match config.hub_mode {
                    HubMode::RchHttp {} => refresh_hub_directory_http(&config).await.ok(),
                    HubMode::RchLxmf {} => refresh_hub_directory_lxmf(&config, &state).await.ok(),
                    HubMode::Disabled {} => None,
                };
                if let Some(destinations) = destinations {
                    sdk.record_hub_directory_updated(destinations.as_slice());
                    bus.emit(NodeEvent::HubDirectoryUpdated {
                        destinations,
                        received_at_ms: now_ms(),
                    });
                }
            }
        });
    }

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Command::Stop { resp } => {
                if let Ok(mut guard) = status.lock() {
                    guard.running = false;
                    bus.emit(NodeEvent::StatusChanged {
                        status: guard.clone(),
                    });
                }
                let _ = resp.send(Ok(()));
                break;
            }
            Command::AnnounceNow { resp } => {
                let caps = announce_capabilities.lock().await.clone();
                transport
                    .send_announce(&app_destination, Some(caps.as_bytes()))
                    .await;
                transport
                    .send_announce(
                        &lxmf_destination,
                        encode_delivery_display_name_app_data(config.name.as_str()).as_deref(),
                    )
                    .await;
                let _ = resp.send(Ok(()));
            }
            Command::SetLogLevel { level } => {
                crate::logger::NodeLogger::global().set_level(level);
            }
            Command::RequestPeerIdentity {
                destination_hex,
                resp,
            } => {
                let result = async {
                    let destination = parse_address_hash(destination_hex.as_str())?;
                    transport.request_path(&destination, None, None).await;
                    let desc = ensure_destination_desc(&state, destination, None).await?;
                    let identity_hex = desc.identity.address_hash.to_hex_string();
                    emit_peer_resolved(&state, &bus, identity_hex.as_str()).await;
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SetAnnounceCapabilities {
                capability_string,
                resp,
            } => {
                *announce_capabilities.lock().await = capability_string;
                let caps = announce_capabilities.lock().await.clone();
                transport
                    .send_announce(&app_destination, Some(caps.as_bytes()))
                    .await;
                let _ = resp.send(Ok(()));
            }
            Command::ConnectPeer {
                destination_hex,
                resp,
            } => {
                let destination_hex_copy = destination_hex.clone();
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex.clone(),
                            state: PeerState::Connecting {},
                            last_error: None,
                        },
                    });
                    state
                        .sdk
                        .record_peer_changed(&destination_hex, PeerState::Connecting {}, None);
                    connected_peers.lock().await.insert(dest);
                    // Fire a path request in the background; direct send will resolve identity on demand.
                    transport.request_path(&dest, None, None).await;
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex.clone(),
                            state: PeerState::Connected {},
                            last_error: None,
                        },
                    });
                    state
                        .sdk
                        .record_peer_changed(&destination_hex, PeerState::Connected {}, None);
                    Ok::<(), NodeError>(())
                }
                .await;
                if let Err(err) = &result {
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex_copy.clone(),
                            state: PeerState::Disconnected {},
                            last_error: Some(err.to_string()),
                        },
                    });
                    state.sdk.record_peer_changed(
                        &destination_hex_copy,
                        PeerState::Disconnected {},
                        Some(err.to_string().as_str()),
                    );
                }
                let _ = resp.send(result);
            }
            Command::DisconnectPeer {
                destination_hex,
                resp,
            } => {
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    connected_peers.lock().await.remove(&dest);
                    // Clean up any stale link from older builds if present.
                    if let Some(link) = out_links.lock().await.remove(&dest) {
                        link.lock().await.close();
                    }
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex,
                            state: PeerState::Disconnected {},
                            last_error: None,
                        },
                    });
                    state.sdk.record_peer_changed(
                        &address_hash_to_hex(&dest),
                        PeerState::Disconnected {},
                        None,
                    );
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                resp,
            } => {
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    let metadata = fields_bytes
                        .as_deref()
                        .and_then(parse_mission_sync_metadata);
                    let lxmf_report = if fields_bytes.is_some() {
                        #[cfg(feature = "legacy-lxmf-runtime")]
                        {
                            Some(send_lxmf_message(&state, dest, &bytes, fields_bytes.clone()).await?)
                        }
                        #[cfg(not(feature = "legacy-lxmf-runtime"))]
                        {
                            Some(
                                state
                                    .sdk
                                    .send_lxmf(
                                        dest,
                                        &bytes,
                                        None,
                                        fields_bytes.clone(),
                                        metadata.clone(),
                                    )
                                    .await?,
                            )
                        }
                    } else {
                        None
                    };
                    let outcome = if let Some(report) = lxmf_report.as_ref() {
                        report.outcome
                    } else {
                        send_transport_packet_with_path_retry(&transport, dest, &bytes).await
                    };
                    let mapped = send_outcome_to_udl(outcome);
                    bus.emit(NodeEvent::PacketSent {
                        destination_hex: destination_hex.clone(),
                        bytes: bytes.clone(),
                        outcome: mapped,
                    });

                    if let Some(report) = lxmf_report.as_ref() {
                        if let Some(metadata) = report.metadata.as_ref() {
                            if metadata.is_event_related() {
                                info!(
                                    "[lxmf][events] outbound kind={} name={} destination={} message_id={} event_uid={} mission_uid={} correlation={}",
                                    metadata.primary_kind(),
                                    metadata.primary_name().unwrap_or("-"),
                                    report.resolved_destination_hex.as_str(),
                                    report.message_id_hex,
                                    metadata.event_uid.as_deref().unwrap_or("-"),
                                    metadata.mission_uid.as_deref().unwrap_or("-"),
                                    metadata.correlation_id.as_deref().unwrap_or("-"),
                                );
                            }
                        }

                        if let Some(registered) = register_pending_lxmf_delivery(
                            &state,
                            report,
                        )
                        .await
                        {
                            let pending = &registered.pending;
                            if matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                                state.sdk.record_delivery_sent(
                                    &pending.message_id_hex,
                                    &pending.destination_hex,
                                    pending.correlation_id.as_deref(),
                                    pending.command_id.as_deref(),
                                    pending.command_type.as_deref(),
                                    pending.event_uid.as_deref(),
                                    pending.mission_uid.as_deref(),
                                );
                                emit_lxmf_delivery(
                                    &bus,
                                    &pending,
                                    LxmfDeliveryStatus::Sent {},
                                    None,
                                );
                                info!(
                                    "[lxmf][events] sent message_id={} destination={} command={} correlation={}",
                                    pending.message_id_hex,
                                    pending.destination_hex,
                                    pending.command_type.as_deref().unwrap_or("-"),
                                    pending.correlation_id.as_deref().unwrap_or("-"),
                                );
                                if let Some(buffered_ack) = registered.buffered_ack {
                                    let tracking_key = pending
                                        .correlation_id
                                        .as_deref()
                                        .or(pending.command_id.as_deref())
                                        .map(ToOwned::to_owned);
                                    if buffered_ack.source_hex == pending.destination_hex {
                                        if let Some(tracking_key) = tracking_key.as_deref() {
                                            state
                                                .pending_lxmf_deliveries
                                                .lock()
                                                .await
                                                .remove(tracking_key);
                                        }
                                        state.sdk.record_delivery_acknowledged(
                                            &pending.message_id_hex,
                                            &pending.destination_hex,
                                            Some(buffered_ack.source_hex.as_str()),
                                            pending.correlation_id.as_deref(),
                                            pending.command_id.as_deref(),
                                            pending.command_type.as_deref(),
                                            pending.event_uid.as_deref(),
                                            pending.mission_uid.as_deref(),
                                            buffered_ack.detail.as_deref(),
                                        );
                                        emit_lxmf_delivery_with_source(
                                            &bus,
                                            pending,
                                            Some(buffered_ack.source_hex.clone()),
                                            LxmfDeliveryStatus::Acknowledged {},
                                            buffered_ack.detail.clone(),
                                        );
                                        info!(
                                            "[lxmf][events] acknowledged buffered message_id={} destination={} command={} correlation={} detail={}",
                                            pending.message_id_hex,
                                            pending.destination_hex,
                                            pending.command_type.as_deref().unwrap_or("-"),
                                            pending.correlation_id.as_deref().unwrap_or("-"),
                                            buffered_ack.detail.as_deref().unwrap_or("-"),
                                        );
                                    } else {
                                        if let Some(tracking_key) = tracking_key {
                                            state
                                                .pending_lxmf_acknowledgements
                                                .lock()
                                                .await
                                                .insert(tracking_key, buffered_ack.clone());
                                        }
                                        info!(
                                            "[lxmf][events] buffered acknowledgement source mismatch message_id={} destination={} source={}",
                                            pending.message_id_hex,
                                            pending.destination_hex,
                                            buffered_ack.source_hex,
                                        );
                                    }
                                }
                            } else {
                                let failure_detail = format!("{mapped:?}");
                                {
                                    let tracking_key = pending
                                        .correlation_id
                                        .as_deref()
                                        .or(pending.command_id.as_deref())
                                        .map(ToOwned::to_owned);
                                    if let Some(tracking_key) = tracking_key {
                                        state.pending_lxmf_deliveries.lock().await.remove(&tracking_key);
                                    }
                                }
                                state.sdk.record_delivery_failed(
                                    &pending.message_id_hex,
                                    &pending.destination_hex,
                                    pending.correlation_id.as_deref(),
                                    pending.command_id.as_deref(),
                                    pending.command_type.as_deref(),
                                    pending.event_uid.as_deref(),
                                    pending.mission_uid.as_deref(),
                                    Some(failure_detail.as_str()),
                                );
                                emit_lxmf_delivery(
                                    &bus,
                                    &pending,
                                    LxmfDeliveryStatus::Failed {},
                                    Some(failure_detail.clone()),
                                );
                                info!(
                                    "[lxmf][events] failed message_id={} destination={} command={} correlation={} outcome={:?}",
                                    pending.message_id_hex,
                                    pending.destination_hex,
                                    pending.command_type.as_deref().unwrap_or("-"),
                                    pending.correlation_id.as_deref().unwrap_or("-"),
                                    mapped,
                                );
                            }
                        }
                    }

                    if matches!(
                        outcome,
                        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                    ) {
                        Ok(())
                    } else {
                        Err(NodeError::NetworkError {})
                    }
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SendLxmf { request, resp } => {
                let result = async {
                    let destination = parse_address_hash(request.destination_hex.as_str())?;
                    let body_bytes = request.body_utf8.as_bytes().to_vec();
                        let report = state
                          .sdk
                          .send_lxmf(
                              destination,
                              body_bytes.as_slice(),
                              request.title.clone(),
                              None,
                              None,
                          )
                          .await?;
                    let method = if report.used_resource {
                        MessageMethod::Resource {}
                    } else {
                        MessageMethod::Direct {}
                    };
                    let state_value = if matches!(
                        report.outcome,
                        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                    ) {
                        MessageState::SentDirect {}
                    } else {
                        MessageState::Failed {}
                    };
                    let detail = if matches!(state_value, MessageState::Failed {}) {
                        Some(format!("{:?}", send_outcome_to_udl(report.outcome)))
                    } else {
                        None
                    };
                    let conversation_id = conversation_id_for(report.resolved_destination_hex.as_str());
                    let record = MessageRecord {
                        message_id_hex: report.message_id_hex.clone(),
                        conversation_id,
                        direction: MessageDirection::Outbound {},
                        destination_hex: report.resolved_destination_hex.clone(),
                        source_hex: Some(address_hash_to_hex(
                            &state.lxmf_destination.lock().await.desc.address_hash,
                        )),
                        title: request.title.clone(),
                        body_utf8: request.body_utf8.clone(),
                        method,
                        state: state_value,
                        detail: detail.clone(),
                        sent_at_ms: Some(now_ms()),
                        received_at_ms: None,
                        updated_at_ms: now_ms(),
                    };
                    upsert_message_record(&state, &bus, record, false).await;
                    state.messaging.lock().await.store_outbound(sdkmsg::StoredOutboundMessage {
                        request: to_sdk_send_request(&request),
                        message_id_hex: report.message_id_hex.clone(),
                    });
                    if let Some(receipt_hash_hex) = report.receipt_hash_hex.as_ref() {
                        if let Ok(mut guard) = receipt_message_ids.lock() {
                            guard.insert(receipt_hash_hex.clone(), report.message_id_hex.clone());
                        }
                    }
                    Ok::<String, NodeError>(report.message_id_hex)
                }
                .await;
                let _ = resp.send(result);
            }
            Command::RetryLxmf {
                message_id_hex,
                resp,
            } => {
                let result = async {
                    let outbound = state
                        .messaging
                        .lock()
                        .await
                        .outbound(message_id_hex.as_str())
                        .ok_or(NodeError::InvalidConfig {})?;
                    let destination = parse_address_hash(outbound.request.destination_hex.as_str())?;
                    let report = state
                        .sdk
                        .send_lxmf(
                            destination,
                            outbound.request.body_utf8.as_bytes(),
                            outbound.request.title.clone(),
                            None,
                            None,
                        )
                        .await?;
                    let retried = MessageRecord {
                        message_id_hex: report.message_id_hex.clone(),
                        conversation_id: conversation_id_for(
                            report.resolved_destination_hex.as_str(),
                        ),
                        direction: MessageDirection::Outbound {},
                        destination_hex: report.resolved_destination_hex.clone(),
                        source_hex: Some(address_hash_to_hex(
                            &state.lxmf_destination.lock().await.desc.address_hash,
                        )),
                        title: outbound.request.title.clone(),
                        body_utf8: outbound.request.body_utf8.clone(),
                        method: if report.used_resource {
                            MessageMethod::Resource {}
                        } else {
                            MessageMethod::Direct {}
                        },
                        state: MessageState::SentDirect {},
                        detail: Some(format!("retry of {}", outbound.message_id_hex)),
                        sent_at_ms: Some(now_ms()),
                        received_at_ms: None,
                        updated_at_ms: now_ms(),
                    };
                    upsert_message_record(&state, &bus, retried, false).await;
                    state.messaging.lock().await.store_outbound(sdkmsg::StoredOutboundMessage {
                        request: outbound.request,
                        message_id_hex: report.message_id_hex.clone(),
                    });
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::CancelLxmf {
                message_id_hex,
                resp,
            } => {
                let result = async {
                    let updated = state
                        .messaging
                        .lock()
                        .await
                        .update_message(
                            message_id_hex.as_str(),
                            sdkmsg::MessageState::Cancelled,
                            Some("cancelled locally".to_string()),
                            now_ms(),
                        )
                        .map(from_sdk_message_record)
                        .ok_or(NodeError::InvalidConfig {})?;
                    upsert_message_record(&state, &bus, updated, false).await;
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SetActivePropagationNode {
                destination_hex,
                resp,
            } => {
                let status_update = from_sdk_sync_status(
                    state
                        .messaging
                        .lock()
                        .await
                        .set_active_propagation_node(destination_hex),
                );
                bus.emit(NodeEvent::SyncUpdated {
                    status: status_update,
                });
                let _ = resp.send(Ok(()));
            }
            Command::RequestLxmfSync { limit, resp } => {
                let requested_at_ms = now_ms();
                let status_update = from_sdk_sync_status(state.messaging.lock().await.update_sync_status(
                    |status| {
                        status.phase = sdkmsg::SyncPhase::Failed;
                        status.requested_at_ms = Some(requested_at_ms);
                        status.completed_at_ms = Some(now_ms());
                        status.messages_received = 0;
                        status.detail = Some(match limit {
                            Some(value) => format!(
                                "Propagation sync is not implemented in the mobile runtime yet (requested limit {value})."
                            ),
                            None => {
                                "Propagation sync is not implemented in the mobile runtime yet."
                                    .to_string()
                            }
                        });
                    },
                ));
                bus.emit(NodeEvent::SyncUpdated {
                    status: status_update,
                });
                let _ = resp.send(Err(NodeError::InternalError {}));
            }
            Command::ListAnnounces { resp } => {
                let records = state
                    .messaging
                    .lock()
                    .await
                    .list_announces()
                    .into_iter()
                    .map(from_sdk_announce_record)
                    .collect::<Vec<_>>();
                let _ = resp.send(Ok(records));
            }
            Command::ListPeers { resp } => {
                let _ = resp.send(Ok(snapshot_peer_records(&state).await));
            }
            Command::ListConversations { resp } => {
                let _ = resp.send(Ok(conversation_records_snapshot(&state).await));
            }
            Command::ListMessages {
                conversation_id,
                resp,
            } => {
                let _ = resp.send(Ok(
                    message_records_snapshot(&state, conversation_id.as_deref()).await,
                ));
            }
            Command::GetLxmfSyncStatus { resp } => {
                let _ = resp.send(Ok(from_sdk_sync_status(
                    state.messaging.lock().await.sync_status(),
                )));
            }
            Command::BroadcastBytes { bytes, resp } => {
                let result = async {
                    let peers = connected_peers
                        .lock()
                        .await
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let mut sent_any = false;
                    for dest in peers {
                        let outcome =
                            send_transport_packet_with_path_retry(&transport, dest, &bytes).await;
                        bus.emit(NodeEvent::PacketSent {
                            destination_hex: address_hash_to_hex(&dest),
                            bytes: bytes.clone(),
                            outcome: send_outcome_to_udl(outcome),
                        });
                        if matches!(
                            outcome,
                            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                        ) {
                            sent_any = true;
                        }
                    }

                    if sent_any {
                        Ok::<(), NodeError>(())
                    } else {
                        Err(NodeError::NetworkError {})
                    }
                }
                .await;
                let _ = resp.send(result);
            }
            Command::RefreshHubDirectory { resp } => {
                let result = match config.hub_mode {
                    HubMode::Disabled {} => Err(NodeError::InvalidConfig {}),
                    HubMode::RchHttp {} => refresh_hub_directory_http(&config).await,
                    HubMode::RchLxmf {} => refresh_hub_directory_lxmf(&config, &state).await,
                }
                .map(|destinations| {
                    state
                        .sdk
                        .record_hub_directory_updated(destinations.as_slice());
                    bus.emit(NodeEvent::HubDirectoryUpdated {
                        destinations,
                        received_at_ms: now_ms(),
                    });
                });
                let _ = resp.send(result.map(|_| ()));
            }
        }
    }

    let _ = state.sdk.shutdown().await;

    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
}

fn identity_path(storage_dir: &Path) -> PathBuf {
    storage_dir.join("identity.hex")
}

pub fn load_or_create_identity(
    storage_dir: Option<&str>,
    name: &str,
) -> Result<PrivateIdentity, NodeError> {
    let Some(dir) = storage_dir.map(str::trim).filter(|v| !v.is_empty()) else {
        // Deterministic fallback for dev.
        return Ok(PrivateIdentity::new_from_name(name));
    };

    let dir = PathBuf::from(dir);
    fs::create_dir_all(&dir).map_err(|_| NodeError::IoError {})?;
    let path = identity_path(&dir);

    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(|_| NodeError::IoError {})?;
        let hex = raw.trim();
        return PrivateIdentity::new_from_hex_string(hex).map_err(|_| NodeError::IoError {});
    }

    let identity = PrivateIdentity::new_from_rand(OsRng);
    fs::write(&path, identity.to_hex_string()).map_err(|_| NodeError::IoError {})?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mission_sync_metadata_extracts_command_fields() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(LXMF_FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-123"),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("corr-123"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("entry_uid"),
                            MsgPackValue::from("evt-123"),
                        ),
                        (
                            MsgPackValue::from("mission_uid"),
                            MsgPackValue::from("default"),
                        ),
                    ]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let metadata = parse_mission_sync_metadata(&bytes).expect("metadata");

        assert_eq!(metadata.command_id.as_deref(), Some("cmd-123"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("corr-123"));
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.log_entry.upsert")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("evt-123"));
        assert_eq!(metadata.mission_uid.as_deref(), Some("default"));
        assert!(metadata.is_event_related());
    }

    #[test]
    fn parse_mission_sync_metadata_extracts_result_and_event_fields() {
        let fields = MsgPackValue::Map(vec![
            (
                MsgPackValue::from(LXMF_FIELD_RESULTS),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("command_id"),
                        MsgPackValue::from("cmd-123"),
                    ),
                    (
                        MsgPackValue::from("correlation_id"),
                        MsgPackValue::from("corr-123"),
                    ),
                    (MsgPackValue::from("status"), MsgPackValue::from("accepted")),
                ]),
            ),
            (
                MsgPackValue::from(LXMF_FIELD_EVENT),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("event_type"),
                        MsgPackValue::from("mission.registry.log_entry.upserted"),
                    ),
                    (
                        MsgPackValue::from("payload"),
                        MsgPackValue::Map(vec![
                            (
                                MsgPackValue::from("entry_uid"),
                                MsgPackValue::from("evt-123"),
                            ),
                            (
                                MsgPackValue::from("mission_uid"),
                                MsgPackValue::from("default"),
                            ),
                        ]),
                    ),
                ]),
            ),
        ]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let metadata = parse_mission_sync_metadata(&bytes).expect("metadata");

        assert_eq!(metadata.command_id.as_deref(), Some("cmd-123"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("corr-123"));
        assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
        assert_eq!(
            metadata.event_type.as_deref(),
            Some("mission.registry.log_entry.upserted")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("evt-123"));
        assert_eq!(metadata.mission_uid.as_deref(), Some("default"));
        assert!(metadata.is_event_related());
    }

    #[test]
    fn parse_mission_sync_metadata_accepts_full_rch_command_envelope() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(LXMF_FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-123"),
                ),
                (
                    MsgPackValue::from("source"),
                    MsgPackValue::Map(vec![(
                        MsgPackValue::from("rns_identity"),
                        MsgPackValue::from("abcdef0123456789"),
                    )]),
                ),
                (
                    MsgPackValue::from("timestamp"),
                    MsgPackValue::from("2026-03-13T12:00:00Z"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("entry_uid"),
                            MsgPackValue::from("evt-123"),
                        ),
                        (
                            MsgPackValue::from("mission_uid"),
                            MsgPackValue::from("mission-1"),
                        ),
                        (
                            MsgPackValue::from("content"),
                            MsgPackValue::from("Operator note"),
                        ),
                        (
                            MsgPackValue::from("callsign"),
                            MsgPackValue::from("EAGLE-1"),
                        ),
                        (
                            MsgPackValue::from("keywords"),
                            MsgPackValue::Array(vec![MsgPackValue::from("audit")]),
                        ),
                        (
                            MsgPackValue::from("content_hashes"),
                            MsgPackValue::Array(vec![]),
                        ),
                    ]),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("ui-save-42"),
                ),
                (
                    MsgPackValue::from("topics"),
                    MsgPackValue::Array(vec![
                        MsgPackValue::from("mission-1"),
                        MsgPackValue::from("audit"),
                    ]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let metadata = parse_mission_sync_metadata(&bytes).expect("metadata");

        assert_eq!(metadata.command_id.as_deref(), Some("cmd-123"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("ui-save-42"));
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.log_entry.upsert")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("evt-123"));
        assert_eq!(metadata.mission_uid.as_deref(), Some("mission-1"));
        assert!(metadata.is_event_related());
    }
}
