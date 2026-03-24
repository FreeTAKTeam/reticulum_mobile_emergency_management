use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

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
use reticulum::destination::link::{Link, LinkEvent, LinkStatus};
use reticulum::destination::{DestinationDesc, DestinationName, SingleOutputDestination};
use reticulum::hash::AddressHash;
use reticulum::identity::PrivateIdentity;
use reticulum::packet::LXMF_MAX_PAYLOAD;
use reticulum::packet::{
    ContextFlag, DestinationType, Header, HeaderType, IfacFlag, Packet, PacketContext,
    PacketDataBuffer, PacketType, PropagationType,
};
use reticulum::resource::ResourceEventKind;
use reticulum::transport::{SendPacketOutcome as RnsSendOutcome, Transport};
use rand_core::OsRng;
use serde_json::{json, Value as JsonValue};
use tokio::runtime::Handle;
use tokio::sync::Mutex as TokioMutex;

use crate::mission_sync::MissionSyncMetadata;
use crate::runtime::{lxmf_private_identity, LxmfSendReport};
use crate::types::{
    LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfFallbackStage, NodeError, PeerState,
    SendMode,
};

const SDK_CAUSE_LXMF_PACKET_TOO_LARGE: &str = "LxmfPacketTooLarge";
const RESOURCE_TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);
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

fn lxmf_identity(
    identity: &reticulum::identity::Identity,
) -> lxmf::identity::Identity {
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
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);

const EXT_FIELDS_BASE64: &str = "reticulum.fields_base64";
const EXT_RAW_BYTES_BASE64: &str = "reticulum.raw_bytes_base64";
const EXT_SEND_MODE: &str = "reticulum.send_mode";
const EXT_USE_PROPAGATION_NODE: &str = "reticulum.use_propagation_node";
const EVENT_PACKET_RECEIVED: &str = "reticulum.packet_received";
const EVENT_ANNOUNCE_RECEIVED: &str = "reticulum.announce_received";
const EVENT_PEER_CHANGED: &str = "reticulum.peer_changed";
const EVENT_HUB_DIRECTORY_UPDATED: &str = "reticulum.hub_directory_updated";
const EVENT_DELIVERY_UPDATED: &str = "reticulum.delivery_updated";

#[derive(Clone)]
pub(crate) struct SdkTransportState {
    pub(crate) identity: PrivateIdentity,
    pub(crate) transport: Arc<Transport>,
    pub(crate) lxmf_destination: Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
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
        self.send_report_order.retain(|value| value != &message_id_hex);
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

