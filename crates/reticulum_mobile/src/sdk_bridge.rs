use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use log::{debug, info, warn};
use lxmf_runtime::{
    DeliveryMethod as RuntimeDeliveryMethod, DeliveryOutcome as RuntimeDeliveryOutcome,
    DeliveryRepresentation as RuntimeDeliveryRepresentation, InProcessBackend,
    InProcessBackendConfig,
};
use lxmf_sdk::{
    Client, DeliveryState, LxmfSdk, MessageId, SdkConfig, SdkError, SendRequest, Severity,
    ShutdownMode, StartRequest,
};
use rand_core::OsRng;
use reticulum::runtime::{ReceivedData, SendPacketOutcome as RnsSendOutcome, Transport};
use reticulum::transport::destination::link::{Link, LinkEvent, LinkStatus};
use reticulum::transport::destination::{DestinationDesc, DestinationName, SingleInputDestination};
use reticulum::transport::hash::{address_hash, AddressHash};
use reticulum::transport::identity::{DecryptIdentity, PrivateIdentity};
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
use crate::runtime::LxmfSendReport;
use crate::types::{
    HubDirectorySnapshot, LxmfDeliveryMethod, LxmfDeliveryRepresentation, NodeError, PeerState,
    SendMode,
};

const SDK_CAUSE_LXMF_PACKET_TOO_LARGE: &str = "LxmfPacketTooLarge";
const PROPAGATION_CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
const PROPAGATION_FETCH_CONTROL_TIMEOUT: Duration = Duration::from_secs(90);
const PROPAGATION_FETCH_BATCH_SIZE: usize = 1;
const PROPAGATION_PURGE_BATCH_SIZE: usize = 8;
const PROPAGATION_FETCH_TRANSFER_LIMIT_KB: u64 = 10_240;
fn metadata_is_accepted_result(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.result_present && metadata.result_status.as_deref() == Some("accepted")
    })
}

fn metadata_uses_compact_eam_tracking_marker(metadata: &MissionSyncMetadata) -> bool {
    metadata.command_type.as_deref() == Some("mission.registry.eam.upsert")
        && metadata.command_id.as_deref() == Some("m")
        && metadata
            .correlation_id
            .as_deref()
            .is_none_or(|value| value == "m")
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

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_PROPAGATION_NAME: (&str, &str) = ("lxmf", "propagation");
const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_LINK_CONNECT_ATTEMPTS: usize = 3;
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);

const EXT_FIELDS_BASE64: &str = "reticulum.fields_base64";
const EXT_RAW_BYTES_BASE64: &str = "reticulum.raw_bytes_base64";
const EXT_SEND_MODE: &str = "reticulum.send_mode";
const EXT_USE_PROPAGATION_NODE: &str = "reticulum.use_propagation_node";
const EXT_PROPAGATION_RELAY_HEX: &str = "reticulum.propagation_relay_hex";
const EXT_ACCEPTED_RESULT_ACK: &str = "reticulum.accepted_result_ack";
const EXT_LINK_CONNECT_TIMEOUT_MS: &str = "reticulum.link_connect_timeout_ms";
const EXT_DIRECT_PACKET_MAX_WIRE_BYTES: &str = "reticulum.direct_packet_max_wire_bytes";
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
    client: Arc<Client<InProcessBackend>>,
    transport: SdkTransportState,
}

