use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::announce_metadata::{
    normalize_rem_display_name, parse_announce_metadata, supports_mission_traffic,
};
use crate::delivery_policy;
use crate::delivery_policy::normalize_hex_32;
use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_RESULTS};
use crate::messaging_compat as sdkmsg;
use crate::mission_commands::{canonical_command_type, checklist_arg_code};
use crate::mission_sync::{parse_mission_sync_metadata, MissionSyncMetadata};
use crate::msgpack_values::{
    msgpack_bool, msgpack_f64, msgpack_get_indexed, msgpack_get_named, msgpack_hex_or_string,
    msgpack_map_entries, msgpack_string,
};
use crate::sos::{location_from_alert, received_alert_from_sos};
use crate::sos_fields::{extract_text_coordinates, parse_sos_fields, sos_kind_from_text};
use crossbeam_channel as cb;
use flate2::read::ZlibDecoder;
use fs_err as fs;
use log::{debug, info, warn};
use lxmf::announce::display_name_from_delivery_app_data;
#[cfg(test)]
use lxmf::announce::encode_delivery_display_name_app_data;
use lxmf::message::Message as LxmfMessage;
use lxmf::message::WireMessage as LxmfWireMessage;
use lxmf_sdk::messaging::AnnounceRecord as LxmfSdkAnnounceRecord;
use rand_core::OsRng;
use reticulum::runtime::{
    DeliveryReceipt, ReceiptHandler, SendPacketOutcome as RnsSendOutcome, Transport,
    TransportConfig,
};
use reticulum::transport::destination::link::{Link, LinkEvent, LinkStatus};
use reticulum::transport::destination::{
    DestinationDesc, DestinationName, SingleInputDestination, SingleOutputDestination,
};
use reticulum::transport::hash::AddressHash;
use reticulum::transport::identity::PrivateIdentity;
use reticulum::transport::iface::tcp_client::TcpClient;
use reticulum::transport::packet::{Packet, PacketDataBuffer, PacketType, PropagationType};
use reticulum::transport::resource::ResourceEventKind;
use rmpv::Value as MsgPackValue;
#[cfg(target_os = "android")]
use rns_transport::iface::lora::LoraConfig;
#[cfg(target_os = "android")]
use rns_transport::iface::rnode_ble::{
    NativeRnodeBleKissInterface, NativeRnodeBleSettings, RnodeBleKissConfig,
    RNODE_BLE_READ_FRAME_TIMEOUT,
};
#[cfg(target_os = "android")]
use rns_transport::iface::{IfaceRole, InterfaceMode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tokio::net::{lookup_host, TcpStream};
use tokio::sync::{mpsc, Mutex as TokioMutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};

#[path = "../runtime_projection.rs"]
mod runtime_projection;

use crate::app_state::{
    canonicalize_chat_message, checklist_task_status_for, find_checklist_task_mut,
    normalize_checklist_record, normalize_optional_string, set_checklist_last_changed_by,
    AppStateStore, ConversationPeerResolver,
};
use crate::event_bus::EventBus;
use crate::sdk_bridge::{RuntimeLxmfSdk, SdkTransportState};
use crate::types::{
    AnnounceClass, AnnounceRecord, ApplicationAckState, ChecklistCellRecord, ChecklistColumnRecord,
    ChecklistColumnType, ChecklistRecord, ChecklistSyncState, ChecklistSystemColumnKey,
    ChecklistTaskRecord, ChecklistTaskStatus, ChecklistUserTaskStatus, ConversationRecord,
    EamProjectionRecord, EamSourceRecord, EventProjectionRecord, HubCallerMembershipRecord,
    HubDirectoryPeerRecord, HubDirectorySnapshot, HubMode, HubTeamMemberRecord, HubTeamRecord,
    InterfaceStatusRecord, LogLevel, LxmfDeliveryMethod,
    LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfDeliveryUpdate, LxmfFallbackStage,
    MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent,
    NodeStatus, OperationalNotice, PeerChange, PeerRecord, PeerState, ProjectionScope,
    RnodeConnectionMode, RnodeSettingsRecord, RuntimeReadinessState, SavedPeerRecord,
    SendLxmfRequest, SendMode, SendOutcome, SosDeviceTelemetryRecord, SosMessageKind, SyncPhase,
    SyncStatus, TelemetryPositionRecord, TransportDeliveryState, YELLOW_TEAM_UID,
    HUB_DIRECTORY_SCHEMA_VERSION, canonical_team_color_for_uid,
};

use self::runtime_projection::RuntimeProjectionJournal;

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const DESTINATION_KIND_APP: &str = "app";
const DESTINATION_KIND_LXMF_DELIVERY: &str = "lxmf_delivery";
const DESTINATION_KIND_LXMF_PROPAGATION: &str = "lxmf_propagation";
const DESTINATION_KIND_OTHER: &str = "other";
const TCP_CLIENT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const TCP_CLIENT_INTERFACE_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const TCP_CLIENT_READINESS_CHECK_INTERVAL: Duration = Duration::from_secs(30);
#[cfg(target_os = "android")]
const RNODE_BLE_INTERFACE_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const LXMF_PROPAGATION_NAME: (&str, &str) = ("lxmf", "propagation");
const STARTUP_ANNOUNCE_DELAYS_SECS: [u64; 3] = [0, 10, 30];
const MIN_EFFECTIVE_ANNOUNCE_INTERVAL_SECONDS: u32 = 60;
const PEER_PRESENCE_GRACE_SECONDS: u32 = 60;
const INTERFACE_TRAFFIC_LOG_INTERVAL: Duration = Duration::from_secs(60);
const PASSIVE_PEER_RESOLUTION_MIN_INTERVAL_MS: u64 = 10_000;
const SAVED_PEER_ROUTE_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const SAVED_PEER_LINK_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(10);
const SAVED_PEER_LINK_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MANAGED_PEER_LINK_RECONNECT_TIMEOUT: Duration = Duration::from_secs(80);
const SAVED_PEER_LINK_BACKOFF_BASE_MS: u64 = 2_000;
const SAVED_PEER_LINK_BACKOFF_MAX_MS: u64 = 60_000;
const SAVED_PEER_LINK_BACKOFF_MAX_ATTEMPTS: u32 = 6;
const AUTO_PROPAGATION_SYNC_INTERVAL: Duration = Duration::from_secs(30);
const AUTO_PROPAGATION_SYNC_LIMIT: u32 = 100;
const RCH_SERVER_FEATURE_CAPABILITIES: [&str; 5] = [
    "topic_broker",
    "group_chat",
    "attachments",
    "telemetry_relay",
    "tak_bridge",
];

const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const RNODE_BLE_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(75);
const RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES: usize = 145;
const RNODE_BLE_RESOURCE_RETRY_INTERVAL_SECS: u64 = 8;
const RNODE_BLE_RESOURCE_RETRY_LIMIT: u8 = 24;
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);
const DEFAULT_LXMF_ACK_TIMEOUT: Duration = Duration::from_secs(30);
const PROPAGATED_LXMF_ACK_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
const DEFAULT_BUFFERED_ACK_TTL: Duration = Duration::from_secs(5 * 60);
const DEFAULT_RECEIPT_TRACKING_TTL: Duration = Duration::from_secs(10 * 60);
const PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS: usize = 6;
#[cfg(not(test))]
const PROPAGATION_SYNC_RELAY_SELECTION_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
const PROPAGATION_SYNC_RELAY_SELECTION_WAIT: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const PROPAGATION_SYNC_RELAY_SELECTION_POLL: Duration = Duration::from_millis(500);
#[cfg(test)]
const PROPAGATION_SYNC_RELAY_SELECTION_POLL: Duration = Duration::from_millis(10);
const SEND_TASK_CONCURRENCY_LIMIT: usize = 8;
const MISSION_SEND_TASK_RESERVED_LIMIT: usize = 2;
const MISSION_ACK_SEND_TASK_RESERVED_LIMIT: usize = 1;
const MISSION_PROPAGATION_SEND_TASK_RESERVED_LIMIT: usize = 1;
const MISSION_RECOVERY_SEND_TASK_RESERVED_LIMIT: usize = 3;
const GENERAL_SEND_TASK_CONCURRENCY_LIMIT: usize = SEND_TASK_CONCURRENCY_LIMIT
    - MISSION_SEND_TASK_RESERVED_LIMIT
    - MISSION_ACK_SEND_TASK_RESERVED_LIMIT
    - MISSION_PROPAGATION_SEND_TASK_RESERVED_LIMIT
    - MISSION_RECOVERY_SEND_TASK_RESERVED_LIMIT;
