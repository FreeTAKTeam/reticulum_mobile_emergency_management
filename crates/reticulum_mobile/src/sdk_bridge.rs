use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use log::{info, warn};
use lxmf_runtime::{
    DeliveryMethod as RuntimeDeliveryMethod, DeliveryOutcome as RuntimeDeliveryOutcome,
    DeliveryRepresentation as RuntimeDeliveryRepresentation, InProcessBackend,
    InProcessBackendConfig,
};
use lxmf_sdk::{
    Client, DeliveryState, LxmfSdk, MessageId, SdkConfig, SdkError, SendRequest, Severity,
    ShutdownMode, StartRequest,
};
use reticulum::runtime::{SendPacketOutcome as RnsSendOutcome, Transport};
use reticulum::transport::destination::SingleInputDestination;
use reticulum::transport::hash::AddressHash;
use reticulum::transport::identity::PrivateIdentity;
use serde_json::{json, Value as JsonValue};
use tokio::runtime::Handle;
use tokio::sync::Mutex as TokioMutex;

use crate::delivery_policy::normalize_hex_32;
use crate::mission_sync::MissionSyncMetadata;
use crate::runtime::LxmfSendReport;
use crate::types::{
    HubDirectorySnapshot, LxmfDeliveryMethod, LxmfDeliveryRepresentation, NodeError, PeerState,
    SendMode,
};

const SDK_CAUSE_LXMF_PACKET_TOO_LARGE: &str = "LxmfPacketTooLarge";
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

pub(crate) fn map_sdk_error_to_node_error(err: SdkError) -> NodeError {
    match err.cause_code.as_deref() {
        Some("UnresolvedRecipient") => return NodeError::UnresolvedRecipient {},
        Some("PayloadTooLarge") => return NodeError::PayloadTooLarge {},
        Some("StoreFull") => return NodeError::StoreFull {},
        Some("Expired") => return NodeError::Expired {},
        _ => {}
    }
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
    pub(crate) active_propagation_node_hex: Arc<TokioMutex<Option<String>>>,
}

pub(crate) struct RuntimeLxmfSdk {
    client: Arc<Client<InProcessBackend>>,
    transport: SdkTransportState,
    send_sequence: AtomicU64,
    pub(crate) local_recovery_inflight: Arc<AtomicBool>,
    pub(crate) local_propagation: Option<lxmf_runtime::LocalPropagation>,
}

include!("sdk_bridge/runtime_send.rs");
include!("sdk_bridge/local_propagation.rs");
include!("sdk_bridge/event_recording.rs");

include!("sdk_bridge/link_resolution.rs");
