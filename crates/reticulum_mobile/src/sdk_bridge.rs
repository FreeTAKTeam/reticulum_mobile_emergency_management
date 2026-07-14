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

use crate::delivery_policy::normalize_hex_32;
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

include!("sdk_bridge/runtime_send.rs");
include!("sdk_bridge/event_recording.rs");

include!("sdk_bridge/propagation_fetch.rs");
include!("sdk_bridge/link_control.rs");
include!("sdk_bridge/propagation_crypto.rs");
include!("sdk_bridge/link_resolution.rs");

#[cfg(test)]
mod tests {
    include!("sdk_bridge/tests/propagation.rs");
}