const LXMF_DIRECT_ATTEMPTS: usize = 5;
const DIRECT_DELIVERY_FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
const MISSION_DIRECT_PRIORITY_FREE_HOPS: u8 = 2;
const MISSION_DIRECT_PRIORITY_DELAY_PER_HOP: Duration = Duration::from_millis(80);
const MISSION_DIRECT_PRIORITY_MAX_DELAY: Duration = Duration::from_millis(800);
const CHAT_DELIVERY_ACK_TITLE: &str = "REM delivery ack";
const CHAT_DELIVERY_ACK_PREFIX: &str = "REM_DELIVERY_ACK:";
const DEFAULT_EAM_GROUP_NAME: &str = "YELLOW";
const DEFAULT_R3AKT_MISSION_UID: &str = "r3akt-default-mission";

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| crate::numeric::u128_to_u64_saturating(d.as_millis()))
        .unwrap_or(0)
}

fn current_timestamp_rfc3339() -> String {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| crate::numeric::u64_to_i64_saturating(duration.as_secs()))
        .unwrap_or(0);
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn timestamp_ms_to_rfc3339(timestamp_ms: u64) -> String {
    let seconds_since_epoch = crate::numeric::u64_to_i64_saturating(timestamp_ms / 1_000);
    let millis = timestamp_ms % 1_000;
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}