impl RuntimeLxmfSdk {
    pub(crate) async fn new(runtime_id: String, transport: SdkTransportState) -> Self {
        let source_destination = transport.lxmf_destination.lock().await.desc.address_hash;
        let backend = InProcessBackend::new(InProcessBackendConfig::new(
            runtime_id,
            Handle::current(),
            transport.transport.clone(),
            transport.identity.clone(),
            source_destination,
        ));
        Self {
            client: Arc::new(Client::new(backend)),
            transport,
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

    pub(crate) async fn send_lxmf_via_propagation_relay(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<String>,
        fields_bytes: Option<Vec<u8>>,
        metadata: Option<MissionSyncMetadata>,
        propagation_relay_hex: String,
    ) -> Result<LxmfSendReport, NodeError> {
        self.send_lxmf_with_direct_attempt(
            destination,
            content,
            title,
            fields_bytes,
            metadata,
            SendMode::PropagationOnly {},
            None,
            None,
            None,
            Some(propagation_relay_hex),
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
        link_connect_timeout: Option<Duration>,
        direct_packet_max_wire_bytes: Option<usize>,
        propagation_relay_hex: Option<String>,
    ) -> Result<LxmfSendReport, NodeError> {
        let source = self
            .transport
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
        if let Some(propagation_relay_hex) = propagation_relay_hex
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request =
                request.with_extension(EXT_PROPAGATION_RELAY_HEX, json!(propagation_relay_hex));
        }
        if metadata_is_accepted_result(metadata.as_ref()) {
            request = request.with_extension(EXT_ACCEPTED_RESULT_ACK, json!(true));
        }
        if let Some(link_connect_timeout) = link_connect_timeout {
            let timeout_ms = link_connect_timeout.as_millis().min(u128::from(u64::MAX)) as u64;
            request = request.with_extension(EXT_LINK_CONNECT_TIMEOUT_MS, json!(timeout_ms));
        }
        if let Some(direct_packet_max_wire_bytes) = direct_packet_max_wire_bytes {
            request = request.with_extension(
                EXT_DIRECT_PACKET_MAX_WIRE_BYTES,
                json!(direct_packet_max_wire_bytes),
            );
        }
        if let Some(correlation_id) = metadata
            .as_ref()
            .and_then(|value| value.correlation_id.clone())
        {
            request = request.with_correlation_id(correlation_id);
        }
        if let Some(idempotency_key) = metadata.as_ref().and_then(|value| {
            (!metadata_uses_compact_eam_tracking_marker(value))
                .then(|| value.tracking_key().map(ToOwned::to_owned))
                .flatten()
        }) {
            request = request.with_idempotency_key(idempotency_key_for_send_attempt(
                &idempotency_key,
                send_mode,
                direct_attempt,
            ));
        }

        let active_relay = self
            .transport
            .active_propagation_node_hex
            .lock()
            .await
            .clone()
            .map(|value| parse_address_hash(value.trim()))
            .transpose()?;
        self.client
            .backend()
            .set_propagation_relay(active_relay)
            .map_err(map_sdk_error_to_node_error)?;

        let client = self.client.clone();
        let message_id = tokio::task::spawn_blocking(move || client.send(request))
            .await
            .map_err(|_| NodeError::InternalError {})?
            .map_err(|err| {
                warn!("in-process LXMF send failed destination={requested_destination_hex}: {err}");
                map_sdk_error_to_node_error(err)
            })?;
        let report = self
            .client
            .backend()
            .send_report(&message_id)
            .map_err(map_sdk_error_to_node_error)?
            .ok_or(NodeError::InternalError {})?;

        let outcome = match report.outcome {
            RuntimeDeliveryOutcome::SentDirect => RnsSendOutcome::SentDirect,
            RuntimeDeliveryOutcome::SentBroadcast => RnsSendOutcome::SentBroadcast,
        };
        let method = match report.method {
            RuntimeDeliveryMethod::Opportunistic => LxmfDeliveryMethod::Opportunistic {},
            RuntimeDeliveryMethod::Direct => LxmfDeliveryMethod::Direct {},
            RuntimeDeliveryMethod::Propagated => LxmfDeliveryMethod::Propagated {},
        };
        let representation = match report.representation {
            RuntimeDeliveryRepresentation::Packet => LxmfDeliveryRepresentation::Packet {},
            RuntimeDeliveryRepresentation::Resource => LxmfDeliveryRepresentation::Resource {},
        };

        if let Some(metadata) = metadata.as_ref().filter(|value| value.is_event_related()) {
            info!(
                "[lxmf][events][sdk] attempting send requested_destination={} resolved_destination={} kind={} name={} message_id={} event_uid={} mission_uid={} correlation={}",
                requested_destination_hex,
                report.resolved_destination,
                metadata.primary_kind(),
                metadata.primary_name().unwrap_or("-"),
                report.message_id.0,
                metadata.event_uid.as_deref().unwrap_or("-"),
                metadata.mission_uid.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
            );
        }

        let track_delivery_timeout = metadata
            .as_ref()
            .is_some_and(|value| value.command_present && value.tracking_key().is_some());

        Ok(LxmfSendReport {
            outcome,
            message_id_hex: report.message_id.0,
            resolved_destination_hex: report.resolved_destination,
            metadata,
            track_delivery_timeout,
            used_propagation_node: matches!(method, LxmfDeliveryMethod::Propagated {}),
            method,
            representation,
            relay_destination_hex: report.relay_destination,
            fallback_stage: None,
            receipt_hash_hex: report.receipt_hash,
        })
    }

    pub(crate) async fn fetch_propagated_lxmf_from_relay(
        &self,
        relay_hex: &str,
        limit: Option<u32>,
        direct_iface_hex: Option<&str>,
    ) -> Result<PropagationFetchResult, NodeError> {
        let relay_hex = relay_hex.trim();
        if relay_hex.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        compat_fetch_propagated_lxmf(&self.transport, relay_hex, limit, direct_iface_hex).await
    }

    fn record_event(&self, event_type: &str, severity: Severity, payload: JsonValue) {
        if let Err(err) = self
            .client
            .backend()
            .record_event(event_type, severity, payload)
        {
            warn!("failed to record {event_type} SDK event: {err}");
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery events preserve routing and mission correlation fields"
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
        let message_id = MessageId(message_id_hex.to_owned());
        if let Err(err) = self.client.backend().record_delivery(
            &message_id,
            delivery_state.clone(),
            detail.map(ToOwned::to_owned),
        ) {
            warn!("failed to update SDK delivery {message_id_hex}: {err}");
            return;
        }
        self.record_event(
            EVENT_DELIVERY_UPDATED,
            if matches!(
                delivery_state,
                DeliveryState::Failed | DeliveryState::Rejected | DeliveryState::Expired
            ) {
                Severity::Warn
            } else {
                Severity::Info
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

    pub(crate) fn record_packet_received(
        &self,
        destination_hex: &str,
        source_hex: Option<&str>,
        bytes: &[u8],
        fields_bytes: Option<&[u8]>,
    ) {
        self.record_event(
            EVENT_PACKET_RECEIVED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "source_hex": source_hex,
                "bytes_base64": BASE64_STANDARD.encode(bytes),
                "fields_base64": fields_bytes.map(|value| BASE64_STANDARD.encode(value)),
            }),
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
        self.record_event(
            EVENT_ANNOUNCE_RECEIVED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "identity_hex": identity_hex,
                "destination_kind": destination_kind,
                "app_data": app_data,
                "hops": hops,
                "interface_hex": interface_hex,
            }),
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
        self.record_event(
            EVENT_PEER_CHANGED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "state": state_name,
                "last_error": last_error,
            }),
        );
    }

    pub(crate) fn record_hub_directory_updated(&self, snapshot: &HubDirectorySnapshot) {
        self.record_event(
            EVENT_HUB_DIRECTORY_UPDATED,
            Severity::Info,
            json!({
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
            }),
        );
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
        self.record_delivery_update(
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
        self.record_delivery_update(
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
        self.record_delivery_update(
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
        self.record_delivery_update(
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
    use crate::runtime::lxmf_private_identity;
    use lxmf::message::{Payload, WireMessage as LxmfWireMessage};
    use reticulum::transport::destination::SingleOutputDestination;
    use reticulum::transport::identity::EncryptIdentity;

    const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");

    fn lxmf_identity(
        identity: &reticulum::transport::identity::Identity,
    ) -> lxmf::identity::Identity {
        lxmf::identity::Identity::new_from_slices(
            identity.public_key_bytes(),
            identity.verifying_key_bytes(),
        )
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
