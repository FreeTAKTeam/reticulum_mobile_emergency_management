use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use log::{debug, info};
use lxmf::message::{
    decide_delivery, DeliveryDecision, Message as LxmfMessage, MessageMethod as LxmfRepresentation,
    TransportMethod, WireMessage as LxmfWireMessage,
};
use lxmf_sdk::{
    Ack, CancelResult, Client, ConfigPatch, DeliverySnapshot, DeliveryState, EventBatch,
    EventCursor, LxmfSdk, MessageId, NegotiationRequest, NegotiationResponse, RuntimeSnapshot,
    RuntimeState, SdkBackend, SdkConfig, SdkError, SdkEvent, SendRequest, Severity, ShutdownMode,
    StartRequest,
};
use rand_core::OsRng;
use reticulum::runtime::{ReceivedData, SendPacketOutcome as RnsSendOutcome, Transport};
use reticulum::transport::destination::link::{Link, LinkEvent, LinkStatus};
use reticulum::transport::destination::{
    DestinationDesc, DestinationName, SingleInputDestination, SingleOutputDestination,
};
use reticulum::transport::hash::{address_hash, AddressHash};
use reticulum::transport::identity::{DecryptIdentity, PrivateIdentity};
use reticulum::transport::packet::LXMF_MAX_PAYLOAD;
use reticulum::transport::packet::{
    ContextFlag, DestinationType, Header, HeaderType, IfacFlag, Packet, PacketContext,
    PacketDataBuffer, PacketType, PropagationType,
};
use reticulum::transport::resource::{ResourceEvent, ResourceEventKind};
use serde_json::{json, Value as JsonValue};
use tokio::runtime::Handle;
use tokio::sync::Mutex as TokioMutex;
use x25519_dalek::PublicKey;

use crate::mission_sync::MissionSyncMetadata;
use crate::runtime::{lxmf_private_identity, LxmfSendReport};
use crate::types::{
    HubDirectorySnapshot, LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfFallbackStage,
    NodeError, PeerState, SendMode,
};

const SDK_CAUSE_LXMF_PACKET_TOO_LARGE: &str = "LxmfPacketTooLarge";
const RESOURCE_TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);
const ACCEPTED_RESULT_RESOURCE_TRANSFER_TIMEOUT: Duration = Duration::from_secs(8);
const PROPAGATION_CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
const PROPAGATION_FETCH_CONTROL_TIMEOUT: Duration = Duration::from_secs(90);
const PROPAGATION_FETCH_BATCH_SIZE: usize = 1;
const PROPAGATION_PURGE_BATCH_SIZE: usize = 8;
const PROPAGATION_FETCH_TRANSFER_LIMIT_KB: u64 = 10_240;
const PROPAGATION_STAMP_BYTES: [u8; 32] = [0u8; 32];
const COMPAT_EVENT_RETENTION_LIMIT: usize = 2_048;
const COMPAT_DELIVERY_RETENTION_LIMIT: usize = 1_024;
const COMPAT_SEND_REPORT_RETENTION_LIMIT: usize = 512;

fn sdk_internal(message: impl Into<String>) -> SdkError {
    SdkError::new(
        lxmf_sdk::error_code::INTERNAL,
        lxmf_sdk::ErrorCategory::Internal,
        message,
    )
}

fn sdk_validation(message: impl Into<String>) -> SdkError {
    SdkError::new(
        lxmf_sdk::error_code::VALIDATION_INVALID_ARGUMENT,
        lxmf_sdk::ErrorCategory::Validation,
        message,
    )
    .with_user_actionable(true)
}

fn sdk_transport(message: impl Into<String>) -> SdkError {
    SdkError::new(
        lxmf_sdk::error_code::INTERNAL,
        lxmf_sdk::ErrorCategory::Transport,
        message,
    )
}

fn delivery_method_from_transport(method: TransportMethod) -> LxmfDeliveryMethod {
    match method {
        TransportMethod::Opportunistic => LxmfDeliveryMethod::Opportunistic {},
        TransportMethod::Direct => LxmfDeliveryMethod::Direct {},
        TransportMethod::Propagated => LxmfDeliveryMethod::Propagated {},
        TransportMethod::Paper => LxmfDeliveryMethod::Direct {},
    }
}

fn delivery_representation_from_lxmf(method: LxmfRepresentation) -> LxmfDeliveryRepresentation {
    match method {
        LxmfRepresentation::Packet => LxmfDeliveryRepresentation::Packet {},
        LxmfRepresentation::Resource => LxmfDeliveryRepresentation::Resource {},
        LxmfRepresentation::Paper => LxmfDeliveryRepresentation::Packet {},
        LxmfRepresentation::Unknown => LxmfDeliveryRepresentation::Packet {},
    }
}

fn transport_method_for_send_mode(
    send_mode: SendMode,
    has_cached_direct_link: bool,
    has_delivery_ratchet: bool,
) -> TransportMethod {
    match send_mode {
        SendMode::PropagationOnly {} => TransportMethod::Propagated,
        SendMode::DirectOnly {} => TransportMethod::Direct,
        SendMode::Auto {} => {
            if has_cached_direct_link {
                TransportMethod::Direct
            } else if has_delivery_ratchet {
                TransportMethod::Opportunistic
            } else {
                TransportMethod::Direct
            }
        }
    }
}

fn metadata_is_accepted_result(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.result_present && metadata.result_status.as_deref() == Some("accepted")
    })
}

#[cfg(test)]
fn idempotency_key_for_send_mode(base_key: &str, send_mode: SendMode) -> String {
    idempotency_key_for_send_attempt(base_key, send_mode, None)
}

fn idempotency_key_for_send_attempt(
    base_key: &str,
    send_mode: SendMode,
    direct_attempt: Option<usize>,
) -> String {
    if let Some(attempt) = direct_attempt {
        return match send_mode {
            SendMode::PropagationOnly {} => format!("{base_key}:propagation"),
            SendMode::DirectOnly {} | SendMode::Auto {} => {
                format!("{base_key}:direct-attempt-{attempt}")
            }
        };
    }

    match send_mode {
        SendMode::PropagationOnly {} => format!("{base_key}:propagation"),
        SendMode::DirectOnly {} => format!("{base_key}:direct"),
        SendMode::Auto {} => base_key.to_string(),
    }
}

fn lxmf_identity(identity: &reticulum::transport::identity::Identity) -> lxmf::identity::Identity {
    lxmf::identity::Identity::new_from_slices(
        identity.public_key_bytes(),
        identity.verifying_key_bytes(),
    )
}

fn map_sdk_error_to_node_error(err: SdkError) -> NodeError {
    if err.cause_code.as_deref() == Some(SDK_CAUSE_LXMF_PACKET_TOO_LARGE) {
        return NodeError::LxmfPacketTooLarge {};
    }

    match err.category {
        lxmf_sdk::ErrorCategory::Validation => NodeError::InvalidConfig {},
        lxmf_sdk::ErrorCategory::Transport => NodeError::NetworkError {},
        lxmf_sdk::ErrorCategory::Timeout => NodeError::Timeout {},
        _ => NodeError::InternalError {},
    }
}

fn make_sdk_event(
    runtime_id: &str,
    seq_no: u64,
    event_type: &str,
    severity: Severity,
    payload: JsonValue,
) -> SdkEvent {
    serde_json::from_value(json!({
        "event_id": format!("{runtime_id}-{seq_no}"),
        "runtime_id": runtime_id,
        "stream_id": "reticulum-mobile",
        "seq_no": seq_no,
        "contract_version": 2,
        "ts_ms": crate::runtime::now_ms(),
        "event_type": event_type,
        "severity": severity,
        "source_component": "reticulum_mobile",
        "operation_id": null,
        "message_id": payload.get("message_id").and_then(JsonValue::as_str),
        "peer_id": payload
            .get("destination_hex")
            .or_else(|| payload.get("source_hex"))
            .and_then(JsonValue::as_str),
        "correlation_id": payload.get("correlation_id").and_then(JsonValue::as_str),
        "trace_id": null,
        "payload": payload,
        "extensions": {},
    }))
    .expect("valid sdk event")
}

fn make_delivery_snapshot(
    message_id_hex: &str,
    state: DeliveryState,
    terminal: bool,
    attempts: u32,
    reason_code: Option<String>,
) -> DeliverySnapshot {
    serde_json::from_value(json!({
        "message_id": message_id_hex,
        "state": state,
        "terminal": terminal,
        "last_updated_ms": crate::runtime::now_ms(),
        "attempts": attempts,
        "reason_code": reason_code,
    }))
    .expect("valid delivery snapshot")
}

fn make_negotiation_response(runtime_id: String) -> NegotiationResponse {
    serde_json::from_value(json!({
        "runtime_id": runtime_id,
        "active_contract_version": 2,
        "effective_capabilities": [
            "sdk.capability.event_stream",
            "sdk.capability.cursor_replay",
            "sdk.capability.receipt_terminality",
            "sdk.capability.config_revision_cas",
            "sdk.capability.idempotency_ttl",
            "reticulum.capability.raw_bytes",
            "reticulum.capability.msgpack_fields"
        ],
        "effective_limits": {
            "max_poll_events": 128,
            "max_event_bytes": 65536,
            "max_batch_bytes": 1048576,
            "max_extension_keys": 16,
            "idempotency_ttl_ms": 43200000
        },
        "contract_release": "v2.5",
        "schema_namespace": "v2"
    }))
    .expect("valid negotiation response")
}

fn make_ack(revision: Option<u64>) -> Ack {
    serde_json::from_value(json!({
        "accepted": true,
        "revision": revision,
    }))
    .expect("valid ack")
}

fn make_event_batch(
    events: Vec<SdkEvent>,
    next_cursor: EventCursor,
    high_watermark: u64,
) -> EventBatch {
    serde_json::from_value(json!({
        "events": events,
        "next_cursor": next_cursor.0,
        "dropped_count": 0,
        "snapshot_high_watermark_seq_no": high_watermark,
        "extensions": {},
    }))
    .expect("valid event batch")
}

fn make_runtime_snapshot(
    runtime_id: &str,
    config_revision: u64,
    event_stream_position: u64,
    queued_messages: u64,
    in_flight_messages: u64,
) -> RuntimeSnapshot {
    serde_json::from_value(json!({
        "runtime_id": runtime_id,
        "state": RuntimeState::Running,
        "active_contract_version": 2,
        "event_stream_position": event_stream_position,
        "config_revision": config_revision,
        "queued_messages": queued_messages,
        "in_flight_messages": in_flight_messages,
    }))
    .expect("valid runtime snapshot")
}

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const LXMF_PROPAGATION_NAME: (&str, &str) = ("lxmf", "propagation");
const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const ACCEPTED_RESULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_LINK_CONNECT_ATTEMPTS: usize = 3;
const ACCEPTED_RESULT_LINK_CONNECT_ATTEMPTS: usize = 1;
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);

const EXT_FIELDS_BASE64: &str = "reticulum.fields_base64";
const EXT_RAW_BYTES_BASE64: &str = "reticulum.raw_bytes_base64";
const EXT_SEND_MODE: &str = "reticulum.send_mode";
const EXT_USE_PROPAGATION_NODE: &str = "reticulum.use_propagation_node";
const EXT_ACCEPTED_RESULT_ACK: &str = "reticulum.accepted_result_ack";
const EVENT_PACKET_RECEIVED: &str = "reticulum.packet_received";
const EVENT_ANNOUNCE_RECEIVED: &str = "reticulum.announce_received";
const EVENT_PEER_CHANGED: &str = "reticulum.peer_changed";
const EVENT_HUB_DIRECTORY_UPDATED: &str = "reticulum.hub_directory_updated";
const EVENT_DELIVERY_UPDATED: &str = "reticulum.delivery_updated";

#[derive(Clone)]
pub(crate) struct SdkTransportState {
    pub(crate) identity: PrivateIdentity,
    pub(crate) transport: Arc<Transport>,
    pub(crate) lxmf_destination: Arc<TokioMutex<SingleInputDestination>>,
    pub(crate) known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    pub(crate) out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<Link>>>>>,
    pub(crate) active_propagation_node_hex: Arc<TokioMutex<Option<String>>>,
    pub(crate) ratchet_store_path: Option<PathBuf>,
}

struct CompatBackendState {
    runtime_id: String,
    config_revision: u64,
    events: VecDeque<SdkEvent>,
    deliveries: HashMap<String, DeliverySnapshot>,
    send_reports: HashMap<String, CompatSendReport>,
    send_report_order: VecDeque<String>,
}

fn has_delivery_ratchet(state: &SdkTransportState, destination: &AddressHash) -> bool {
    state
        .ratchet_store_path
        .as_ref()
        .map(|path| path.join(destination.to_hex_string()))
        .is_some_and(|path| path.is_file())
}