    fn record_hub_directory_updated(&self, destinations: &[String]) {
        let payload = json!({
            "destinations": destinations,
        });
        if let Ok(mut state) = self.state.lock() {
            state.push_event(EVENT_HUB_DIRECTORY_UPDATED, Severity::Info, payload);
        }
    }

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
    used_resource: bool,
    used_propagation_node: bool,
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    relay_destination_hex: Option<String>,
    fallback_stage: Option<LxmfFallbackStage>,
    receipt_hash_hex: Option<String>,
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
        request = request.with_extension(EXT_SEND_MODE, json!(match send_mode {
            SendMode::Auto {} => "Auto",
            SendMode::DirectOnly {} => "DirectOnly",
            SendMode::PropagationOnly {} => "PropagationOnly",
        }));
        if matches!(send_mode, SendMode::PropagationOnly {}) {
            request = request.with_extension(EXT_USE_PROPAGATION_NODE, json!(true));
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
            request = request.with_idempotency_key(idempotency_key);
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
            used_resource: report.used_resource,
            used_propagation_node: report.used_propagation_node,
            method: report.method,
            representation: report.representation,
            relay_destination_hex: report.relay_destination_hex,
            fallback_stage: report.fallback_stage,
            receipt_hash_hex: report.receipt_hash_hex,
        })
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

    pub(crate) fn record_hub_directory_updated(&self, destinations: &[String]) {
        self.client
            .backend()
            .record_hub_directory_updated(destinations);
    }

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
    let desired_method = transport_method_for_send_mode(
        send_mode,
        has_cached_direct_link,
        has_delivery_ratchet(&state, &remote_desc.address_hash),
    );
    let DeliveryDecision { method, representation } =
        decide_delivery(desired_method, false, wire.len())
            .map_err(|err| sdk_validation(format!("failed to choose lxmf delivery representation: {err}")))?;
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
            used_resource: false,
            used_propagation_node: false,
            method: method_value,
            representation: representation_value,
            relay_destination_hex: None,
            fallback_stage: None,
            receipt_hash_hex: Some(receipt_hash_hex),
        });
    }

    let link = ensure_lxmf_output_link(&state, remote_desc)
        .await
        .map_err(|_| sdk_transport("failed to activate lxmf link"))?;
    let link_id = *link.lock().await.id();
    if matches!(representation, LxmfRepresentation::Resource) {
        let mut resource_events = state.transport.resource_events();
        let resource_hash = state
            .transport
            .send_resource(&link_id, wire.clone(), None)
            .await
            .map_err(|_| sdk_transport("failed to start lxmf resource transfer"))?;
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
        let deadline = tokio::time::Instant::now() + RESOURCE_TRANSFER_TIMEOUT;
        loop {
            if tokio::time::Instant::now() >= deadline {
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
                            return Ok(CompatSendReport {
                                outcome: RnsSendOutcome::SentDirect,
                                message_id_hex,
                                resolved_destination_hex,
                                used_resource: true,
                                used_propagation_node: false,
                                method: method_value,
                                representation: representation_value,
                                relay_destination_hex: None,
                                fallback_stage: None,
                                receipt_hash_hex: None,
                            });
                        }
                        ResourceEventKind::Complete(_) => {}
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err(sdk_transport("resource event stream closed"));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Err(_) => return Err(sdk_transport("lxmf resource transfer timed out")),
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

    Ok(CompatSendReport {
        outcome,
        message_id_hex,
        resolved_destination_hex,
        used_resource: false,
        used_propagation_node: false,
        method: method_value,
        representation: representation_value,
        relay_destination_hex: None,
        fallback_stage: None,
        receipt_hash_hex: Some(receipt_hash_hex),
    })
}

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
        .pack_propagation_with_rng(
            &lxmf_identity(&remote_desc.identity),
            crate::runtime::now_ms() as f64 / 1000.0,
            OsRng,
        )
        .map_err(|_| sdk_internal("failed to encode propagated lxmf payload"))?;
    let relay_destination_hex = relay_desc.address_hash.to_hex_string();

    info!(
        "[lxmf][events][sdk] path=propagation requested_destination={} resolved_destination={} relay_destination={} message_id={} wire_bytes={} propagated_bytes={} max_wire_bytes={}",
        requested_destination_hex,
        resolved_destination_hex,
        relay_destination_hex,
        message_id_hex,
        wire.len(),
        propagated_payload.len(),
        LXMF_MAX_PAYLOAD,
    );

    if propagated_payload.len() > LXMF_MAX_PAYLOAD {
        let link = ensure_lxmf_output_link(state, relay_desc)
            .await
            .map_err(|_| sdk_transport("failed to activate propagation relay link"))?;
        let link_id = *link.lock().await.id();
        let mut resource_events = state.transport.resource_events();
        let resource_hash = state
            .transport
            .send_resource(&link_id, propagated_payload.clone(), None)
            .await
            .map_err(|_| sdk_transport("failed to start propagated lxmf relay resource transfer"))?;
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
                return Err(sdk_transport("propagated lxmf relay resource transfer timed out"));
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
                                used_resource: true,
                                used_propagation_node: true,
                                method,
                                representation,
                                relay_destination_hex: Some(relay_destination_hex.clone()),
                                fallback_stage,
                                receipt_hash_hex: None,
                            });
                        }
                        ResourceEventKind::Complete(_) => {}
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err(sdk_transport("propagation relay resource event stream closed"));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Err(_) => {
                    return Err(sdk_transport("propagated lxmf relay resource transfer timed out"));
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
    if !matches!(outcome, RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast) {
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
        used_resource: false,
        used_propagation_node: true,
        method,
        representation,
        relay_destination_hex: Some(relay_destination_hex),
        fallback_stage,
        receipt_hash_hex: None,
    })
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
) -> Result<Arc<TokioMutex<Link>>, NodeError> {
    const MAX_ATTEMPTS: usize = 3;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    for attempt in 0..MAX_ATTEMPTS {
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

        match wait_for_link_active(&state.transport, &link).await {
            Ok(()) => return Ok(link),
            Err(err) => {
                let stale = state.out_links.lock().await.remove(&desc.address_hash);
                if let Some(stale) = stale {
                    stale.lock().await.close();
                }
                info!(
                    "[lxmf][events][sdk] link activation failed destination={} attempt={} reason={}",
                    desc.address_hash.to_hex_string(),
                    attempt + 1,
                    err,
                );
                if attempt + 1 == MAX_ATTEMPTS {
                    return Err(err);
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(NodeError::Timeout {})
}

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
            used_resource: true,
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
        assert!(first.used_resource);
        assert!(second.used_resource);
    }

    #[test]
    fn compat_backend_caps_event_history() {
        let backend = CompatBackend::new_for_tests("runtime-test");

        for seq in 0..(COMPAT_EVENT_RETENTION_LIMIT + 8) {
            backend.record_peer_changed(format!("dest-{seq}").as_str(), "connected", None);
        }

        let state = backend.state.lock().expect("state lock");
        assert_eq!(state.events.len(), COMPAT_EVENT_RETENTION_LIMIT);
        assert_eq!(state.last_seq_no(), (COMPAT_EVENT_RETENTION_LIMIT + 8) as u64);
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
                    used_resource: false,
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
        assert!(state.send_reports.contains_key(&format!(
            "msg-{}",
            COMPAT_SEND_REPORT_RETENTION_LIMIT + 7
        )));
    }
}