impl CompatBackendState {
    fn new(runtime_id: String) -> Self {
        Self {
            runtime_id,
            config_revision: 1,
            events: VecDeque::new(),
            deliveries: HashMap::new(),
            send_reports: HashMap::new(),
            send_report_order: VecDeque::new(),
        }
    }

    fn last_seq_no(&self) -> u64 {
        self.events.back().map(|event| event.seq_no).unwrap_or(0)
    }

    fn next_seq_no(&self) -> u64 {
        self.last_seq_no() + 1
    }

    fn push_event(&mut self, event_type: &str, severity: Severity, payload: JsonValue) {
        let seq_no = self.next_seq_no();
        self.events.push_back(make_sdk_event(
            &self.runtime_id,
            seq_no,
            event_type,
            severity,
            payload,
        ));
        while self.events.len() > COMPAT_EVENT_RETENTION_LIMIT {
            self.events.pop_front();
        }
    }

    fn update_delivery(
        &mut self,
        message_id_hex: &str,
        state: DeliveryState,
        reason_code: Option<String>,
    ) {
        let terminal = matches!(
            state,
            DeliveryState::Delivered
                | DeliveryState::Failed
                | DeliveryState::Cancelled
                | DeliveryState::Expired
                | DeliveryState::Rejected
                | DeliveryState::Unknown
        );
        let attempts = self
            .deliveries
            .get(message_id_hex)
            .map(|snapshot| snapshot.attempts)
            .unwrap_or(0)
            + 1;
        self.deliveries.insert(
            message_id_hex.to_string(),
            make_delivery_snapshot(message_id_hex, state, terminal, attempts, reason_code),
        );
        self.prune_deliveries();
    }

    fn record_send_report(&mut self, report: CompatSendReport) {
        let message_id_hex = report.message_id_hex.clone();
        self.send_reports.insert(message_id_hex.clone(), report);
        self.send_report_order
            .retain(|value| value != &message_id_hex);
        self.send_report_order.push_back(message_id_hex);
        while self.send_report_order.len() > COMPAT_SEND_REPORT_RETENTION_LIMIT {
            if let Some(evicted) = self.send_report_order.pop_front() {
                self.send_reports.remove(&evicted);
            }
        }
    }

    fn prune_deliveries(&mut self) {
        if self.deliveries.len() <= COMPAT_DELIVERY_RETENTION_LIMIT {
            return;
        }

        let mut terminal = self
            .deliveries
            .iter()
            .filter_map(|(message_id_hex, snapshot)| {
                snapshot
                    .terminal
                    .then_some((message_id_hex.clone(), snapshot.last_updated_ms))
            })
            .collect::<Vec<_>>();
        terminal.sort_by_key(|(_, updated_at_ms)| *updated_at_ms);

        for (message_id_hex, _) in terminal {
            if self.deliveries.len() <= COMPAT_DELIVERY_RETENTION_LIMIT {
                break;
            }
            self.deliveries.remove(&message_id_hex);
        }

        if self.deliveries.len() <= COMPAT_DELIVERY_RETENTION_LIMIT {
            return;
        }

        let mut oldest = self
            .deliveries
            .iter()
            .map(|(message_id_hex, snapshot)| (message_id_hex.clone(), snapshot.last_updated_ms))
            .collect::<Vec<_>>();
        oldest.sort_by_key(|(_, updated_at_ms)| *updated_at_ms);
        for (message_id_hex, _) in oldest {
            if self.deliveries.len() <= COMPAT_DELIVERY_RETENTION_LIMIT {
                break;
            }
            self.deliveries.remove(&message_id_hex);
        }
    }
}

#[derive(Clone)]
struct CompatBackend {
    handle: Option<Handle>,
    transport: Option<SdkTransportState>,
    state: Arc<StdMutex<CompatBackendState>>,
}

impl CompatBackend {
    fn new(runtime_id: String, handle: Handle, transport: SdkTransportState) -> Self {
        Self {
            handle: Some(handle),
            transport: Some(transport),
            state: Arc::new(StdMutex::new(CompatBackendState::new(runtime_id))),
        }
    }

    #[cfg(test)]
    fn new_for_tests(runtime_id: &str) -> Self {
        Self {
            handle: None,
            transport: None,
            state: Arc::new(StdMutex::new(CompatBackendState::new(
                runtime_id.to_string(),
            ))),
        }
    }

    fn send_report(&self, message_id_hex: &str) -> Option<CompatSendReport> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.send_reports.get(message_id_hex).cloned())
    }

    fn record_packet_received(
        &self,
        destination_hex: &str,
        source_hex: Option<&str>,
        bytes: &[u8],
        fields_bytes: Option<&[u8]>,
    ) {
        let payload = json!({
            "destination_hex": destination_hex,
            "source_hex": source_hex,
            "bytes_base64": BASE64_STANDARD.encode(bytes),
            "fields_base64": fields_bytes.map(|value| BASE64_STANDARD.encode(value)),
        });
        if let Ok(mut state) = self.state.lock() {
            state.push_event(EVENT_PACKET_RECEIVED, Severity::Info, payload);
        }
    }

    fn record_announce_received(
        &self,
        destination_hex: &str,
        identity_hex: &str,
        destination_kind: &str,
        app_data: &str,
        hops: u8,
        interface_hex: &str,
    ) {
        let payload = json!({
            "destination_hex": destination_hex,
            "identity_hex": identity_hex,
            "destination_kind": destination_kind,
            "app_data": app_data,
            "hops": hops,
            "interface_hex": interface_hex,
        });
        if let Ok(mut state) = self.state.lock() {
            state.push_event(EVENT_ANNOUNCE_RECEIVED, Severity::Info, payload);
        }
    }

    fn record_peer_changed(
        &self,
        destination_hex: &str,
        state_name: &str,
        last_error: Option<&str>,
    ) {
        let payload = json!({
            "destination_hex": destination_hex,
            "state": state_name,
            "last_error": last_error,
        });
        if let Ok(mut state) = self.state.lock() {
            state.push_event(EVENT_PEER_CHANGED, Severity::Info, payload);
        }
    }

    fn record_hub_directory_updated(&self, snapshot: &HubDirectorySnapshot) {
        let payload = json!({
            "effective_connected_mode": snapshot.effective_connected_mode,
            "items": snapshot.items.iter().map(|item| json!({
                "identity": item.identity,
                "destination_hash": item.destination_hash,
                "display_name": item.display_name,
                "announce_capabilities": item.announce_capabilities,
                "client_type": item.client_type,
                "registered_mode": item.registered_mode,
                "last_seen": item.last_seen,
                "status": item.status,
            })).collect::<Vec<_>>(),
            "received_at_ms": snapshot.received_at_ms,
        });
        if let Ok(mut state) = self.state.lock() {
            state.push_event(EVENT_HUB_DIRECTORY_UPDATED, Severity::Info, payload);
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery events preserve separate routing and mission correlation fields"
    )]
    fn record_delivery_update(
        &self,
        message_id_hex: &str,
        delivery_state: DeliveryState,
        destination_hex: &str,
        source_hex: Option<&str>,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        let reason_code = detail.map(ToOwned::to_owned);
        if let Ok(mut state) = self.state.lock() {
            state.update_delivery(message_id_hex, delivery_state.clone(), reason_code.clone());
            state.push_event(
                EVENT_DELIVERY_UPDATED,
                match delivery_state {
                    DeliveryState::Failed | DeliveryState::Rejected | DeliveryState::Expired => {
                        Severity::Warn
                    }
                    _ => Severity::Info,
                },
                json!({
                    "message_id": message_id_hex,
                    "destination_hex": destination_hex,
                    "source_hex": source_hex,
                    "correlation_id": correlation_id,
                    "command_id": command_id,
                    "command_type": command_type,
                    "event_uid": event_uid,
                    "mission_uid": mission_uid,
                    "status": format!("{delivery_state:?}"),
                    "detail": detail,
                }),
            );
        }
    }
}

impl SdkBackend for CompatBackend {
    fn negotiate(&self, _req: NegotiationRequest) -> Result<NegotiationResponse, SdkError> {
        let runtime_id = self
            .state
            .lock()
            .map(|state| state.runtime_id.clone())
            .unwrap_or_else(|_| "reticulum-mobile".to_string());
        Ok(make_negotiation_response(runtime_id))
    }

    fn send(&self, req: SendRequest) -> Result<MessageId, SdkError> {
        let Some(handle) = self.handle.clone() else {
            return Err(sdk_internal("compat backend missing runtime handle"));
        };
        let Some(transport) = self.transport.clone() else {
            return Err(sdk_internal("compat backend missing transport state"));
        };
        let report = handle.block_on(async move { compat_send_lxmf(transport, &req).await })?;
        if let Ok(mut state) = self.state.lock() {
            let delivery_state = match report.outcome {
                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast => DeliveryState::Sent,
                _ => DeliveryState::Failed,
            };
            let reason_code = match report.outcome {
                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast => None,
                _ => Some(format!("{:?}", report.outcome)),
            };
            state.update_delivery(&report.message_id_hex, delivery_state, reason_code);
            state.record_send_report(report.clone());
        }
        Ok(MessageId(report.message_id_hex))
    }

    fn cancel(&self, _id: MessageId) -> Result<CancelResult, SdkError> {
        Ok(CancelResult::Unsupported)
    }

    fn status(&self, id: MessageId) -> Result<Option<DeliverySnapshot>, SdkError> {
        Ok(self
            .state
            .lock()
            .ok()
            .and_then(|state| state.deliveries.get(id.0.as_str()).cloned()))
    }

    fn configure(&self, _expected_revision: u64, _patch: ConfigPatch) -> Result<Ack, SdkError> {
        let revision = self
            .state
            .lock()
            .map(|mut state| {
                state.config_revision += 1;
                state.config_revision
            })
            .unwrap_or(1);
        Ok(make_ack(Some(revision)))
    }

    fn poll_events(&self, cursor: Option<EventCursor>, max: usize) -> Result<EventBatch, SdkError> {
        let cursor_seq = cursor
            .as_ref()
            .and_then(|value| value.0.parse::<u64>().ok())
            .unwrap_or(0);
        let state = self
            .state
            .lock()
            .map_err(|_| sdk_internal("compat backend event queue poisoned"))?;
        let events = state
            .events
            .iter()
            .filter(|event| event.seq_no > cursor_seq)
            .take(max)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = events
            .last()
            .map(|event| EventCursor(event.seq_no.to_string()))
            .unwrap_or_else(|| EventCursor(state.last_seq_no().to_string()));
        Ok(make_event_batch(events, next_cursor, state.last_seq_no()))
    }

    fn snapshot(&self) -> Result<RuntimeSnapshot, SdkError> {
        let state = self
            .state
            .lock()
            .map_err(|_| sdk_internal("compat backend snapshot poisoned"))?;
        let queued_messages = state
            .deliveries
            .values()
            .filter(|snapshot| {
                matches!(
                    snapshot.state,
                    DeliveryState::Queued | DeliveryState::Dispatching | DeliveryState::InFlight
                )
            })
            .count();
        let in_flight_messages = state
            .deliveries
            .values()
            .filter(|snapshot| !snapshot.terminal)
            .count();
        Ok(make_runtime_snapshot(
            &state.runtime_id,
            state.config_revision,
            state.last_seq_no(),
            queued_messages as u64,
            in_flight_messages as u64,
        ))
    }

    fn shutdown(&self, _mode: ShutdownMode) -> Result<Ack, SdkError> {
        Ok(make_ack(None))
    }
}

#[derive(Clone)]
struct CompatSendReport {
    outcome: RnsSendOutcome,
    message_id_hex: String,
    resolved_destination_hex: String,
    used_propagation_node: bool,
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    relay_destination_hex: Option<String>,
    fallback_stage: Option<LxmfFallbackStage>,
    receipt_hash_hex: Option<String>,
}

pub(crate) struct PropagationFetchResult {
    pub(crate) destination_hex: String,
    pub(crate) available_count: usize,
    pub(crate) fetched_count: usize,
    pub(crate) fetched_entry_count: usize,
    pub(crate) extracted_payload_count: usize,
    pub(crate) imported_wires: Vec<Vec<u8>>,
    pub(crate) failed_count: usize,
    pub(crate) malformed_count: usize,
    pub(crate) decrypt_failed_count: usize,
}

pub(crate) struct RuntimeLxmfSdk {
    client: Arc<Client<CompatBackend>>,
}

impl RuntimeLxmfSdk {
    pub(crate) fn new(runtime_id: String, transport: SdkTransportState) -> Self {
        let backend = CompatBackend::new(runtime_id, Handle::current(), transport);
        Self {
            client: Arc::new(Client::new(backend)),
        }
    }

    pub(crate) async fn start(&self) -> Result<(), NodeError> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            let mut config = SdkConfig::desktop_local_default();
            config.rpc_backend = None;
            client
                .start(
                    StartRequest::new(config)
                        .with_requested_capability("reticulum.capability.raw_bytes")
                        .with_requested_capability("reticulum.capability.msgpack_fields"),
                )
                .map(|_| ())
        })
        .await
        .map_err(|_| NodeError::InternalError {})?
        .map_err(|_| NodeError::InternalError {})
    }

    pub(crate) async fn shutdown(&self) -> Result<(), NodeError> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || client.shutdown(ShutdownMode::Graceful).map(|_| ()))
            .await
            .map_err(|_| NodeError::InternalError {})?
            .map_err(|_| NodeError::InternalError {})
    }

    pub(crate) async fn send_lxmf(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<String>,
        fields_bytes: Option<Vec<u8>>,
        metadata: Option<MissionSyncMetadata>,
        send_mode: SendMode,
    ) -> Result<LxmfSendReport, NodeError> {
        self.send_lxmf_with_direct_attempt(
            destination,
            content,
            title,
            fields_bytes,
            metadata,
            send_mode,
            None,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "send boundary keeps payload, metadata, mode, and retry identity explicit"
    )]
    pub(crate) async fn send_lxmf_with_direct_attempt(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<String>,
        fields_bytes: Option<Vec<u8>>,
        metadata: Option<MissionSyncMetadata>,
        send_mode: SendMode,
        direct_attempt: Option<usize>,
    ) -> Result<LxmfSendReport, NodeError> {
        let source = self
            .client
            .backend()
            .transport
            .as_ref()
            .ok_or(NodeError::InternalError {})?
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .to_hex_string();
        let requested_destination_hex = destination.to_hex_string();
        let mut request = SendRequest::new(
            source,
            requested_destination_hex.clone(),
            json!({
                "encoding": "base64",
                "title": title.clone().unwrap_or_default(),
                "content_base64": BASE64_STANDARD.encode(content),
            }),
        )
        .with_extension(EXT_RAW_BYTES_BASE64, json!(BASE64_STANDARD.encode(content)));
        if let Some(fields_bytes) = fields_bytes.as_ref() {
            request = request.with_extension(
                EXT_FIELDS_BASE64,
                json!(BASE64_STANDARD.encode(fields_bytes)),
            );
        }
        request = request.with_extension(
            EXT_SEND_MODE,
            json!(match send_mode {
                SendMode::Auto {} => "Auto",
                SendMode::DirectOnly {} => "DirectOnly",
                SendMode::PropagationOnly {} => "PropagationOnly",
            }),
        );
        if matches!(send_mode, SendMode::PropagationOnly {}) {
            request = request.with_extension(EXT_USE_PROPAGATION_NODE, json!(true));
        }
        if metadata_is_accepted_result(metadata.as_ref()) {
            request = request.with_extension(EXT_ACCEPTED_RESULT_ACK, json!(true));
        }
        if let Some(correlation_id) = metadata
            .as_ref()
            .and_then(|value| value.correlation_id.clone())
        {
            request = request.with_correlation_id(correlation_id);
        }
        if let Some(idempotency_key) = metadata
            .as_ref()
            .and_then(|value| value.tracking_key().map(ToOwned::to_owned))
        {
            request = request.with_idempotency_key(idempotency_key_for_send_attempt(
                &idempotency_key,
                send_mode,
                direct_attempt,
            ));
        }

        let client = self.client.clone();
        let message_id = tokio::task::spawn_blocking(move || client.send(request))
            .await
            .map_err(|_| NodeError::InternalError {})?
            .map_err(map_sdk_error_to_node_error)?;
        let report = self
            .client
            .backend()
            .send_report(message_id.0.as_str())
            .ok_or(NodeError::InternalError {})?;

        if let Some(metadata) = metadata.as_ref().filter(|value| value.is_event_related()) {
            info!(
                "[lxmf][events][sdk] attempting send requested_destination={} resolved_destination={} kind={} name={} message_id={} event_uid={} mission_uid={} correlation={}",
                requested_destination_hex,
                report.resolved_destination_hex,
                metadata.primary_kind(),
                metadata.primary_name().unwrap_or("-"),
                report.message_id_hex,
                metadata.event_uid.as_deref().unwrap_or("-"),
                metadata.mission_uid.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
            );
        }

        let track_delivery_timeout = metadata
            .as_ref()
            .is_some_and(|value| value.command_present && value.tracking_key().is_some());

        Ok(LxmfSendReport {
            outcome: report.outcome,
            message_id_hex: report.message_id_hex,
            resolved_destination_hex: report.resolved_destination_hex,
            metadata,
            track_delivery_timeout,
            used_propagation_node: report.used_propagation_node,
            method: report.method,
            representation: report.representation,
            relay_destination_hex: report.relay_destination_hex,
            fallback_stage: report.fallback_stage,
            receipt_hash_hex: report.receipt_hash_hex,
        })
    }

    pub(crate) async fn fetch_propagated_lxmf_from_relay(
        &self,
        relay_hex: &str,
        limit: Option<u32>,
        direct_iface_hex: Option<&str>,
    ) -> Result<PropagationFetchResult, NodeError> {
        let state = self
            .client
            .backend()
            .transport
            .as_ref()
            .ok_or(NodeError::InternalError {})?;
        let relay_hex = relay_hex.trim();
        if relay_hex.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        compat_fetch_propagated_lxmf(state, relay_hex, limit, direct_iface_hex).await
    }

    pub(crate) fn record_packet_received(
        &self,
        destination_hex: &str,
        source_hex: Option<&str>,
        bytes: &[u8],
        fields_bytes: Option<&[u8]>,
    ) {
        self.client.backend().record_packet_received(
            destination_hex,
            source_hex,
            bytes,
            fields_bytes,
        );
    }

    pub(crate) fn record_announce_received(
        &self,
        destination_hex: &str,
        identity_hex: &str,
        destination_kind: &str,
        app_data: &str,
        hops: u8,
        interface_hex: &str,
    ) {
        self.client.backend().record_announce_received(
            destination_hex,
            identity_hex,
            destination_kind,
            app_data,
            hops,
            interface_hex,
        );
    }

    pub(crate) fn record_peer_changed(
        &self,
        destination_hex: &str,
        state: PeerState,
        last_error: Option<&str>,
    ) {
        let state_name = match state {
            PeerState::Connecting {} => "connecting",
            PeerState::Connected {} => "connected",
            PeerState::Disconnected {} => "disconnected",
        };
        self.client
            .backend()
            .record_peer_changed(destination_hex, state_name, last_error);
    }

    pub(crate) fn record_hub_directory_updated(&self, snapshot: &HubDirectorySnapshot) {
        self.client.backend().record_hub_directory_updated(snapshot);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_sent(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
    ) {
        self.client.backend().record_delivery_update(
            message_id_hex,
            DeliveryState::Sent,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            None,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps source and mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_acknowledged(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        source_hex: Option<&str>,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.client.backend().record_delivery_update(
            message_id_hex,
            DeliveryState::Delivered,
            destination_hex,
            source_hex,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_failed(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.client.backend().record_delivery_update(
            message_id_hex,
            DeliveryState::Failed,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_timed_out(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.client.backend().record_delivery_update(
            message_id_hex,
            DeliveryState::Expired,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }
}

async fn compat_send_lxmf(
    state: SdkTransportState,
    req: &SendRequest,
) -> Result<CompatSendReport, SdkError> {
    let destination = parse_address_hash(req.destination.as_str())
        .map_err(|_| sdk_validation("invalid destination hash"))?;
    let content_base64 = req
        .extensions
        .get(EXT_RAW_BYTES_BASE64)
        .and_then(JsonValue::as_str)
        .or_else(|| {
            req.payload
                .get("content_base64")
                .and_then(JsonValue::as_str)
        })
        .ok_or_else(|| sdk_validation("missing raw payload"))?;
    let content = BASE64_STANDARD
        .decode(content_base64)
        .map_err(|_| sdk_validation("invalid payload base64"))?;
    let fields_bytes = req
        .extensions
        .get(EXT_FIELDS_BASE64)
        .and_then(JsonValue::as_str)
        .map(|value| {
            BASE64_STANDARD
                .decode(value)
                .map_err(|_| sdk_validation("invalid fields base64"))
        })
        .transpose()?;
    let use_propagation_node = req
        .extensions
        .get(EXT_USE_PROPAGATION_NODE)
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let send_mode = if use_propagation_node {
        SendMode::PropagationOnly {}
    } else {
        match req
            .extensions
            .get(EXT_SEND_MODE)
            .and_then(JsonValue::as_str)
            .unwrap_or("Auto")
        {
            "DirectOnly" => SendMode::DirectOnly {},
            "PropagationOnly" => SendMode::PropagationOnly {},
            _ => SendMode::Auto {},
        }
    };
    let is_accepted_result_ack = req
        .extensions
        .get(EXT_ACCEPTED_RESULT_ACK)
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let link_connect_timeout = if is_accepted_result_ack {
        ACCEPTED_RESULT_LINK_CONNECT_TIMEOUT
    } else {
        DEFAULT_LINK_CONNECT_TIMEOUT
    };
    let link_connect_attempts = if is_accepted_result_ack {
        ACCEPTED_RESULT_LINK_CONNECT_ATTEMPTS
    } else {
        DEFAULT_LINK_CONNECT_ATTEMPTS
    };
    let resource_transfer_timeout = if is_accepted_result_ack {
        ACCEPTED_RESULT_RESOURCE_TRANSFER_TIMEOUT
    } else {
        RESOURCE_TRANSFER_TIMEOUT
    };

    let remote_desc = resolve_lxmf_destination_desc(&state, destination)
        .await
        .map_err(|_| sdk_transport("failed to resolve destination"))?;
    let requested_destination_hex = destination.to_hex_string();
    let resolved_destination_hex = remote_desc.address_hash.to_hex_string();

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
    message.set_content_from_bytes(content.as_slice());
    message.fields = match fields_bytes.as_ref() {
        Some(bytes) => Some(
            rmp_serde::from_slice(bytes).map_err(|_| sdk_validation("invalid msgpack fields"))?,
        ),
        None => None,
    };

    let signer =
        lxmf_private_identity(&state.identity).map_err(|_| sdk_internal("invalid signer"))?;
    let wire = message
        .to_wire(Some(&signer))
        .map_err(|_| sdk_internal("failed to encode lxmf wire message"))?;
    debug!(
        "[lxmf][debug][sdk] compat_send_lxmf wire ready requested_destination={} resolved_destination={} content_bytes={} fields_bytes={} wire_bytes={} max_wire_bytes={}",
        requested_destination_hex,
        resolved_destination_hex,
        content.len(),
        fields_bytes.as_ref().map(Vec::len).unwrap_or(0),
        wire.len(),
        LXMF_MAX_PAYLOAD,
    );
    let message_id_hex = LxmfWireMessage::unpack(&wire)
        .map(|wire| hex::encode(wire.message_id()))
        .map_err(|_| sdk_internal("failed to unpack lxmf message id"))?;

    let cached_link = state
        .out_links
        .lock()
        .await
        .get(&remote_desc.address_hash)
        .cloned();
    let has_cached_direct_link = if let Some(link) = cached_link {
        matches!(link.lock().await.status(), LinkStatus::Active)
    } else {
        false
    };
    let desired_method = if is_accepted_result_ack {
        TransportMethod::Opportunistic
    } else {
        transport_method_for_send_mode(
            send_mode,
            has_cached_direct_link,
            has_delivery_ratchet(&state, &remote_desc.address_hash),
        )
    };
    let DeliveryDecision {
        method,
        representation,
    } = decide_delivery(desired_method, false, wire.len()).map_err(|err| {
        sdk_validation(format!(
            "failed to choose lxmf delivery representation: {err}"
        ))
    })?;
    let method_value = delivery_method_from_transport(method);
    let representation_value = delivery_representation_from_lxmf(representation);

    if matches!(method, TransportMethod::Propagated) {
        return compat_send_lxmf_via_propagation(
            &state,
            &remote_desc,
            wire.as_slice(),
            requested_destination_hex.as_str(),
            resolved_destination_hex.as_str(),
            message_id_hex.as_str(),
            method_value,
            representation_value,
            None,
        )
        .await;
    }

    if matches!(method, TransportMethod::Opportunistic) {
        let packet = Packet {
            header: Header {
                ifac_flag: IfacFlag::Open,
                header_type: HeaderType::Type1,
                context_flag: ContextFlag::Unset,
                propagation_type: PropagationType::Transport,
                destination_type: DestinationType::Single,
                packet_type: PacketType::Data,
                hops: 0,
            },
            ifac: None,
            destination: remote_desc.address_hash,
            transport: None,
            context: PacketContext::None,
            data: PacketDataBuffer::new_from_slice(&wire),
        };
        let receipt_hash_hex = hex::encode(packet.hash().to_bytes());
        info!(
            "[lxmf][events][sdk] path=opportunistic representation=packet requested_destination={} resolved_destination={} message_id={} wire_bytes={} max_wire_bytes={}",
            requested_destination_hex,
            resolved_destination_hex,
            message_id_hex,
            wire.len(),
            LXMF_MAX_PAYLOAD,
        );
        let outcome = state.transport.send_packet_with_outcome(packet).await;
        return Ok(CompatSendReport {
            outcome,
            message_id_hex,
            resolved_destination_hex,
            used_propagation_node: false,
            method: method_value,
            representation: representation_value,
            relay_destination_hex: None,
            fallback_stage: None,
            receipt_hash_hex: Some(receipt_hash_hex),
        });
    }

    let remote_destination_hash = remote_desc.address_hash;
    let link = ensure_lxmf_output_link(
        &state,
        remote_desc,
        Some(requested_destination_hex.as_str()),
        Some(resolved_destination_hex.as_str()),
        link_connect_timeout,
        link_connect_attempts,
    )
    .await
    .map_err(|_| sdk_transport("failed to activate lxmf link"))?;
    let link_id = *link.lock().await.id();
    if matches!(representation, LxmfRepresentation::Resource) {
        let mut resource_events = state.transport.resource_events();
        let resource_hash = match state
            .transport
            .send_resource(&link_id, wire.clone(), None)
            .await
        {
            Ok(hash) => hash,
            Err(_) => {
                clear_lxmf_output_link(&state, &remote_destination_hash).await;
                return Err(sdk_transport("failed to start lxmf resource transfer"));
            }
        };
        let resource_hash_hex = hex::encode(resource_hash.as_slice());
        info!(
            "[lxmf][events][sdk] path=direct representation=resource requested_destination={} resolved_destination={} message_id={} resource_hash={} wire_bytes={} max_wire_bytes={}",
            requested_destination_hex,
            resolved_destination_hex,
            message_id_hex,
            resource_hash_hex,
            wire.len(),
            LXMF_MAX_PAYLOAD,
        );
        let deadline = tokio::time::Instant::now() + resource_transfer_timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                clear_lxmf_output_link(&state, &remote_destination_hash).await;
                return Err(sdk_transport("lxmf resource transfer timed out"));
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, resource_events.recv()).await {
                Ok(Ok(event)) => {
                    if event.hash != resource_hash {
                        continue;
                    }
                    match event.kind {
                        ResourceEventKind::Progress(progress) => {
                            debug!(
                                "[lxmf][debug][sdk] resource fallback progress requested_destination={} resolved_destination={} message_id={} resource_hash={} received_bytes={} total_bytes={} received_parts={} total_parts={}",
                                requested_destination_hex,
                                resolved_destination_hex,
                                message_id_hex,
                                resource_hash_hex,
                                progress.received_bytes,
                                progress.total_bytes,
                                progress.received_parts,
                                progress.total_parts,
                            );
                        }
                        ResourceEventKind::OutboundComplete => {
                            info!(
                                "[lxmf][events][sdk] path=direct representation=resource complete requested_destination={} resolved_destination={} message_id={} resource_hash={}",
                                requested_destination_hex,
                                resolved_destination_hex,
                                message_id_hex,
                                resource_hash_hex,
                            );
                            if is_accepted_result_ack {
                                clear_lxmf_output_link(&state, &remote_destination_hash).await;
                            }
                            return Ok(CompatSendReport {
                                outcome: RnsSendOutcome::SentDirect,
                                message_id_hex,
                                resolved_destination_hex,
                                used_propagation_node: false,
                                method: method_value,
                                representation: representation_value,
                                relay_destination_hex: None,
                                fallback_stage: None,
                                receipt_hash_hex: None,
                            });
                        }
                        ResourceEventKind::Complete(_) => {}
                        ResourceEventKind::InboundFailed(failure) => {
                            return Err(sdk_transport(format!(
                                "lxmf resource inbound transfer failed: {}",
                                failure.reason
                            )));
                        }
                        ResourceEventKind::OutboundFailed => {
                            clear_lxmf_output_link(&state, &remote_destination_hash).await;
                            return Err(sdk_transport("lxmf resource transfer failed"));
                        }
                        ResourceEventKind::OutboundCancelled => {
                            clear_lxmf_output_link(&state, &remote_destination_hash).await;
                            return Err(sdk_transport("lxmf resource transfer cancelled"));
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    clear_lxmf_output_link(&state, &remote_destination_hash).await;
                    return Err(sdk_transport("resource event stream closed"));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Err(_) => {
                    clear_lxmf_output_link(&state, &remote_destination_hash).await;
                    return Err(sdk_transport("lxmf resource transfer timed out"));
                }
            }
        }
    }
    info!(
        "[lxmf][events][sdk] path=direct representation=packet requested_destination={} resolved_destination={} message_id={} wire_bytes={} max_wire_bytes={}",
        requested_destination_hex,
        resolved_destination_hex,
        message_id_hex,
        wire.len(),
        LXMF_MAX_PAYLOAD,
    );
    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|_| sdk_internal("failed to create transport packet"))?;
    let receipt_hash_hex = hex::encode(packet.hash().to_bytes());
    let outcome = state.transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        clear_lxmf_output_link(&state, &remote_destination_hash).await;
    }

    Ok(CompatSendReport {
        outcome,
        message_id_hex,
        resolved_destination_hex,
        used_propagation_node: false,
        method: method_value,
        representation: representation_value,
        relay_destination_hex: None,
        fallback_stage: None,
        receipt_hash_hex: Some(receipt_hash_hex),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "propagation send boundary keeps resolved routing and delivery metadata explicit"
)]
async fn compat_send_lxmf_via_propagation(
    state: &SdkTransportState,
    remote_desc: &DestinationDesc,
    wire: &[u8],
    requested_destination_hex: &str,
    resolved_destination_hex: &str,
    message_id_hex: &str,
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    fallback_stage: Option<LxmfFallbackStage>,
) -> Result<CompatSendReport, SdkError> {
    let relay_hex = state
        .active_propagation_node_hex
        .lock()
        .await
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| sdk_transport("no active propagation relay selected"))?;
    let relay_hash = parse_address_hash(relay_hex.as_str())
        .map_err(|_| sdk_validation("invalid active propagation relay hash"))?;
    let relay_desc = resolve_propagation_destination_desc(state, relay_hash)
        .await
        .map_err(|_| sdk_transport("failed to resolve propagation relay"))?;
    let propagated_payload = LxmfWireMessage::unpack(wire)
        .map_err(|_| sdk_internal("failed to unpack lxmf wire message"))?
        .pack_propagation_with_options_and_rng(
            &lxmf_identity(&remote_desc.identity),
            crate::runtime::now_ms() as f64 / 1000.0,
            Some(PROPAGATION_STAMP_BYTES.as_slice()),
            OsRng,
        )
        .map(|(payload, _transient_id)| payload)
        .map_err(|_| sdk_internal("failed to encode propagated lxmf payload"))?;
    let relay_destination_hex = relay_desc.address_hash.to_hex_string();

    info!(
        "[lxmf][events][sdk] path=propagation requested_destination={} resolved_destination={} recipient_identity={} relay_destination={} message_id={} wire_bytes={} propagated_bytes={} max_wire_bytes={}",
        requested_destination_hex,
        resolved_destination_hex,
        remote_desc.identity.address_hash.to_hex_string(),
        relay_destination_hex,
        message_id_hex,
        wire.len(),
        propagated_payload.len(),
        LXMF_MAX_PAYLOAD,
    );

    if propagated_payload.len() > LXMF_MAX_PAYLOAD {
        let link = ensure_lxmf_output_link(
            state,
            relay_desc,
            Some(requested_destination_hex),
            Some(resolved_destination_hex),
            DEFAULT_LINK_CONNECT_TIMEOUT,
            DEFAULT_LINK_CONNECT_ATTEMPTS,
        )
        .await
        .map_err(|_| sdk_transport("failed to activate propagation relay link"))?;
        let link_id = *link.lock().await.id();
        let mut resource_events = state.transport.resource_events();
        let resource_hash = state
            .transport
            .send_resource(&link_id, propagated_payload.clone(), None)
            .await
            .map_err(|_| {
                sdk_transport("failed to start propagated lxmf relay resource transfer")
            })?;
        let resource_hash_hex = hex::encode(resource_hash.as_slice());
        info!(
            "[lxmf][events][sdk] path=propagation representation=resource requested_destination={} resolved_destination={} relay_destination={} message_id={} resource_hash={} propagated_bytes={} max_wire_bytes={}",
            requested_destination_hex,
            resolved_destination_hex,
            relay_destination_hex,
            message_id_hex,
            resource_hash_hex,
            propagated_payload.len(),
            LXMF_MAX_PAYLOAD,
        );

        let deadline = tokio::time::Instant::now() + RESOURCE_TRANSFER_TIMEOUT;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(sdk_transport(
                    "propagated lxmf relay resource transfer timed out",
                ));
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, resource_events.recv()).await {
                Ok(Ok(event)) => {
                    if event.hash != resource_hash {
                        continue;
                    }
                    match event.kind {
                        ResourceEventKind::Progress(progress) => {
                            debug!(
                                "[lxmf][debug][sdk] path=propagation representation=resource progress requested_destination={} resolved_destination={} relay_destination={} message_id={} resource_hash={} received_bytes={} total_bytes={} received_parts={} total_parts={}",
                                requested_destination_hex,
                                resolved_destination_hex,
                                relay_destination_hex,
                                message_id_hex,
                                resource_hash_hex,
                                progress.received_bytes,
                                progress.total_bytes,
                                progress.received_parts,
                                progress.total_parts,
                            );
                        }
                        ResourceEventKind::OutboundComplete => {
                            info!(
                                "[lxmf][events][sdk] path=propagation representation=resource complete requested_destination={} resolved_destination={} relay_destination={} message_id={} resource_hash={}",
                                requested_destination_hex,
                                resolved_destination_hex,
                                relay_destination_hex,
                                message_id_hex,
                                resource_hash_hex,
                            );
                            return Ok(CompatSendReport {
                                outcome: RnsSendOutcome::SentDirect,
                                message_id_hex: message_id_hex.to_string(),
                                resolved_destination_hex: resolved_destination_hex.to_string(),
                                used_propagation_node: true,
                                method,
                                representation,
                                relay_destination_hex: Some(relay_destination_hex.clone()),
                                fallback_stage,
                                receipt_hash_hex: None,
                            });
                        }
                        ResourceEventKind::Complete(_) => {}
                        ResourceEventKind::InboundFailed(failure) => {
                            return Err(sdk_transport(format!(
                                "propagated lxmf relay inbound resource transfer failed: {}",
                                failure.reason
                            )));
                        }
                        ResourceEventKind::OutboundFailed => {
                            return Err(sdk_transport(
                                "propagated lxmf relay resource transfer failed",
                            ));
                        }
                        ResourceEventKind::OutboundCancelled => {
                            return Err(sdk_transport(
                                "propagated lxmf relay resource transfer cancelled",
                            ));
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err(sdk_transport(
                        "propagation relay resource event stream closed",
                    ));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Err(_) => {
                    return Err(sdk_transport(
                        "propagated lxmf relay resource transfer timed out",
                    ));
                }
            }
        }
    }

    let mut relay_data = PacketDataBuffer::new();
    relay_data
        .write(propagated_payload.as_slice())
        .map_err(|_| sdk_transport("propagated relay payload too large"))?;
    let relay_packet = Packet {
        header: Header {
            ifac_flag: IfacFlag::Open,
            header_type: HeaderType::Type1,
            context_flag: ContextFlag::Unset,
            propagation_type: PropagationType::Broadcast,
            destination_type: DestinationType::Single,
            packet_type: PacketType::Data,
            hops: 0,
        },
        ifac: None,
        destination: relay_desc.address_hash,
        transport: None,
        context: PacketContext::None,
        data: relay_data,
    };
    let outcome = state.transport.send_packet_with_outcome(relay_packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(sdk_transport(format!(
            "propagated relay send failed: {outcome:?}"
        )));
    }

    info!(
        "[lxmf][events][sdk] propagated relay send requested_destination={} resolved_destination={} relay_destination={} message_id={}",
        requested_destination_hex,
        resolved_destination_hex,
        relay_destination_hex,
        message_id_hex,
    );

    Ok(CompatSendReport {
        outcome,
        message_id_hex: message_id_hex.to_string(),
        resolved_destination_hex: resolved_destination_hex.to_string(),
        used_propagation_node: true,
        method,
        representation,
        relay_destination_hex: Some(relay_destination_hex),
        fallback_stage,
        receipt_hash_hex: None,
    })
}

async fn compat_fetch_propagated_lxmf(
    state: &SdkTransportState,
    relay_hex: &str,
    limit: Option<u32>,
    direct_iface_hex: Option<&str>,
) -> Result<PropagationFetchResult, NodeError> {
    let relay_hash = parse_address_hash(relay_hex)?;
    let relay_desc = resolve_propagation_destination_desc(state, relay_hash).await?;
    let direct_iface = direct_iface_hex.and_then(|value| parse_address_hash(value).ok());
    let (destination_hex, destination_hash, local_identity) = {
        let destination = state.lxmf_destination.lock().await;
        (
            destination.desc.address_hash.to_hex_string(),
            destination.desc.address_hash,
            destination.identity.clone(),
        )
    };

    let available_value = propagation_remote_control_request(
        state,
        relay_desc,
        "/get",
        rmpv::Value::Array(vec![rmpv::Value::Nil, rmpv::Value::Nil]),
        PROPAGATION_CONTROL_TIMEOUT,
        direct_iface,
        1,
    )
    .await?;
    let mut transient_ids = rmpv_binary_array(&available_value)?;
    let available_count = transient_ids.len();
    apply_fetch_limit(&mut transient_ids, limit);
    info!(
        "[sync] propagation sync available relay={} destination={} available={} requested={}",
        relay_hex,
        destination_hex,
        available_count,
        transient_ids.len()
    );
    if transient_ids.is_empty() {
        clear_lxmf_output_link(state, &relay_desc.address_hash).await;
        return Ok(PropagationFetchResult {
            destination_hex,
            available_count,
            fetched_count: 0,
            fetched_entry_count: 0,
            extracted_payload_count: 0,
            imported_wires: Vec::new(),
            failed_count: 0,
            malformed_count: 0,
            decrypt_failed_count: 0,
        });
    }

    let mut payloads = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut fetched_entry_count = 0usize;
    let mut malformed_count = 0usize;
    let mut failed_count = 0usize;
    let mut decrypt_failed_count = 0usize;
    let mut last_fetch_error: Option<NodeError> = None;
    let mut fetch_queue = propagation_fetch_batches(transient_ids.as_slice())
        .into_iter()
        .enumerate()
        .collect::<VecDeque<_>>();
    while let Some((batch_index, batch)) = fetch_queue.pop_front() {
        let batch_len = batch.len();
        let fetch_ids =
            rmpv::Value::Array(batch.clone().into_iter().map(rmpv::Value::Binary).collect());
        let fetched_value = match propagation_remote_control_request(
            state,
            relay_desc,
            "/get",
            rmpv::Value::Array(vec![
                fetch_ids,
                rmpv::Value::Nil,
                rmpv::Value::from(PROPAGATION_FETCH_TRANSFER_LIMIT_KB),
            ]),
            PROPAGATION_FETCH_CONTROL_TIMEOUT,
            direct_iface,
            1,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                if batch_len > 1 {
                    info!(
                        "[sync] propagation sync fetch batch split relay={} destination={} batch={} size={} reason={}",
                        relay_hex, destination_hex, batch_index, batch_len, err
                    );
                    for transient_id in batch.into_iter().rev() {
                        fetch_queue.push_front((batch_index, vec![transient_id]));
                    }
                    continue;
                }
                failed_count = failed_count.saturating_add(batch_len);
                info!(
                    "[sync] propagation sync fetch batch failed relay={} destination={} batch={} reason={}",
                    relay_hex, destination_hex, batch_index, err
                );
                last_fetch_error = Some(err);
                continue;
            }
        };
        match rmpv_propagation_payload_array(&fetched_value) {
            Ok(batch_payloads) => {
                fetched_entry_count = fetched_entry_count.saturating_add(batch_len);
                if batch_payloads.len() == batch_len {
                    payloads.extend(
                        batch
                            .into_iter()
                            .zip(batch_payloads)
                            .map(|(transient_id, payload)| (Some(transient_id), payload)),
                    );
                } else {
                    payloads.extend(batch_payloads.into_iter().map(|payload| (None, payload)));
                }
            }
            Err(err) => {
                if batch_len > 1 {
                    info!(
                        "[sync] propagation sync malformed fetch batch split relay={} destination={} batch={} size={} shape={}",
                        relay_hex,
                        destination_hex,
                        batch_index,
                        batch_len,
                        rmpv_shape(&fetched_value)
                    );
                    for transient_id in batch.into_iter().rev() {
                        fetch_queue.push_front((batch_index, vec![transient_id]));
                    }
                    continue;
                }
                failed_count = failed_count.saturating_add(batch_len);
                malformed_count = malformed_count.saturating_add(batch_len);
                info!(
                    "[sync] propagation sync malformed fetch response relay={} destination={} batch={} shape={}",
                    relay_hex,
                    destination_hex,
                    batch_index,
                    rmpv_shape(&fetched_value)
                );
                last_fetch_error = Some(err);
            }
        }
    }
    clear_lxmf_output_link(state, &relay_desc.address_hash).await;
    if payloads.is_empty() {
        if let Some(err) = last_fetch_error {
            return Err(err);
        }
    }
    let extracted_payload_count = payloads.len();
    let fetched_count = fetched_entry_count;
    let mut imported_wires = Vec::with_capacity(extracted_payload_count);
    let mut fetched_transient_ids_to_purge = Vec::<Vec<u8>>::new();
    for (index, (transient_id, payload)) in payloads.into_iter().enumerate() {
        match decrypt_local_propagated_wire(&local_identity, &destination_hash, payload.as_slice())
        {
            Ok(wire) => {
                if let Some(transient_id) = transient_id {
                    fetched_transient_ids_to_purge.push(transient_id);
                }
                imported_wires.push(wire);
            }
            Err(err) => {
                failed_count = failed_count.saturating_add(1);
                decrypt_failed_count = decrypt_failed_count.saturating_add(1);
                let transient_destination_hex = payload
                    .get(..16)
                    .map(hex::encode)
                    .unwrap_or_else(|| "-".to_string());
                let retained = queue_fetched_transient_id_for_purge(
                    &mut fetched_transient_ids_to_purge,
                    transient_id,
                );
                info!(
                    "[sync] propagated payload import failed relay={} destination={} local_identity={} transient_destination={} index={} reason={} retained={}",
                    relay_hex,
                    destination_hex,
                    local_identity.address_hash().to_hex_string(),
                    transient_destination_hex,
                    index,
                    err,
                    retained
                );
            }
        }
    }
    fetched_transient_ids_to_purge.sort();
    fetched_transient_ids_to_purge.dedup();
    if !fetched_transient_ids_to_purge.is_empty() {
        let purge_count = fetched_transient_ids_to_purge.len();
        let mut purged_count = 0usize;
        let mut purge_failed_count = 0usize;
        for batch in propagation_purge_batches(&fetched_transient_ids_to_purge) {
            let batch_count = batch.len();
            let haves = rmpv::Value::Array(batch.into_iter().map(rmpv::Value::Binary).collect());
            match propagation_remote_control_fire_and_forget(
                state,
                relay_desc,
                "/get",
                rmpv::Value::Array(vec![rmpv::Value::Nil, haves]),
                direct_iface,
            )
            .await
            {
                Ok(_) => {
                    purged_count = purged_count.saturating_add(batch_count);
                }
                Err(err) => {
                    purge_failed_count = purge_failed_count.saturating_add(batch_count);
                    info!(
                        "[sync] propagation sync purge batch failed relay={} destination={} purged={} reason={}",
                        relay_hex, destination_hex, batch_count, err
                    );
                }
            }
        }
        if purged_count > 0 {
            info!(
                "[sync] propagation sync queued purge for fetched entries relay={} destination={} purged={} failed={}",
                relay_hex, destination_hex, purged_count, purge_failed_count
            );
        } else if purge_failed_count > 0 {
            info!(
                "[sync] propagation sync purge failed relay={} destination={} purged={} reason=all_batches_failed",
                relay_hex, destination_hex, purge_count
            );
        }
    }

    Ok(PropagationFetchResult {
        destination_hex,
        available_count,
        fetched_count,
        fetched_entry_count,
        extracted_payload_count,
        imported_wires,
        failed_count,
        malformed_count,
        decrypt_failed_count,
    })
}

async fn propagation_remote_control_request(
    state: &SdkTransportState,
    relay_desc: DestinationDesc,
    path: &str,
    data: rmpv::Value,
    timeout: Duration,
    direct_iface: Option<AddressHash>,
    max_attempts: usize,
) -> Result<rmpv::Value, NodeError> {
    let mut last_error = None;
    for attempt in 0..max_attempts.max(1) {
        let relay_destination_hex = relay_desc.address_hash.to_hex_string();
        let link = ensure_lxmf_output_link(
            state,
            relay_desc,
            Some(path),
            Some(relay_destination_hex.as_str()),
            DEFAULT_LINK_CONNECT_TIMEOUT,
            DEFAULT_LINK_CONNECT_ATTEMPTS,
        )
        .await?;
        let link_id = *link.lock().await.id();
        let identify_payload = build_link_identify_payload(&state.identity, &link_id);
        if let Err(err) = send_link_context_packet(
            &state.transport,
            &link,
            PacketContext::LinkIdentify,
            identify_payload.as_slice(),
            direct_iface,
        )
        .await
        {
            clear_lxmf_output_link(state, &relay_desc.address_hash).await;
            info!(
                "[sync] propagation control identify failed relay={} path={} attempt={} reason={}",
                relay_desc.address_hash.to_hex_string(),
                path,
                attempt + 1,
                err
            );
            last_error = Some(err);
            continue;
        }

        let mut data_rx = state.transport.received_data_events();
        let mut resource_rx = state.transport.resource_events();
        let request_payload = build_link_request_payload(path, data.clone())?;
        let request_id = match send_link_context_packet(
            &state.transport,
            &link,
            PacketContext::Request,
            request_payload.as_slice(),
            direct_iface,
        )
        .await
        {
            Ok(Some(request_id)) => request_id,
            Ok(None) => return Err(NodeError::InternalError {}),
            Err(err) => {
                clear_lxmf_output_link(state, &relay_desc.address_hash).await;
                info!(
                    "[sync] propagation control request failed relay={} path={} attempt={} reason={}",
                    relay_desc.address_hash.to_hex_string(),
                    path,
                    attempt + 1,
                    err
                );
                last_error = Some(err);
                continue;
            }
        };

        match wait_for_link_request_response(
            &mut data_rx,
            &mut resource_rx,
            relay_desc.address_hash,
            link_id,
            request_id,
            timeout,
        )
        .await
        {
            Ok(value) => return Ok(value),
            Err(err) => {
                clear_lxmf_output_link(state, &relay_desc.address_hash).await;
                debug!(
                    "[sync] propagation control response unavailable relay={} path={} attempt={} reason={}",
                    relay_desc.address_hash.to_hex_string(),
                    path,
                    attempt + 1,
                    err
                );
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or(NodeError::Timeout {}))
}

async fn propagation_remote_control_fire_and_forget(
    state: &SdkTransportState,
    relay_desc: DestinationDesc,
    path: &str,
    data: rmpv::Value,
    direct_iface: Option<AddressHash>,
) -> Result<(), NodeError> {
    let relay_destination_hex = relay_desc.address_hash.to_hex_string();
    let link = ensure_lxmf_output_link(
        state,
        relay_desc,
        Some(path),
        Some(relay_destination_hex.as_str()),
        DEFAULT_LINK_CONNECT_TIMEOUT,
        DEFAULT_LINK_CONNECT_ATTEMPTS,
    )
    .await?;
    let link_id = *link.lock().await.id();
    let identify_payload = build_link_identify_payload(&state.identity, &link_id);
    if let Err(err) = send_link_context_packet(
        &state.transport,
        &link,
        PacketContext::LinkIdentify,
        identify_payload.as_slice(),
        direct_iface,
    )
    .await
    {
        clear_lxmf_output_link(state, &relay_desc.address_hash).await;
        info!(
            "[sync] propagation control identify failed relay={} path={} reason={}",
            relay_desc.address_hash.to_hex_string(),
            path,
            err
        );
        return Err(err);
    }

    let request_payload = build_link_request_payload(path, data)?;
    match send_link_context_packet(
        &state.transport,
        &link,
        PacketContext::Request,
        request_payload.as_slice(),
        direct_iface,
    )
    .await
    {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(NodeError::InternalError {}),
        Err(err) => {
            clear_lxmf_output_link(state, &relay_desc.address_hash).await;
            info!(
                "[sync] propagation control request failed relay={} path={} reason={}",
                relay_desc.address_hash.to_hex_string(),
                path,
                err
            );
            Err(err)
        }
    }
}

fn build_link_identify_payload(identity: &PrivateIdentity, link_id: &AddressHash) -> Vec<u8> {
    let identity_value = identity.as_identity();
    let mut public_key = Vec::with_capacity(64);
    public_key.extend_from_slice(identity_value.public_key.as_bytes());
    public_key.extend_from_slice(identity_value.verifying_key.as_bytes());

    let mut signed_data = Vec::with_capacity(16 + public_key.len());
    signed_data.extend_from_slice(link_id.as_slice());
    signed_data.extend_from_slice(public_key.as_slice());
    let signature = identity.sign(signed_data.as_slice());

    let mut payload = Vec::with_capacity(public_key.len() + signature.to_bytes().len());
    payload.extend_from_slice(public_key.as_slice());
    payload.extend_from_slice(signature.to_bytes().as_slice());
    payload
}

fn build_link_request_payload(path: &str, data: rmpv::Value) -> Result<Vec<u8>, NodeError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let path_hash = address_hash(path.as_bytes());
    rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::F64(timestamp),
        rmpv::Value::Binary(path_hash.to_vec()),
        data,
    ]))
    .map_err(|_| NodeError::InternalError {})
}

async fn send_link_context_packet(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
    context: PacketContext,
    payload: &[u8],
    direct_iface: Option<AddressHash>,
) -> Result<Option<[u8; 16]>, NodeError> {
    let packet = {
        let guard = link.lock().await;
        if guard.status() != LinkStatus::Active {
            return Err(NodeError::Timeout {});
        }

        let mut packet_data = PacketDataBuffer::new();
        let cipher_len = {
            let ciphertext = guard
                .encrypt(payload, packet_data.accuire_buf_max())
                .map_err(|_| NodeError::InternalError {})?;
            ciphertext.len()
        };
        packet_data.resize(cipher_len);

        Packet {
            header: Header {
                ifac_flag: IfacFlag::Open,
                header_type: HeaderType::Type1,
                context_flag: ContextFlag::Unset,
                propagation_type: PropagationType::Broadcast,
                destination_type: DestinationType::Link,
                packet_type: PacketType::Data,
                hops: 0,
            },
            ifac: None,
            destination: *guard.id(),
            transport: None,
            context,
            data: packet_data,
        }
    };

    let request_id = if context == PacketContext::Request {
        let hash = packet.hash().to_bytes();
        let mut request_id = [0u8; 16];
        request_id.copy_from_slice(&hash[..16]);
        Some(request_id)
    } else {
        None
    };

    if let Some(iface) = direct_iface {
        transport.send_direct(iface, packet).await;
        return Ok(request_id);
    }

    let outcome = transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(NodeError::NetworkError {});
    }
    Ok(request_id)
}

async fn wait_for_link_request_response(
    data_rx: &mut tokio::sync::broadcast::Receiver<ReceivedData>,
    resource_rx: &mut tokio::sync::broadcast::Receiver<ResourceEvent>,
    expected_destination: AddressHash,
    expected_link_id: AddressHash,
    request_id: [u8; 16],
    timeout: Duration,
) -> Result<rmpv::Value, NodeError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(NodeError::Timeout {});
        }
        let remaining = deadline.saturating_duration_since(now);

        tokio::select! {
            _ = tokio::time::sleep(remaining) => {
                return Err(NodeError::Timeout {});
            }
            result = data_rx.recv() => {
                match result {
                    Ok(event) => {
                        if !link_response_destination_matches(
                            event.destination,
                            expected_destination,
                            expected_link_id,
                        ) {
                            continue;
                        }
                        if let Some((response_id, payload)) =
                            parse_link_response_frame(event.data.as_slice())
                        {
                            if response_id == request_id {
                                return Ok(payload);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(NodeError::InternalError {});
                    }
                }
            }
            result = resource_rx.recv() => {
                match result {
                    Ok(event) => {
                        let ResourceEventKind::Complete(complete) = event.kind else {
                            continue;
                        };
                        if event.link_id != expected_link_id {
                            continue;
                        }
                        if let Some((response_id, payload)) =
                            parse_link_response_frame(complete.data.as_slice())
                        {
                            if response_id == request_id {
                                return Ok(payload);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(NodeError::InternalError {});
                    }
                }
            }
        }
    }
}

fn link_response_destination_matches(
    actual: AddressHash,
    expected_destination: AddressHash,
    expected_link_id: AddressHash,
) -> bool {
    actual == expected_link_id || actual == expected_destination
}

fn parse_link_response_frame(bytes: &[u8]) -> Option<([u8; 16], rmpv::Value)> {
    let value = rmp_serde::from_slice::<rmpv::Value>(bytes).ok()?;
    let rmpv::Value::Array(entries) = value else {
        return None;
    };
    if entries.len() != 2 {
        return None;
    }
    let request_bytes = value_to_bytes(entries.first()?)?;
    if request_bytes.len() != 16 {
        return None;
    }
    let mut request_id = [0u8; 16];
    request_id.copy_from_slice(request_bytes.as_slice());
    Some((request_id, entries.get(1)?.clone()))
}

fn value_to_bytes(value: &rmpv::Value) -> Option<Vec<u8>> {
    match value {
        rmpv::Value::Binary(bytes) => Some(bytes.clone()),
        rmpv::Value::String(text) => {
            let value = text.as_str()?;
            if let Ok(decoded) = hex::decode(value) {
                return Some(decoded);
            }
            Some(value.as_bytes().to_vec())
        }
        _ => None,
    }
}

fn rmpv_propagation_envelope_payloads(value: &rmpv::Value) -> Option<Vec<Vec<u8>>> {
    let rmpv::Value::Array(entries) = value else {
        return None;
    };
    if entries.len() < 2 {
        return None;
    }
    let timestamp_like = matches!(
        entries.first(),
        Some(rmpv::Value::F32(_)) | Some(rmpv::Value::F64(_)) | Some(rmpv::Value::Integer(_))
    );
    if !timestamp_like {
        return None;
    }
    let rmpv::Value::Array(payloads) = &entries[1] else {
        return None;
    };
    let decoded = payloads
        .iter()
        .map(value_to_bytes)
        .collect::<Option<Vec<_>>>()?;
    (!decoded.is_empty()).then_some(decoded)
}

fn propagation_payloads_from_bytes(bytes: &[u8]) -> Vec<Vec<u8>> {
    if let Ok(value) = rmp_serde::from_slice::<rmpv::Value>(bytes) {
        if let Some(payloads) = rmpv_propagation_envelope_payloads(&value) {
            return payloads;
        }
    }
    vec![bytes.to_vec()]
}

fn propagation_payloads_from_fetch_entry(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    if let Some(payloads) = rmpv_propagation_envelope_payloads(value) {
        return Ok(payloads);
    }
    match value {
        rmpv::Value::Binary(bytes) => Ok(propagation_payloads_from_bytes(bytes)),
        rmpv::Value::String(text) => {
            let value = text.as_str().ok_or(NodeError::InternalError {})?;
            let bytes = hex::decode(value).unwrap_or_else(|_| value.as_bytes().to_vec());
            Ok(propagation_payloads_from_bytes(bytes.as_slice()))
        }
        rmpv::Value::Array(entries) => {
            if entries.len() >= 2 {
                if let Some(payloads) = rmpv_propagation_envelope_payloads(&entries[1]) {
                    return Ok(payloads);
                }
                if let Some(bytes) = entries.get(1).and_then(value_to_bytes) {
                    return Ok(propagation_payloads_from_bytes(bytes.as_slice()));
                }
            }
            Err(NodeError::InternalError {})
        }
        _ => Err(NodeError::InternalError {}),
    }
}

fn rmpv_binary_array(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    let rmpv::Value::Array(values) = value else {
        return Err(NodeError::InternalError {});
    };
    values
        .iter()
        .map(|value| match value {
            rmpv::Value::Binary(bytes) => Ok(bytes.clone()),
            _ => Err(NodeError::InternalError {}),
        })
        .collect()
}

fn rmpv_propagation_payload_array(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    if let Some(payloads) = rmpv_propagation_envelope_payloads(value) {
        return Ok(payloads);
    }
    let rmpv::Value::Array(values) = value else {
        return Err(NodeError::InternalError {});
    };
    let mut payloads = Vec::new();
    for value in values {
        payloads.extend(propagation_payloads_from_fetch_entry(value)?);
    }
    Ok(payloads)
}

fn rmpv_shape(value: &rmpv::Value) -> String {
    match value {
        rmpv::Value::Nil => "nil".to_string(),
        rmpv::Value::Boolean(_) => "bool".to_string(),
        rmpv::Value::Integer(_) => "int".to_string(),
        rmpv::Value::F32(_) | rmpv::Value::F64(_) => "float".to_string(),
        rmpv::Value::String(_) => "string".to_string(),
        rmpv::Value::Binary(bytes) => format!("bin({})", bytes.len()),
        rmpv::Value::Array(values) => {
            let preview = values
                .iter()
                .take(4)
                .map(rmpv_shape)
                .collect::<Vec<_>>()
                .join(",");
            if values.len() > 4 {
                format!("array({})[{preview},...]", values.len())
            } else {
                format!("array({})[{preview}]", values.len())
            }
        }
        rmpv::Value::Map(values) => format!("map({})", values.len()),
        rmpv::Value::Ext(_, bytes) => format!("ext({})", bytes.len()),
    }
}

fn apply_fetch_limit(transient_ids: &mut Vec<Vec<u8>>, limit: Option<u32>) {
    if let Some(limit) = limit {
        transient_ids.truncate(limit as usize);
    }
}

fn propagation_fetch_batches(transient_ids: &[Vec<u8>]) -> Vec<Vec<Vec<u8>>> {
    transient_ids
        .chunks(PROPAGATION_FETCH_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn propagation_purge_batches(transient_ids: &[Vec<u8>]) -> Vec<Vec<Vec<u8>>> {
    transient_ids
        .chunks(PROPAGATION_PURGE_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn queue_fetched_transient_id_for_purge(
    purge_queue: &mut Vec<Vec<u8>>,
    transient_id: Option<Vec<u8>>,
) -> bool {
    if let Some(transient_id) = transient_id {
        purge_queue.push(transient_id);
        false
    } else {
        true
    }
}

#[derive(Debug, Eq, PartialEq)]
enum PropagationPayloadDecryptError {
    PayloadTooShort { len: usize },
    DestinationMismatch { expected: String, actual: String },
    DecryptFailed,
}

impl fmt::Display for PropagationPayloadDecryptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooShort { len } => {
                write!(f, "payload too short for propagation transient len={}", len)
            }
            Self::DestinationMismatch { expected, actual } => write!(
                f,
                "destination prefix mismatch expected={} actual={}",
                expected, actual
            ),
            Self::DecryptFailed => write!(f, "propagation transient decrypt failed"),
        }
    }
}

fn decrypt_local_propagated_wire(
    identity: &PrivateIdentity,
    destination_hash: &AddressHash,
    transient_payload: &[u8],
) -> Result<Vec<u8>, PropagationPayloadDecryptError> {
    if transient_payload.len() <= 16 + 32 {
        return Err(PropagationPayloadDecryptError::PayloadTooShort {
            len: transient_payload.len(),
        });
    }
    if &transient_payload[..16] != destination_hash.as_slice() {
        return Err(PropagationPayloadDecryptError::DestinationMismatch {
            expected: destination_hash.to_hex_string(),
            actual: hex::encode(&transient_payload[..16]),
        });
    }

    for strip_stamp in [false, true] {
        let payload = if strip_stamp {
            if transient_payload.len() <= 16 + 32 + 32 {
                continue;
            }
            &transient_payload[..transient_payload.len() - 32]
        } else {
            transient_payload
        };

        let ciphertext = &payload[16..];
        if let Ok(decrypted) =
            decrypt_propagation_ciphertext(identity, destination_hash, ciphertext)
        {
            let mut wire = Vec::with_capacity(16 + decrypted.len());
            wire.extend_from_slice(destination_hash.as_slice());
            wire.extend_from_slice(decrypted.as_slice());
            return Ok(wire);
        }
    }

    Err(PropagationPayloadDecryptError::DecryptFailed)
}

fn decrypt_propagation_ciphertext(
    identity: &PrivateIdentity,
    destination_hash: &AddressHash,
    ciphertext: &[u8],
) -> Result<Vec<u8>, PropagationPayloadDecryptError> {
    if ciphertext.len() <= 32 {
        return Err(PropagationPayloadDecryptError::DecryptFailed);
    }
    let Ok(ephemeral_key) = <[u8; 32]>::try_from(&ciphertext[..32]) else {
        return Err(PropagationPayloadDecryptError::DecryptFailed);
    };
    let public_key = PublicKey::from(ephemeral_key);
    let token = &ciphertext[32..];

    let mut salts = Vec::with_capacity(2);
    salts.push(identity.address_hash().as_slice());
    if destination_hash.as_slice() != identity.address_hash().as_slice() {
        salts.push(destination_hash.as_slice());
    }

    for salt in salts {
        let derived_key = identity.derive_key(&public_key, Some(salt));
        let mut plaintext = vec![0u8; token.len()];
        let Ok(decrypted_len) = identity
            .decrypt(OsRng, token, &derived_key, &mut plaintext)
            .map(|decrypted| decrypted.len())
        else {
            continue;
        };
        plaintext.truncate(decrypted_len);
        return Ok(plaintext);
    }

    Err(PropagationPayloadDecryptError::DecryptFailed)
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

async fn ensure_destination_desc(
    state: &SdkTransportState,
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

async fn resolve_lxmf_destination_desc(
    state: &SdkTransportState,
    destination: AddressHash,
) -> Result<DestinationDesc, NodeError> {
    let desc = ensure_destination_desc(state, destination, None).await?;
    let lxmf_destination = SingleOutputDestination::new(
        desc.identity,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    );
    Ok(lxmf_destination.desc)
}

async fn resolve_propagation_destination_desc(
    state: &SdkTransportState,
    destination: AddressHash,
) -> Result<DestinationDesc, NodeError> {
    ensure_destination_desc(
        state,
        destination,
        Some(DestinationName::new(
            LXMF_PROPAGATION_NAME.0,
            LXMF_PROPAGATION_NAME.1,
        )),
    )
    .await
}

async fn ensure_lxmf_output_link(
    state: &SdkTransportState,
    desc: DestinationDesc,
    requested_destination_hex: Option<&str>,
    resolved_destination_hex: Option<&str>,
    connect_timeout: Duration,
    max_attempts: usize,
) -> Result<Arc<TokioMutex<Link>>, NodeError> {
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    for attempt in 0..max_attempts.max(1) {
        state
            .transport
            .request_path(&desc.address_hash, None, None)
            .await;

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

        match wait_for_link_active(&state.transport, &link, connect_timeout).await {
            Ok(()) => return Ok(link),
            Err(err) => {
                let stale = state.out_links.lock().await.remove(&desc.address_hash);
                if let Some(stale) = stale {
                    stale.lock().await.close();
                }
                if attempt + 1 == max_attempts.max(1) {
                    log_lxmf_link_activation_failure(
                        "failed",
                        &desc,
                        requested_destination_hex,
                        resolved_destination_hex,
                        attempt + 1,
                        &err,
                    );
                    return Err(err);
                }
                log_lxmf_link_activation_failure(
                    "retry",
                    &desc,
                    requested_destination_hex,
                    resolved_destination_hex,
                    attempt + 1,
                    &err,
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(NodeError::Timeout {})
}

fn log_lxmf_link_activation_failure(
    status: &str,
    desc: &DestinationDesc,
    requested_destination_hex: Option<&str>,
    resolved_destination_hex: Option<&str>,
    attempt: usize,
    err: &NodeError,
) {
    if let (Some(requested_destination_hex), Some(resolved_destination_hex)) =
        (requested_destination_hex, resolved_destination_hex)
    {
        info!(
            "[lxmf][events][sdk] link activation {status} requested_destination={} resolved_destination={} link_destination={} attempt={} reason={}",
            requested_destination_hex,
            resolved_destination_hex,
            desc.address_hash.to_hex_string(),
            attempt,
            err,
        );
        return;
    }

    info!(
        "[lxmf][events][sdk] link activation {status} destination={} attempt={} reason={}",
        desc.address_hash.to_hex_string(),
        attempt,
        err,
    );
}

async fn clear_lxmf_output_link(state: &SdkTransportState, destination: &AddressHash) {
    let stale = state.out_links.lock().await.remove(destination);
    if let Some(stale) = stale {
        stale.lock().await.close();
    }
}

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
    timeout: Duration,
) -> Result<(), NodeError> {
    if link.lock().await.status() == LinkStatus::Active {
        return Ok(());
    }

    let link_id = *link.lock().await.id();
    let mut events = transport.out_link_events();
    let deadline = tokio::time::Instant::now() + timeout;

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

#[cfg(test)]
mod tests {
    use super::*;
    use lxmf::message::Payload;
    use reticulum::transport::identity::EncryptIdentity;

    #[test]
    fn send_request_preserves_raw_payload_and_fields_extensions() {
        let backend = CompatBackend::new_for_tests("runtime-test");
        let client = Client::new(backend);

        let mut config = SdkConfig::desktop_local_default();
        config.rpc_backend = None;
        client.start(StartRequest::new(config)).expect("start");

        let raw_payload = BASE64_STANDARD.encode(b"hello");
        let fields_payload = BASE64_STANDARD.encode([1u8, 2, 3]);
        let req = SendRequest::new(
            "source",
            "0123456789abcdef0123456789abcdef",
            json!({"content_base64": raw_payload}),
        )
        .with_extension(
            EXT_RAW_BYTES_BASE64,
            json!(BASE64_STANDARD.encode(b"hello")),
        )
        .with_extension(
            EXT_FIELDS_BASE64,
            json!(BASE64_STANDARD.encode([1u8, 2, 3])),
        );

        assert_eq!(
            req.extensions
                .get(EXT_RAW_BYTES_BASE64)
                .and_then(JsonValue::as_str),
            Some(raw_payload.as_str())
        );
        assert_eq!(
            req.extensions
                .get(EXT_FIELDS_BASE64)
                .and_then(JsonValue::as_str),
            Some(fields_payload.as_str())
        );
    }

    #[test]
    fn propagation_retry_idempotency_key_does_not_reuse_direct_send() {
        assert_eq!(
            idempotency_key_for_send_mode("mission-corr-1", SendMode::Auto {}),
            "mission-corr-1"
        );
        assert_eq!(
            idempotency_key_for_send_mode("mission-corr-1", SendMode::DirectOnly {}),
            "mission-corr-1:direct"
        );
        assert_eq!(
            idempotency_key_for_send_mode("mission-corr-1", SendMode::PropagationOnly {}),
            "mission-corr-1:propagation"
        );
    }

    #[test]
    fn direct_retry_idempotency_keys_are_unique_per_link_attempt() {
        assert_eq!(
            idempotency_key_for_send_attempt("mission-corr-1", SendMode::Auto {}, Some(1)),
            "mission-corr-1:direct-attempt-1"
        );
        assert_eq!(
            idempotency_key_for_send_attempt("mission-corr-1", SendMode::Auto {}, Some(2)),
            "mission-corr-1:direct-attempt-2"
        );
        assert_eq!(
            idempotency_key_for_send_attempt("mission-corr-1", SendMode::DirectOnly {}, Some(3)),
            "mission-corr-1:direct-attempt-3"
        );
        assert_eq!(
            idempotency_key_for_send_attempt(
                "mission-corr-1",
                SendMode::PropagationOnly {},
                Some(1)
            ),
            "mission-corr-1:propagation"
        );
    }

    #[test]
    fn delivery_updates_map_to_sdk_terminal_states() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        backend.record_delivery_update(
            "msg-1",
            DeliveryState::Sent,
            "dest-1",
            None,
            Some("corr-1"),
            Some("cmd-1"),
            Some("mission.registry.log_entry.upsert"),
            Some("evt-1"),
            Some("mission-1"),
            None,
        );
        backend.record_delivery_update(
            "msg-1",
            DeliveryState::Delivered,
            "dest-1",
            Some("src-1"),
            Some("corr-1"),
            Some("cmd-1"),
            Some("mission.registry.log_entry.upsert"),
            Some("evt-1"),
            Some("mission-1"),
            Some("accepted"),
        );

        let snapshot = backend
            .status(MessageId("msg-1".to_string()))
            .expect("status")
            .expect("snapshot");

        assert_eq!(snapshot.state, DeliveryState::Delivered);
        assert!(snapshot.terminal);
        assert_eq!(snapshot.reason_code.as_deref(), Some("accepted"));
    }

    #[test]
    fn poll_events_returns_delivery_and_peer_events_in_order() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        backend.record_peer_changed("dest-1", "connected", None);
        backend.record_delivery_update(
            "msg-1",
            DeliveryState::Failed,
            "dest-1",
            None,
            Some("corr-1"),
            Some("cmd-1"),
            Some("mission.registry.log_entry.upsert"),
            Some("evt-1"),
            Some("mission-1"),
            Some("network"),
        );

        let batch = backend.poll_events(None, 10).expect("batch");

        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].event_type, EVENT_PEER_CHANGED);
        assert_eq!(batch.events[1].event_type, EVENT_DELIVERY_UPDATED);
        assert_eq!(batch.next_cursor.0, "2");
    }

    #[test]
    fn send_reports_are_reusable_for_idempotent_sdk_replays() {
        let backend = CompatBackend::new_for_tests("runtime-test");
        let report = CompatSendReport {
            outcome: RnsSendOutcome::SentDirect,
            message_id_hex: "msg-1".to_string(),
            resolved_destination_hex: "dest-1".to_string(),
            used_propagation_node: false,
            method: LxmfDeliveryMethod::Direct {},
            representation: LxmfDeliveryRepresentation::Resource {},
            relay_destination_hex: None,
            fallback_stage: None,
            receipt_hash_hex: None,
        };
        {
            let mut state = backend.state.lock().expect("state lock");
            state
                .send_reports
                .insert(report.message_id_hex.clone(), report.clone());
        }

        let first = backend.send_report("msg-1").expect("first lookup");
        let second = backend.send_report("msg-1").expect("second lookup");

        assert_eq!(first.message_id_hex, "msg-1");
        assert_eq!(second.message_id_hex, "msg-1");
        assert!(matches!(
            first.representation,
            LxmfDeliveryRepresentation::Resource {}
        ));
        assert!(matches!(
            second.representation,
            LxmfDeliveryRepresentation::Resource {}
        ));
    }

    #[test]
    fn compat_backend_caps_event_history() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        for seq in 0..(COMPAT_EVENT_RETENTION_LIMIT + 8) {
            backend.record_peer_changed(format!("dest-{seq}").as_str(), "connected", None);
        }

        let state = backend.state.lock().expect("state lock");
        assert_eq!(state.events.len(), COMPAT_EVENT_RETENTION_LIMIT);
        assert_eq!(
            state.last_seq_no(),
            (COMPAT_EVENT_RETENTION_LIMIT + 8) as u64
        );
        assert_eq!(state.events.front().map(|event| event.seq_no), Some(9));
    }

    #[test]
    fn compat_backend_prunes_terminal_deliveries_first() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        backend.record_delivery_update(
            "queued",
            DeliveryState::Queued,
            "dest-queued",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        for index in 0..(COMPAT_DELIVERY_RETENTION_LIMIT + 16) {
            backend.record_delivery_update(
                format!("msg-{index}").as_str(),
                DeliveryState::Delivered,
                format!("dest-{index}").as_str(),
                None,
                None,
                None,
                None,
                None,
                None,
                Some("accepted"),
            );
        }

        let state = backend.state.lock().expect("state lock");
        assert!(state.deliveries.contains_key("queued"));
        assert_eq!(state.deliveries.len(), COMPAT_DELIVERY_RETENTION_LIMIT);
    }

    #[test]
    fn compat_backend_prunes_old_send_reports() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        {
            let mut state = backend.state.lock().expect("state lock");
            for index in 0..(COMPAT_SEND_REPORT_RETENTION_LIMIT + 8) {
                state.record_send_report(CompatSendReport {
                    outcome: RnsSendOutcome::SentDirect,
                    message_id_hex: format!("msg-{index}"),
                    resolved_destination_hex: format!("dest-{index}"),
                    used_propagation_node: false,
                    method: LxmfDeliveryMethod::Direct {},
                    representation: LxmfDeliveryRepresentation::Packet {},
                    relay_destination_hex: None,
                    fallback_stage: None,
                    receipt_hash_hex: None,
                });
            }
        }

        let state = backend.state.lock().expect("state lock");
        assert_eq!(state.send_reports.len(), COMPAT_SEND_REPORT_RETENTION_LIMIT);
        assert!(!state.send_reports.contains_key("msg-0"));
        assert!(state
            .send_reports
            .contains_key(&format!("msg-{}", COMPAT_SEND_REPORT_RETENTION_LIMIT + 7)));
    }

    #[test]
    fn propagation_get_response_binary_array_parses_payloads() {
        let value = rmpv::Value::Array(vec![
            rmpv::Value::Binary(vec![1, 2, 3]),
            rmpv::Value::Binary(vec![4, 5, 6]),
        ]);

        let parsed = rmpv_binary_array(&value).expect("binary array");

        assert_eq!(parsed, vec![vec![1, 2, 3], vec![4, 5, 6]]);
        assert!(rmpv_binary_array(&rmpv::Value::Nil).is_err());
        assert!(rmpv_binary_array(&rmpv::Value::Array(vec![rmpv::Value::Nil])).is_err());
    }

    #[test]
    fn propagation_fetch_payload_array_accepts_binary_and_id_payload_pairs() {
        let value = rmpv::Value::Array(vec![
            rmpv::Value::Binary(vec![1, 2, 3]),
            rmpv::Value::Array(vec![
                rmpv::Value::Binary(vec![0xAA; 32]),
                rmpv::Value::Binary(vec![4, 5, 6]),
            ]),
        ]);

        let parsed = rmpv_propagation_payload_array(&value).expect("payload array");

        assert_eq!(parsed, vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }

    #[test]
    fn propagation_fetch_payload_array_unwraps_msgpack_envelopes() {
        let first_transient = vec![0x11; 48];
        let second_transient = vec![0x22; 64];
        let third_transient = vec![0x33; 80];
        let third_envelope = rmpv::Value::Array(vec![
            rmpv::Value::F64(1_779_000_002.0),
            rmpv::Value::Array(vec![rmpv::Value::Binary(third_transient.clone())]),
        ]);
        let first_envelope = rmp_serde::to_vec(&rmpv::Value::Array(vec![
            rmpv::Value::F64(1_779_000_000.0),
            rmpv::Value::Array(vec![rmpv::Value::Binary(first_transient.clone())]),
        ]))
        .expect("encode first envelope");
        let second_envelope = rmp_serde::to_vec(&rmpv::Value::Array(vec![
            rmpv::Value::F64(1_779_000_001.0),
            rmpv::Value::Array(vec![rmpv::Value::Binary(second_transient.clone())]),
        ]))
        .expect("encode second envelope");
        let value = rmpv::Value::Array(vec![
            rmpv::Value::Binary(first_envelope),
            rmpv::Value::Array(vec![
                rmpv::Value::Binary(vec![0xAA; 32]),
                rmpv::Value::Binary(second_envelope),
            ]),
            rmpv::Value::Array(vec![rmpv::Value::Binary(vec![0xBB; 32]), third_envelope]),
        ]);

        let parsed = rmpv_propagation_payload_array(&value).expect("payload array");

        assert_eq!(
            parsed,
            vec![first_transient, second_transient, third_transient]
        );
    }

    #[test]
    fn propagation_link_response_frame_rejects_malformed_payloads() {
        let request_id = [0x11; 16];
        let valid = rmp_serde::to_vec(&rmpv::Value::Array(vec![
            rmpv::Value::Binary(request_id.to_vec()),
            rmpv::Value::Binary(vec![0x22]),
        ]))
        .expect("encode response");

        let parsed = parse_link_response_frame(valid.as_slice()).expect("valid response");
        assert_eq!(parsed.0, request_id);
        assert_eq!(parsed.1, rmpv::Value::Binary(vec![0x22]));

        let wrong_shape = rmp_serde::to_vec(&rmpv::Value::Array(vec![rmpv::Value::Binary(
            request_id.to_vec(),
        )]))
        .expect("encode malformed response");
        assert!(parse_link_response_frame(wrong_shape.as_slice()).is_none());

        let wrong_id_len = rmp_serde::to_vec(&rmpv::Value::Array(vec![
            rmpv::Value::Binary(vec![0x11; 15]),
            rmpv::Value::Binary(vec![0x22]),
        ]))
        .expect("encode malformed response");
        assert!(parse_link_response_frame(wrong_id_len.as_slice()).is_none());
    }

    #[test]
    fn propagation_response_destination_accepts_link_or_destination() {
        let expected_destination = AddressHash::new([0xAA; 16]);
        let expected_link_id = AddressHash::new([0xBB; 16]);

        assert!(link_response_destination_matches(
            expected_link_id,
            expected_destination,
            expected_link_id,
        ));
        assert!(link_response_destination_matches(
            expected_destination,
            expected_destination,
            expected_link_id,
        ));
        assert!(!link_response_destination_matches(
            AddressHash::new([0xCC; 16]),
            expected_destination,
            expected_link_id,
        ));
    }

    #[test]
    fn propagation_fetch_limit_truncates_transient_ids() {
        let mut ids = vec![vec![1], vec![2], vec![3]];

        apply_fetch_limit(&mut ids, Some(2));

        assert_eq!(ids, vec![vec![1], vec![2]]);
        apply_fetch_limit(&mut ids, None);
        assert_eq!(ids, vec![vec![1], vec![2]]);
    }

    #[test]
    fn propagation_fetch_batches_are_bounded() {
        let ids = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];

        let batches = propagation_fetch_batches(ids.as_slice());

        assert_eq!(
            batches,
            vec![
                vec![vec![1]],
                vec![vec![2]],
                vec![vec![3]],
                vec![vec![4]],
                vec![vec![5]]
            ]
        );
    }

    #[test]
    fn propagation_purge_batches_are_bounded() {
        let ids = (0u8..20).map(|value| vec![value]).collect::<Vec<_>>();

        let batches = propagation_purge_batches(ids.as_slice());

        assert_eq!(batches.len(), 3);
        assert!(batches
            .iter()
            .all(|batch| batch.len() <= PROPAGATION_PURGE_BATCH_SIZE));
        assert_eq!(
            batches[0],
            (0u8..8).map(|value| vec![value]).collect::<Vec<_>>()
        );
        assert_eq!(
            batches[2],
            (16u8..20).map(|value| vec![value]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn propagation_decrypt_failures_with_transient_ids_are_purged() {
        let mut purge_queue = Vec::new();

        let retained = queue_fetched_transient_id_for_purge(&mut purge_queue, Some(vec![0xAA]));

        assert!(!retained);
        assert_eq!(purge_queue, vec![vec![0xAA]]);

        let retained = queue_fetched_transient_id_for_purge(&mut purge_queue, None);

        assert!(retained);
        assert_eq!(purge_queue, vec![vec![0xAA]]);
    }

    #[test]
    fn propagated_payload_decrypts_only_for_local_destination() {
        let receiver = PrivateIdentity::new_from_name("propagation-sync-receiver");
        let sender = PrivateIdentity::new_from_name("propagation-sync-sender");
        let other = PrivateIdentity::new_from_name("propagation-sync-other");
        let mut destination = [0u8; 16];
        destination.copy_from_slice(receiver.address_hash().as_slice());
        let mut source = [0u8; 16];
        source.copy_from_slice(sender.address_hash().as_slice());
        let payload = Payload::new(
            1_779_000_000.0,
            Some(b"sync-content".to_vec()),
            Some(b"sync-title".to_vec()),
            None,
            None,
        );
        let mut wire = LxmfWireMessage::new(destination, source, payload);
        wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
            .expect("sign wire");
        let packed = wire.pack().expect("pack wire");
        let (transient, _) = wire
            .pack_propagation_transient_with_rng(&lxmf_identity(receiver.as_identity()), OsRng)
            .expect("pack transient");

        let decrypted =
            decrypt_local_propagated_wire(&receiver, receiver.address_hash(), transient.as_slice())
                .expect("decrypt local propagated wire");

        assert_eq!(decrypted, packed);
        assert!(decrypt_local_propagated_wire(
            &receiver,
            other.address_hash(),
            transient.as_slice()
        )
        .is_err());
    }

    #[test]
    fn propagated_payload_decrypt_accepts_delivery_hash_salt() {
        let receiver = PrivateIdentity::new_from_name("propagation-delivery-salt-receiver");
        let sender = PrivateIdentity::new_from_name("propagation-delivery-salt-sender");
        let delivery_hash = AddressHash::new_from_hex_string("42424242424242424242424242424242")
            .expect("delivery hash");
        assert_ne!(delivery_hash.as_slice(), receiver.address_hash().as_slice());
        let payload = b"delivery-hash-salted-propagation";
        let receiver_public = PublicKey::from(*receiver.as_identity().public_key.as_bytes());
        let derived_key = sender.derive_key(&receiver_public, Some(delivery_hash.as_slice()));
        let mut token_buf = vec![0u8; payload.len() + 256];
        let token = sender
            .encrypt(OsRng, payload, &derived_key, &mut token_buf)
            .expect("encrypt with delivery hash salt");

        let mut transient = Vec::new();
        transient.extend_from_slice(delivery_hash.as_slice());
        transient.extend_from_slice(sender.as_identity().public_key.as_bytes());
        transient.extend_from_slice(token);

        let decrypted =
            decrypt_local_propagated_wire(&receiver, &delivery_hash, transient.as_slice())
                .expect("delivery hash salt fallback should decrypt");

        assert_eq!(&decrypted[16..], payload);
    }

    #[test]
    fn propagated_payload_decrypts_lxmf_delivery_destination_hash() {
        let receiver = PrivateIdentity::new_from_name("propagation-lxmf-delivery-receiver");
        let sender = PrivateIdentity::new_from_name("propagation-lxmf-delivery-sender");
        let delivery_destination = SingleOutputDestination::new(
            *receiver.as_identity(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        );
        let delivery_hash = delivery_destination.desc.address_hash;
        assert_ne!(delivery_hash.as_slice(), receiver.address_hash().as_slice());
        let mut destination = [0u8; 16];
        destination.copy_from_slice(delivery_hash.as_slice());
        let mut source = [0u8; 16];
        source.copy_from_slice(sender.address_hash().as_slice());
        let payload = Payload::new(
            1_779_000_050.0,
            Some(b"delivery-destination-content".to_vec()),
            Some(b"delivery-destination-title".to_vec()),
            None,
            None,
        );
        let mut wire = LxmfWireMessage::new(destination, source, payload);
        wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
            .expect("sign wire");
        let packed = wire.pack().expect("pack wire");
        let envelope = wire
            .pack_propagation_with_rng(
                &lxmf_identity(receiver.as_identity()),
                1_779_000_050.0,
                OsRng,
            )
            .expect("pack envelope");
        let payloads = propagation_payloads_from_bytes(envelope.as_slice());

        let decrypted =
            decrypt_local_propagated_wire(&receiver, &delivery_hash, payloads[0].as_slice())
                .expect("decrypt delivery-destination propagated wire");

        assert_eq!(decrypted, packed);
    }

    #[test]
    fn propagated_payload_decrypt_error_identifies_destination_mismatch() {
        let receiver = PrivateIdentity::new_from_name("propagation-error-receiver");
        let other = PrivateIdentity::new_from_name("propagation-error-other");
        let mut payload = vec![0u8; 16 + 32 + 1];
        payload[..16].copy_from_slice(other.address_hash().as_slice());

        let err =
            decrypt_local_propagated_wire(&receiver, receiver.address_hash(), payload.as_slice())
                .expect_err("wrong destination should fail before decrypt");

        assert!(err
            .to_string()
            .contains("destination prefix mismatch expected="));
        assert!(err
            .to_string()
            .contains(receiver.address_hash().to_hex_string().as_str()));
        assert!(err
            .to_string()
            .contains(other.address_hash().to_hex_string().as_str()));
    }

    #[test]
    fn propagated_envelope_from_fetch_response_decrypts_for_local_destination() {
        let receiver = PrivateIdentity::new_from_name("propagation-envelope-receiver");
        let sender = PrivateIdentity::new_from_name("propagation-envelope-sender");
        let mut destination = [0u8; 16];
        destination.copy_from_slice(receiver.address_hash().as_slice());
        let mut source = [0u8; 16];
        source.copy_from_slice(sender.address_hash().as_slice());
        let payload = Payload::new(
            1_779_000_100.0,
            Some(b"enveloped-sync-content".to_vec()),
            Some(b"enveloped-sync-title".to_vec()),
            None,
            None,
        );
        let mut wire = LxmfWireMessage::new(destination, source, payload);
        wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
            .expect("sign wire");
        let packed = wire.pack().expect("pack wire");
        let envelope = wire
            .pack_propagation_with_rng(
                &lxmf_identity(receiver.as_identity()),
                1_779_000_100.0,
                OsRng,
            )
            .expect("pack envelope");
        let fetched = rmpv::Value::Array(vec![rmpv::Value::Binary(envelope)]);
        let payloads = rmpv_propagation_payload_array(&fetched).expect("payloads");

        assert_eq!(payloads.len(), 1);
        let decrypted = decrypt_local_propagated_wire(
            &receiver,
            receiver.address_hash(),
            payloads[0].as_slice(),
        )
        .expect("decrypt local propagated wire");

        assert_eq!(decrypted, packed);
    }
}
