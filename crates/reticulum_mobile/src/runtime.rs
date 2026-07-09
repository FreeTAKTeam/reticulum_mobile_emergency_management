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
use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_RESULTS};
use crate::messaging_compat as sdkmsg;
use crate::mission_commands::{canonical_command_type, checklist_arg_code};
use crate::mission_sync::{parse_mission_sync_metadata, MissionSyncMetadata};
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

#[path = "runtime_projection.rs"]
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
    EamProjectionRecord, EamSourceRecord, EventProjectionRecord, HubDirectoryPeerRecord,
    HubDirectorySnapshot, HubMode, InterfaceStatusRecord, LogLevel, LxmfDeliveryMethod,
    LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfDeliveryUpdate, LxmfFallbackStage,
    MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig, NodeError, NodeEvent,
    NodeStatus, OperationalNotice, PeerChange, PeerRecord, PeerState, ProjectionScope,
    RnodeConnectionMode, RnodeSettingsRecord, RuntimeReadinessState, SavedPeerRecord,
    SendLxmfRequest, SendMode, SendOutcome, SosDeviceTelemetryRecord, SosMessageKind, SyncPhase,
    SyncStatus, TelemetryPositionRecord, TransportDeliveryState,
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
const MIN_EFFECTIVE_ANNOUNCE_INTERVAL_SECONDS: u32 = 3600;
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
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn current_timestamp_rfc3339() -> String {
    let seconds_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
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
    let seconds_since_epoch = (timestamp_ms / 1_000) as i64;
    let millis = timestamp_ms % 1_000;
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

#[derive(Debug, Clone)]
struct OperationalAck {
    destination_hex: String,
    command_id: String,
    correlation_id: Option<String>,
    command_type: Option<String>,
}

fn operational_ack_from_metadata(
    source_hex: Option<&str>,
    metadata: Option<&MissionSyncMetadata>,
) -> Option<OperationalAck> {
    let metadata = metadata?;
    if metadata.result_present || !metadata.command_present {
        return None;
    }
    let destination_hex = normalize_hex_32(source_hex?)?;
    let command_id = metadata
        .command_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    Some(OperationalAck {
        destination_hex,
        command_id,
        correlation_id: metadata
            .correlation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        command_type: metadata
            .command_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
fn build_operational_ack_fields(
    ack: &OperationalAck,
    by_identity: &str,
) -> Result<Vec<u8>, NodeError> {
    let mut result_entries = vec![
        (
            MsgPackValue::from("command_id"),
            MsgPackValue::from(ack.command_id.as_str()),
        ),
        (MsgPackValue::from("status"), MsgPackValue::from("accepted")),
        (
            MsgPackValue::from("accepted_at"),
            MsgPackValue::from(current_timestamp_rfc3339().as_str()),
        ),
        (
            MsgPackValue::from("by_identity"),
            MsgPackValue::from(by_identity),
        ),
    ];
    if let Some(correlation_id) = ack.correlation_id.as_deref() {
        result_entries.push((
            MsgPackValue::from("correlation_id"),
            MsgPackValue::from(correlation_id),
        ));
    }
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(result_entries),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
}

fn compact_event_uid_ack_value(command_id: &str) -> Option<MsgPackValue> {
    let value = command_id.strip_prefix("log-entry-")?;
    let event_uid = if value.starts_with("evt-") && value.len() >= 40 {
        &value[..40]
    } else {
        value
    };
    let normalized = event_uid
        .trim_start_matches("evt-")
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if normalized.len() != 32 {
        return None;
    }
    hex::decode(normalized).ok().map(MsgPackValue::Binary)
}

fn build_compact_operational_ack_fields(ack: &OperationalAck) -> Result<Vec<u8>, NodeError> {
    let result_entries = if ack.command_type.as_deref() == Some("mission.registry.log_entry.upsert")
    {
        if let Some(event_uid) = compact_event_uid_ack_value(ack.command_id.as_str()) {
            vec![(MsgPackValue::from("u"), event_uid)]
        } else {
            vec![
                (
                    MsgPackValue::from("i"),
                    MsgPackValue::from(ack.command_id.as_str()),
                ),
                (MsgPackValue::from("s"), MsgPackValue::from("a")),
            ]
        }
    } else {
        vec![
            (
                MsgPackValue::from("i"),
                MsgPackValue::from(ack.command_id.as_str()),
            ),
            (MsgPackValue::from("s"), MsgPackValue::from("a")),
        ]
    };
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(result_entries),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
}

fn telemetry_position_from_sos(
    callsign: &str,
    telemetry: Option<&SosDeviceTelemetryRecord>,
    fallback_updated_at_ms: u64,
) -> Option<TelemetryPositionRecord> {
    let telemetry = telemetry?;
    let lat = telemetry.lat?;
    let lon = telemetry.lon?;
    let callsign = callsign.trim();
    if callsign.is_empty() {
        return None;
    }

    Some(TelemetryPositionRecord {
        callsign: callsign.to_ascii_lowercase(),
        lat,
        lon,
        alt: telemetry.alt,
        course: telemetry.course,
        speed: telemetry.speed,
        accuracy: telemetry.accuracy,
        updated_at_ms: if telemetry.updated_at_ms > 0 {
            telemetry.updated_at_ms
        } else {
            fallback_updated_at_ms
        },
    })
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn eam_status_rank(value: &str) -> u8 {
    match value {
        "Green" => 1,
        "Yellow" => 2,
        "Red" => 3,
        _ => 0,
    }
}

fn derive_eam_overall_status(record: &EamProjectionRecord) -> Option<String> {
    let mut best_status: Option<&str> = None;
    for value in [
        record.security_status.as_str(),
        record.capability_status.as_str(),
        record.preparedness_status.as_str(),
        record.medical_status.as_str(),
        record.mobility_status.as_str(),
        record.comms_status.as_str(),
    ] {
        if eam_status_rank(value) >= eam_status_rank(best_status.unwrap_or_default()) {
            best_status = Some(value);
        }
    }
    best_status
        .filter(|value| !value.is_empty() && *value != "Unknown")
        .map(str::to_string)
}

#[derive(Debug)]
enum EamCommandAction {
    Upsert(Box<EamProjectionRecord>),
    Delete {
        callsign: String,
        deleted_at_ms: u64,
    },
}

fn eam_command_action_from_command(
    envelope: MissionCommandEnvelope<EamUpsertCommandArgs>,
    projection: Option<EamProjectionRecord>,
    received_at_ms: u64,
) -> Option<EamCommandAction> {
    let command_type = canonical_command_type(envelope.command_type.as_str());
    if command_type != "mission.registry.eam.upsert" {
        return None;
    }

    if let Some(mut projection) = projection {
        if projection.callsign.trim().is_empty() {
            return None;
        }
        projection.group_name = if projection.group_name.trim().is_empty() {
            DEFAULT_EAM_GROUP_NAME.to_string()
        } else {
            projection.group_name.trim().to_string()
        };
        if projection.overall_status.is_none() {
            projection.overall_status = derive_eam_overall_status(&projection);
        }
        projection.sync_state = Some("synced".to_string());
        projection.sync_error = None;
        projection.last_synced_at_ms = Some(received_at_ms);
        projection.updated_at_ms = projection.updated_at_ms.max(received_at_ms);
        return Some(EamCommandAction::Upsert(Box::new(projection)));
    }

    if envelope.args.callsign.trim().is_empty()
        || envelope.args.team_member_uid.trim().is_empty()
        || envelope.args.team_uid.trim().is_empty()
    {
        return None;
    }

    let mut record = EamProjectionRecord {
        callsign: envelope.args.callsign.trim().to_string(),
        group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
        security_status: envelope.args.security_status,
        capability_status: envelope.args.capability_status,
        preparedness_status: envelope.args.preparedness_status,
        medical_status: envelope.args.medical_status,
        mobility_status: envelope.args.mobility_status,
        comms_status: envelope.args.comms_status,
        notes: envelope.args.notes,
        updated_at_ms: received_at_ms,
        deleted_at_ms: None,
        eam_uid: envelope.args.eam_uid,
        team_member_uid: Some(envelope.args.team_member_uid),
        team_uid: Some(envelope.args.team_uid),
        reported_at: envelope.args.reported_at.or(Some(envelope.timestamp)),
        reported_by: envelope
            .args
            .reported_by
            .or(envelope.source.display_name.clone()),
        overall_status: None,
        confidence: envelope.args.confidence,
        ttl_seconds: envelope.args.ttl_seconds,
        source: Some(EamSourceRecord {
            rns_identity: envelope
                .args
                .source
                .as_ref()
                .map(|value| value.rns_identity.clone())
                .unwrap_or(envelope.source.rns_identity),
            display_name: envelope
                .args
                .source
                .and_then(|value| value.display_name)
                .or(envelope.source.display_name),
        }),
        sync_state: Some("synced".to_string()),
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: Some(received_at_ms),
    };
    record.overall_status = derive_eam_overall_status(&record);
    Some(EamCommandAction::Upsert(Box::new(record)))
}

fn compact_eam_fallback_callsign(
    explicit: Option<String>,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<String> {
    explicit
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            source_display_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            source_hex
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.chars().take(8).collect())
        })
}

fn compact_eam_fallback_team_member_uid(
    explicit: Option<String>,
    source_hex: Option<&str>,
) -> Option<String> {
    explicit
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            source_hex
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn compact_eam_status_char(value: char) -> Option<String> {
    match value {
        'G' => Some("Green".to_string()),
        'Y' => Some("Yellow".to_string()),
        'R' => Some("Red".to_string()),
        'U' => Some("Unknown".to_string()),
        _ => None,
    }
}

fn compact_eam_action_from_body(
    body_utf8: &str,
    received_at_ms: u64,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<EamCommandAction> {
    let mut parts = body_utf8.trim().split('|');
    if parts.next()? != "E" {
        return None;
    }
    let callsign = compact_eam_fallback_callsign(
        parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        source_hex,
        source_display_name,
    )?;
    let status_codes = parts.next()?.trim();
    if parts.next().is_some() || status_codes.chars().count() != 6 {
        return None;
    }
    let mut statuses = status_codes.chars().map(compact_eam_status_char);
    let mut record = EamProjectionRecord {
        callsign,
        group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
        security_status: statuses.next()??,
        capability_status: statuses.next()??,
        preparedness_status: statuses.next()??,
        medical_status: statuses.next()??,
        mobility_status: statuses.next()??,
        comms_status: statuses.next()??,
        notes: None,
        updated_at_ms: received_at_ms,
        deleted_at_ms: None,
        eam_uid: None,
        team_member_uid: compact_eam_fallback_team_member_uid(None, source_hex),
        team_uid: None,
        reported_at: None,
        reported_by: source_display_name.map(str::to_string),
        overall_status: None,
        confidence: None,
        ttl_seconds: None,
        source: source_hex.map(|source_hex| EamSourceRecord {
            rns_identity: source_hex.to_string(),
            display_name: source_display_name.map(str::to_string),
        }),
        sync_state: Some("synced".to_string()),
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: Some(received_at_ms),
    };
    record.overall_status = derive_eam_overall_status(&record);
    Some(EamCommandAction::Upsert(Box::new(record)))
}

fn eam_command_action_from_fields(
    fields_bytes: &[u8],
    received_at_ms: u64,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<EamCommandAction> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())?;
        if command_type == "mission.registry.eam.delete" {
            let args =
                msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
            let callsign = msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string)?;
            if callsign.trim().is_empty() {
                return None;
            }
            let deleted_at_ms = msgpack_get_named(args, &["deleted_at_ms", "d"])
                .and_then(msgpack_u64)
                .unwrap_or(received_at_ms);
            return Some(EamCommandAction::Delete {
                callsign,
                deleted_at_ms,
            });
        }
        if command_type != "mission.registry.eam.upsert" {
            continue;
        }
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(command_map, &["source", "s"])
            .and_then(msgpack_map_entries)
            .or_else(|| msgpack_get_named(args, &["source", "s"]).and_then(msgpack_map_entries));
        let field_source_display_name = source
            .and_then(|source_map| msgpack_get_named(source_map, &["display_name", "n"]))
            .and_then(msgpack_string);
        let source_display_name = field_source_display_name
            .clone()
            .or_else(|| source_display_name.map(str::to_string));
        let callsign = compact_eam_fallback_callsign(
            msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string),
            source_hex,
            source_display_name.as_deref(),
        )?;
        let team_member_uid = compact_eam_fallback_team_member_uid(
            msgpack_get_named(args, &["team_member_uid", "tm"]).and_then(msgpack_hex_or_string),
            source_hex,
        );
        let compact_statuses = msgpack_eam_status_array(args);
        let mut record = EamProjectionRecord {
            callsign,
            group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
            security_status: compact_statuses[0]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["security_status", "ss"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            capability_status: compact_statuses[1]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["capability_status", "ca"])
                        .and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            preparedness_status: compact_statuses[2]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["preparedness_status", "pr"])
                        .and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            medical_status: compact_statuses[3]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["medical_status", "me"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            mobility_status: compact_statuses[4]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["mobility_status", "mo"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            comms_status: compact_statuses[5]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["comms_status", "co"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            notes: msgpack_get_named(args, &["notes", "no"]).and_then(msgpack_string),
            updated_at_ms: received_at_ms,
            deleted_at_ms: None,
            eam_uid: msgpack_get_named(args, &["eam_uid", "u"]).and_then(msgpack_eam_uid),
            team_member_uid,
            team_uid: msgpack_get_named(args, &["team_uid", "tu"]).and_then(msgpack_string),
            reported_at: msgpack_get_named(args, &["reported_at", "ra"]).and_then(msgpack_string),
            reported_by: msgpack_get_named(args, &["reported_by", "rb"])
                .and_then(msgpack_string)
                .or_else(|| source_display_name.clone()),
            overall_status: msgpack_get_named(args, &["overall_status", "os"])
                .and_then(msgpack_eam_status),
            confidence: msgpack_get_named(args, &["confidence", "cf"]).and_then(msgpack_f64),
            ttl_seconds: msgpack_get_named(args, &["ttl_seconds", "ttl"]).and_then(msgpack_u64),
            source: source
                .map(|source_map| EamSourceRecord {
                    rns_identity: msgpack_get_named(source_map, &["rns_identity", "r"])
                        .and_then(msgpack_hex_or_string)
                        .or_else(|| source_hex.map(str::to_string))
                        .unwrap_or_default(),
                    display_name: source_display_name.clone(),
                })
                .or_else(|| {
                    source_hex.map(|source_hex| EamSourceRecord {
                        rns_identity: source_hex.to_string(),
                        display_name: source_display_name,
                    })
                }),
            sync_state: Some("synced".to_string()),
            sync_error: None,
            draft_created_at_ms: None,
            last_synced_at_ms: Some(received_at_ms),
        };
        if record.callsign.trim().is_empty() {
            return None;
        }
        record.overall_status = derive_eam_overall_status(&record);
        return Some(EamCommandAction::Upsert(Box::new(record)));
    }

    None
}

async fn persist_received_eam_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    body_utf8: &str,
    source_hex: Option<&str>,
) -> bool {
    let received_at_ms = now_ms();
    let source_display_name = if let Some(source_hex) = source_hex {
        state
            .messaging
            .lock()
            .await
            .peer_by_destination(source_hex)
            .and_then(|peer| peer.display_name)
    } else {
        None
    };
    let parsed_from_fields = fields_bytes
        .and_then(|value| {
            eam_command_action_from_fields(
                value,
                received_at_ms,
                source_hex,
                source_display_name.as_deref(),
            )
        })
        .or_else(|| {
            metadata
                .and_then(|value| value.command_type.as_deref())
                .filter(|value| *value == "mission.registry.eam.upsert")
                .and_then(|_| {
                    compact_eam_action_from_body(
                        body_utf8,
                        received_at_ms,
                        source_hex,
                        source_display_name.as_deref(),
                    )
                })
        });
    if metadata.is_none() && parsed_from_fields.is_none() {
        return false;
    }
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| {
            value != "mission.registry.eam.upsert" && value != "mission.registry.eam.delete"
        })
        && parsed_from_fields.is_none()
    {
        return false;
    }

    let parsed = serde_json::from_str::<EamWireBody>(body_utf8)
        .ok()
        .and_then(|body| {
            eam_command_action_from_command(body.command, body.projection, received_at_ms)
        })
        .or_else(|| {
            serde_json::from_str::<MissionCommandEnvelope<EamUpsertCommandArgs>>(body_utf8)
                .ok()
                .and_then(|command| eam_command_action_from_command(command, None, received_at_ms))
        })
        .or(parsed_from_fields);

    let Some(action) = parsed else {
        return false;
    };

    match action {
        EamCommandAction::Upsert(record) => match state.app_state.upsert_eam(record.as_ref()) {
            Ok(invalidation) => {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                if let Ok(summary) = state.app_state.bump_projection_revision(
                    ProjectionScope::OperationalSummary {},
                    None,
                    Some("eam-received".to_string()),
                ) {
                    bus.emit(NodeEvent::ProjectionInvalidated {
                        invalidation: summary,
                    });
                }
                true
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to persist inbound eam callsign={} reason={}",
                        record.callsign, err
                    ),
                });
                false
            }
        },
        EamCommandAction::Delete {
            callsign,
            deleted_at_ms,
        } => match state.app_state.delete_eam(&callsign, deleted_at_ms) {
            Ok(invalidation) => {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                if let Ok(summary) = state.app_state.bump_projection_revision(
                    ProjectionScope::OperationalSummary {},
                    None,
                    Some("eam-deleted".to_string()),
                ) {
                    bus.emit(NodeEvent::ProjectionInvalidated {
                        invalidation: summary,
                    });
                }
                true
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to delete inbound eam callsign={} reason={}",
                        callsign, err
                    ),
                });
                false
            }
        },
    }
}

fn expand_event_wire_content(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.contains('/') || trimmed.is_empty() {
        trimmed.to_string()
    } else if trimmed.len() <= 8 && trimmed.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        format!("MECP/2/{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn event_projection_from_fields(
    fields_bytes: &[u8],
    content_bytes: Option<&[u8]>,
    source_identity_fallback: Option<&str>,
    source_display_name_fallback: Option<&str>,
    received_at_ms: u64,
) -> Option<EventProjectionRecord> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string());
        if command_type
            .as_deref()
            .is_some_and(|value| value != "mission.registry.log_entry.upsert")
        {
            continue;
        }
        let command_type =
            command_type.unwrap_or_else(|| "mission.registry.log_entry.upsert".to_string());
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries);
        let uid = msgpack_get_named(args, &["entry_uid", "u"]).and_then(msgpack_event_uid)?;
        let mission_uid = msgpack_get_named(args, &["mission_uid", "m"])
            .and_then(msgpack_mission_uid)
            .unwrap_or_else(|| DEFAULT_R3AKT_MISSION_UID.to_string());
        let content = msgpack_get_named(args, &["content", "ct"])
            .and_then(msgpack_string)
            .or_else(|| {
                content_bytes.and_then(|bytes| {
                    let text = String::from_utf8_lossy(bytes).trim().to_string();
                    (!text.is_empty()).then_some(expand_event_wire_content(text.as_str()))
                })
            })?;
        let callsign = msgpack_get_named(args, &["callsign", "cs"])
            .and_then(msgpack_string)
            .or_else(|| {
                source.and_then(|source_map| {
                    msgpack_get_named(source_map, &["display_name", "n"]).and_then(msgpack_string)
                })
            })
            .or_else(|| {
                source_display_name_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                source_identity_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.chars().take(8).collect())
            })?;
        let timestamp = msgpack_get_named(command_map, &["timestamp", "ts"])
            .and_then(msgpack_timestamp)
            .or_else(|| msgpack_get_named(args, &["server_time", "st"]).and_then(msgpack_timestamp))
            .or_else(|| msgpack_get_named(args, &["client_time", "ct"]).and_then(msgpack_timestamp))
            .unwrap_or_else(current_timestamp_rfc3339);
        let command_id = msgpack_get_named(args, &["ci"])
            .and_then(|value| event_command_id_from_tail(uid.as_str(), value))
            .or_else(|| {
                msgpack_get_named(command_map, &["command_id", "i"]).and_then(msgpack_string)
            })
            .unwrap_or_else(|| format!("log-entry-{uid}"));
        let source_identity = msgpack_get_named(args, &["source_identity", "si"])
            .and_then(msgpack_string)
            .or_else(|| {
                source.and_then(|source_map| {
                    msgpack_get_named(source_map, &["rns_identity", "r"])
                        .and_then(msgpack_hex_or_string)
                })
            })
            .or_else(|| {
                source_identity_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })?;
        if uid.trim().is_empty()
            || mission_uid.trim().is_empty()
            || content.trim().is_empty()
            || callsign.trim().is_empty()
            || timestamp.trim().is_empty()
            || command_id.trim().is_empty()
            || source_identity.trim().is_empty()
        {
            return None;
        }
        let topics = msgpack_get_named(command_map, &["topics", "to"])
            .and_then(|value| msgpack_event_topics(value, mission_uid.as_str()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| vec![mission_uid.clone()]);
        let server_time = msgpack_get_named(args, &["server_time", "st"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(|| timestamp.clone());
        let client_time = msgpack_get_named(args, &["client_time", "ct"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(|| timestamp.clone());
        let correlation_id = msgpack_get_named(command_map, &["correlation_id", "c"])
            .and_then(msgpack_string)
            .or_else(|| Some(command_id.clone()));
        return Some(EventProjectionRecord {
            uid,
            command_id,
            source_identity,
            source_display_name: msgpack_get_named(args, &["source_display_name", "sn"])
                .and_then(msgpack_string)
                .or_else(|| {
                    source.and_then(|source_map| {
                        msgpack_get_named(source_map, &["display_name", "n"])
                            .and_then(msgpack_string)
                    })
                })
                .or_else(|| {
                    source_display_name_fallback
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                }),
            timestamp,
            command_type,
            mission_uid,
            content,
            callsign,
            server_time: Some(server_time),
            client_time: Some(client_time),
            keywords: msgpack_get_named(args, &["keywords", "kw"])
                .and_then(msgpack_event_keywords)
                .unwrap_or_default(),
            content_hashes: msgpack_get_named(args, &["content_hashes", "ch"])
                .and_then(msgpack_string_vec)
                .unwrap_or_default(),
            updated_at_ms: received_at_ms,
            deleted_at_ms: msgpack_get_named(args, &["deleted_at_ms", "d"]).and_then(msgpack_u64),
            correlation_id,
            topics,
        });
    }

    None
}

fn telemetry_position_from_fields(
    fields_bytes: &[u8],
    received_at_ms: u64,
) -> Option<TelemetryPositionRecord> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())?;
        if command_type != "mission.registry.telemetry.upsert" {
            continue;
        }
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let callsign = msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string)?;
        let lat = msgpack_get_named(args, &["lat", "la"]).and_then(msgpack_f64)?;
        let lon = msgpack_get_named(args, &["lon", "lo"]).and_then(msgpack_f64)?;
        if callsign.trim().is_empty() || !lat.is_finite() || !lon.is_finite() {
            return None;
        }
        return Some(TelemetryPositionRecord {
            callsign: callsign.trim().to_string(),
            lat,
            lon,
            alt: msgpack_get_named(args, &["alt", "al"]).and_then(msgpack_f64),
            course: msgpack_get_named(args, &["course", "cr"]).and_then(msgpack_f64),
            speed: msgpack_get_named(args, &["speed", "sp"]).and_then(msgpack_f64),
            accuracy: msgpack_get_named(args, &["accuracy", "ac"]).and_then(msgpack_f64),
            updated_at_ms: msgpack_get_named(args, &["updated_at_ms", "updatedAt", "u"])
                .and_then(msgpack_u64)
                .unwrap_or(received_at_ms),
        });
    }

    None
}

async fn persist_received_telemetry_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
) -> bool {
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| value != "mission.registry.telemetry.upsert")
    {
        return false;
    }

    let Some(record) =
        fields_bytes.and_then(|value| telemetry_position_from_fields(value, now_ms()))
    else {
        return false;
    };

    match state.app_state.record_local_telemetry_fix(&record) {
        Ok(invalidation) => {
            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            if let Ok(summary) = state.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("telemetry-received".to_string()),
            ) {
                bus.emit(NodeEvent::ProjectionInvalidated {
                    invalidation: summary,
                });
            }
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound telemetry callsign={} reason={}",
                    record.callsign, err
                ),
            });
            false
        }
    }
}

async fn persist_received_event_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    content_bytes: Option<&[u8]>,
    source_identity_fallback: Option<&str>,
) -> bool {
    let source_display_name = if let Some(source_hex) = source_identity_fallback {
        state
            .messaging
            .lock()
            .await
            .peer_by_destination(source_hex)
            .and_then(|peer| peer.display_name)
    } else {
        None
    };
    let parsed_from_fields = fields_bytes.and_then(|value| {
        event_projection_from_fields(
            value,
            content_bytes,
            source_identity_fallback,
            source_display_name.as_deref(),
            now_ms(),
        )
    });
    if metadata.is_none() && parsed_from_fields.is_none() {
        return false;
    }
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| value != "mission.registry.log_entry.upsert")
        && parsed_from_fields.is_none()
    {
        return false;
    }

    let Some(record) = parsed_from_fields else {
        return false;
    };

    match state.app_state.upsert_event(&record) {
        Ok(invalidation) => {
            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            if let Ok(summary) = state.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("event-received".to_string()),
            ) {
                bus.emit(NodeEvent::ProjectionInvalidated {
                    invalidation: summary,
                });
            }
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound event uid={} reason={}",
                    record.uid, err
                ),
            });
            false
        }
    }
}

fn parse_rfc3339_sort_key(timestamp: &str) -> Option<(i64, u32)> {
    let trimmed = timestamp.trim();
    let suffix = trimmed.strip_suffix('Z')?;
    let (date, time) = suffix.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let (time_main, fraction) = match time.split_once('.') {
        Some((main, fraction)) => (main, Some(fraction)),
        None => (time, None),
    };
    let mut time_parts = time_main.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second = time_parts.next()?.parse::<i64>().ok()?;
    if time_parts.next().is_some() {
        return None;
    }

    let nanos = match fraction {
        Some(value) => {
            if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            let truncated = &value[..value.len().min(9)];
            let mut padded = truncated.to_string();
            while padded.len() < 9 {
                padded.push('0');
            }
            padded.parse::<u32>().ok()?
        }
        None => 0,
    };

    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch = era * 146_097 + doe - 719_468;
    let seconds_since_epoch = days_since_epoch * 86_400 + hour * 3_600 + minute * 60 + second;
    Some((seconds_since_epoch, nanos))
}

fn incoming_timestamp_is_newer(local_timestamp: Option<&str>, incoming_timestamp: &str) -> bool {
    match (
        local_timestamp.and_then(parse_rfc3339_sort_key),
        parse_rfc3339_sort_key(incoming_timestamp),
    ) {
        (None, Some(_)) => true,
        (Some(local), Some(incoming)) => local < incoming,
        _ => local_timestamp.is_none_or(|local| local < incoming_timestamp),
    }
}

fn checklist_command_source_identity(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries)?;
    msgpack_get_named(source, &["rns_identity", "r"]).and_then(msgpack_hex_or_string)
}

fn checklist_command_source_display_name(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries)?;
    let display_name =
        msgpack_get_named(source, &["display_name", "n"]).and_then(msgpack_string)?;
    normalize_optional_string(Some(display_name.as_str()))
}

fn apply_checklist_creator_from_command(
    checklist: &mut ChecklistRecord,
    args: &[(MsgPackValue, MsgPackValue)],
    command_map: &[(MsgPackValue, MsgPackValue)],
    source_identity: Option<&str>,
) {
    if let Some(created_by) = msgpack_get_checklist_arg(args, "created_by_team_member_rns_identity")
        .and_then(msgpack_string)
    {
        checklist.created_by_team_member_rns_identity = created_by;
    }
    if checklist
        .created_by_team_member_rns_identity
        .trim()
        .is_empty()
    {
        checklist.created_by_team_member_rns_identity =
            source_identity.unwrap_or_default().to_string();
    }
    checklist.created_by_team_member_display_name =
        msgpack_get_checklist_arg(args, "created_by_team_member_display_name")
            .and_then(msgpack_string)
            .and_then(|value| normalize_optional_string(Some(value.as_str())))
            .or_else(|| checklist_command_source_display_name(command_map))
            .or(checklist.created_by_team_member_display_name.take());
}

fn emit_checklist_invalidations(
    bus: &EventBus,
    invalidations: Vec<crate::types::ProjectionInvalidation>,
) {
    for invalidation in invalidations {
        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    }
}

fn upsert_inbound_checklist(
    app_state: &AppStateStore,
    bus: &EventBus,
    checklist: &ChecklistRecord,
    reason: &str,
) -> bool {
    match app_state.upsert_checklist(checklist, reason) {
        Ok(invalidations) => {
            emit_checklist_invalidations(bus, invalidations);
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound checklist uid={} reason={reason} error={err}",
                    checklist.uid
                ),
            });
            false
        }
    }
}

fn blank_checklist_record(
    checklist_uid: &str,
    timestamp: &str,
    source_identity: Option<&str>,
) -> ChecklistRecord {
    ChecklistRecord {
        uid: checklist_uid.to_string(),
        mission_uid: None,
        template_uid: None,
        template_version: None,
        template_name: None,
        name: String::new(),
        description: String::new(),
        start_time: None,
        mode: crate::types::ChecklistMode::Online {},
        sync_state: ChecklistSyncState::Synced {},
        origin_type: crate::types::ChecklistOriginType::RchTemplate {},
        checklist_status: ChecklistTaskStatus::Pending {},
        created_at: Some(timestamp.to_string()),
        created_by_team_member_rns_identity: source_identity.unwrap_or_default().to_string(),
        created_by_team_member_display_name: None,
        updated_at: Some(timestamp.to_string()),
        last_changed_by_team_member_rns_identity: normalize_optional_string(source_identity),
        deleted_at: None,
        uploaded_at: None,
        participant_rns_identities: source_identity
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        expected_task_count: None,
        progress_percent: 0.0,
        counts: crate::types::ChecklistStatusCounts {
            pending_count: 0,
            late_count: 0,
            complete_count: 0,
        },
        columns: Vec::new(),
        tasks: Vec::new(),
        feed_publications: Vec::new(),
    }
}

fn hidden_placeholder_checklist_record(checklist_uid: &str, timestamp: &str) -> ChecklistRecord {
    let mut record = blank_checklist_record(checklist_uid, timestamp, None);
    record.deleted_at = Some(timestamp.to_string());
    record.updated_at = Some(timestamp.to_string());
    record
}

fn is_hidden_placeholder_checklist(record: &ChecklistRecord) -> bool {
    record.deleted_at.is_some()
        && record.mission_uid.is_none()
        && record.template_uid.is_none()
        && record.template_version.is_none()
        && record.template_name.is_none()
        && record.name.is_empty()
        && record.description.is_empty()
        && record.start_time.is_none()
        && record.created_by_team_member_rns_identity.trim().is_empty()
}

fn should_apply_inbound_checklist_create(
    existing: Option<&ChecklistRecord>,
    timestamp: &str,
) -> bool {
    let Some(record) = existing else {
        return true;
    };
    if is_hidden_placeholder_checklist(record) {
        return true;
    }
    incoming_timestamp_is_newer(record.updated_at.as_deref(), timestamp)
        && record
            .deleted_at
            .as_deref()
            .is_none_or(|deleted_at| incoming_timestamp_is_newer(Some(deleted_at), timestamp))
}

fn checklist_delete_record_from_command(
    existing: Option<ChecklistRecord>,
    checklist_uid: &str,
    timestamp: &str,
    source_identity: Option<&str>,
) -> Option<ChecklistRecord> {
    if existing.as_ref().is_some_and(|checklist| {
        !incoming_timestamp_is_newer(checklist.updated_at.as_deref(), timestamp)
            || checklist
                .deleted_at
                .as_deref()
                .is_some_and(|deleted_at| !incoming_timestamp_is_newer(Some(deleted_at), timestamp))
    }) {
        return None;
    }

    let mut checklist =
        existing.unwrap_or_else(|| blank_checklist_record(checklist_uid, timestamp, None));
    checklist.deleted_at = Some(timestamp.to_string());
    checklist.updated_at = Some(timestamp.to_string());
    set_checklist_last_changed_by(&mut checklist, source_identity);
    normalize_checklist_record(&mut checklist);
    Some(checklist)
}

fn timestamp_is_newer(left: Option<&str>, right: Option<&str>) -> bool {
    match (
        left.and_then(parse_rfc3339_sort_key),
        right.and_then(parse_rfc3339_sort_key),
    ) {
        (Some(left), Some(right)) => left > right,
        (Some(_), None) => true,
        (None, Some(_)) | (None, None) => false,
    }
}

fn timestamp_is_at_least(left: Option<&str>, right: Option<&str>) -> bool {
    match (
        left.and_then(parse_rfc3339_sort_key),
        right.and_then(parse_rfc3339_sort_key),
    ) {
        (Some(left), Some(right)) => left >= right,
        (Some(_), None) | (None, None) => true,
        (None, Some(_)) => false,
    }
}

fn newest_timestamp<'a>(left: Option<&'a str>, right: Option<&'a str>) -> Option<&'a str> {
    if timestamp_is_at_least(left, right) {
        left.or(right)
    } else {
        right.or(left)
    }
}

fn task_freshness_timestamp(task: &ChecklistTaskRecord) -> Option<&str> {
    newest_timestamp(task.deleted_at.as_deref(), task.updated_at.as_deref())
}

fn merge_uploaded_cells(
    mut local_cells: Vec<ChecklistCellRecord>,
    incoming_cells: Vec<ChecklistCellRecord>,
) -> Vec<ChecklistCellRecord> {
    for incoming_cell in incoming_cells {
        if let Some(index) = local_cells
            .iter()
            .position(|cell| cell.column_uid == incoming_cell.column_uid)
        {
            if timestamp_is_newer(
                incoming_cell.updated_at.as_deref(),
                local_cells[index].updated_at.as_deref(),
            ) {
                local_cells[index] = incoming_cell;
            }
        } else {
            local_cells.push(incoming_cell);
        }
    }
    local_cells
}

fn merge_uploaded_task_record(
    local_task: ChecklistTaskRecord,
    incoming_task: ChecklistTaskRecord,
) -> ChecklistTaskRecord {
    let local_task_at = task_freshness_timestamp(&local_task);
    let incoming_task_at = task_freshness_timestamp(&incoming_task);
    if local_task.deleted_at.is_some()
        && timestamp_is_at_least(local_task.deleted_at.as_deref(), incoming_task_at)
    {
        return local_task;
    }
    if incoming_task.deleted_at.is_some()
        && timestamp_is_at_least(incoming_task.deleted_at.as_deref(), local_task_at)
    {
        return incoming_task;
    }

    let mut merged = if timestamp_is_newer(
        incoming_task.updated_at.as_deref(),
        local_task.updated_at.as_deref(),
    ) {
        incoming_task.clone()
    } else {
        local_task.clone()
    };
    merged.cells = merge_uploaded_cells(local_task.cells, incoming_task.cells);
    merged
}

fn merge_uploaded_columns(
    mut local_columns: Vec<ChecklistColumnRecord>,
    incoming_columns: Vec<ChecklistColumnRecord>,
) -> Vec<ChecklistColumnRecord> {
    for incoming_column in incoming_columns {
        if !local_columns
            .iter()
            .any(|column| column.column_uid == incoming_column.column_uid)
        {
            local_columns.push(incoming_column);
        }
    }
    local_columns
}

fn merge_uploaded_tasks(
    mut local_tasks: Vec<ChecklistTaskRecord>,
    incoming_tasks: Vec<ChecklistTaskRecord>,
) -> Vec<ChecklistTaskRecord> {
    for incoming_task in incoming_tasks {
        if let Some(index) = local_tasks
            .iter()
            .position(|task| task.task_uid == incoming_task.task_uid)
        {
            let local_task = local_tasks[index].clone();
            local_tasks[index] = merge_uploaded_task_record(local_task, incoming_task);
        } else {
            local_tasks.push(incoming_task);
        }
    }
    local_tasks
}

fn merge_uploaded_participants(
    mut local_participants: Vec<String>,
    incoming_participants: Vec<String>,
    source_identity: Option<&str>,
) -> Vec<String> {
    for participant in incoming_participants {
        if !local_participants.iter().any(|value| value == &participant) {
            local_participants.push(participant);
        }
    }
    if let Some(source_identity) = normalize_optional_string(source_identity) {
        if !local_participants
            .iter()
            .any(|value| value == &source_identity)
        {
            local_participants.push(source_identity);
        }
    }
    local_participants
}

fn merge_uploaded_feed_publications(
    mut local_publications: Vec<crate::types::ChecklistFeedPublicationRecord>,
    incoming_publications: Vec<crate::types::ChecklistFeedPublicationRecord>,
) -> Vec<crate::types::ChecklistFeedPublicationRecord> {
    for incoming_publication in incoming_publications {
        if !local_publications
            .iter()
            .any(|publication| publication.publication_uid == incoming_publication.publication_uid)
        {
            local_publications.push(incoming_publication);
        }
    }
    local_publications
}

fn prepare_uploaded_snapshot(
    mut incoming: ChecklistRecord,
    timestamp: &str,
    source_identity: Option<&str>,
) -> ChecklistRecord {
    incoming.deleted_at = None;
    incoming.uploaded_at = normalize_optional_string(
        incoming
            .uploaded_at
            .clone()
            .or_else(|| Some(timestamp.to_string()))
            .as_deref(),
    );
    if incoming.created_at.is_none() {
        incoming.created_at = Some(timestamp.to_string());
    }
    if incoming.updated_at.is_none() {
        incoming.updated_at = Some(timestamp.to_string());
    }
    if incoming
        .created_by_team_member_rns_identity
        .trim()
        .is_empty()
    {
        incoming.created_by_team_member_rns_identity =
            source_identity.unwrap_or_default().to_string();
    }
    set_checklist_last_changed_by(&mut incoming, source_identity);
    incoming.participant_rns_identities = merge_uploaded_participants(
        Vec::new(),
        incoming.participant_rns_identities,
        source_identity,
    );
    incoming.sync_state = ChecklistSyncState::Synced {};
    if incoming.expected_task_count.is_none() {
        incoming.expected_task_count = Some(
            incoming
                .tasks
                .iter()
                .filter(|task| task.deleted_at.is_none())
                .count() as u32,
        );
    }
    normalize_checklist_record(&mut incoming);
    incoming
}

fn merge_uploaded_checklist_snapshot(
    existing: Option<ChecklistRecord>,
    incoming: ChecklistRecord,
    timestamp: &str,
    source_identity: Option<&str>,
) -> Option<ChecklistRecord> {
    let incoming = prepare_uploaded_snapshot(incoming, timestamp, source_identity);
    let incoming_snapshot_at = incoming
        .uploaded_at
        .as_deref()
        .or(incoming.updated_at.as_deref())
        .unwrap_or(timestamp)
        .to_string();
    let incoming_content_at = incoming
        .updated_at
        .as_deref()
        .unwrap_or(incoming_snapshot_at.as_str())
        .to_string();
    let Some(existing) = existing else {
        return Some(incoming);
    };
    if is_hidden_placeholder_checklist(&existing) {
        return Some(incoming);
    }
    if existing.deleted_at.as_deref().is_some_and(|deleted_at| {
        !incoming_timestamp_is_newer(Some(deleted_at), incoming_content_at.as_str())
    }) {
        return None;
    }

    let incoming_metadata_is_newer = incoming_timestamp_is_newer(
        existing.updated_at.as_deref(),
        incoming
            .updated_at
            .as_deref()
            .unwrap_or(incoming_snapshot_at.as_str()),
    );
    let mut merged = if incoming_metadata_is_newer {
        let mut record = incoming.clone();
        record.created_at = existing.created_at.clone().or(record.created_at);
        if record.created_by_team_member_rns_identity.trim().is_empty() {
            record.created_by_team_member_rns_identity =
                existing.created_by_team_member_rns_identity.clone();
        }
        record
    } else {
        existing.clone()
    };

    merged.deleted_at = None;
    merged.sync_state = ChecklistSyncState::Synced {};
    merged.uploaded_at = newest_timestamp(
        merged.uploaded_at.as_deref(),
        incoming.uploaded_at.as_deref(),
    )
    .map(ToString::to_string);
    merged.updated_at =
        newest_timestamp(merged.updated_at.as_deref(), incoming.updated_at.as_deref())
            .map(ToString::to_string);
    merged.columns = merge_uploaded_columns(existing.columns, incoming.columns);
    merged.tasks = merge_uploaded_tasks(existing.tasks, incoming.tasks);
    merged.participant_rns_identities = merge_uploaded_participants(
        existing.participant_rns_identities,
        incoming.participant_rns_identities,
        source_identity,
    );
    merged.expected_task_count = incoming
        .expected_task_count
        .or(existing.expected_task_count)
        .or_else(|| {
            Some(
                merged
                    .tasks
                    .iter()
                    .filter(|task| task.deleted_at.is_none())
                    .count() as u32,
            )
        });
    merged.feed_publications =
        merge_uploaded_feed_publications(existing.feed_publications, incoming.feed_publications);
    set_checklist_last_changed_by(&mut merged, source_identity);
    normalize_checklist_record(&mut merged);
    Some(merged)
}

fn hydrate_checklist_from_local_template(
    app_state: &AppStateStore,
    checklist: &mut ChecklistRecord,
) {
    if !checklist.tasks.is_empty() {
        return;
    }
    let Some(template_uid) = checklist
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(Some(template)) = app_state.get_checklist_template(template_uid) else {
        return;
    };

    if checklist.columns.is_empty() {
        checklist.columns = template.columns;
    }
    checklist.tasks = template.tasks;
    checklist.template_version = Some(template.version);
    checklist.template_name = Some(template.name);
    checklist.origin_type = template.origin_type;
    checklist.expected_task_count = Some(
        checklist
            .tasks
            .iter()
            .filter(|task| task.deleted_at.is_none())
            .count() as u32,
    );
}

fn blank_task_cells(columns: &[ChecklistColumnRecord], task_uid: &str) -> Vec<ChecklistCellRecord> {
    columns
        .iter()
        .map(|column| ChecklistCellRecord {
            cell_uid: format!("{task_uid}:{}", column.column_uid),
            task_uid: task_uid.to_string(),
            column_uid: column.column_uid.clone(),
            value: None,
            updated_at: None,
            updated_by_team_member_rns_identity: None,
        })
        .collect()
}

fn checklist_column_type_from_wire(value: &str) -> ChecklistColumnType {
    match value.trim().to_ascii_uppercase().as_str() {
        "LONG_STRING" => ChecklistColumnType::LongString {},
        "INTEGER" => ChecklistColumnType::Integer {},
        "ACTUAL_TIME" => ChecklistColumnType::ActualTime {},
        "RELATIVE_TIME" => ChecklistColumnType::RelativeTime {},
        _ => ChecklistColumnType::ShortString {},
    }
}

fn checklist_system_key_from_wire(value: &str) -> Option<ChecklistSystemColumnKey> {
    match value.trim().to_ascii_uppercase().as_str() {
        "DUE_RELATIVE_DTG" => Some(ChecklistSystemColumnKey::DueRelativeDtg {}),
        _ => None,
    }
}

fn checklist_column_from_patch(
    patch: &[(MsgPackValue, MsgPackValue)],
    fallback_display_order: u32,
) -> Option<ChecklistColumnRecord> {
    let column_uid = msgpack_get_checklist_arg(patch, "column_uid").and_then(msgpack_string)?;
    let column_name = msgpack_get_checklist_arg(patch, "column_name")
        .and_then(msgpack_string)
        .unwrap_or_else(|| column_uid.clone());
    let display_order = msgpack_get_checklist_arg(patch, "display_order")
        .and_then(msgpack_u64)
        .map_or(fallback_display_order, |value| value as u32);
    let column_type = msgpack_get_checklist_arg(patch, "column_type")
        .and_then(msgpack_string)
        .map_or(ChecklistColumnType::ShortString {}, |value| {
            checklist_column_type_from_wire(value.as_str())
        });
    let column_editable = msgpack_get_checklist_arg(patch, "column_editable")
        .and_then(msgpack_bool)
        .unwrap_or(true);
    let background_color =
        msgpack_get_checklist_arg(patch, "row_background_color").and_then(msgpack_string);
    let text_color = msgpack_get_checklist_arg(patch, "text_color").and_then(msgpack_string);
    let is_removable = msgpack_get_checklist_arg(patch, "is_removable")
        .and_then(msgpack_bool)
        .unwrap_or(true);
    let system_key = msgpack_get_checklist_arg(patch, "system_key")
        .and_then(msgpack_string)
        .and_then(|value| checklist_system_key_from_wire(value.as_str()));

    Some(ChecklistColumnRecord {
        column_uid,
        column_name,
        display_order,
        column_type,
        column_editable,
        background_color,
        text_color,
        is_removable,
        system_key,
    })
}

fn merge_checklist_column(checklist: &mut ChecklistRecord, incoming: ChecklistColumnRecord) {
    if let Some(existing) = checklist
        .columns
        .iter_mut()
        .find(|column| column.column_uid == incoming.column_uid)
    {
        *existing = incoming;
    } else {
        checklist.columns.push(incoming);
        checklist.columns.sort_by_key(|column| column.display_order);
    }
}

fn should_apply_inbound_task_status(
    task: &ChecklistTaskRecord,
    incoming_status: ChecklistUserTaskStatus,
    incoming_timestamp: &str,
    inserted_placeholder: bool,
) -> bool {
    if inserted_placeholder {
        return true;
    }
    match (task.user_status, incoming_status) {
        (ChecklistUserTaskStatus::Pending {}, ChecklistUserTaskStatus::Complete {}) => true,
        (ChecklistUserTaskStatus::Complete {}, ChecklistUserTaskStatus::Pending {}) => {
            task.completed_at.as_deref().map_or_else(
                || incoming_timestamp_is_newer(task.updated_at.as_deref(), incoming_timestamp),
                |completed_at| incoming_timestamp_is_newer(Some(completed_at), incoming_timestamp),
            )
        }
        _ => incoming_timestamp_is_newer(task.updated_at.as_deref(), incoming_timestamp),
    }
}

fn placeholder_task_record(task_uid: &str, timestamp: &str) -> ChecklistTaskRecord {
    ChecklistTaskRecord {
        task_uid: task_uid.to_string(),
        number: 0,
        user_status: ChecklistUserTaskStatus::Pending {},
        task_status: ChecklistTaskStatus::Pending {},
        is_late: false,
        updated_at: Some(timestamp.to_string()),
        deleted_at: None,
        custom_status: None,
        due_relative_minutes: None,
        due_dtg: None,
        notes: None,
        row_background_color: None,
        line_break_enabled: false,
        completed_at: None,
        completed_by_team_member_rns_identity: None,
        legacy_value: None,
        cells: Vec::new(),
    }
}

fn tombstoned_task_record(task_uid: &str, timestamp: &str) -> ChecklistTaskRecord {
    ChecklistTaskRecord {
        task_uid: task_uid.to_string(),
        number: 0,
        user_status: ChecklistUserTaskStatus::Pending {},
        task_status: ChecklistTaskStatus::Pending {},
        is_late: false,
        updated_at: Some(timestamp.to_string()),
        deleted_at: Some(timestamp.to_string()),
        custom_status: None,
        due_relative_minutes: None,
        due_dtg: None,
        notes: None,
        row_background_color: None,
        line_break_enabled: false,
        completed_at: None,
        completed_by_team_member_rns_identity: None,
        legacy_value: None,
        cells: Vec::new(),
    }
}

fn checklist_snapshot_json_from_command(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    if let Some(snapshot) = msgpack_get_named(command_map, &["snapshot", "sn"]) {
        let json = msgpack_value_to_json(snapshot)?;
        return serde_json::to_string(&json).ok();
    }
    if let Some(snapshot_json) =
        msgpack_get_named(command_map, &["snapshot_json", "sj"]).and_then(msgpack_string)
    {
        return Some(snapshot_json);
    }
    None
}

fn checklist_snapshot_json_from_content(
    content_bytes: Option<&[u8]>,
    checklist_uid: &str,
) -> Option<String> {
    let content = content_bytes?;
    let snapshot_payload = rmp_serde::from_slice::<MsgPackValue>(content).ok()?;
    let entries = msgpack_map_entries(&snapshot_payload)?;
    let payload_type = msgpack_get_named(entries, &["type"]).and_then(msgpack_string)?;
    if let Some(payload_uid) =
        msgpack_get_named(entries, &["checklist_uid"]).and_then(msgpack_string)
    {
        if payload_uid != checklist_uid {
            return None;
        }
    }
    match payload_type.as_str() {
        "rem.checklist.snapshot.v1" => {
            let snapshot = msgpack_get_named(entries, &["snapshot"])?;
            let json = msgpack_value_to_json(snapshot)?;
            serde_json::to_string(&json).ok()
        }
        "rem.checklist.snapshot.v2" => {
            let encoding = msgpack_get_named(entries, &["encoding"])
                .and_then(msgpack_string)
                .unwrap_or_default();
            if encoding != "zlib+msgpack" {
                return None;
            }
            let MsgPackValue::Binary(compressed_snapshot) =
                msgpack_get_named(entries, &["snapshot"])?
            else {
                return None;
            };
            let mut decoder = ZlibDecoder::new(compressed_snapshot.as_slice());
            let mut snapshot_msgpack = Vec::new();
            decoder.read_to_end(&mut snapshot_msgpack).ok()?;
            let snapshot =
                rmp_serde::from_slice::<MsgPackValue>(snapshot_msgpack.as_slice()).ok()?;
            let json = msgpack_value_to_json(&snapshot)?;
            serde_json::to_string(&json).ok()
        }
        _ => None,
    }
}

fn msgpack_json_arg<T: DeserializeOwned>(
    args: &[(MsgPackValue, MsgPackValue)],
    key: &str,
) -> Option<T> {
    msgpack_get_checklist_arg(args, key)
        .and_then(msgpack_value_to_json)
        .and_then(|value| serde_json::from_value(value).ok())
}

fn msgpack_get_checklist_arg<'a>(
    args: &'a [(MsgPackValue, MsgPackValue)],
    key: &str,
) -> Option<&'a MsgPackValue> {
    if let Some(code) = checklist_arg_code(key) {
        msgpack_get_named(args, &[key, code])
    } else {
        msgpack_get_named(args, &[key])
    }
}

fn msgpack_checklist_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().map(|value| format!("chk-{value}")),
        _ => msgpack_string(value),
    }
}

fn msgpack_checklist_template_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => match value.as_u64()? {
            1 => Some("tmpl-24-hour-survival-pack".to_string()),
            2 => Some("tmpl-72-hour-home-preparedness".to_string()),
            3 => Some("tmpl-vehicle-emergency-preparedness".to_string()),
            _ => None,
        },
        _ => msgpack_string(value),
    }
}

fn positional_checklist_command_args(
    command: &MsgPackValue,
) -> Option<(String, Vec<(MsgPackValue, MsgPackValue)>)> {
    let MsgPackValue::Array(values) = command else {
        return None;
    };
    let command_type = match values.first()? {
        MsgPackValue::Integer(value) if value.as_u64() == Some(1) => {
            "checklist.create.online".to_string()
        }
        value => {
            msgpack_string(value).map(|value| canonical_command_type(value.as_str()).to_string())?
        }
    };
    if command_type != "checklist.create.online" || values.len() < 5 {
        return None;
    }
    Some((
        command_type,
        vec![
            (
                MsgPackValue::from("cl"),
                values.get(1).expect("checked length").clone(),
            ),
            (
                MsgPackValue::from("m"),
                values.get(2).expect("checked length").clone(),
            ),
            (
                MsgPackValue::from("tp"),
                values.get(3).expect("checked length").clone(),
            ),
            (
                MsgPackValue::from("n"),
                values.get(4).expect("checked length").clone(),
            ),
        ],
    ))
}

fn msgpack_value_to_json(value: &MsgPackValue) -> Option<serde_json::Value> {
    match value {
        MsgPackValue::Nil => Some(serde_json::Value::Null),
        MsgPackValue::Boolean(value) => Some(serde_json::Value::Bool(*value)),
        MsgPackValue::Integer(value) => {
            if let Some(value) = value.as_u64() {
                Some(serde_json::Value::Number(serde_json::Number::from(value)))
            } else {
                value
                    .as_i64()
                    .map(serde_json::Number::from)
                    .map(serde_json::Value::Number)
            }
        }
        MsgPackValue::F32(value) => {
            serde_json::Number::from_f64(f64::from(*value)).map(serde_json::Value::Number)
        }
        MsgPackValue::F64(value) => {
            serde_json::Number::from_f64(*value).map(serde_json::Value::Number)
        }
        MsgPackValue::String(value) => value
            .as_str()
            .map(|value| serde_json::Value::String(value.to_string())),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone())
            .ok()
            .map(serde_json::Value::String),
        MsgPackValue::Array(values) => values
            .iter()
            .map(msgpack_value_to_json)
            .collect::<Option<Vec<_>>>()
            .map(serde_json::Value::Array),
        MsgPackValue::Map(entries) => {
            let mut object = serde_json::Map::new();
            for (key, value) in entries {
                object.insert(msgpack_string(key)?, msgpack_value_to_json(value)?);
            }
            Some(serde_json::Value::Object(object))
        }
        MsgPackValue::Ext(_, _) => None,
    }
}

fn ensure_task_for_incoming_update(
    checklist: &mut ChecklistRecord,
    task_uid: &str,
    timestamp: &str,
    number: Option<u32>,
) -> bool {
    if checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
        return false;
    }
    let mut task = placeholder_task_record(task_uid, timestamp);
    if let Some(number) = number.filter(|value| *value > 0) {
        task.number = number;
    }
    checklist.tasks.push(task);
    true
}

fn task_needs_row_metadata_hydration(task: &ChecklistTaskRecord) -> bool {
    task.number == 0
        && task.legacy_value.is_none()
        && task.due_relative_minutes.is_none()
        && task.due_dtg.is_none()
        && task.notes.is_none()
}

fn checklist_task_from_row_add_args(
    args: &[(MsgPackValue, MsgPackValue)],
    task_uid: &str,
    number: u32,
    timestamp: &str,
) -> Option<ChecklistTaskRecord> {
    msgpack_json_arg::<ChecklistTaskRecord>(args, "task").map(|mut task| {
        task.task_uid = task_uid.to_string();
        task.number = number;
        task.deleted_at = None;
        task.updated_at =
            newest_timestamp(task.updated_at.as_deref(), Some(timestamp)).map(ToString::to_string);
        for cell in &mut task.cells {
            cell.task_uid = task_uid.to_string();
            if cell.cell_uid.trim().is_empty() {
                cell.cell_uid = format!("{}:{}", task_uid, cell.column_uid);
            }
        }
        task
    })
}

fn persist_received_checklist_if_present(
    app_state: &AppStateStore,
    bus: &EventBus,
    _metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    content_bytes: Option<&[u8]>,
) -> bool {
    let Some(fields_bytes) = fields_bytes else {
        return false;
    };
    let fields = match rmp_serde::from_slice::<MsgPackValue>(fields_bytes) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(field_entries) = msgpack_map_entries(&fields) else {
        return false;
    };
    let Some(commands) = msgpack_get_indexed(field_entries, FIELD_COMMANDS) else {
        return false;
    };
    let MsgPackValue::Array(command_entries) = commands else {
        return false;
    };

    let mut persisted_any = false;
    let mut handled_any = false;
    for command in command_entries {
        let command_map_storage;
        let args_storage;
        let (command_map, args_override) = if let Some(command_map) = msgpack_map_entries(command) {
            (command_map, None)
        } else if let Some((command_type, args)) = positional_checklist_command_args(command) {
            command_map_storage = vec![(MsgPackValue::from("t"), MsgPackValue::from(command_type))];
            args_storage = args;
            (
                command_map_storage.as_slice(),
                Some(args_storage.as_slice()),
            )
        } else {
            continue;
        };
        let Some(command_type) = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())
        else {
            continue;
        };
        if !command_type.starts_with("checklist.") {
            continue;
        }
        let timestamp = msgpack_get_named(command_map, &["timestamp", "ts"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(current_timestamp_rfc3339);
        let source_identity = checklist_command_source_identity(command_map);
        let map_args = msgpack_get_named(command_map, &["args", "a"])
            .and_then(msgpack_map_entries)
            .unwrap_or(command_map);
        let args = args_override.unwrap_or(map_args);

        match command_type.as_str() {
            "checklist.create.online" => {
                let checklist_uid = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                    .or_else(|| {
                        msgpack_get_named(command_map, &["command_id", "i"])
                            .and_then(msgpack_string)
                            .map(|value| value.trim_start_matches("cmd-").to_string())
                    });
                let Some(checklist_uid) = checklist_uid else {
                    continue;
                };
                let Some(mission_uid) =
                    msgpack_get_checklist_arg(args, "mission_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(template_uid) = msgpack_get_checklist_arg(args, "template_uid")
                    .and_then(msgpack_checklist_template_uid)
                else {
                    continue;
                };
                let Some(name) = msgpack_get_checklist_arg(args, "name").and_then(msgpack_string)
                else {
                    continue;
                };
                let description = msgpack_get_checklist_arg(args, "description")
                    .and_then(msgpack_string)
                    .unwrap_or_default();
                let start_time =
                    msgpack_get_checklist_arg(args, "start_time").and_then(msgpack_string);
                let existing = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .unwrap_or_default();
                if !should_apply_inbound_checklist_create(existing.as_ref(), timestamp.as_str()) {
                    continue;
                }
                let mut checklist = match existing {
                    Some(record)
                        if record.deleted_at.is_some()
                            && !is_hidden_placeholder_checklist(&record) =>
                    {
                        blank_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                            source_identity.as_deref(),
                        )
                    }
                    Some(record) => record,
                    None => blank_checklist_record(
                        checklist_uid.as_str(),
                        timestamp.as_str(),
                        source_identity.as_deref(),
                    ),
                };
                checklist.mission_uid = Some(mission_uid);
                checklist.template_uid = Some(template_uid);
                checklist.name = name;
                checklist.description = description;
                checklist.start_time = start_time;
                if let Some(columns) =
                    msgpack_json_arg::<Vec<ChecklistColumnRecord>>(args, "columns")
                {
                    checklist.columns = columns;
                }
                if let Some(tasks) = msgpack_json_arg::<Vec<ChecklistTaskRecord>>(args, "tasks") {
                    checklist.tasks = tasks;
                }
                if let Some(participants) =
                    msgpack_json_arg::<Vec<String>>(args, "participant_rns_identities")
                {
                    checklist.participant_rns_identities = merge_uploaded_participants(
                        checklist.participant_rns_identities,
                        participants,
                        source_identity.as_deref(),
                    );
                }
                if let Some(total_tasks) =
                    msgpack_get_checklist_arg(args, "total_tasks").and_then(msgpack_u64)
                {
                    checklist.expected_task_count = Some(total_tasks as u32);
                }
                if let Some(created_at) =
                    msgpack_get_checklist_arg(args, "created_at").and_then(msgpack_string)
                {
                    checklist.created_at = Some(created_at);
                }
                if let Some(uploaded_at) =
                    msgpack_get_checklist_arg(args, "uploaded_at").and_then(msgpack_string)
                {
                    checklist.uploaded_at = Some(uploaded_at);
                }
                checklist.updated_at = Some(timestamp.clone());
                checklist.deleted_at = None;
                if checklist.created_at.is_none() {
                    checklist.created_at = Some(timestamp.clone());
                }
                apply_checklist_creator_from_command(
                    &mut checklist,
                    args,
                    command_map,
                    source_identity.as_deref(),
                );
                if let Some(source_identity) = checklist_command_source_identity(command_map) {
                    if !checklist
                        .participant_rns_identities
                        .iter()
                        .any(|value| value == &source_identity)
                    {
                        checklist.participant_rns_identities.push(source_identity);
                    }
                }
                if let Some(snapshot_json) =
                    checklist_snapshot_json_from_content(content_bytes, checklist_uid.as_str())
                {
                    if let Ok(mut snapshot) =
                        serde_json::from_str::<ChecklistRecord>(snapshot_json.as_str())
                    {
                        snapshot.uid = checklist_uid.clone();
                        if snapshot.mission_uid.is_none() {
                            snapshot.mission_uid = checklist.mission_uid.clone();
                        }
                        if snapshot.template_uid.is_none() {
                            snapshot.template_uid = checklist.template_uid.clone();
                        }
                        if snapshot.name.trim().is_empty() {
                            snapshot.name = checklist.name.clone();
                        }
                        if snapshot.description.trim().is_empty() {
                            snapshot.description = checklist.description.clone();
                        }
                        if snapshot.start_time.is_none() {
                            snapshot.start_time = checklist.start_time.clone();
                        }
                        if snapshot.created_at.is_none() {
                            snapshot.created_at = checklist.created_at.clone();
                        }
                        if snapshot
                            .created_by_team_member_rns_identity
                            .trim()
                            .is_empty()
                        {
                            snapshot.created_by_team_member_rns_identity =
                                checklist.created_by_team_member_rns_identity.clone();
                        }
                        if snapshot.created_by_team_member_display_name.is_none() {
                            snapshot.created_by_team_member_display_name =
                                checklist.created_by_team_member_display_name.clone();
                        }
                        if snapshot.uploaded_at.is_none() {
                            snapshot.uploaded_at = checklist.uploaded_at.clone();
                        }
                        snapshot.updated_at = Some(timestamp.clone());
                        snapshot.deleted_at = None;
                        snapshot.sync_state = ChecklistSyncState::Synced {};
                        snapshot.participant_rns_identities = merge_uploaded_participants(
                            checklist.participant_rns_identities,
                            snapshot.participant_rns_identities,
                            source_identity.as_deref(),
                        );
                        set_checklist_last_changed_by(&mut snapshot, source_identity.as_deref());
                        normalize_checklist_record(&mut snapshot);
                        checklist = snapshot;
                    }
                }
                hydrate_checklist_from_local_template(app_state, &mut checklist);
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-create",
                );
            }
            "checklist.upload" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(snapshot_json) =
                    checklist_snapshot_json_from_content(content_bytes, checklist_uid.as_str())
                        .or_else(|| checklist_snapshot_json_from_command(command_map))
                else {
                    continue;
                };
                let Ok(mut checklist) =
                    serde_json::from_str::<ChecklistRecord>(snapshot_json.as_str())
                else {
                    continue;
                };
                checklist.uid = checklist_uid.clone();
                let existing = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .unwrap_or_default();
                let Some(checklist) = merge_uploaded_checklist_snapshot(
                    existing,
                    checklist,
                    timestamp.as_str(),
                    source_identity.as_deref(),
                ) else {
                    continue;
                };
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-upload",
                );
            }
            "checklist.update" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                if !incoming_timestamp_is_newer(checklist.updated_at.as_deref(), timestamp.as_str())
                    || (checklist.deleted_at.is_some()
                        && !is_hidden_placeholder_checklist(&checklist))
                {
                    continue;
                }
                let Some(patch) =
                    msgpack_get_checklist_arg(args, "patch").and_then(msgpack_map_entries)
                else {
                    continue;
                };
                if let Some(value) =
                    msgpack_get_checklist_arg(patch, "mission_uid").and_then(msgpack_string)
                {
                    checklist.mission_uid = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) = msgpack_get_checklist_arg(patch, "template_uid")
                    .and_then(msgpack_checklist_template_uid)
                {
                    checklist.template_uid = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) =
                    msgpack_get_checklist_arg(patch, "name").and_then(msgpack_string)
                {
                    checklist.name = value.trim().to_string();
                }
                if let Some(value) =
                    msgpack_get_checklist_arg(patch, "description").and_then(msgpack_string)
                {
                    checklist.description = value.trim().to_string();
                }
                if let Some(value) =
                    msgpack_get_checklist_arg(patch, "start_time").and_then(msgpack_string)
                {
                    checklist.start_time = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(column) =
                    checklist_column_from_patch(patch, checklist.columns.len() as u32)
                {
                    merge_checklist_column(&mut checklist, column);
                }
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-update",
                );
            }
            "checklist.delete" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(checklist) = checklist_delete_record_from_command(
                    app_state
                        .get_checklist_any(checklist_uid.as_str())
                        .ok()
                        .flatten(),
                    checklist_uid.as_str(),
                    timestamp.as_str(),
                    source_identity.as_deref(),
                ) else {
                    continue;
                };
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-delete",
                );
            }
            "checklist.task.row.add" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(number) = msgpack_get_checklist_arg(args, "number").and_then(msgpack_u64)
                else {
                    continue;
                };
                let incoming_task_payload = checklist_task_from_row_add_args(
                    args,
                    task_uid.as_str(),
                    number as u32,
                    timestamp.as_str(),
                );
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                if checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                    !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                }) || (checklist.deleted_at.is_some()
                    && !is_hidden_placeholder_checklist(&checklist))
                {
                    continue;
                }
                if let Some(task) = checklist
                    .tasks
                    .iter()
                    .find(|task| task.task_uid == task_uid)
                {
                    if incoming_task_payload.is_none()
                        && (task.deleted_at.as_deref().is_some_and(|deleted_at| {
                            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                        }) || (!task_needs_row_metadata_hydration(task)
                            && !incoming_timestamp_is_newer(
                                task.updated_at.as_deref(),
                                timestamp.as_str(),
                            )))
                    {
                        continue;
                    }
                }
                let due_relative_minutes = msgpack_get_checklist_arg(args, "due_relative_minutes")
                    .and_then(msgpack_u64)
                    .map(|value| value as u32);
                let legacy_value =
                    msgpack_get_checklist_arg(args, "legacy_value").and_then(msgpack_string);
                let due_dtg = msgpack_get_checklist_arg(args, "due_dtg").and_then(msgpack_string);
                let notes = msgpack_get_checklist_arg(args, "notes").and_then(msgpack_string);
                if let Some(incoming_task) = incoming_task_payload {
                    if let Some(index) = checklist
                        .tasks
                        .iter()
                        .position(|task| task.task_uid == task_uid)
                    {
                        let local_task = checklist.tasks[index].clone();
                        checklist.tasks[index] =
                            merge_uploaded_task_record(local_task, incoming_task);
                    } else {
                        checklist.tasks.push(incoming_task);
                    }
                } else if let Some(task) = checklist
                    .tasks
                    .iter_mut()
                    .find(|task| task.task_uid == task_uid)
                {
                    task.number = number as u32;
                    task.due_relative_minutes = due_relative_minutes;
                    task.due_dtg = due_dtg.clone();
                    task.notes = notes.clone();
                    task.legacy_value = legacy_value;
                    task.deleted_at = None;
                    task.updated_at =
                        newest_timestamp(task.updated_at.as_deref(), Some(timestamp.as_str()))
                            .map(ToString::to_string);
                } else {
                    let cells = blank_task_cells(checklist.columns.as_slice(), task_uid.as_str());
                    checklist.tasks.push(ChecklistTaskRecord {
                        task_uid,
                        number: number as u32,
                        user_status: ChecklistUserTaskStatus::Pending {},
                        task_status: ChecklistTaskStatus::Pending {},
                        is_late: false,
                        updated_at: Some(timestamp.clone()),
                        deleted_at: None,
                        custom_status: None,
                        due_relative_minutes,
                        due_dtg,
                        notes,
                        row_background_color: None,
                        line_break_enabled: false,
                        completed_at: None,
                        completed_by_team_member_rns_identity: None,
                        legacy_value,
                        cells,
                    });
                }
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-task-row-add",
                );
            }
            "checklist.task.row.delete" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let existing = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten();
                if existing.as_ref().is_some_and(|checklist| {
                    checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                        !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                    }) || (checklist.deleted_at.is_some()
                        && !is_hidden_placeholder_checklist(checklist))
                }) {
                    continue;
                }
                let mut checklist = existing.unwrap_or_else(|| {
                    hidden_placeholder_checklist_record(checklist_uid.as_str(), timestamp.as_str())
                });
                if let Some(existing_task) = checklist
                    .tasks
                    .iter()
                    .find(|task| task.task_uid == task_uid)
                {
                    if !incoming_timestamp_is_newer(
                        existing_task.updated_at.as_deref(),
                        timestamp.as_str(),
                    ) || existing_task
                        .deleted_at
                        .as_deref()
                        .is_some_and(|deleted_at| {
                            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                        })
                    {
                        continue;
                    }
                }
                if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
                    checklist.tasks.push(tombstoned_task_record(
                        task_uid.as_str(),
                        timestamp.as_str(),
                    ));
                }
                if let Some(task) = checklist
                    .tasks
                    .iter_mut()
                    .find(|task| task.task_uid == task_uid)
                {
                    task.deleted_at = Some(timestamp.clone());
                    task.updated_at = Some(timestamp.clone());
                } else {
                    continue;
                }
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-task-row-delete",
                );
            }
            "checklist.task.status.set" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let incoming_number = msgpack_get_checklist_arg(args, "number")
                    .and_then(msgpack_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|value| *value > 0);
                let explicit_task_uid =
                    msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string);
                if explicit_task_uid.is_none() && incoming_number.is_none() {
                    continue;
                }
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                let hidden_placeholder = is_hidden_placeholder_checklist(&checklist);
                if !hidden_placeholder
                    && (checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                        !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                    }) || checklist.deleted_at.is_some())
                {
                    handled_any = true;
                    continue;
                }
                let Some(task_uid) = explicit_task_uid.or_else(|| {
                    incoming_number.and_then(|number| {
                        checklist
                            .tasks
                            .iter()
                            .find(|task| task.number == number && task.deleted_at.is_none())
                            .map(|task| task.task_uid.clone())
                    })
                }) else {
                    continue;
                };
                let resolved_task_uid = if checklist
                    .tasks
                    .iter()
                    .any(|task| task.task_uid == task_uid && task.deleted_at.is_none())
                {
                    task_uid.clone()
                } else {
                    incoming_number
                        .and_then(|number| {
                            checklist
                                .tasks
                                .iter()
                                .find(|task| task.number == number && task.deleted_at.is_none())
                                .map(|task| task.task_uid.clone())
                        })
                        .unwrap_or_else(|| task_uid.clone())
                };
                let inserted_placeholder = ensure_task_for_incoming_update(
                    &mut checklist,
                    resolved_task_uid.as_str(),
                    timestamp.as_str(),
                    incoming_number,
                );
                let Ok(task) = find_checklist_task_mut(&mut checklist, resolved_task_uid.as_str())
                else {
                    continue;
                };
                let user_status = if msgpack_get_checklist_arg(args, "completed")
                    .and_then(msgpack_bool)
                    .unwrap_or(false)
                {
                    ChecklistUserTaskStatus::Complete {}
                } else {
                    match msgpack_get_checklist_arg(args, "user_status")
                        .and_then(msgpack_string)
                        .as_deref()
                    {
                        Some("COMPLETE") => ChecklistUserTaskStatus::Complete {},
                        _ => ChecklistUserTaskStatus::Pending {},
                    }
                };
                if !should_apply_inbound_task_status(
                    task,
                    user_status,
                    timestamp.as_str(),
                    inserted_placeholder,
                ) {
                    handled_any = true;
                    continue;
                }
                task.user_status = user_status;
                task.task_status = checklist_task_status_for(task.user_status, task.is_late);
                task.updated_at = Some(timestamp.clone());
                if task.task_status.is_complete() {
                    task.completed_at = Some(timestamp.clone());
                    task.completed_by_team_member_rns_identity =
                        msgpack_get_checklist_arg(args, "changed_by_team_member_rns_identity")
                            .and_then(msgpack_string)
                            .or_else(|| source_identity.clone());
                } else {
                    task.completed_at = None;
                    task.completed_by_team_member_rns_identity = None;
                }
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-task-status",
                );
            }
            "checklist.task.row.style.set" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                if checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                    !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                }) || (checklist.deleted_at.is_some()
                    && !is_hidden_placeholder_checklist(&checklist))
                {
                    continue;
                }
                let inserted_placeholder = ensure_task_for_incoming_update(
                    &mut checklist,
                    task_uid.as_str(),
                    timestamp.as_str(),
                    None,
                );
                let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
                    continue;
                };
                if !inserted_placeholder
                    && !incoming_timestamp_is_newer(task.updated_at.as_deref(), timestamp.as_str())
                {
                    continue;
                }
                if let Some(value) =
                    msgpack_get_checklist_arg(args, "row_background_color").and_then(msgpack_string)
                {
                    task.row_background_color = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) =
                    msgpack_get_checklist_arg(args, "line_break_enabled").and_then(msgpack_bool)
                {
                    task.line_break_enabled = value;
                }
                task.updated_at = Some(timestamp.clone());
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-task-row-style",
                );
            }
            "checklist.task.cell.set" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(column_uid) =
                    msgpack_get_checklist_arg(args, "column_uid").and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(value) = msgpack_get_checklist_arg(args, "value").and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                if checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                    !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                }) || (checklist.deleted_at.is_some()
                    && !is_hidden_placeholder_checklist(&checklist))
                {
                    continue;
                }
                if !checklist
                    .columns
                    .iter()
                    .any(|column| column.column_uid == column_uid)
                {
                    let display_order = checklist.columns.len() as u32;
                    checklist.columns.push(ChecklistColumnRecord {
                        column_uid: column_uid.clone(),
                        column_name: column_uid.clone(),
                        display_order,
                        column_type: ChecklistColumnType::ShortString {},
                        column_editable: true,
                        background_color: None,
                        text_color: None,
                        is_removable: true,
                        system_key: None,
                    });
                }
                if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
                    checklist.tasks.push(placeholder_task_record(
                        task_uid.as_str(),
                        timestamp.as_str(),
                    ));
                }
                let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
                    continue;
                };
                if let Some(cell) = task.cells.iter().find(|cell| cell.column_uid == column_uid) {
                    if !incoming_timestamp_is_newer(cell.updated_at.as_deref(), timestamp.as_str())
                    {
                        continue;
                    }
                }
                if let Some(cell) = task
                    .cells
                    .iter_mut()
                    .find(|cell| cell.column_uid == column_uid)
                {
                    cell.value = Some(value);
                    cell.updated_at = Some(timestamp.clone());
                    cell.updated_by_team_member_rns_identity =
                        msgpack_get_checklist_arg(args, "updated_by_team_member_rns_identity")
                            .and_then(msgpack_string)
                            .or_else(|| source_identity.clone());
                } else {
                    task.cells.push(ChecklistCellRecord {
                        cell_uid: format!("{}:{column_uid}", task.task_uid),
                        task_uid: task.task_uid.clone(),
                        column_uid: column_uid.clone(),
                        value: Some(value),
                        updated_at: Some(timestamp.clone()),
                        updated_by_team_member_rns_identity: msgpack_get_checklist_arg(
                            args,
                            "updated_by_team_member_rns_identity",
                        )
                        .and_then(msgpack_string)
                        .or_else(|| source_identity.clone()),
                    });
                }
                task.updated_at = Some(timestamp.clone());
                checklist.updated_at = Some(timestamp.clone());
                set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
                normalize_checklist_record(&mut checklist);
                persisted_any |= upsert_inbound_checklist(
                    app_state,
                    bus,
                    &checklist,
                    "checklist-received-task-cell",
                );
            }
            "checklist.join" => {
                let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
                    .and_then(msgpack_checklist_uid)
                else {
                    continue;
                };
                let Some(source_identity) = source_identity.clone() else {
                    continue;
                };
                let mut checklist = app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        hidden_placeholder_checklist_record(
                            checklist_uid.as_str(),
                            timestamp.as_str(),
                        )
                    });
                if checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                    !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                }) || (checklist.deleted_at.is_some()
                    && !is_hidden_placeholder_checklist(&checklist))
                {
                    continue;
                }
                if !checklist
                    .participant_rns_identities
                    .iter()
                    .any(|value| value == &source_identity)
                {
                    let changed_by = source_identity.clone();
                    checklist.participant_rns_identities.push(source_identity);
                    checklist.updated_at = Some(timestamp.clone());
                    set_checklist_last_changed_by(&mut checklist, Some(changed_by.as_str()));
                    normalize_checklist_record(&mut checklist);
                    persisted_any |= upsert_inbound_checklist(
                        app_state,
                        bus,
                        &checklist,
                        "checklist-received-join",
                    );
                }
            }
            _ => {}
        }
    }
    persisted_any || handled_any
}

#[derive(Debug, Deserialize)]
struct MissionWireSource {
    rns_identity: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EamUpsertCommandArgs {
    callsign: String,
    team_member_uid: String,
    team_uid: String,
    security_status: String,
    capability_status: String,
    preparedness_status: String,
    medical_status: String,
    mobility_status: String,
    comms_status: String,
    eam_uid: Option<String>,
    reported_by: Option<String>,
    reported_at: Option<String>,
    notes: Option<String>,
    confidence: Option<f64>,
    ttl_seconds: Option<u64>,
    source: Option<MissionWireSource>,
}

#[derive(Debug, Deserialize)]
struct MissionCommandEnvelope<T> {
    source: MissionWireSource,
    timestamp: String,
    command_type: String,
    args: T,
}

#[derive(Debug, Deserialize)]
struct EamWireBody {
    command: MissionCommandEnvelope<EamUpsertCommandArgs>,
    projection: Option<EamProjectionRecord>,
}

fn msgpack_map_entries(value: &MsgPackValue) -> Option<&[(MsgPackValue, MsgPackValue)]> {
    match value {
        MsgPackValue::Map(entries) => Some(entries.as_slice()),
        _ => None,
    }
}

fn msgpack_get_indexed(
    entries: &[(MsgPackValue, MsgPackValue)],
    key: i64,
) -> Option<&MsgPackValue> {
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

fn msgpack_get_named<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    keys: &[&str],
) -> Option<&'a MsgPackValue> {
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
        MsgPackValue::String(value) => value.as_str().map(str::to_string),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone()).ok(),
        _ => None,
    }
}

fn msgpack_hex_or_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => Some(hex::encode(value)),
        _ => msgpack_string(value),
    }
}

fn msgpack_event_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => {
            let hex = hex::encode(value);
            Some(format!(
                "evt-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_eam_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => {
            let hex = hex::encode(value);
            Some(format!(
                "eam-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_eam_status(value: &MsgPackValue) -> Option<String> {
    msgpack_string(value).map(|status| match status.as_str() {
        "G" => "Green".to_string(),
        "Y" => "Yellow".to_string(),
        "R" => "Red".to_string(),
        "U" => "Unknown".to_string(),
        _ => status,
    })
}

fn msgpack_eam_status_array<'a>(
    args: &'a [(MsgPackValue, MsgPackValue)],
) -> [Option<&'a MsgPackValue>; 6] {
    let mut statuses = [None, None, None, None, None, None];
    if let Some(values) =
        msgpack_get_named(args, &["statuses", "s"]).and_then(MsgPackValue::as_array)
    {
        for (index, value) in values.iter().take(statuses.len()).enumerate() {
            statuses[index] = Some(value);
        }
    }
    statuses
}

fn event_command_id_from_tail(uid: &str, value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(bytes) if bytes.len() == 16 => {
            let hex = hex::encode(bytes);
            Some(format!(
                "log-entry-{uid}-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => {
            let tail = msgpack_string(value)?;
            if tail.starts_with("log-entry-") {
                Some(tail)
            } else {
                Some(format!("log-entry-{uid}-{tail}"))
            }
        }
    }
}

fn msgpack_mission_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) if value.as_u64() == Some(0) => {
            Some(DEFAULT_R3AKT_MISSION_UID.to_string())
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_timestamp(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().map(|timestamp| {
            if timestamp < 10_000_000_000 {
                timestamp_ms_to_rfc3339(timestamp.saturating_mul(1_000))
            } else {
                timestamp_ms_to_rfc3339(timestamp)
            }
        }),
        _ => msgpack_string(value),
    }
}

fn msgpack_string_vec(value: &MsgPackValue) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(entries.iter().filter_map(msgpack_string).collect())
}

fn msgpack_event_keywords(value: &MsgPackValue) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(
        entries
            .iter()
            .filter_map(|entry| {
                let keyword = msgpack_string(entry)?;
                if keyword.len() <= 4 && keyword.chars().all(|ch| ch.is_ascii_alphanumeric()) {
                    Some(format!("r3akt:event-type:{keyword}"))
                } else {
                    Some(keyword)
                }
            })
            .collect(),
    )
}

fn msgpack_event_topics(value: &MsgPackValue, mission_uid: &str) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(
        entries
            .iter()
            .filter_map(|entry| match entry {
                MsgPackValue::Integer(value) if value.as_u64() == Some(0) => {
                    Some(mission_uid.to_string())
                }
                MsgPackValue::Integer(value) if value.as_u64() == Some(1) => {
                    Some("Default".to_string())
                }
                _ => msgpack_string(entry),
            })
            .collect(),
    )
}

fn msgpack_bool(value: &MsgPackValue) -> Option<bool> {
    match value {
        MsgPackValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

fn msgpack_f64(value: &MsgPackValue) -> Option<f64> {
    match value {
        MsgPackValue::F32(value) => Some(f64::from(*value)),
        MsgPackValue::F64(value) => Some(*value),
        MsgPackValue::Integer(value) => value.as_i64().map(|entry| entry as f64),
        _ => None,
    }
}

fn msgpack_u64(value: &MsgPackValue) -> Option<u64> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().or_else(|| {
            value
                .as_i64()
                .and_then(|entry| (entry >= 0).then_some(entry as u64))
        }),
        _ => None,
    }
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

#[derive(Default)]
struct InterfaceTrafficSample {
    packets: u64,
    bytes: u64,
    announces: u64,
    data: u64,
    proofs: u64,
    link_requests: u64,
}

impl InterfaceTrafficSample {
    fn record(&mut self, packet: &Packet) {
        self.packets = self.packets.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(packet.data.as_slice().len() as u64);
        match packet.header.packet_type {
            PacketType::Announce => {
                self.announces = self.announces.saturating_add(1);
            }
            PacketType::Data => {
                self.data = self.data.saturating_add(1);
            }
            PacketType::Proof => {
                self.proofs = self.proofs.saturating_add(1);
            }
            PacketType::LinkRequest => {
                self.link_requests = self.link_requests.saturating_add(1);
            }
        }
    }
}

type ActiveInterfaceRegistry = Arc<TokioMutex<HashMap<AddressHash, InterfaceStatusRecord>>>;

fn interface_status_kind(label: &str) -> &'static str {
    if interface_label_is_rnode_ble(label) {
        "rnode_ble"
    } else {
        "tcp_client"
    }
}

fn new_interface_status(
    interface: AddressHash,
    label: String,
    state: &'static str,
) -> InterfaceStatusRecord {
    let kind = interface_status_kind(&label).to_string();
    InterfaceStatusRecord {
        interface_hex: interface.to_hex_string(),
        label,
        kind,
        state: state.to_string(),
        last_error: None,
        rx_packets: 0,
        rx_bytes: 0,
        last_activity_ms: 0,
    }
}

async fn publish_interface_registry_snapshot(
    active_interface_registry: &ActiveInterfaceRegistry,
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    changed: Option<InterfaceStatusRecord>,
) {
    let mut interfaces = active_interface_registry
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    interfaces.sort_by(|left, right| left.label.cmp(&right.label));
    if let Ok(mut guard) = status.lock() {
        guard.interfaces = interfaces;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
    if let Some(status) = changed {
        bus.emit(NodeEvent::InterfaceStatusChanged { status });
    }
}

fn effective_announce_interval_seconds(configured_seconds: u32) -> u32 {
    configured_seconds.max(MIN_EFFECTIVE_ANNOUNCE_INTERVAL_SECONDS)
}

fn spawn_interface_traffic_monitor(
    transport: Arc<Transport>,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    tokio::spawn(async move {
        let mut rx = transport.iface_rx();
        let mut interval = tokio::time::interval(INTERFACE_TRAFFIC_LOG_INTERVAL);
        let mut samples = HashMap::<AddressHash, InterfaceTrafficSample>::new();
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if samples.is_empty() {
                        continue;
                    }
                    let mut changed = Vec::new();
                    let mut rows = samples.drain().collect::<Vec<_>>();
                    rows.sort_by_key(|(_, sample)| std::cmp::Reverse(sample.bytes));
                    {
                        let mut endpoints = active_interface_registry.lock().await;
                        for (interface, sample) in rows.iter() {
                            if let Some(record) = endpoints.get_mut(interface) {
                                record.rx_packets = record.rx_packets.saturating_add(sample.packets);
                                record.rx_bytes = record.rx_bytes.saturating_add(sample.bytes);
                                record.last_activity_ms = now_ms();
                                changed.push(record.clone());
                            }
                        }
                    }
                    for (interface, sample) in rows {
                        let endpoints = active_interface_registry.lock().await;
                        let endpoint = endpoints
                            .get(&interface)
                            .map(|record| record.label.as_str())
                            .unwrap_or("unknown");
                        info!(
                            "[iface][rx] endpoint=<{}> iface={} packets={} bytes={} announces={} data={} proofs={} link_requests={}",
                            endpoint,
                            interface,
                            sample.packets,
                            sample.bytes,
                            sample.announces,
                            sample.data,
                            sample.proofs,
                            sample.link_requests,
                        );
                    }
                    for status_update in changed {
                        publish_interface_registry_snapshot(
                            &active_interface_registry,
                            &status,
                            &bus,
                            Some(status_update),
                        )
                        .await;
                    }
                }
                message = rx.recv() => {
                    match message {
                        Ok(message) => {
                            samples
                                .entry(message.address)
                                .or_default()
                                .record(&message.packet);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("[iface][rx] monitor lagged skipped={}", skipped);
                        }
                    }
                }
            }
        }
    });
}

async fn announce_destinations(
    transport: &Arc<Transport>,
    _app_destination: &Arc<TokioMutex<SingleInputDestination>>,
    lxmf_destination: &Arc<TokioMutex<SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<String>>,
    reason: &str,
) {
    let caps = announce_capabilities.lock().await.clone();
    let lxmf_hex = lxmf_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    info!(
        "[announce] sending reason={} kind={} destination={}",
        reason, DESTINATION_KIND_LXMF_DELIVERY, lxmf_hex,
    );
    transport
        .set_destination_announce_app_data(lxmf_destination, Some(caps.as_bytes().to_vec()))
        .await;
    send_announce_with_trace(
        transport,
        lxmf_destination,
        Some(caps.as_bytes()),
        reason,
        DESTINATION_KIND_LXMF_DELIVERY,
    )
    .await;
}

async fn send_announce_with_trace(
    transport: &Arc<Transport>,
    destination: &Arc<TokioMutex<SingleInputDestination>>,
    app_data: Option<&[u8]>,
    reason: &str,
    destination_kind: &str,
) {
    let (destination_hex, app_data_len, packet) = {
        let mut destination = destination.lock().await;
        let destination_hex = destination.desc.address_hash.to_hex_string();
        let app_data_len = app_data.map(|value| value.len()).unwrap_or(0);
        let packet = destination
            .announce(OsRng, app_data)
            .expect("valid announce packet");
        (destination_hex, app_data_len, packet)
    };
    let trace = transport.send_packet_with_trace(packet).await;
    info!(
        "[announce][tx] reason={} kind={} destination={} app_data_len={} outcome={:?} broadcast={} direct_iface={} matched={} sent={} failed={}",
        reason,
        destination_kind,
        destination_hex,
        app_data_len,
        trace.outcome,
        trace.broadcast,
        trace
            .direct_iface
            .map(|iface| iface.to_hex_string())
            .unwrap_or_else(|| "none".to_string()),
        trace.dispatch.matched_ifaces,
        trace.dispatch.sent_ifaces,
        trace.dispatch.failed_ifaces,
    );
}

fn announce_destination_kind_from_name_hash(name_hash: &[u8]) -> &'static str {
    let app_name = DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1);
    if name_hash == app_name.as_name_hash_slice() {
        return DESTINATION_KIND_APP;
    }

    let lxmf_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    if name_hash == lxmf_name.as_name_hash_slice() {
        return DESTINATION_KIND_LXMF_DELIVERY;
    }

    let propagation_name = DestinationName::new(LXMF_PROPAGATION_NAME.0, LXMF_PROPAGATION_NAME.1);
    if name_hash == propagation_name.as_name_hash_slice() {
        return DESTINATION_KIND_LXMF_PROPAGATION;
    }

    DESTINATION_KIND_OTHER
}

fn app_data_has_rem_peer_capabilities(app_data: &str) -> bool {
    supports_mission_traffic(Some(app_data))
}

fn classify_announce(destination_kind: &str, app_data: &str) -> AnnounceClass {
    let tokens = parse_announce_metadata(app_data).capability_tokens;
    if tokens.iter().any(|token| token == "r3akt")
        && RCH_SERVER_FEATURE_CAPABILITIES
            .iter()
            .any(|capability| tokens.iter().any(|token| token == capability))
    {
        return AnnounceClass::RchHubServer {};
    }

    match destination_kind {
        DESTINATION_KIND_LXMF_PROPAGATION => AnnounceClass::PropagationNode {},
        DESTINATION_KIND_LXMF_DELIVERY => AnnounceClass::LxmfDelivery {},
        _ => {
            if supports_mission_traffic(Some(app_data)) {
                return AnnounceClass::PeerApp {};
            }
            AnnounceClass::Other {}
        }
    }
}

fn announce_class_is_operator_relevant(class: AnnounceClass) -> bool {
    matches!(class, AnnounceClass::RchHubServer {})
}

fn announce_is_operator_relevant(class: AnnounceClass, is_rem_capable_lxmf_delivery: bool) -> bool {
    announce_class_is_operator_relevant(class)
        || (matches!(class, AnnounceClass::LxmfDelivery {}) && is_rem_capable_lxmf_delivery)
}

fn operator_label(display_name: Option<&str>, fallback_hex: &str) -> String {
    display_name
        .and_then(normalize_rem_display_name)
        .unwrap_or_else(|| fallback_hex.to_ascii_lowercase())
}

fn short_destination_hex(value: &str) -> String {
    let prefix = value
        .chars()
        .take(5)
        .collect::<String>()
        .to_ascii_lowercase();
    if prefix.len() < value.len() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn operator_announce_message(
    announce_class: AnnounceClass,
    is_rem_capable_lxmf_delivery: bool,
    display_name: Option<&str>,
    destination_hex: &str,
    _identity_hex: &str,
    hops: u8,
) -> Option<String> {
    if !announce_is_operator_relevant(announce_class, is_rem_capable_lxmf_delivery) {
        return None;
    }

    let subject = operator_label(display_name, destination_hex);
    let prefix = match announce_class {
        AnnounceClass::RchHubServer {} => "RCH hub",
        AnnounceClass::LxmfDelivery {} if is_rem_capable_lxmf_delivery => "",
        _ => return None,
    };
    let label = if prefix.is_empty() {
        subject
    } else {
        format!("{prefix} {subject}")
    };
    Some(format!(
        "[announce] {label} dest={} hops={hops}.",
        short_destination_hex(destination_hex),
    ))
}

fn emit_operational_notice(bus: &EventBus, level: LogLevel, message: impl Into<String>) {
    bus.emit(NodeEvent::OperationalNotice {
        notice: OperationalNotice {
            level,
            message: message.into(),
            at_ms: now_ms(),
        },
    });
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

fn to_sdk_transport_delivery_state(
    state: TransportDeliveryState,
) -> sdkmsg::TransportDeliveryState {
    match state {
        TransportDeliveryState::Queued {} => sdkmsg::TransportDeliveryState::Queued,
        TransportDeliveryState::Sending {} => sdkmsg::TransportDeliveryState::Sending,
        TransportDeliveryState::SentDirect {} => sdkmsg::TransportDeliveryState::SentDirect,
        TransportDeliveryState::SentToPropagation {} => {
            sdkmsg::TransportDeliveryState::SentToPropagation
        }
        TransportDeliveryState::TransportDelivered {} => {
            sdkmsg::TransportDeliveryState::TransportDelivered
        }
        TransportDeliveryState::Failed {} => sdkmsg::TransportDeliveryState::Failed,
        TransportDeliveryState::TimedOut {} => sdkmsg::TransportDeliveryState::TimedOut,
        TransportDeliveryState::Cancelled {} => sdkmsg::TransportDeliveryState::Cancelled,
    }
}

fn from_sdk_transport_delivery_state(
    state: sdkmsg::TransportDeliveryState,
) -> TransportDeliveryState {
    match state {
        sdkmsg::TransportDeliveryState::Queued => TransportDeliveryState::Queued {},
        sdkmsg::TransportDeliveryState::Sending => TransportDeliveryState::Sending {},
        sdkmsg::TransportDeliveryState::SentDirect => TransportDeliveryState::SentDirect {},
        sdkmsg::TransportDeliveryState::SentToPropagation => {
            TransportDeliveryState::SentToPropagation {}
        }
        sdkmsg::TransportDeliveryState::TransportDelivered => {
            TransportDeliveryState::TransportDelivered {}
        }
        sdkmsg::TransportDeliveryState::Failed => TransportDeliveryState::Failed {},
        sdkmsg::TransportDeliveryState::TimedOut => TransportDeliveryState::TimedOut {},
        sdkmsg::TransportDeliveryState::Cancelled => TransportDeliveryState::Cancelled {},
    }
}

fn to_sdk_application_ack_state(state: ApplicationAckState) -> sdkmsg::ApplicationAckState {
    match state {
        ApplicationAckState::NotRequired {} => sdkmsg::ApplicationAckState::NotRequired,
        ApplicationAckState::Waiting {} => sdkmsg::ApplicationAckState::Waiting,
        ApplicationAckState::Accepted {} => sdkmsg::ApplicationAckState::Accepted,
        ApplicationAckState::Completed {} => sdkmsg::ApplicationAckState::Completed,
        ApplicationAckState::Rejected {} => sdkmsg::ApplicationAckState::Rejected,
        ApplicationAckState::Failed {} => sdkmsg::ApplicationAckState::Failed,
    }
}

fn from_sdk_application_ack_state(state: sdkmsg::ApplicationAckState) -> ApplicationAckState {
    match state {
        sdkmsg::ApplicationAckState::NotRequired => ApplicationAckState::NotRequired {},
        sdkmsg::ApplicationAckState::Waiting => ApplicationAckState::Waiting {},
        sdkmsg::ApplicationAckState::Accepted => ApplicationAckState::Accepted {},
        sdkmsg::ApplicationAckState::Completed => ApplicationAckState::Completed {},
        sdkmsg::ApplicationAckState::Rejected => ApplicationAckState::Rejected {},
        sdkmsg::ApplicationAckState::Failed => ApplicationAckState::Failed {},
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

fn to_sdk_send_mode(mode: SendMode) -> sdkmsg::SendMode {
    match mode {
        SendMode::Auto {} => sdkmsg::SendMode::Auto,
        SendMode::DirectOnly {} => sdkmsg::SendMode::DirectOnly,
        SendMode::PropagationOnly {} => sdkmsg::SendMode::PropagationOnly,
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
    let parsed_display_name = parse_announce_metadata(&record.app_data).display_name;
    let announce_class = classify_announce(&record.destination_kind, &record.app_data);
    AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        announce_class,
        app_data: record.app_data,
        display_name: record.display_name.or(parsed_display_name),
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn normalize_announce_app_data(app_data: &[u8]) -> String {
    String::from_utf8(app_data.to_vec()).unwrap_or_else(|_| hex::encode(app_data))
}

fn lxmf_sdk_announce_record_from_raw(
    destination_hex: impl Into<String>,
    identity_hex: impl Into<String>,
    destination_kind: impl Into<String>,
    app_data: &[u8],
    hops: u8,
    interface_hex: impl Into<String>,
    received_at_ms: u64,
) -> LxmfSdkAnnounceRecord {
    let destination_kind = destination_kind.into();
    let display_name = if destination_kind == DESTINATION_KIND_LXMF_DELIVERY {
        display_name_from_delivery_app_data(app_data).into_display_name_option()
    } else {
        None
    };
    LxmfSdkAnnounceRecord {
        destination_hex: destination_hex.into(),
        identity_hex: identity_hex.into(),
        destination_kind,
        app_data: normalize_announce_app_data(app_data),
        display_name,
        hops,
        interface_hex: interface_hex.into(),
        received_at_ms,
    }
}

trait IntoDisplayNameOption {
    fn into_display_name_option(self) -> Option<String>;
}

impl IntoDisplayNameOption for Option<String> {
    fn into_display_name_option(self) -> Option<String> {
        self
    }
}

impl<E> IntoDisplayNameOption for Result<Option<String>, E> {
    fn into_display_name_option(self) -> Option<String> {
        self.unwrap_or(None)
    }
}

fn to_compat_announce_record(record: &LxmfSdkAnnounceRecord) -> sdkmsg::AnnounceRecord {
    sdkmsg::AnnounceRecord {
        destination_hex: record.destination_hex.clone(),
        identity_hex: record.identity_hex.clone(),
        destination_kind: record.destination_kind.clone(),
        app_data: record.app_data.clone(),
        display_name: record.display_name.clone(),
        hops: record.hops,
        interface_hex: record.interface_hex.clone(),
        received_at_ms: record.received_at_ms,
    }
}

fn from_lxmf_sdk_announce_record(record: LxmfSdkAnnounceRecord) -> AnnounceRecord {
    let parsed_display_name = parse_announce_metadata(&record.app_data).display_name;
    let announce_class = classify_announce(&record.destination_kind, &record.app_data);
    AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        announce_class,
        app_data: record.app_data,
        display_name: record.display_name.or(parsed_display_name),
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
        requested_destination_hex: record.requested_destination_hex,
        delivery_destination_hex: record.delivery_destination_hex,
        recipient_identity_hex: record.recipient_identity_hex,
        last_wire_message_id_hex: record.last_wire_message_id_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: to_sdk_message_method(record.method),
        state: to_sdk_message_state(record.state),
        transport_state: to_sdk_transport_delivery_state(record.transport_state),
        application_ack_state: to_sdk_application_ack_state(record.application_ack_state),
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
        requested_destination_hex: record.requested_destination_hex,
        delivery_destination_hex: record.delivery_destination_hex,
        recipient_identity_hex: record.recipient_identity_hex,
        last_wire_message_id_hex: record.last_wire_message_id_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: from_sdk_message_method(record.method),
        state: from_sdk_message_state(record.state),
        transport_state: from_sdk_transport_delivery_state(record.transport_state),
        application_ack_state: from_sdk_application_ack_state(record.application_ack_state),
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
        saved: record.saved,
        stale: record.stale,
        active_link: record.active_link,
        hub_derived: false,
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}

fn from_sdk_peer_change(change: sdkmsg::PeerChange) -> PeerChange {
    PeerChange {
        destination_hex: change.destination_hex,
        identity_hex: change.identity_hex,
        lxmf_destination_hex: change.lxmf_destination_hex,
        display_name: change.display_name,
        app_data: change.app_data,
        state: from_sdk_peer_state(change.state),
        saved: change.saved,
        stale: change.stale,
        active_link: change.active_link,
        last_error: change.last_error,
        last_resolution_error: change.last_resolution_error,
        last_resolution_attempt_at_ms: change.last_resolution_attempt_at_ms,
        last_seen_at_ms: change.last_seen_at_ms,
        announce_last_seen_at_ms: change.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: change.lxmf_last_seen_at_ms,
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

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn conversation_peer_resolver(peers: &[PeerRecord]) -> ConversationPeerResolver {
    let mut resolver = ConversationPeerResolver::default();
    for peer in peers {
        let destination_hex = match trimmed_non_empty(Some(peer.destination_hex.as_str())) {
            Some(value) => value,
            None => continue,
        };
        let lxmf_destination_hex = trimmed_non_empty(peer.lxmf_destination_hex.as_deref());
        let identity_hex = trimmed_non_empty(peer.identity_hex.as_deref());
        let canonical_id = identity_hex
            .clone()
            .or_else(|| lxmf_destination_hex.clone())
            .unwrap_or_else(|| destination_hex.clone());
        let peer_destination_hex = lxmf_destination_hex
            .clone()
            .unwrap_or_else(|| destination_hex.clone());
        let mut aliases = vec![destination_hex];
        if let Some(lxmf_destination_hex) = lxmf_destination_hex {
            aliases.push(lxmf_destination_hex);
        }
        if let Some(identity_hex) = identity_hex {
            aliases.push(identity_hex);
        }
        resolver.insert(
            aliases,
            canonical_id,
            peer_destination_hex,
            peer.display_name.clone(),
        );
    }
    resolver
}

fn conversation_delete_keys(conversation_id: &str, peers: &[PeerRecord]) -> Vec<String> {
    let normalized_conversation_id = conversation_id.trim().to_ascii_lowercase();
    if normalized_conversation_id.is_empty() {
        return Vec::new();
    }

    let mut keys = HashSet::from([normalized_conversation_id.clone()]);
    for peer in peers {
        let aliases = [
            trimmed_non_empty(Some(peer.destination_hex.as_str())),
            trimmed_non_empty(peer.lxmf_destination_hex.as_deref()),
            trimmed_non_empty(peer.identity_hex.as_deref()),
        ];
        let matches_peer = aliases.iter().flatten().any(|alias| {
            alias
                .trim()
                .eq_ignore_ascii_case(normalized_conversation_id.as_str())
        });
        if matches_peer {
            for alias in aliases.into_iter().flatten() {
                keys.insert(alias.trim().to_ascii_lowercase());
            }
        }
    }
    let mut out = keys.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn to_sdk_sync_status(status: SyncStatus) -> Option<sdkmsg::SyncStatus> {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn to_sdk_send_request(request: &SendLxmfRequest) -> sdkmsg::SendMessageRequest {
    sdkmsg::SendMessageRequest {
        destination_hex: request.destination_hex.clone(),
        body_utf8: request.body_utf8.clone(),
        title: request.title.clone(),
        send_mode: to_sdk_send_mode(request.send_mode),
        use_propagation_node: matches!(request.send_mode, SendMode::PropagationOnly {}),
    }
}

#[derive(Debug, Clone)]
struct PendingLxmfResend {
    requested_destination_hex: String,
    body: Vec<u8>,
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: MissionSyncMetadata,
    send_task_class: SendTaskClass,
    original_send_mode: SendMode,
    direct_ack_retry_attempted: bool,
    propagation_fallback_attempted: bool,
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
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    relay_destination_hex: Option<String>,
    fallback_stage: Option<LxmfFallbackStage>,
    resend: Option<PendingLxmfResend>,
    sent_at_ms: u64,
}

#[derive(Debug, Clone)]
struct PendingLxmfAcknowledgement {
    source_hex: String,
    detail: Option<String>,
    application_ack_state: ApplicationAckState,
    buffered_at_ms: u64,
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
    pub(crate) used_propagation_node: bool,
    pub(crate) method: LxmfDeliveryMethod,
    pub(crate) representation: LxmfDeliveryRepresentation,
    pub(crate) relay_destination_hex: Option<String>,
    pub(crate) fallback_stage: Option<LxmfFallbackStage>,
    pub(crate) receipt_hash_hex: Option<String>,
}

struct RuntimeReceiptBridge {
    receipt_message_ids: Arc<Mutex<HashMap<String, ReceiptMessageTracking>>>,
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Debug, Clone)]
struct ReceiptMessageTracking {
    message_id_hex: String,
    recorded_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendTaskClass {
    Mission,
    MissionAck,
    MissionPropagation,
    MissionRecovery,
    General,
}

impl SendTaskClass {
    fn from_lxmf_request(
        has_fields: bool,
        metadata: Option<&MissionSyncMetadata>,
        send_mode: &SendMode,
    ) -> Self {
        if !has_fields {
            return Self::General;
        }
        if !metadata.is_some_and(MissionSyncMetadata::is_mission_related) {
            return Self::General;
        }
        if is_accepted_result_metadata(metadata) {
            return Self::MissionAck;
        }
        if is_sos_status_metadata(metadata) {
            return Self::MissionRecovery;
        }
        if matches!(send_mode, SendMode::PropagationOnly {}) {
            Self::MissionPropagation
        } else {
            Self::Mission
        }
    }

    fn propagation_equivalent(self) -> Self {
        match self {
            Self::Mission | Self::MissionAck | Self::MissionPropagation => Self::MissionPropagation,
            Self::MissionRecovery => Self::MissionRecovery,
            Self::General => Self::General,
        }
    }

    fn direct_recovery_equivalent(self) -> Self {
        match self {
            Self::Mission | Self::MissionAck | Self::MissionPropagation | Self::MissionRecovery => {
                Self::MissionRecovery
            }
            Self::General => Self::General,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mission => "mission-direct",
            Self::MissionAck => "mission-ack",
            Self::MissionPropagation => "mission-propagation",
            Self::MissionRecovery => "mission-recovery",
            Self::General => "general",
        }
    }
}

fn should_emit_global_send_bytes_error(send_task_class: SendTaskClass) -> bool {
    matches!(send_task_class, SendTaskClass::General)
}

#[derive(Clone)]
struct SendTaskPermits {
    general: Arc<Semaphore>,
    mission: Arc<Semaphore>,
    mission_ack: Arc<Semaphore>,
    mission_propagation: Arc<Semaphore>,
    mission_recovery: Arc<Semaphore>,
}

impl SendTaskPermits {
    fn new() -> Self {
        Self {
            general: Arc::new(Semaphore::new(GENERAL_SEND_TASK_CONCURRENCY_LIMIT)),
            mission: Arc::new(Semaphore::new(MISSION_SEND_TASK_RESERVED_LIMIT)),
            mission_ack: Arc::new(Semaphore::new(MISSION_ACK_SEND_TASK_RESERVED_LIMIT)),
            mission_propagation: Arc::new(Semaphore::new(
                MISSION_PROPAGATION_SEND_TASK_RESERVED_LIMIT,
            )),
            mission_recovery: Arc::new(Semaphore::new(MISSION_RECOVERY_SEND_TASK_RESERVED_LIMIT)),
        }
    }

    #[cfg(test)]
    fn with_limits(general: usize, mission: usize) -> Self {
        Self {
            general: Arc::new(Semaphore::new(general)),
            mission: Arc::new(Semaphore::new(mission)),
            mission_ack: Arc::new(Semaphore::new(1)),
            mission_propagation: Arc::new(Semaphore::new(mission)),
            mission_recovery: Arc::new(Semaphore::new(1)),
        }
    }

    async fn acquire(&self, class: SendTaskClass) -> Result<OwnedSemaphorePermit, NodeError> {
        match class {
            SendTaskClass::Mission => self
                .mission
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
            SendTaskClass::MissionAck => self
                .mission_ack
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
            SendTaskClass::MissionPropagation => self
                .mission_propagation
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
            SendTaskClass::MissionRecovery => self
                .mission_recovery
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
            SendTaskClass::General => self
                .general
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
        }
    }
}

#[derive(Clone, Default)]
struct DirectDeliveryHealth {
    cooldown_until_ms: Arc<Mutex<HashMap<String, u64>>>,
}

impl DirectDeliveryHealth {
    fn mark_unhealthy<'a, I>(&self, destinations: I, until_ms: u64)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return;
        };
        for destination in destinations {
            if let Some(normalized) = normalize_hex_32(destination) {
                guard.insert(normalized, until_ms);
            }
        }
    }

    fn clear<'a, I>(&self, destinations: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return;
        };
        for destination in destinations {
            if let Some(normalized) = normalize_hex_32(destination) {
                guard.remove(normalized.as_str());
            }
        }
    }

    fn is_available(&self, destination: &str, now_ms: u64) -> bool {
        let Some(normalized) = normalize_hex_32(destination) else {
            return true;
        };
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return true;
        };
        match guard.get(normalized.as_str()).copied() {
            Some(until_ms) if until_ms > now_ms => false,
            Some(_) => {
                guard.remove(normalized.as_str());
                true
            }
            None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManagedPeerLinkKind {
    App,
    LxmfDelivery,
}

impl ManagedPeerLinkKind {
    fn destination_name(self) -> DestinationName {
        match self {
            Self::App => DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
            Self::LxmfDelivery => DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManagedPeerLinkTarget {
    destination_hex: String,
    kind: ManagedPeerLinkKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ManagedPeerLinkBackoff {
    attempts: u32,
    next_retry_at_ms: u64,
    last_failure_reason: Option<String>,
}

impl ManagedPeerLinkBackoff {
    fn next_delay_ms(&self) -> u64 {
        let exponent = self
            .attempts
            .saturating_sub(1)
            .min(SAVED_PEER_LINK_BACKOFF_MAX_ATTEMPTS);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        SAVED_PEER_LINK_BACKOFF_BASE_MS
            .saturating_mul(multiplier)
            .min(SAVED_PEER_LINK_BACKOFF_MAX_MS)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ManagedPeerReconnectStart {
    Started(ManagedPeerLinkTarget),
    Backoff {
        next_retry_at_ms: u64,
        last_failure_reason: Option<String>,
    },
    AlreadyReconnecting,
    NotDesired,
}

#[derive(Clone, Default)]
struct ManagedPeerLinks {
    desired: Arc<TokioMutex<HashMap<String, ManagedPeerLinkTarget>>>,
    reconnecting: Arc<TokioMutex<HashMap<String, ManagedPeerLinkKind>>>,
    failures: Arc<TokioMutex<HashMap<String, ManagedPeerLinkBackoff>>>,
}

impl ManagedPeerLinks {
    async fn add_desired(&self, target: ManagedPeerLinkTarget) {
        self.desired
            .lock()
            .await
            .insert(target.destination_hex.clone(), target);
    }

    async fn remove_desired<'a, I>(&self, destinations: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let normalized = destinations
            .into_iter()
            .filter_map(normalize_hex_32)
            .collect::<Vec<_>>();
        if normalized.is_empty() {
            return;
        }
        {
            let mut desired = self.desired.lock().await;
            for destination in &normalized {
                desired.remove(destination.as_str());
            }
        }
        let mut reconnecting = self.reconnecting.lock().await;
        for destination in normalized {
            reconnecting.remove(destination.as_str());
            self.failures.lock().await.remove(destination.as_str());
        }
    }

    async fn desired_targets(&self) -> Vec<ManagedPeerLinkTarget> {
        let now = now_ms();
        let desired = self.desired.lock().await;
        let failures = self.failures.lock().await;
        desired
            .values()
            .filter(|target| {
                failures
                    .get(target.destination_hex.as_str())
                    .is_none_or(|failure| failure.next_retry_at_ms <= now)
            })
            .cloned()
            .collect()
    }

    async fn clear_failure(&self, destination_hex: &str) {
        if let Some(normalized) = normalize_hex_32(destination_hex) {
            self.failures.lock().await.remove(normalized.as_str());
        }
    }

    async fn begin_reconnect(&self, destination_hex: &str) -> ManagedPeerReconnectStart {
        let Some(normalized) = normalize_hex_32(destination_hex) else {
            return ManagedPeerReconnectStart::NotDesired;
        };
        let now = now_ms();
        let Some(target) = self.desired.lock().await.get(normalized.as_str()).cloned() else {
            return ManagedPeerReconnectStart::NotDesired;
        };
        if let Some(failure) = self.failures.lock().await.get(normalized.as_str()) {
            if failure.next_retry_at_ms > now {
                return ManagedPeerReconnectStart::Backoff {
                    next_retry_at_ms: failure.next_retry_at_ms,
                    last_failure_reason: failure.last_failure_reason.clone(),
                };
            }
        }
        let mut reconnecting = self.reconnecting.lock().await;
        if let Some(active_kind) = reconnecting.get(normalized.as_str()) {
            if *active_kind == target.kind {
                return ManagedPeerReconnectStart::AlreadyReconnecting;
            }
            if !matches!(
                (*active_kind, target.kind),
                (ManagedPeerLinkKind::App, ManagedPeerLinkKind::LxmfDelivery)
            ) {
                return ManagedPeerReconnectStart::AlreadyReconnecting;
            }
        }
        reconnecting.insert(normalized.clone(), target.kind);
        ManagedPeerReconnectStart::Started(target)
    }

    async fn finish_reconnect(&self, target: &ManagedPeerLinkTarget, result: Result<(), String>) {
        if let Some(normalized) = normalize_hex_32(target.destination_hex.as_str()) {
            let obsolete_reconnect = {
                let mut reconnecting = self.reconnecting.lock().await;
                match reconnecting.get(normalized.as_str()).copied() {
                    Some(kind) if kind == target.kind => {
                        reconnecting.remove(normalized.as_str());
                        false
                    }
                    Some(_) => true,
                    None => false,
                }
            };
            if obsolete_reconnect {
                return;
            }
            match result {
                Ok(()) => {
                    self.failures.lock().await.remove(normalized.as_str());
                }
                Err(reason) => {
                    let mut failures = self.failures.lock().await;
                    let failure = failures.entry(normalized).or_default();
                    failure.attempts = failure
                        .attempts
                        .saturating_add(1)
                        .min(SAVED_PEER_LINK_BACKOFF_MAX_ATTEMPTS);
                    failure.last_failure_reason = Some(reason);
                    failure.next_retry_at_ms = now_ms().saturating_add(failure.next_delay_ms());
                }
            }
        }
    }
}

#[derive(Clone)]
struct MissionDestinationLocks {
    locks: Arc<TokioMutex<HashMap<String, Arc<TokioMutex<()>>>>>,
}

impl MissionDestinationLocks {
    fn new() -> Self {
        Self {
            locks: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    async fn acquire(&self, destination_hex: &str) -> Result<OwnedMutexGuard<()>, NodeError> {
        let key = normalize_hex_32(destination_hex)
            .unwrap_or_else(|| destination_hex.trim().to_ascii_lowercase());
        if key.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let lock = {
            let mut guard = self.locks.lock().await;
            guard
                .entry(key)
                .or_insert_with(|| Arc::new(TokioMutex::new(())))
                .clone()
        };
        Ok(lock.lock_owned().await)
    }
}

fn log_send_task(class: SendTaskClass, message: String) {
    match class {
        SendTaskClass::Mission
        | SendTaskClass::MissionAck
        | SendTaskClass::MissionPropagation
        | SendTaskClass::MissionRecovery => {
            info!("{message}")
        }
        SendTaskClass::General => debug!("{message}"),
    }
}

impl ReceiptHandler for RuntimeReceiptBridge {
    fn on_receipt(&self, receipt: &DeliveryReceipt) {
        let packet_hash_hex = hex::encode(receipt.message_id);
        let Some(message_id_hex) = self
            .receipt_message_ids
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&packet_hash_hex))
            .map(|tracking| tracking.message_id_hex)
        else {
            return;
        };
        let _ = self.tx.send(message_id_hex);
    }
}

fn transport_state_for_lxmf_status(status: LxmfDeliveryStatus) -> TransportDeliveryState {
    match status {
        LxmfDeliveryStatus::Sent {} => TransportDeliveryState::SentDirect {},
        LxmfDeliveryStatus::SentToPropagation {} => TransportDeliveryState::SentToPropagation {},
        LxmfDeliveryStatus::Acknowledged {} => TransportDeliveryState::TransportDelivered {},
        LxmfDeliveryStatus::Failed {} => TransportDeliveryState::Failed {},
        LxmfDeliveryStatus::TimedOut {} => TransportDeliveryState::TimedOut {},
    }
}

fn application_ack_state_for_lxmf_status(status: LxmfDeliveryStatus) -> ApplicationAckState {
    match status {
        LxmfDeliveryStatus::Acknowledged {} => ApplicationAckState::Accepted {},
        LxmfDeliveryStatus::Failed {} | LxmfDeliveryStatus::TimedOut {} => {
            ApplicationAckState::Failed {}
        }
        LxmfDeliveryStatus::Sent {} | LxmfDeliveryStatus::SentToPropagation {} => {
            ApplicationAckState::Waiting {}
        }
    }
}

fn application_ack_state_for_mission_metadata(
    metadata: &MissionSyncMetadata,
) -> ApplicationAckState {
    if metadata.result_present {
        return match metadata
            .result_status
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("completed" | "complete" | "done" | "success" | "succeeded" | "ok") => {
                ApplicationAckState::Completed {}
            }
            Some("rejected" | "reject" | "denied" | "declined") => ApplicationAckState::Rejected {},
            Some("failed" | "failure" | "error" | "timeout" | "timed_out" | "cancelled") => {
                ApplicationAckState::Failed {}
            }
            _ => ApplicationAckState::Accepted {},
        };
    }

    if metadata.event_present {
        ApplicationAckState::Accepted {}
    } else {
        ApplicationAckState::Waiting {}
    }
}

fn transport_state_for_message_state(state: MessageState) -> TransportDeliveryState {
    match state {
        MessageState::Queued {} | MessageState::PathRequested {} => {
            TransportDeliveryState::Queued {}
        }
        MessageState::LinkEstablishing {} | MessageState::Sending {} => {
            TransportDeliveryState::Sending {}
        }
        MessageState::SentDirect {} => TransportDeliveryState::SentDirect {},
        MessageState::SentToPropagation {} => TransportDeliveryState::SentToPropagation {},
        MessageState::Delivered {} | MessageState::Received {} => {
            TransportDeliveryState::TransportDelivered {}
        }
        MessageState::Failed {} => TransportDeliveryState::Failed {},
        MessageState::TimedOut {} => TransportDeliveryState::TimedOut {},
        MessageState::Cancelled {} => TransportDeliveryState::Cancelled {},
    }
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
            transport_state: transport_state_for_lxmf_status(status),
            application_ack_state: application_ack_state_for_lxmf_status(status),
            method: pending.method,
            representation: pending.representation,
            relay_destination_hex: pending.relay_destination_hex.clone(),
            fallback_stage: pending.fallback_stage,
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
    application_ack_state: ApplicationAckState,
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
            transport_state: transport_state_for_lxmf_status(status),
            application_ack_state,
            method: pending.method,
            representation: pending.representation,
            relay_destination_hex: pending.relay_destination_hex.clone(),
            fallback_stage: pending.fallback_stage,
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

fn app_data_from_hub_directory_capabilities(capabilities: &[String]) -> Option<String> {
    (!capabilities.is_empty()).then(|| capabilities.join(","))
}

fn merge_hub_directory_peer_records(
    peers: &mut Vec<PeerRecord>,
    snapshot: Option<&HubDirectorySnapshot>,
    local_app_destination_hex: &str,
) {
    let Some(snapshot) = snapshot else {
        return;
    };

    let local_app_destination_hex = normalize_hex_32(local_app_destination_hex);
    let mut existing_by_destination = peers
        .iter()
        .enumerate()
        .filter_map(|(index, peer)| {
            normalize_hex_32(peer.destination_hex.as_str()).map(|destination| (destination, index))
        })
        .collect::<HashMap<_, _>>();

    for item in &snapshot.items {
        let Some(destination_hex) = normalize_hex_32(item.destination_hash.as_str()) else {
            continue;
        };
        if local_app_destination_hex.as_deref() == Some(destination_hex.as_str()) {
            continue;
        }

        let item_identity_hex = normalize_hex_32(item.identity.as_str());
        let item_app_data = app_data_from_hub_directory_capabilities(&item.announce_capabilities);

        if let Some(index) = existing_by_destination
            .get(destination_hex.as_str())
            .copied()
        {
            let peer = &mut peers[index];
            peer.hub_derived = true;
            if peer.identity_hex.is_none() {
                peer.identity_hex = item_identity_hex.clone();
            }
            if peer.display_name.is_none() {
                peer.display_name = item.display_name.clone();
            }
            if peer.app_data.as_deref().is_none_or(str::is_empty) {
                peer.app_data = item_app_data.clone();
            }
            continue;
        }

        peers.push(PeerRecord {
            destination_hex: destination_hex.clone(),
            identity_hex: item_identity_hex,
            lxmf_destination_hex: None,
            display_name: item.display_name.clone(),
            app_data: item_app_data,
            state: PeerState::Disconnected {},
            saved: false,
            stale: false,
            active_link: false,
            hub_derived: true,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: None,
            last_seen_at_ms: snapshot.received_at_ms,
            announce_last_seen_at_ms: None,
            lxmf_last_seen_at_ms: None,
        });
        existing_by_destination.insert(destination_hex, peers.len().saturating_sub(1));
    }
}

async fn snapshot_peer_records(state: &NodeRuntimeState) -> Vec<PeerRecord> {
    let mut peers = state
        .messaging
        .lock()
        .await
        .list_peers()
        .into_iter()
        .map(from_sdk_peer_record)
        .collect::<Vec<_>>();
    let hub_directory_snapshot = state
        .hub_directory_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    merge_hub_directory_peer_records(
        &mut peers,
        hub_directory_snapshot.as_ref(),
        state.app_destination_hex.as_str(),
    );
    peers
}

async fn refresh_peer_snapshot(state: &NodeRuntimeState) -> bool {
    let peers = snapshot_peer_records(state).await;
    let changed = state
        .projection_journal
        .record_peers(peers.clone(), Some("peer-snapshot-refresh"));
    if let Ok(mut guard) = state.peers_snapshot.lock() {
        *guard = peers;
    }
    changed
}

fn refresh_sync_status_snapshot(state: &NodeRuntimeState, status: &SyncStatus) -> bool {
    let changed = state
        .projection_journal
        .record_sync_status(status.clone(), Some("sync-status-refresh"));
    if let Ok(mut guard) = state.sync_status_snapshot.lock() {
        *guard = status.clone();
    }
    changed
}

async fn emit_sync_status_update(
    state: &NodeRuntimeState,
    bus: &EventBus,
    phase: sdkmsg::SyncPhase,
    requested_at_ms: u64,
    messages_received: u32,
    detail: Option<String>,
    completed: bool,
) -> SyncStatus {
    let status_update =
        from_sdk_sync_status(state.messaging.lock().await.update_sync_status(|status| {
            status.phase = phase;
            status.requested_at_ms = Some(requested_at_ms);
            status.completed_at_ms = completed.then(now_ms);
            status.messages_received = messages_received;
            status.detail = detail;
        }));
    if refresh_sync_status_snapshot(state, &status_update) {
        bus.emit(NodeEvent::SyncUpdated {
            status: status_update.clone(),
        });
    }
    status_update
}

fn projection_journal_path(storage_dir: Option<&str>) -> Option<PathBuf> {
    storage_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|dir| PathBuf::from(dir).join(runtime_projection::PERSIST_FILENAME))
}

struct RestoredSavedPeerManagement {
    route_request_destinations: Vec<String>,
    link_targets: Vec<ManagedPeerLinkTarget>,
    pruned_destinations: Vec<String>,
}

fn record_saved_peer_profile(messaging: &mut sdkmsg::MessagingStore, peer: &SavedPeerRecord) {
    messaging.record_saved_peer_profile(
        peer.destination_hex.as_str(),
        peer.identity_hex.as_deref(),
        peer.lxmf_destination_hex.as_deref(),
        peer.app_data.as_deref(),
        peer.display_name.as_deref().or(peer.label.as_deref()),
        peer.last_route_seen_at_ms,
        peer.last_hops,
    );
}

fn saved_peer_matches_selected_destination(
    peer: &SavedPeerRecord,
    selected_hex: &str,
    selected_identity_hex: Option<&str>,
) -> bool {
    peer.destination_hex.eq_ignore_ascii_case(selected_hex)
        || peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(selected_hex))
        || selected_identity_hex.is_some_and(|identity_hex| {
            peer.identity_hex
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(identity_hex))
        })
}

async fn selected_saved_peer_record(
    state: &NodeRuntimeState,
    selected_destination_hex: &str,
) -> Result<SavedPeerRecord, NodeError> {
    let normalized_selected =
        normalize_hex_32(selected_destination_hex).ok_or(NodeError::InvalidConfig {})?;
    let current_peer = peer_for_any_destination_hex(state, normalized_selected.as_str()).await;
    let selected_identity_hex = current_peer
        .as_ref()
        .and_then(|peer| peer.identity_hex.as_deref());
    let destination_hex = current_peer
        .as_ref()
        .map(|peer| peer.destination_hex.clone())
        .unwrap_or_else(|| normalized_selected.clone());
    let mut existing = state.app_state.get_saved_peers()?.into_iter().find(|peer| {
        saved_peer_matches_selected_destination(
            peer,
            normalized_selected.as_str(),
            selected_identity_hex,
        ) || peer
            .destination_hex
            .eq_ignore_ascii_case(destination_hex.as_str())
    });
    let now = now_ms();
    let mut record = existing.take().unwrap_or(SavedPeerRecord {
        destination_hex: destination_hex.clone(),
        label: None,
        saved_at_ms: now,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    });
    record.destination_hex = destination_hex;
    record.saved_at_ms = now;
    if let Some(peer) = current_peer {
        record.identity_hex = peer.identity_hex.or(record.identity_hex);
        record.lxmf_destination_hex = peer
            .lxmf_destination_hex
            .or(record.lxmf_destination_hex)
            .or(Some(normalized_selected));
        record.app_data = peer.app_data.or(record.app_data);
        record.display_name = peer.display_name.or(record.display_name);
        record.last_route_seen_at_ms = peer
            .lxmf_last_seen_at_ms
            .or(peer.announce_last_seen_at_ms)
            .or(record.last_route_seen_at_ms);
    } else if record.lxmf_destination_hex.is_none() {
        record.lxmf_destination_hex = Some(normalized_selected);
    }
    Ok(record)
}

async fn persist_selected_peer_destination(
    state: &NodeRuntimeState,
    bus: &EventBus,
    selected_destination_hex: &str,
) -> Result<SavedPeerRecord, NodeError> {
    let peer = selected_saved_peer_record(state, selected_destination_hex).await?;
    let invalidation = state.app_state.upsert_saved_peer(&peer)?;
    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    {
        let mut messaging = state.messaging.lock().await;
        messaging.mark_peer_saved(peer.destination_hex.as_str(), true);
        record_saved_peer_profile(&mut messaging, &peer);
    }
    Ok(peer)
}

fn restore_saved_peer_management(
    messaging: &mut sdkmsg::MessagingStore,
    saved_peers: &[SavedPeerRecord],
) -> RestoredSavedPeerManagement {
    let mut restored_destinations = Vec::new();
    let mut seen_destinations = HashSet::new();
    for peer in saved_peers {
        let Some(destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if !seen_destinations.insert(destination_hex.clone()) {
            continue;
        }
        messaging.mark_peer_saved(destination_hex.as_str(), true);
        record_saved_peer_profile(messaging, peer);
        restored_destinations.push(destination_hex);
    }
    let pruned_destinations = messaging.prune_saved_destinations_with_non_rem_announce_evidence();
    if !pruned_destinations.is_empty() {
        let pruned_set = pruned_destinations.iter().collect::<HashSet<_>>();
        restored_destinations.retain(|destination| !pruned_set.contains(destination));
    }
    let mut link_targets = Vec::new();
    let mut seen_link_targets = HashSet::new();
    for destination_hex in &restored_destinations {
        if let Some(target) = messaging
            .peer_by_destination(destination_hex.as_str())
            .and_then(|peer| managed_peer_link_target(&peer))
            .filter(|target| seen_link_targets.insert(target.destination_hex.clone()))
        {
            link_targets.push(target);
        }
    }
    RestoredSavedPeerManagement {
        route_request_destinations: restored_destinations,
        link_targets,
        pruned_destinations,
    }
}

fn normalized_saved_peer_destinations(saved_peers: &[SavedPeerRecord]) -> Vec<String> {
    let mut destinations = saved_peers
        .iter()
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    destinations.sort();
    destinations.dedup();
    destinations
}

fn normalized_unique_destinations(destinations: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut normalized = destinations
        .into_iter()
        .filter_map(|destination| normalize_hex_32(destination.as_str()))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

async fn mark_peer_destinations_ignored(state: &NodeRuntimeState, destinations: &[String]) {
    let destinations = normalized_unique_destinations(destinations.iter().cloned());
    if destinations.is_empty() {
        return;
    }
    {
        let mut ignored = state.ignored_peer_destinations.lock().await;
        ignored.extend(destinations.iter().cloned());
    }
    if let Err(err) = state.app_state.add_ignored_peer_destinations(&destinations) {
        debug!(
            "[peers] failed to persist ignored destinations={} reason={}",
            destinations.join(","),
            err,
        );
    }
}

async fn clear_ignored_peer_destinations(state: &NodeRuntimeState, destinations: &[String]) {
    let destinations = normalized_unique_destinations(destinations.iter().cloned());
    if destinations.is_empty() {
        return;
    }
    {
        let mut ignored = state.ignored_peer_destinations.lock().await;
        for destination in &destinations {
            ignored.remove(destination);
        }
    }
    if let Err(err) = state
        .app_state
        .remove_ignored_peer_destinations(&destinations)
    {
        debug!(
            "[peers] failed to clear ignored destinations={} reason={}",
            destinations.join(","),
            err,
        );
    }
}

async fn peer_destinations_are_ignored(
    state: &NodeRuntimeState,
    destinations: impl IntoIterator<Item = String>,
) -> bool {
    let destinations = normalized_unique_destinations(destinations);
    if destinations.is_empty() {
        return false;
    }
    let ignored = state.ignored_peer_destinations.lock().await;
    destinations
        .iter()
        .any(|destination| ignored.contains(destination))
}

async fn apply_saved_peer_management_projection(
    state: &NodeRuntimeState,
    bus: &EventBus,
    saved_peers: &[SavedPeerRecord],
) -> Result<(), NodeError> {
    let desired_destinations = normalized_saved_peer_destinations(saved_peers);
    let desired_set = desired_destinations.iter().cloned().collect::<HashSet<_>>();
    clear_ignored_peer_destinations(state, desired_destinations.as_slice()).await;

    let (cleanup_destinations, changed_destinations, desired_targets) = {
        let mut messaging = state.messaging.lock().await;
        let previous_saved = messaging.saved_destination_hexes();
        let previous_saved_set = previous_saved.iter().cloned().collect::<HashSet<_>>();
        let removed_saved = previous_saved_set
            .difference(&desired_set)
            .cloned()
            .collect::<HashSet<_>>();
        let previous_peers = messaging.list_peers();
        let mut cleanup_destinations = removed_saved.clone();

        for peer in &previous_peers {
            let equivalents = equivalent_peer_destinations(peer)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let removed_match = equivalents
                .iter()
                .any(|destination| removed_saved.contains(destination));
            let still_desired_match = equivalents
                .iter()
                .any(|destination| desired_set.contains(destination));
            if removed_match && !still_desired_match {
                cleanup_destinations.extend(equivalents);
            }
        }

        let (added, removed) =
            messaging.replace_saved_destinations(desired_destinations.iter().map(String::as_str));
        for peer in saved_peers {
            record_saved_peer_profile(&mut messaging, peer);
        }
        let now = now_ms();
        for destination in &cleanup_destinations {
            if desired_set.contains(destination) {
                continue;
            }
            messaging.mark_peer_saved(destination, false);
            messaging.set_peer_active_link(destination, false, now);
        }

        let mut changed_destinations = cleanup_destinations.iter().cloned().collect::<Vec<_>>();
        changed_destinations.extend(added);
        changed_destinations.extend(removed);
        changed_destinations.sort();
        changed_destinations.dedup();

        let mut seen_targets = HashSet::<String>::new();
        let desired_targets = messaging
            .list_peers()
            .into_iter()
            .filter(|peer| peer.saved)
            .filter_map(|peer| managed_peer_link_target(&peer))
            .filter(|target| seen_targets.insert(target.destination_hex.clone()))
            .collect::<Vec<_>>();

        let mut cleanup_destinations = cleanup_destinations.into_iter().collect::<Vec<_>>();
        cleanup_destinations.sort();
        cleanup_destinations.dedup();

        (cleanup_destinations, changed_destinations, desired_targets)
    };

    cleanup_removed_saved_destinations(state, cleanup_destinations.as_slice()).await;
    mark_peer_destinations_ignored(state, cleanup_destinations.as_slice()).await;

    for target in desired_targets {
        add_desired_managed_peer_link_and_schedule(state, bus, target, "saved-peers-updated").await;
    }

    for destination in &changed_destinations {
        emit_peer_changed(state, bus, destination).await;
    }
    sync_auto_propagation_node(state, bus).await;
    Ok(())
}

async fn cleanup_removed_saved_destinations(state: &NodeRuntimeState, destinations: &[String]) {
    if destinations.is_empty() {
        return;
    }
    state
        .direct_delivery_health
        .clear(destinations.iter().map(String::as_str));
    state
        .managed_peer_links
        .remove_desired(destinations.iter().map(String::as_str))
        .await;
    let links_to_close = {
        let mut connected = state.connected_peers.lock().await;
        let mut out_links = state.out_links.lock().await;
        let mut links_to_close = Vec::new();
        for destination in destinations {
            let Ok(address_hash) = parse_address_hash(destination.as_str()) else {
                continue;
            };
            connected.remove(&address_hash);
            if let Some(link) = out_links.remove(&address_hash) {
                links_to_close.push(link);
            }
        }
        links_to_close
    };
    for link in links_to_close {
        link.lock().await.close();
    }
}

async fn seed_runtime_projection_snapshot(
    state: &NodeRuntimeState,
    snapshot: &runtime_projection::RuntimeProjectionSnapshot,
) {
    let sync_status = snapshot.sync_status();
    *state.active_propagation_node_hex.lock().await =
        sync_status.active_propagation_node_hex.clone();
    let mut messaging = state.messaging.lock().await;
    messaging.update_sync_status(|current| {
        if let Some(sdk_sync_status) = to_sdk_sync_status(sync_status.clone()) {
            *current = sdk_sync_status;
        }
    });
    // Saved peer management survives restart, but availability and "seen"
    // timestamps must be rebuilt from fresh announces after startup.
    for peer in snapshot.restored_peers() {
        messaging.mark_peer_saved(peer.destination_hex.as_str(), peer.saved);
        messaging.record_resolution_error(
            peer.destination_hex.as_str(),
            peer.last_resolution_error.clone(),
        );
    }
    for message in snapshot.messages() {
        messaging.upsert_message(to_sdk_message_record(message));
    }
}

fn sdk_peer_is_directly_reachable(peer: &sdkmsg::PeerRecord) -> bool {
    peer.active_link && matches!(peer.state, sdkmsg::PeerState::Connected)
}

fn sdk_peer_has_known_delivery_route(peer: &sdkmsg::PeerRecord) -> bool {
    peer.identity_hex
        .as_deref()
        .and_then(normalize_hex_32)
        .is_some()
        && sdk_peer_has_known_lxmf_route(peer)
}

fn mark_peer_link_state(
    messaging: &mut sdkmsg::MessagingStore,
    link_destination_hex: &str,
    canonical_destination_hex: &str,
    active: bool,
    changed_at_ms: u64,
) {
    messaging.set_peer_active_link(link_destination_hex, active, changed_at_ms);
    if link_destination_hex != canonical_destination_hex {
        messaging.set_peer_active_link(canonical_destination_hex, active, changed_at_ms);
    }
}

fn mark_peer_active_after_successful_link(
    messaging: &mut sdkmsg::MessagingStore,
    link_destination_hex: &str,
    canonical_destination_hex: &str,
    changed_at_ms: u64,
) {
    mark_peer_link_state(
        messaging,
        link_destination_hex,
        canonical_destination_hex,
        true,
        changed_at_ms,
    );
}

async fn record_peer_link_state(
    state: &NodeRuntimeState,
    bus: &EventBus,
    link_destination_hex: &str,
    active: bool,
) {
    let canonical_destination_hex =
        canonical_app_destination_hex(state, link_destination_hex).await;
    if active {
        clear_peer_direct_delivery_unhealthy(
            state,
            link_destination_hex,
            Some(canonical_destination_hex.as_str()),
        )
        .await;
    }
    let change = {
        let mut messaging = state.messaging.lock().await;
        if active {
            mark_peer_active_after_successful_link(
                &mut messaging,
                link_destination_hex,
                canonical_destination_hex.as_str(),
                now_ms(),
            );
        } else {
            mark_peer_link_state(
                &mut messaging,
                link_destination_hex,
                canonical_destination_hex.as_str(),
                false,
                now_ms(),
            );
        }
        messaging
            .peer_change_for_destination(canonical_destination_hex.as_str())
            .map(from_sdk_peer_change)
    };
    if let Some(change) = change.as_ref() {
        debug!(
            "[peers][link-state] link_destination={} canonical_destination={} active={} projected_destination={} state={:?} saved={} stale={} active_link={} identity={} lxmf={} announce_seen={} lxmf_seen={} last_error={}",
            link_destination_hex,
            canonical_destination_hex,
            active,
            change.destination_hex,
            change.state,
            change.saved,
            change.stale,
            change.active_link,
            change.identity_hex.as_deref().unwrap_or("-"),
            change.lxmf_destination_hex.as_deref().unwrap_or("-"),
            change
                .announce_last_seen_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            change
                .lxmf_last_seen_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            change.last_error.as_deref().unwrap_or("-"),
        );
    } else {
        debug!(
            "[peers][link-state] link_destination={} canonical_destination={} active={} projected_destination=- state=missing",
            link_destination_hex, canonical_destination_hex, active,
        );
    }
    if let Some(change) = change {
        state.sdk.record_peer_changed(
            &change.destination_hex,
            change.state,
            change.last_error.as_deref(),
        );
    }
    emit_peer_changed(state, bus, canonical_destination_hex.as_str()).await;
    sync_auto_propagation_node(state, bus).await;
}

fn sdk_peer_is_direct_delivery_ready(peer: &sdkmsg::PeerRecord, has_active_relay: bool) -> bool {
    let _ = has_active_relay;
    delivery_policy::peer_is_direct_delivery_ready(peer)
}

fn sdk_peer_has_known_lxmf_route(peer: &sdkmsg::PeerRecord) -> bool {
    delivery_policy::peer_has_known_lxmf_route(peer)
}

fn sdk_peer_has_observed_lxmf_delivery_route(peer: &sdkmsg::PeerRecord) -> bool {
    delivery_policy::peer_has_observed_lxmf_delivery_route(
        peer,
        now_ms(),
        sdkmsg::DEFAULT_PEER_STALE_AFTER_MS,
    )
}

async fn saved_peer_prefers_propagation(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    has_active_relay: bool,
    direct_priority_hops: Option<u8>,
) -> bool {
    if !has_active_relay {
        return false;
    }

    let normalized_destination = requested_destination_hex.to_ascii_lowercase();
    let canonical_destination =
        canonical_app_destination_hex(state, normalized_destination.as_str()).await;
    if !saved_peer_matches_destination(
        state,
        normalized_destination.as_str(),
        canonical_destination.as_str(),
    )
    .await
    {
        return false;
    }
    let Some(peer) = peer_for_any_destination_hex(state, canonical_destination.as_str()).await
    else {
        return true;
    };

    if saved_peer_stored_route_prefers_propagation(&peer, has_active_relay, direct_priority_hops) {
        return true;
    }
    !sdk_peer_is_direct_delivery_ready(&peer, has_active_relay)
        && !sdk_peer_has_known_lxmf_route(&peer)
}

fn saved_peer_stored_route_prefers_propagation(
    peer: &sdkmsg::PeerRecord,
    has_active_relay: bool,
    direct_priority_hops: Option<u8>,
) -> bool {
    delivery_policy::saved_route_prefers_propagation(
        peer,
        has_active_relay,
        sdk_peer_is_directly_reachable(peer),
        direct_priority_hops,
        MISSION_DIRECT_PRIORITY_FREE_HOPS,
    )
}

async fn saved_peer_can_try_stored_lxmf_route(
    state: &NodeRuntimeState,
    normalized_destination: &str,
    canonical_destination: &str,
) -> bool {
    if !saved_peer_matches_destination(state, normalized_destination, canonical_destination).await {
        return false;
    }
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_has_known_lxmf_route(&peer))
}

async fn saved_peer_has_direct_ready_route(
    state: &NodeRuntimeState,
    canonical_destination: &str,
    has_active_relay: bool,
) -> bool {
    if !peer_direct_delivery_available(state, canonical_destination).await {
        return false;
    }
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_is_direct_delivery_ready(&peer, has_active_relay))
}

async fn saved_peer_has_current_lxmf_route(
    state: &NodeRuntimeState,
    canonical_destination: &str,
) -> bool {
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_has_observed_lxmf_delivery_route(&peer))
}

async fn saved_peer_matches_destination(
    state: &NodeRuntimeState,
    normalized_destination: &str,
    canonical_destination: &str,
) -> bool {
    let saved_peers = match state.app_state.get_saved_peers() {
        Ok(saved_peers) => saved_peers,
        Err(_) => return false,
    };
    saved_peers.iter().any(|peer| {
        [
            normalize_hex_32(peer.destination_hex.as_str()),
            peer.lxmf_destination_hex
                .as_deref()
                .and_then(normalize_hex_32),
        ]
        .into_iter()
        .flatten()
        .any(|destination_hex| {
            destination_hex == canonical_destination || destination_hex == normalized_destination
        })
    })
}

fn mission_direct_priority_delay_for_hops(hops: Option<u8>) -> Duration {
    let Some(hops) = hops else {
        return Duration::ZERO;
    };
    if hops <= MISSION_DIRECT_PRIORITY_FREE_HOPS {
        return Duration::ZERO;
    }

    let delay_units = u32::from(hops - MISSION_DIRECT_PRIORITY_FREE_HOPS);
    (MISSION_DIRECT_PRIORITY_DELAY_PER_HOP * delay_units).min(MISSION_DIRECT_PRIORITY_MAX_DELAY)
}

#[cfg(not(test))]
fn add_normalized_destination_candidate(candidates: &mut HashSet<String>, destination_hex: &str) {
    if let Some(normalized) = normalize_hex_32(destination_hex) {
        candidates.insert(normalized);
    }
}

#[cfg(not(test))]
async fn mission_direct_priority_hops(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    canonical_destination_hex: &str,
) -> Option<u8> {
    let mut candidates = HashSet::<String>::new();
    add_normalized_destination_candidate(&mut candidates, requested_destination_hex);
    add_normalized_destination_candidate(&mut candidates, canonical_destination_hex);

    if let Some(peer) = peer_for_any_destination_hex(state, canonical_destination_hex).await {
        add_normalized_destination_candidate(&mut candidates, peer.destination_hex.as_str());
        if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.as_deref() {
            add_normalized_destination_candidate(&mut candidates, lxmf_destination_hex);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let announces = state.app_state.list_announces().ok()?;
    announces
        .iter()
        .filter_map(|announce| {
            let destination_hex = normalize_hex_32(announce.destination_hex.as_str())?;
            candidates
                .contains(destination_hex.as_str())
                .then_some(announce.hops)
        })
        .min()
}

fn direct_attempt_budget_for_send(
    send_mode: SendMode,
    has_active_relay: bool,
    can_try_stored_lxmf_route: bool,
    has_current_lxmf_route: bool,
    direct_delivery_ready: bool,
    direct_priority_hops: Option<u8>,
) -> usize {
    delivery_policy::direct_attempt_budget_for_send(
        send_mode,
        has_active_relay,
        can_try_stored_lxmf_route,
        has_current_lxmf_route,
        direct_delivery_ready,
        direct_priority_hops,
        MISSION_DIRECT_PRIORITY_FREE_HOPS,
        LXMF_DIRECT_ATTEMPTS,
    )
}

fn direct_attempt_send_mode(send_mode: SendMode) -> SendMode {
    match send_mode {
        SendMode::Auto {} | SendMode::DirectOnly {} => SendMode::DirectOnly {},
        SendMode::PropagationOnly {} => SendMode::PropagationOnly {},
    }
}

fn should_try_propagation_after_direct_failure(
    send_mode: SendMode,
    is_accepted_result: bool,
    has_active_relay: bool,
    saved_peer: bool,
    retriable: bool,
) -> bool {
    matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && has_active_relay
        && saved_peer
        && retriable
}

async fn equivalent_direct_delivery_destinations(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) -> Vec<String> {
    let mut destinations = Vec::<String>::new();
    for destination in [Some(requested_destination_hex), resolved_destination_hex]
        .into_iter()
        .flatten()
    {
        if let Some(normalized) = normalize_hex_32(destination) {
            destinations.push(normalized);
        }
    }

    if let Some(peer) = peer_for_any_destination_hex(state, requested_destination_hex).await {
        destinations.extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
    }
    if let Some(resolved_destination_hex) = resolved_destination_hex {
        if let Some(peer) = peer_for_any_destination_hex(state, resolved_destination_hex).await {
            destinations.extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
        }
    }

    destinations.sort();
    destinations.dedup();

    if destinations.is_empty() {
        return destinations;
    }

    destinations
}

async fn mark_peer_direct_delivery_unhealthy(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }
    let until_ms = now_ms().saturating_add(DIRECT_DELIVERY_FAILURE_COOLDOWN.as_millis() as u64);
    state
        .direct_delivery_health
        .mark_unhealthy(destinations.iter().map(String::as_str), until_ms);
    debug!(
        "[lxmf][mission] marked direct delivery cooldown destinations={} until_ms={}",
        destinations.join(","),
        until_ms,
    );
}

async fn clear_peer_direct_delivery_unhealthy(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }
    state
        .direct_delivery_health
        .clear(destinations.iter().map(String::as_str));
}

async fn close_output_links_for_direct_delivery_failure(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }

    let mut stale_links = Vec::new();
    {
        let mut links = state.out_links.lock().await;
        for destination in &destinations {
            let Ok(address_hash) = parse_address_hash(destination) else {
                continue;
            };
            if let Some(link) = links.remove(&address_hash) {
                stale_links.push((destination.clone(), link));
            }
        }
    }

    for (destination, link) in stale_links {
        link.lock().await.close();
        debug!(
            "[link][maintain] destination={} status=closed reason=direct-delivery-failed",
            destination,
        );
    }
}

async fn peer_direct_delivery_available(state: &NodeRuntimeState, destination_hex: &str) -> bool {
    let destinations = equivalent_direct_delivery_destinations(state, destination_hex, None).await;
    let now = now_ms();
    destinations
        .iter()
        .all(|destination| state.direct_delivery_health.is_available(destination, now))
}

async fn emit_peer_resolved_for_destination(
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: &str,
) {
    if !refresh_peer_snapshot(state).await {
        return;
    }
    if let Some(peer) = state
        .messaging
        .lock()
        .await
        .peer_by_destination(destination_hex)
        .map(from_sdk_peer_record)
    {
        bus.emit(NodeEvent::PeerResolved { peer });
    }
}

async fn emit_peer_changed(state: &NodeRuntimeState, bus: &EventBus, destination_hex: &str) {
    if !refresh_peer_snapshot(state).await {
        return;
    }
    if let Some(change) = state
        .messaging
        .lock()
        .await
        .peer_change_for_destination(destination_hex)
        .map(from_sdk_peer_change)
    {
        bus.emit(NodeEvent::PeerChanged { change });
    }
}

fn peer_matches_hex(peer: &sdkmsg::PeerRecord, normalized_hex: &str) -> bool {
    peer.destination_hex == normalized_hex
        || peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value == normalized_hex)
        || peer
            .identity_hex
            .as_deref()
            .is_some_and(|value| value == normalized_hex)
}

fn equivalent_peer_destinations(peer: &sdkmsg::PeerRecord) -> impl Iterator<Item = &str> {
    [
        Some(peer.destination_hex.as_str()),
        peer.lxmf_destination_hex.as_deref(),
        peer.identity_hex.as_deref(),
    ]
    .into_iter()
    .flatten()
}

fn peer_is_current_send_target(peer: &sdkmsg::PeerRecord) -> bool {
    !peer.stale && (peer.active_link || peer.announce_last_seen_at_ms.is_some())
}

fn delivery_route_unavailable_error() -> NodeError {
    NodeError::NetworkError {}
}

fn resolve_current_lxmf_destination_from_peers(
    peers: &[sdkmsg::PeerRecord],
    destination_hex: &str,
) -> Result<String, NodeError> {
    let normalized_destination =
        normalize_hex_32(destination_hex).ok_or(NodeError::InvalidConfig {})?;

    if let Some(peer) = peers.iter().find(|peer| {
        peer_matches_hex(peer, normalized_destination.as_str()) && peer_is_current_send_target(peer)
    }) {
        if peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value == normalized_destination)
        {
            return Ok(normalized_destination);
        }

        return Ok(peer
            .lxmf_destination_hex
            .clone()
            .unwrap_or_else(|| peer.destination_hex.clone()));
    }

    let stale_equivalent = peers
        .iter()
        .find(|peer| peer_matches_hex(peer, normalized_destination.as_str()));
    let Some(stale_equivalent) = stale_equivalent else {
        return Err(delivery_route_unavailable_error());
    };
    let identity_hex = stale_equivalent.identity_hex.as_deref();
    let lxmf_destination_hex = stale_equivalent
        .lxmf_destination_hex
        .as_deref()
        .or_else(|| {
            if normalized_destination == stale_equivalent.destination_hex {
                None
            } else {
                Some(stale_equivalent.destination_hex.as_str())
            }
        });

    peers
        .iter()
        .find(|peer| {
            peer_is_current_send_target(peer)
                && (lxmf_destination_hex.is_some_and(|destination| {
                    peer_matches_hex(peer, destination)
                        || peer.lxmf_destination_hex.as_deref() == Some(destination)
                }) || identity_hex
                    .is_some_and(|identity| peer.identity_hex.as_deref() == Some(identity)))
        })
        .map(|peer| {
            peer.lxmf_destination_hex
                .clone()
                .unwrap_or_else(|| peer.destination_hex.clone())
        })
        .ok_or_else(delivery_route_unavailable_error)
}

async fn peer_for_any_destination_hex(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<sdkmsg::PeerRecord> {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    let messaging = state.messaging.lock().await;
    messaging
        .peer_by_destination(normalized_destination.as_str())
        .or_else(|| {
            messaging
                .list_peers()
                .into_iter()
                .find(|peer| peer_matches_hex(peer, normalized_destination.as_str()))
        })
}

async fn resolve_current_lxmf_destination_hex(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Result<String, NodeError> {
    let messaging = state.messaging.lock().await;
    let peers = messaging.list_peers();
    match resolve_current_lxmf_destination_from_peers(peers.as_slice(), destination_hex) {
        Ok(destination) => Ok(destination),
        Err(err) => {
            let normalized_destination =
                normalize_hex_32(destination_hex).ok_or(NodeError::InvalidConfig {})?;
            let lxmf_candidates = peers
                .iter()
                .filter(|peer| peer_matches_hex(peer, normalized_destination.as_str()))
                .flat_map(equivalent_peer_destinations)
                .chain(std::iter::once(normalized_destination.as_str()));
            for candidate in lxmf_candidates {
                if let Some(destination) = messaging.current_lxmf_announce_destination(candidate) {
                    return Ok(destination);
                }
            }
            Err(err)
        }
    }
}

async fn resolve_lxmf_destination_hex(state: &NodeRuntimeState, destination_hex: &str) -> String {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    if let Ok(saved_peers) = state.app_state.get_saved_peers() {
        if let Some(peer) = saved_peers.iter().find(|peer| {
            peer.destination_hex
                .eq_ignore_ascii_case(normalized_destination.as_str())
                || peer.lxmf_destination_hex.as_deref().is_some_and(|value| {
                    value.eq_ignore_ascii_case(normalized_destination.as_str())
                })
                || peer.identity_hex.as_deref().is_some_and(|value| {
                    value.eq_ignore_ascii_case(normalized_destination.as_str())
                })
        }) {
            if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.as_deref() {
                return lxmf_destination_hex.to_ascii_lowercase();
            }
            return peer.destination_hex.to_ascii_lowercase();
        }
    }
    let Some(peer) = peer_for_any_destination_hex(state, &normalized_destination).await else {
        return normalized_destination;
    };
    if peer
        .lxmf_destination_hex
        .as_deref()
        .is_some_and(|value| value == normalized_destination)
    {
        return normalized_destination;
    }
    peer.lxmf_destination_hex.unwrap_or(peer.destination_hex)
}

async fn resolve_lxmf_destination_for_send(
    state: &NodeRuntimeState,
    destination_hex: &str,
    require_current_peer: bool,
) -> Result<String, NodeError> {
    if require_current_peer {
        resolve_current_lxmf_destination_hex(state, destination_hex).await
    } else {
        Ok(resolve_lxmf_destination_hex(state, destination_hex).await)
    }
}

async fn canonical_app_destination_hex(state: &NodeRuntimeState, destination_hex: &str) -> String {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    let Some(peer) = peer_for_any_destination_hex(state, &normalized_destination).await else {
        return normalized_destination;
    };
    let Some(identity_hex) = peer.identity_hex.clone() else {
        return peer.destination_hex;
    };
    state
        .messaging
        .lock()
        .await
        .app_destination_for_identity(identity_hex.as_str())
        .unwrap_or(peer.destination_hex)
}

async fn peer_destinations_equivalent(
    state: &NodeRuntimeState,
    left_hex: &str,
    right_hex: &str,
) -> bool {
    let normalized_left = left_hex.to_ascii_lowercase();
    let normalized_right = right_hex.to_ascii_lowercase();
    if normalized_left == normalized_right {
        return true;
    }

    let left_peer = peer_for_any_destination_hex(state, &normalized_left).await;
    let right_peer = peer_for_any_destination_hex(state, &normalized_right).await;
    let (Some(left_peer), Some(right_peer)) = (left_peer, right_peer) else {
        return false;
    };

    if left_peer.identity_hex.is_some() && left_peer.identity_hex == right_peer.identity_hex {
        return true;
    }

    let matches = equivalent_peer_destinations(&left_peer)
        .any(|candidate| equivalent_peer_destinations(&right_peer).any(|other| candidate == other));
    matches
}

async fn has_active_propagation_relay(state: &NodeRuntimeState) -> bool {
    state
        .active_propagation_node_hex
        .lock()
        .await
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn propagation_candidate_sort_key(
    announce: &sdkmsg::AnnounceRecord,
    preferred_destination_hex: Option<&str>,
    current_destination_hex: Option<&str>,
) -> (u8, u8, u8, u64, String) {
    let preferred_rank = if preferred_destination_hex.is_some_and(|preferred| {
        preferred == announce.destination_hex || preferred == announce.identity_hex
    }) {
        0
    } else {
        1
    };
    let current_rank = if preferred_destination_hex.is_none()
        && current_destination_hex.is_some_and(|current| current == announce.destination_hex)
    {
        0
    } else {
        1
    };
    (
        preferred_rank,
        announce.hops,
        current_rank,
        u64::MAX.saturating_sub(announce.received_at_ms),
        announce.destination_hex.clone(),
    )
}

fn propagation_sync_candidate_relays(
    announces: &[sdkmsg::AnnounceRecord],
    active_relay_hex: &str,
    preferred_destination_hex: Option<&str>,
) -> Vec<String> {
    let active_relay_hex = active_relay_hex.trim();
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    let active_matches_preferred =
        preferred_destination_hex.is_some_and(|preferred| preferred == active_relay_hex);
    if !active_relay_hex.is_empty()
        && (preferred_destination_hex.is_none() || active_matches_preferred)
        && seen.insert(active_relay_hex.to_string())
    {
        candidates.push(active_relay_hex.to_string());
    }

    let mut relay_announces = announces
        .iter()
        .filter(|record| record.destination_kind == "lxmf_propagation")
        .collect::<Vec<_>>();
    relay_announces.sort_by_key(|record| {
        propagation_candidate_sort_key(record, preferred_destination_hex, Some(active_relay_hex))
    });
    for record in relay_announces {
        if candidates.len() >= PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS {
            break;
        }
        if seen.insert(record.destination_hex.clone()) {
            candidates.push(record.destination_hex.clone());
        }
    }
    if candidates.is_empty()
        && !active_relay_hex.is_empty()
        && seen.insert(active_relay_hex.to_string())
    {
        candidates.push(active_relay_hex.to_string());
    }
    candidates
}

async fn run_propagation_sync_job(
    state: NodeRuntimeState,
    bus: EventBus,
    limit: Option<u32>,
    requested_at_ms: u64,
    relay_hex: String,
) {
    let mut announces = state.messaging.lock().await.list_announces();
    let mut relay_candidates = propagation_sync_candidate_relays(
        announces.as_slice(),
        relay_hex.as_str(),
        state.preferred_propagation_node_hex.as_deref(),
    );

    info!(
        "[sync] propagation sync started relay={} candidates={} limit={}",
        relay_hex,
        relay_candidates.len(),
        limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    let mut last_failure: Option<(String, NodeError)> = None;
    let mut attempted_relays = HashSet::new();
    while attempted_relays.len() < PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS {
        let Some(relay_candidate) = relay_candidates
            .iter()
            .find(|candidate| !attempted_relays.contains(candidate.as_str()))
            .cloned()
        else {
            break;
        };
        attempted_relays.insert(relay_candidate.clone());
        let attempt_number = attempted_relays.len();
        *state.active_propagation_node_hex.lock().await = Some(relay_candidate.clone());
        let active_status = from_sdk_sync_status(
            state
                .messaging
                .lock()
                .await
                .set_active_propagation_node(Some(relay_candidate.clone())),
        );
        if refresh_sync_status_snapshot(&state, &active_status) {
            bus.emit(NodeEvent::SyncUpdated {
                status: active_status,
            });
        }

        info!(
            "[sync] propagation sync relay attempt relay={} attempt={}/{}",
            relay_candidate, attempt_number, PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS
        );
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::PathRequested,
            requested_at_ms,
            0,
            Some(format!(
                "requesting path to propagation relay {relay_candidate} ({attempt_number}/{})",
                PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS
            )),
            false,
        )
        .await;
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::LinkEstablishing,
            requested_at_ms,
            0,
            Some(format!(
                "establishing propagation link {relay_candidate} ({attempt_number}/{})",
                PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS
            )),
            false,
        )
        .await;
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::RequestSent,
            requested_at_ms,
            0,
            Some(format!(
                "requesting propagated messages ({attempt_number}/{})",
                PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS
            )),
            false,
        )
        .await;

        let result = match state
            .sdk
            .fetch_propagated_lxmf_from_relay(relay_candidate.as_str(), limit, None)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                info!(
                    "[sync] propagation sync relay attempt failed relay={} attempt={}/{} reason={}",
                    relay_candidate, attempt_number, PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS, err
                );
                last_failure = Some((relay_candidate.clone(), err));
                sync_auto_propagation_node(&state, &bus).await;
                announces = state.messaging.lock().await.list_announces();
                let active_relay = state
                    .active_propagation_node_hex
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_else(|| relay_hex.clone());
                for refreshed_candidate in propagation_sync_candidate_relays(
                    announces.as_slice(),
                    active_relay.as_str(),
                    state.preferred_propagation_node_hex.as_deref(),
                ) {
                    if !attempted_relays.contains(refreshed_candidate.as_str())
                        && !relay_candidates.contains(&refreshed_candidate)
                    {
                        relay_candidates.push(refreshed_candidate);
                    }
                }
                continue;
            }
        };

        let destination_hex = result.destination_hex.clone();
        let available_count = result.available_count;
        let fetched_count = result.fetched_count;
        let fetched_entry_count = result.fetched_entry_count;
        let extracted_payload_count = result.extracted_payload_count;
        let failed_count = result.failed_count;
        let malformed_count = result.malformed_count;
        let decrypt_failed_count = result.decrypt_failed_count;
        let imported_count = result.imported_wires.len() as u32;
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::Receiving,
            requested_at_ms,
            0,
            Some(format!(
                "available={available_count} fetched_entries={fetched_entry_count} extracted_payloads={extracted_payload_count} decrypt_failed={decrypt_failed_count}"
            )),
            false,
        )
        .await;
        for wire in result.imported_wires {
            emit_received_payload(
                &state,
                &bus,
                &state.sdk,
                destination_hex.clone(),
                wire,
                None,
                true,
            )
            .await;
        }
        let detail = format!(
            "available={available_count} fetched={fetched_count} fetched_entries={fetched_entry_count} extracted_payloads={extracted_payload_count} imported={imported_count} malformed={malformed_count} decrypt_failed={decrypt_failed_count} failed={failed_count}"
        );
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::Complete,
            requested_at_ms,
            imported_count,
            Some(detail.clone()),
            true,
        )
        .await;
        info!(
            "[sync] propagation sync complete relay={} {}",
            relay_candidate, detail
        );
        state
            .propagation_sync_inflight
            .store(false, Ordering::Release);
        return;
    }

    let (failed_relay, err) =
        last_failure.unwrap_or_else(|| (relay_hex.clone(), NodeError::InvalidConfig {}));
    let detail = format!(
        "propagation sync failed: all relay attempts failed (last relay {failed_relay}: {err})"
    );
    emit_sync_status_update(
        &state,
        &bus,
        sdkmsg::SyncPhase::Failed,
        requested_at_ms,
        0,
        Some(detail.clone()),
        true,
    )
    .await;
    info!("[sync] propagation sync failed reason={detail}");
    state
        .propagation_sync_inflight
        .store(false, Ordering::Release);
}

async fn sync_auto_propagation_node(state: &NodeRuntimeState, bus: &EventBus) {
    let announces = {
        let messaging = state.messaging.lock().await;
        messaging.list_announces()
    };
    let current_destination = state.active_propagation_node_hex.lock().await.clone();
    let desired_destination = announces
        .iter()
        .filter(|record| record.destination_kind == "lxmf_propagation")
        .min_by_key(|record| {
            propagation_candidate_sort_key(
                record,
                state.preferred_propagation_node_hex.as_deref(),
                current_destination.as_deref(),
            )
        })
        .map(|record| record.destination_hex.clone());

    let mut active_guard = state.active_propagation_node_hex.lock().await;
    if *active_guard == desired_destination {
        return;
    }
    info!(
        "[sync] auto propagation relay {}",
        desired_destination
            .as_deref()
            .map(|value| format!("selected {value}"))
            .unwrap_or_else(|| "cleared".to_string())
    );
    *active_guard = desired_destination.clone();
    drop(active_guard);

    let status = from_sdk_sync_status(
        state
            .messaging
            .lock()
            .await
            .set_active_propagation_node(desired_destination),
    );
    if refresh_sync_status_snapshot(state, &status) {
        bus.emit(NodeEvent::SyncUpdated { status });
    }
}

async fn wait_for_active_propagation_relay(
    state: &NodeRuntimeState,
    bus: &EventBus,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + PROPAGATION_SYNC_RELAY_SELECTION_WAIT;
    loop {
        sync_auto_propagation_node(state, bus).await;
        if let Some(relay_hex) = state
            .active_propagation_node_hex
            .lock()
            .await
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            return Some(relay_hex);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(PROPAGATION_SYNC_RELAY_SELECTION_POLL).await;
    }
}

async fn resolve_peer_route(
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: &str,
) -> Result<(), NodeError> {
    let destination = parse_address_hash(destination_hex)?;
    let attempted_at_ms = now_ms();
    {
        let mut messaging = state.messaging.lock().await;
        messaging.record_resolution_attempt(destination_hex, attempted_at_ms);
    }
    emit_peer_changed(state, bus, destination_hex).await;

    state.transport.request_path(&destination, None, None).await;
    let desc = ensure_destination_desc(state, destination, None).await?;
    let identity_hex = desc.identity.address_hash.to_hex_string();
    let lxmf_desc = SingleOutputDestination::new(
        desc.identity,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    )
    .desc;
    let lxmf_destination_hex = lxmf_desc.address_hash.to_hex_string();
    {
        let mut messaging = state.messaging.lock().await;
        messaging.record_resolution_result(
            destination_hex,
            identity_hex.as_str(),
            lxmf_destination_hex.as_str(),
            now_ms(),
        );
    }
    let desired_link_target = {
        let messaging = state.messaging.lock().await;
        if messaging.is_peer_saved(destination_hex) {
            messaging
                .peer_by_destination(destination_hex)
                .and_then(|peer| managed_peer_link_target(&peer))
        } else {
            None
        }
    };
    emit_peer_changed(state, bus, destination_hex).await;
    emit_peer_resolved_for_destination(state, bus, destination_hex).await;
    if let Some(target) = desired_link_target {
        add_desired_managed_peer_link_and_schedule(state, bus, target, "saved-peer-resolution")
            .await;
    }
    sync_auto_propagation_node(state, bus).await;
    Ok(())
}

fn spawn_managed_peer_resolution(state: NodeRuntimeState, bus: EventBus, destination_hex: String) {
    tokio::spawn(async move {
        let Some(destination_hex) = normalize_hex_32(destination_hex.as_str()) else {
            return;
        };
        {
            let mut inflight = state.peer_resolution_inflight.lock().await;
            if !inflight.insert(destination_hex.clone()) {
                return;
            }
        }

        let retry_delays_secs = [0_u64, 3, 8, 15, 30];
        for delay_secs in retry_delays_secs {
            if delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }

            let (should_retry, cached_target) = {
                let messaging = state.messaging.lock().await;
                if !messaging.is_peer_saved(destination_hex.as_str()) {
                    (false, None)
                } else {
                    let peer = messaging.peer_by_destination(destination_hex.as_str());
                    let should_retry = peer
                        .as_ref()
                        .is_none_or(|peer| !sdk_peer_has_known_delivery_route(peer));
                    let cached_target = if should_retry {
                        None
                    } else {
                        peer.as_ref().and_then(managed_peer_link_target)
                    };
                    (should_retry, cached_target)
                }
            };

            if !should_retry {
                if let Some(target) = cached_target {
                    add_desired_managed_peer_link_and_schedule(
                        &state,
                        &bus,
                        target,
                        "saved-peer-resolution-cached",
                    )
                    .await;
                }
                state
                    .peer_resolution_inflight
                    .lock()
                    .await
                    .remove(destination_hex.as_str());
                return;
            }

            if let Err(err) = resolve_peer_route(&state, &bus, destination_hex.as_str()).await {
                state
                    .messaging
                    .lock()
                    .await
                    .record_resolution_error(destination_hex.as_str(), Some(err.to_string()));
                emit_peer_changed(&state, &bus, destination_hex.as_str()).await;
            } else {
                state
                    .peer_resolution_inflight
                    .lock()
                    .await
                    .remove(destination_hex.as_str());
                return;
            }
        }
        state
            .peer_resolution_inflight
            .lock()
            .await
            .remove(destination_hex.as_str());
    });
}

fn saved_peer_destinations_needing_route_refresh(
    messaging: &sdkmsg::MessagingStore,
) -> Vec<String> {
    let mut destinations = messaging
        .list_peers()
        .into_iter()
        .filter(|peer| peer.saved && !sdk_peer_has_known_delivery_route(peer))
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    destinations.sort();
    destinations.dedup();
    destinations
}

fn spawn_passive_peer_resolution(state: NodeRuntimeState, bus: EventBus, destination_hex: String) {
    tokio::spawn(async move {
        let should_resolve = {
            let messaging = state.messaging.lock().await;
            match messaging.peer_by_destination(destination_hex.as_str()) {
                Some(peer) => {
                    (peer.identity_hex.is_none() || peer.lxmf_destination_hex.is_none())
                        && peer
                            .last_resolution_attempt_at_ms
                            .is_none_or(|attempted_at_ms| {
                                now_ms().saturating_sub(attempted_at_ms)
                                    >= PASSIVE_PEER_RESOLUTION_MIN_INTERVAL_MS
                            })
                }
                None => false,
            }
        };
        if !should_resolve {
            return;
        }

        {
            let mut inflight = state.peer_resolution_inflight.lock().await;
            if !inflight.insert(destination_hex.clone()) {
                return;
            }
        }

        let _ = resolve_peer_route(&state, &bus, destination_hex.as_str()).await;
        state
            .peer_resolution_inflight
            .lock()
            .await
            .remove(destination_hex.as_str());
    });
}

async fn upsert_message_record(
    state: &NodeRuntimeState,
    bus: &EventBus,
    message: MessageRecord,
    emit_received: bool,
) {
    let message = canonicalize_chat_message(&message);
    if let Ok(invalidations) = state.app_state.upsert_message(&message) {
        for invalidation in invalidations {
            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
        }
    }
    let changed = state
        .projection_journal
        .record_message(message.clone(), Some("message-upsert"));
    state
        .messaging
        .lock()
        .await
        .upsert_message(to_sdk_message_record(message.clone()));

    if changed {
        if emit_received {
            bus.emit(NodeEvent::MessageReceived {
                message: message.clone(),
            });
        }
        bus.emit(NodeEvent::MessageUpdated { message });
    }
}

async fn delete_conversation_records(
    state: &NodeRuntimeState,
    bus: &EventBus,
    conversation_id: &str,
) -> Result<(), NodeError> {
    let peers = state
        .peers_snapshot
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    let resolver = conversation_peer_resolver(&peers);
    for invalidation in state
        .app_state
        .delete_conversation_resolved(conversation_id, &resolver)?
    {
        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    }

    let delete_keys = conversation_delete_keys(conversation_id, &peers);
    let projection_changed = state.projection_journal.remove_conversation_messages(
        delete_keys.iter().map(String::as_str),
        Some("conversation-deleted"),
    );
    state
        .messaging
        .lock()
        .await
        .delete_conversation_messages(delete_keys.iter().map(String::as_str));
    if projection_changed {
        state.projection_journal.flush_now().await;
    }
    Ok(())
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
    state
        .messaging
        .lock()
        .await
        .list_conversations()
        .into_iter()
        .map(from_sdk_conversation_record)
        .collect()
}

pub enum Command {
    Stop {
        resp: cb::Sender<Result<(), NodeError>>,
    },
    AnnounceNow {},
    ConnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    DisconnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetSavedPeers {
        peers: Vec<SavedPeerRecord>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendBytes {
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
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
    DeleteConversation {
        conversation_id: String,
        resp: cb::Sender<Result<(), NodeError>>,
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
    app_state: AppStateStore,
    identity: PrivateIdentity,
    app_destination_hex: String,
    transport: Arc<Transport>,
    lxmf_destination: Arc<TokioMutex<SingleInputDestination>>,
    peer_resolution_inflight: Arc<TokioMutex<HashSet<String>>>,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<Link>>>>>,
    active_interface_registry: ActiveInterfaceRegistry,
    connected_peers: Arc<TokioMutex<HashSet<AddressHash>>>,
    pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>>,
    pending_lxmf_acknowledgements: Arc<TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>>,
    messaging: Arc<TokioMutex<sdkmsg::MessagingStore>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    hub_directory_snapshot: Arc<Mutex<Option<HubDirectorySnapshot>>>,
    projection_journal: Arc<RuntimeProjectionJournal>,
    sdk: Arc<RuntimeLxmfSdk>,
    active_propagation_node_hex: Arc<TokioMutex<Option<String>>>,
    preferred_propagation_node_hex: Option<String>,
    propagation_sync_inflight: Arc<AtomicBool>,
    direct_delivery_health: DirectDeliveryHealth,
    managed_peer_links: ManagedPeerLinks,
    ignored_peer_destinations: Arc<TokioMutex<HashSet<String>>>,
    send_task_permits: SendTaskPermits,
    mission_destination_locks: MissionDestinationLocks,
}

fn prune_expired_buffered_acknowledgements(
    pending_lxmf_acknowledgements: &mut HashMap<String, PendingLxmfAcknowledgement>,
    now_ms: u64,
) -> usize {
    let before = pending_lxmf_acknowledgements.len();
    pending_lxmf_acknowledgements.retain(|_, pending| {
        now_ms.saturating_sub(pending.buffered_at_ms) < DEFAULT_BUFFERED_ACK_TTL.as_millis() as u64
    });
    before.saturating_sub(pending_lxmf_acknowledgements.len())
}

fn prune_expired_receipt_tracking(
    receipt_message_ids: &mut HashMap<String, ReceiptMessageTracking>,
    now_ms: u64,
) -> usize {
    let before = receipt_message_ids.len();
    receipt_message_ids.retain(|_, tracking| {
        now_ms.saturating_sub(tracking.recorded_at_ms)
            < DEFAULT_RECEIPT_TRACKING_TTL.as_millis() as u64
    });
    before.saturating_sub(receipt_message_ids.len())
}

async fn acquire_send_task_permit(
    permits: &SendTaskPermits,
    class: SendTaskClass,
) -> Result<OwnedSemaphorePermit, NodeError> {
    permits.acquire(class).await
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

async fn ensure_output_link(
    state: &NodeRuntimeState,
    desc: DestinationDesc,
) -> Result<Arc<TokioMutex<Link>>, NodeError> {
    const DEFAULT_MAX_ATTEMPTS: usize = 3;
    const RNODE_BLE_MAX_ATTEMPTS: usize = 1;
    const RETRY_DELAY: Duration = Duration::from_millis(500);
    let rnode_route = destination_uses_rnode_ble_route(state, &desc.address_hash).await;
    let max_attempts = if rnode_route {
        RNODE_BLE_MAX_ATTEMPTS
    } else {
        DEFAULT_MAX_ATTEMPTS
    };
    let connect_timeout = link_connect_timeout(rnode_route);

    for attempt in 0..max_attempts {
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
                if attempt + 1 == max_attempts {
                    return Err(err);
                }
                info!(
                    "[lxmf][events] link activation retry destination={} attempt={} timeout_ms={} reason={}",
                    address_hash_to_hex(&desc.address_hash),
                    attempt + 1,
                    connect_timeout.as_millis(),
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

fn managed_peer_link_target(peer: &sdkmsg::PeerRecord) -> Option<ManagedPeerLinkTarget> {
    let normalized_destination_hex = normalize_hex_32(peer.destination_hex.as_str());
    let has_rem_capabilities = peer
        .app_data
        .as_deref()
        .is_some_and(app_data_has_rem_peer_capabilities);
    let has_saved_lxmf_route_target = peer.saved
        && (peer
            .lxmf_destination_hex
            .as_deref()
            .and_then(normalize_hex_32)
            .is_some()
            || (normalized_destination_hex.is_some()
                && has_rem_capabilities
                && peer.lxmf_last_seen_at_ms.is_some()));
    if peer.stale && !has_saved_lxmf_route_target {
        return None;
    }
    if !peer.saved && !has_rem_capabilities {
        return None;
    }
    if let Some(destination_hex) = peer
        .lxmf_destination_hex
        .as_deref()
        .and_then(normalize_hex_32)
    {
        return Some(ManagedPeerLinkTarget {
            destination_hex,
            kind: ManagedPeerLinkKind::LxmfDelivery,
        });
    }
    normalized_destination_hex.map(|destination_hex| {
        let kind = if peer.saved && has_rem_capabilities && peer.lxmf_last_seen_at_ms.is_some() {
            ManagedPeerLinkKind::LxmfDelivery
        } else {
            ManagedPeerLinkKind::App
        };
        ManagedPeerLinkTarget {
            destination_hex,
            kind,
        }
    })
}

#[cfg(test)]
fn saved_peer_link_targets(peers: &[sdkmsg::PeerRecord]) -> Vec<ManagedPeerLinkTarget> {
    let mut seen = HashSet::<String>::new();
    let mut targets = Vec::<ManagedPeerLinkTarget>::new();
    for peer in peers {
        let Some(target) = managed_peer_link_target(peer) else {
            continue;
        };
        if seen.insert(target.destination_hex.clone()) {
            targets.push(target);
        }
    }
    targets
}

async fn desired_managed_peer_link_target_for_destination(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<ManagedPeerLinkTarget> {
    peer_for_any_destination_hex(state, destination_hex)
        .await
        .and_then(|peer| managed_peer_link_target(&peer))
}

async fn register_desired_managed_peer_link(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<ManagedPeerLinkTarget> {
    if !has_active_reticulum_interface(state).await {
        return None;
    }
    let target = desired_managed_peer_link_target_for_destination(state, destination_hex).await?;
    state.managed_peer_links.add_desired(target.clone()).await;
    Some(target)
}

async fn add_desired_managed_peer_link_and_schedule(
    state: &NodeRuntimeState,
    bus: &EventBus,
    target: ManagedPeerLinkTarget,
    reason: &str,
) {
    state.managed_peer_links.add_desired(target.clone()).await;
    if !has_active_reticulum_interface(state).await {
        info!(
            "[link][maintain] destination={} status=deferred reason={} detail=no-active-reticulum-interface",
            target.destination_hex, reason,
        );
        return;
    }
    if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
        if output_link_is_active(state, &destination).await {
            clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                .await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
            info!(
                "[link][maintain] destination={} status=active reason={}",
                target.destination_hex, reason,
            );
            return;
        }
    }
    state
        .managed_peer_links
        .clear_failure(target.destination_hex.as_str())
        .await;
    match state
        .managed_peer_links
        .begin_reconnect(target.destination_hex.as_str())
        .await
    {
        ManagedPeerReconnectStart::Started(target) => {
            info!(
                "[link][maintain] destination={} status=scheduled reason={}",
                target.destination_hex, reason,
            );
            spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
        }
        ManagedPeerReconnectStart::AlreadyReconnecting => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=reconnecting",
                target.destination_hex, reason,
            );
        }
        ManagedPeerReconnectStart::Backoff {
            next_retry_at_ms,
            last_failure_reason,
        } => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=backoff next_retry_at_ms={} last_failure={}",
                target.destination_hex,
                reason,
                next_retry_at_ms,
                last_failure_reason.as_deref().unwrap_or("-"),
            );
        }
        ManagedPeerReconnectStart::NotDesired => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=not-desired",
                target.destination_hex, reason,
            );
        }
    }
}

async fn output_link_is_active(state: &NodeRuntimeState, destination: &AddressHash) -> bool {
    let link = state.out_links.lock().await.get(destination).cloned();
    let Some(link) = link else {
        return false;
    };
    let active = link.lock().await.status() == LinkStatus::Active;
    active
}

async fn ensure_managed_peer_link(
    state: &NodeRuntimeState,
    bus: &EventBus,
    target: ManagedPeerLinkTarget,
) -> Result<(), NodeError> {
    if !has_active_reticulum_interface(state).await {
        return Err(NodeError::NetworkError {});
    }
    let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) else {
        return Err(NodeError::InvalidConfig {});
    };
    if output_link_is_active(state, &destination).await {
        clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None).await;
        record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
        return Ok(());
    }
    info!(
        "[link][maintain] destination={} status=connecting kind={:?}",
        target.destination_hex, target.kind,
    );
    let desc =
        match ensure_destination_desc(state, destination, Some(target.kind.destination_name()))
            .await
        {
            Ok(desc) => desc,
            Err(err) => {
                mark_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(state, bus, target.destination_hex.as_str(), false).await;
                info!(
                    "[link][maintain] destination={} status=resolve-failed kind={:?} reason={}",
                    target.destination_hex, target.kind, err,
                );
                return Err(err);
            }
        };
    match ensure_output_link(state, desc).await {
        Ok(_) => {
            clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                .await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
            info!(
                "[link][maintain] destination={} status=active kind={:?}",
                target.destination_hex, target.kind,
            );
            Ok(())
        }
        Err(err) => {
            mark_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None).await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), false).await;
            info!(
                "[link][maintain] destination={} status=failed kind={:?} reason={}",
                target.destination_hex, target.kind, err,
            );
            Err(err)
        }
    }
}

async fn maintain_managed_peer_links_once(state: &NodeRuntimeState, bus: &EventBus) {
    if !has_active_reticulum_interface(state).await {
        return;
    }
    let targets = state.managed_peer_links.desired_targets().await;
    for target in targets {
        if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
            if output_link_is_active(state, &destination).await {
                clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
                continue;
            }
        }
        let still_saved_and_current =
            peer_for_any_destination_hex(state, target.destination_hex.as_str())
                .await
                .is_some_and(|peer| managed_peer_link_target(&peer).is_some());
        if still_saved_and_current {
            match state
                .managed_peer_links
                .begin_reconnect(target.destination_hex.as_str())
                .await
            {
                ManagedPeerReconnectStart::Started(target) => {
                    info!(
                        "[link][maintain] destination={} status=scheduled reason=periodic-maintenance",
                        target.destination_hex,
                    );
                    spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
                }
                ManagedPeerReconnectStart::AlreadyReconnecting
                | ManagedPeerReconnectStart::Backoff { .. }
                | ManagedPeerReconnectStart::NotDesired => {}
            }
        } else {
            state
                .managed_peer_links
                .remove_desired([target.destination_hex.as_str()])
                .await;
        }
    }
}

fn spawn_managed_peer_link_reconnect(
    state: NodeRuntimeState,
    bus: EventBus,
    target: ManagedPeerLinkTarget,
) {
    tokio::spawn(async move {
        tokio::time::sleep(SAVED_PEER_LINK_RECONNECT_DELAY).await;
        let result = match tokio::time::timeout(
            MANAGED_PEER_LINK_RECONNECT_TIMEOUT,
            ensure_managed_peer_link(&state, &bus, target.clone()),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                mark_peer_direct_delivery_unhealthy(&state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(&state, &bus, target.destination_hex.as_str(), false).await;
                if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
                    if let Some(stale) = state.out_links.lock().await.remove(&destination) {
                        stale.lock().await.close();
                    }
                }
                info!(
                    "[link][maintain] destination={} status=failed kind={:?} reason=reconnect-timeout timeout_ms={}",
                    target.destination_hex,
                    target.kind,
                    MANAGED_PEER_LINK_RECONNECT_TIMEOUT.as_millis(),
                );
                Err(NodeError::Timeout {})
            }
        };
        state
            .managed_peer_links
            .finish_reconnect(
                &target,
                result.as_ref().map(|_| ()).map_err(ToString::to_string),
            )
            .await;
        if let Err(err) = result {
            info!(
                "[link][maintain] destination={} status=reconnect-backoff reason={}",
                target.destination_hex, err,
            );
        }
    });
}

async fn register_pending_lxmf_delivery(
    state: &NodeRuntimeState,
    report: &LxmfSendReport,
    resend: Option<PendingLxmfResend>,
    message_id_override: Option<String>,
) -> Option<RegisteredPendingLxmfDelivery> {
    if !report.track_delivery_timeout {
        return None;
    }
    let metadata = report.metadata.as_ref()?;
    let tracking_key = metadata.tracking_key()?.to_string();
    let pending = PendingLxmfDelivery {
        message_id_hex: message_id_override.unwrap_or_else(|| report.message_id_hex.clone()),
        destination_hex: report.resolved_destination_hex.clone(),
        correlation_id: metadata.correlation_id.clone(),
        command_id: metadata.command_id.clone(),
        command_type: metadata.command_type.clone(),
        event_uid: metadata.event_uid.clone(),
        mission_uid: metadata.mission_uid.clone(),
        method: report.method,
        representation: report.representation,
        relay_destination_hex: report.relay_destination_hex.clone(),
        fallback_stage: report.fallback_stage,
        resend,
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

async fn has_active_reticulum_interface(state: &NodeRuntimeState) -> bool {
    !state.active_interface_registry.lock().await.is_empty()
}

fn active_interfaces_include_relay_transport(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
) -> bool {
    active_interfaces
        .values()
        .any(|interface| !interface_label_is_rnode_ble(&interface.label))
}

fn active_interfaces_are_rnode_ble_only(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
) -> bool {
    !active_interfaces.is_empty()
        && active_interfaces
            .values()
            .all(|interface| interface_label_is_rnode_ble(&interface.label))
}

fn interface_label_is_rnode_ble(interface: &str) -> bool {
    interface.starts_with("rnode-ble:")
}

fn active_interface_is_rnode_ble(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
    interface: &AddressHash,
) -> bool {
    active_interfaces
        .get(interface)
        .is_some_and(|status| interface_label_is_rnode_ble(&status.label))
}

fn link_connect_timeout(rnode_route: bool) -> Duration {
    if rnode_route {
        RNODE_BLE_LINK_CONNECT_TIMEOUT
    } else {
        DEFAULT_LINK_CONNECT_TIMEOUT
    }
}

async fn destination_uses_rnode_ble_route(
    state: &NodeRuntimeState,
    destination: &AddressHash,
) -> bool {
    let destination_hex = address_hash_to_hex(destination);
    let active_interfaces = state.active_interface_registry.lock().await.clone();
    if active_interfaces_are_rnode_ble_only(&active_interfaces) {
        return true;
    }

    state
        .app_state
        .list_announces()
        .ok()
        .and_then(|announces| {
            announces.into_iter().find(|announce| {
                normalize_hex_32(announce.destination_hex.as_str()).as_deref()
                    == Some(destination_hex.as_str())
            })
        })
        .and_then(|announce| parse_address_hash(announce.interface_hex.as_str()).ok())
        .is_some_and(|interface| active_interface_is_rnode_ble(&active_interfaces, &interface))
}

async fn has_active_relay_transport_interface(state: &NodeRuntimeState) -> bool {
    let active_interfaces = state.active_interface_registry.lock().await;
    active_interfaces_include_relay_transport(&active_interfaces)
}

#[expect(
    clippy::too_many_arguments,
    reason = "resend construction mirrors the persisted pending delivery fields"
)]
fn build_pending_lxmf_resend(
    report: &LxmfSendReport,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    send_mode: SendMode,
    send_task_class: SendTaskClass,
) -> Option<PendingLxmfResend> {
    if !report.track_delivery_timeout
        || !matches!(send_mode, SendMode::Auto {})
        || (report.used_propagation_node
            && !matches!(
                report.fallback_stage,
                Some(LxmfFallbackStage::AfterDirectRetryBudget {})
            ))
    {
        return None;
    }
    let metadata = metadata?;
    if !metadata.command_present || metadata.tracking_key().is_none() {
        return None;
    }
    Some(PendingLxmfResend {
        requested_destination_hex: requested_destination_hex.to_string(),
        body: body.to_vec(),
        title,
        fields_bytes,
        metadata,
        send_task_class,
        original_send_mode: send_mode,
        direct_ack_retry_attempted: matches!(
            report.fallback_stage,
            Some(LxmfFallbackStage::AfterDirectRetryBudget {})
        ),
        propagation_fallback_attempted: matches!(
            report.fallback_stage,
            Some(LxmfFallbackStage::AfterDirectRetryBudget {})
        ),
    })
}

fn pending_tracking_key(pending: &PendingLxmfDelivery) -> Option<String> {
    pending
        .command_id
        .as_deref()
        .or(pending.correlation_id.as_deref())
        .map(ToOwned::to_owned)
}

fn chat_delivery_ack_body(message_id_hex: &str) -> String {
    format!("{CHAT_DELIVERY_ACK_PREFIX}{message_id_hex}")
}

fn parse_chat_delivery_ack_body(body: &str) -> Option<String> {
    let message_id_hex = body.trim().strip_prefix(CHAT_DELIVERY_ACK_PREFIX)?.trim();
    let valid_message_id =
        message_id_hex.len() == 64 && message_id_hex.chars().all(|ch| ch.is_ascii_hexdigit());
    valid_message_id.then(|| message_id_hex.to_ascii_lowercase())
}

fn should_retry_pending_ack_timeout_via_direct(pending: &PendingLxmfDelivery) -> bool {
    pending.resend.as_ref().is_some_and(|resend| {
        matches!(resend.original_send_mode, SendMode::Auto {})
            && !resend.direct_ack_retry_attempted
            && !resend.propagation_fallback_attempted
            && !matches!(pending.method, LxmfDeliveryMethod::Propagated {})
            && pending.relay_destination_hex.is_none()
    })
}

fn should_retry_pending_ack_timeout_via_propagation(
    pending: &PendingLxmfDelivery,
    has_active_relay: bool,
) -> bool {
    has_active_relay
        && pending.resend.as_ref().is_some_and(|resend| {
            matches!(resend.original_send_mode, SendMode::Auto {})
                && !resend.propagation_fallback_attempted
        })
}

fn pending_ack_timeout_elapsed(pending: &PendingLxmfDelivery, now: u64) -> bool {
    let timeout = if matches!(pending.method, LxmfDeliveryMethod::Propagated {})
        || pending.relay_destination_hex.is_some()
    {
        PROPAGATED_LXMF_ACK_TIMEOUT
    } else {
        DEFAULT_LXMF_ACK_TIMEOUT
    };
    now.saturating_sub(pending.sent_at_ms) >= timeout.as_millis() as u64
}

fn record_pending_delivery_timed_out(
    sdk: &RuntimeLxmfSdk,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    detail: &str,
) {
    sdk.record_delivery_timed_out(
        &pending.message_id_hex,
        &pending.destination_hex,
        pending.correlation_id.as_deref(),
        pending.command_id.as_deref(),
        pending.command_type.as_deref(),
        pending.event_uid.as_deref(),
        pending.mission_uid.as_deref(),
        Some(detail),
    );
    emit_lxmf_delivery(
        bus,
        pending,
        LxmfDeliveryStatus::TimedOut {},
        Some(detail.to_string()),
    );
    bus.emit(NodeEvent::Error {
        code: "NetworkError".to_string(),
        message: format!(
            "lxmf delivery acknowledgement timeout destination={} command={} correlation={} detail={detail}",
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
        ),
    });
    info!(
        "[lxmf][mission] timed out message_id={} destination={} command={} correlation={} detail={}",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
        detail,
    );
}

async fn acknowledge_pending_with_buffered_ack(
    state: &NodeRuntimeState,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    buffered_ack: PendingLxmfAcknowledgement,
) -> bool {
    let tracking_key = pending_tracking_key(pending);
    if peer_destinations_equivalent(
        state,
        pending.destination_hex.as_str(),
        buffered_ack.source_hex.as_str(),
    )
    .await
    {
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
            bus,
            pending,
            Some(buffered_ack.source_hex.clone()),
            LxmfDeliveryStatus::Acknowledged {},
            buffered_ack.application_ack_state,
            buffered_ack.detail.clone(),
        );
        info!(
            "[lxmf][mission] acknowledged buffered message_id={} destination={} command={} correlation={} detail={}",
            pending.message_id_hex,
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
            buffered_ack.detail.as_deref().unwrap_or("-"),
        );
        true
    } else {
        if let Some(tracking_key) = tracking_key {
            state
                .pending_lxmf_acknowledgements
                .lock()
                .await
                .insert(tracking_key, buffered_ack.clone());
        }
        info!(
            "[lxmf][mission] buffered acknowledgement source mismatch message_id={} destination={} source={}",
            pending.message_id_hex,
            pending.destination_hex,
            buffered_ack.source_hex,
        );
        false
    }
}

async fn retry_pending_ack_timeout_via_propagation(
    state: &NodeRuntimeState,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
) -> Result<bool, String> {
    let has_active_relay = has_active_propagation_relay(state).await;
    let Some(mut resend) = pending.resend.clone() else {
        return Ok(false);
    };
    if should_retry_pending_ack_timeout_via_direct(pending) {
        resend.direct_ack_retry_attempted = true;
        info!(
            "[lxmf][mission] ack timeout message_id={} destination={} command={} correlation={}; retrying direct delivery",
            pending.message_id_hex,
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
        );
        match send_lxmf_with_delivery_policy(
            state,
            bus,
            resend.requested_destination_hex.as_str(),
            resend.body.as_slice(),
            resend.title.clone(),
            resend.fields_bytes.clone(),
            Some(resend.metadata.clone()),
            SendMode::DirectOnly {},
            resend.send_task_class.direct_recovery_equivalent(),
        )
        .await
        {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                let Some(registered) = register_pending_lxmf_delivery(
                    state,
                    &report,
                    Some(resend),
                    Some(pending.message_id_hex.clone()),
                )
                .await
                else {
                    return Err("direct retry did not register pending delivery".to_string());
                };
                let retry_pending = &registered.pending;
                state.sdk.record_delivery_sent(
                    &retry_pending.message_id_hex,
                    &retry_pending.destination_hex,
                    retry_pending.correlation_id.as_deref(),
                    retry_pending.command_id.as_deref(),
                    retry_pending.command_type.as_deref(),
                    retry_pending.event_uid.as_deref(),
                    retry_pending.mission_uid.as_deref(),
                );
                emit_lxmf_delivery(
                    bus,
                    retry_pending,
                    LxmfDeliveryStatus::Sent {},
                    Some("ack timeout; retrying direct delivery".to_string()),
                );
                info!(
                    "[lxmf][mission] resent direct after ack timeout original_message_id={} retry_message_id={} destination={} command={} correlation={}",
                    retry_pending.message_id_hex,
                    report.message_id_hex,
                    retry_pending.destination_hex,
                    retry_pending.command_type.as_deref().unwrap_or("-"),
                    retry_pending.correlation_id.as_deref().unwrap_or("-"),
                );
                if let Some(buffered_ack) = registered.buffered_ack {
                    acknowledge_pending_with_buffered_ack(state, bus, retry_pending, buffered_ack)
                        .await;
                }
                return Ok(true);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] direct retry after ack timeout failed destination={} command={} correlation={} outcome={:?}",
                    pending.destination_hex,
                    pending.command_type.as_deref().unwrap_or("-"),
                    pending.correlation_id.as_deref().unwrap_or("-"),
                    send_outcome_to_udl(report.outcome),
                );
            }
            Err(err) => {
                info!(
                    "[lxmf][mission] direct retry after ack timeout errored destination={} command={} correlation={} err={}",
                    pending.destination_hex,
                    pending.command_type.as_deref().unwrap_or("-"),
                    pending.correlation_id.as_deref().unwrap_or("-"),
                    err,
                );
            }
        }
    }
    if !should_retry_pending_ack_timeout_via_propagation(pending, has_active_relay) {
        return Ok(false);
    }
    resend.propagation_fallback_attempted = true;
    info!(
        "[lxmf][mission] ack timeout message_id={} destination={} command={} correlation={}; retrying via propagation relay",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
    );
    let report = send_lxmf_with_delivery_policy(
        state,
        bus,
        resend.requested_destination_hex.as_str(),
        resend.body.as_slice(),
        resend.title.clone(),
        resend.fields_bytes.clone(),
        Some(resend.metadata.clone()),
        SendMode::PropagationOnly {},
        resend.send_task_class.direct_recovery_equivalent(),
    )
    .await
    .map_err(|err| err.to_string())?;

    if !lxmf_send_succeeded(report.outcome) {
        return Err(format!("{:?}", send_outcome_to_udl(report.outcome)));
    }

    let Some(registered) = register_pending_lxmf_delivery(
        state,
        &report,
        Some(resend),
        Some(pending.message_id_hex.clone()),
    )
    .await
    else {
        return Err("propagation retry did not register pending delivery".to_string());
    };
    let retry_pending = &registered.pending;
    state.sdk.record_delivery_sent(
        &retry_pending.message_id_hex,
        &retry_pending.destination_hex,
        retry_pending.correlation_id.as_deref(),
        retry_pending.command_id.as_deref(),
        retry_pending.command_type.as_deref(),
        retry_pending.event_uid.as_deref(),
        retry_pending.mission_uid.as_deref(),
    );
    emit_lxmf_delivery(
        bus,
        retry_pending,
        LxmfDeliveryStatus::SentToPropagation {},
        Some("ack timeout; retrying via propagation".to_string()),
    );
    info!(
        "[lxmf][mission] resent after ack timeout original_message_id={} retry_message_id={} destination={} command={} correlation={}",
        retry_pending.message_id_hex,
        report.message_id_hex,
        retry_pending.destination_hex,
        retry_pending.command_type.as_deref().unwrap_or("-"),
        retry_pending.correlation_id.as_deref().unwrap_or("-"),
    );
    if let Some(buffered_ack) = registered.buffered_ack {
        acknowledge_pending_with_buffered_ack(state, bus, retry_pending, buffered_ack).await;
    }
    Ok(true)
}

fn lxmf_send_succeeded(outcome: RnsSendOutcome) -> bool {
    matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    )
}

fn lxmf_delivery_status_for(report: &LxmfSendReport) -> LxmfDeliveryStatus {
    if report.used_propagation_node && lxmf_send_succeeded(report.outcome) {
        LxmfDeliveryStatus::SentToPropagation {}
    } else {
        LxmfDeliveryStatus::Sent {}
    }
}

fn node_error_code(err: &NodeError) -> &'static str {
    match err {
        NodeError::InvalidConfig {} => "InvalidConfig",
        NodeError::IoError {} => "IoError",
        NodeError::NetworkError {} => "NetworkError",
        NodeError::ReticulumError {} => "ReticulumError",
        NodeError::AlreadyRunning {} => "AlreadyRunning",
        NodeError::NotRunning {} => "NotRunning",
        NodeError::Timeout {} => "Timeout",
        NodeError::LxmfWireEncodeError {} => "LxmfWireEncodeError",
        NodeError::LxmfMessageIdParseError {} => "LxmfMessageIdParseError",
        NodeError::LxmfPacketTooLarge {} => "LxmfPacketTooLarge",
        NodeError::LxmfPacketBuildError {} => "LxmfPacketBuildError",
        NodeError::EventStreamClosed {} => "EventStreamClosed",
        NodeError::InternalError {} => "InternalError",
    }
}

fn is_retriable_lxmf_error(err: &NodeError) -> bool {
    matches!(
        err,
        NodeError::NetworkError {}
            | NodeError::Timeout {}
            | NodeError::ReticulumError {}
            | NodeError::InternalError {}
    )
}

fn is_accepted_result_metadata(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.result_present && metadata.result_status.as_deref() == Some("accepted")
    })
}

fn is_sos_status_metadata(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.command_present && metadata.command_type.as_deref() == Some("sos.status")
    })
}

fn should_serialize_lxmf_destination_send(is_accepted_result: bool, is_sos_status: bool) -> bool {
    !is_accepted_result && !is_sos_status
}

#[expect(
    clippy::too_many_arguments,
    reason = "send policy boundary intentionally keeps transport, payload, metadata, and lane selection explicit"
)]
async fn send_lxmf_with_delivery_policy(
    state: &NodeRuntimeState,
    bus: &EventBus,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    send_mode: SendMode,
    send_task_class: SendTaskClass,
) -> Result<LxmfSendReport, NodeError> {
    const RETRY_DELAY: Duration = Duration::from_secs(10);
    const ACCEPTED_RESULT_RETRY_DELAY: Duration = Duration::from_secs(1);
    let has_active_relay = has_active_propagation_relay(state).await;
    let has_active_relay_transport =
        has_active_relay && has_active_relay_transport_interface(state).await;
    let rnode_only_transport = {
        let active_interfaces = state.active_interface_registry.lock().await;
        active_interfaces_are_rnode_ble_only(&active_interfaces)
    };
    let is_accepted_result = is_accepted_result_metadata(metadata.as_ref());
    let is_sos_status = is_sos_status_metadata(metadata.as_ref());
    let retry_delay = if is_accepted_result {
        ACCEPTED_RESULT_RETRY_DELAY
    } else {
        RETRY_DELAY
    };
    let normalized_requested_destination = normalize_hex_32(requested_destination_hex)
        .unwrap_or_else(|| requested_destination_hex.trim().to_ascii_lowercase());
    let canonical_requested_destination =
        canonical_app_destination_hex(state, normalized_requested_destination.as_str()).await;
    let is_saved_peer = saved_peer_matches_destination(
        state,
        normalized_requested_destination.as_str(),
        canonical_requested_destination.as_str(),
    )
    .await;
    let can_try_stored_lxmf_route = matches!(send_mode, SendMode::Auto {})
        && is_saved_peer
        && saved_peer_can_try_stored_lxmf_route(
            state,
            normalized_requested_destination.as_str(),
            canonical_requested_destination.as_str(),
        )
        .await;
    let require_current_peer = !is_accepted_result && !can_try_stored_lxmf_route;
    let direct_delivery_ready = if can_try_stored_lxmf_route {
        saved_peer_has_direct_ready_route(
            state,
            canonical_requested_destination.as_str(),
            has_active_relay_transport,
        )
        .await
    } else {
        false
    };
    let has_current_lxmf_route = if can_try_stored_lxmf_route {
        saved_peer_has_current_lxmf_route(state, canonical_requested_destination.as_str()).await
    } else {
        false
    };
    #[cfg(not(test))]
    let direct_priority_hops = if matches!(send_task_class, SendTaskClass::Mission)
        && matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && is_saved_peer
    {
        mission_direct_priority_hops(
            state,
            requested_destination_hex,
            canonical_requested_destination.as_str(),
        )
        .await
    } else {
        None
    };
    #[cfg(test)]
    let direct_priority_hops = None;
    let direct_attempts = direct_attempt_budget_for_send(
        send_mode,
        has_active_relay_transport,
        can_try_stored_lxmf_route,
        has_current_lxmf_route,
        direct_delivery_ready,
        direct_priority_hops,
    );
    let _destination_send_lock =
        if should_serialize_lxmf_destination_send(is_accepted_result, is_sos_status) {
            Some(
                state
                    .mission_destination_locks
                    .acquire(canonical_requested_destination.as_str())
                    .await?,
            )
        } else {
            None
        };
    let prefer_propagation = matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && has_active_relay_transport
        && is_saved_peer
        && saved_peer_prefers_propagation(
            state,
            requested_destination_hex,
            has_active_relay_transport,
            direct_priority_hops,
        )
        .await;

    if matches!(send_mode, SendMode::PropagationOnly {}) || prefer_propagation {
        let propagation_task_class = send_task_class.propagation_equivalent();
        if prefer_propagation {
            info!(
                "[lxmf][mission] saved peer {} is better suited for relay delivery; using propagation relay priority_hops={}",
                requested_destination_hex,
                direct_priority_hops
                    .map(|hops| hops.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
        let resolved_destination_hex =
            resolve_lxmf_destination_for_send(state, requested_destination_hex, false).await?;
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        log_send_task(
            propagation_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                propagation_task_class.label(),
                requested_destination_hex,
            ),
        );
        let _permit =
            acquire_send_task_permit(&state.send_task_permits, propagation_task_class).await?;
        log_send_task(
            propagation_task_class,
            format!(
                "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                propagation_task_class.label(),
                requested_destination_hex,
            ),
        );
        return send_lxmf_via_propagation_candidates(
            state,
            destination,
            requested_destination_hex,
            body,
            title,
            fields_bytes,
            metadata,
        )
        .await;
    }

    #[cfg(not(test))]
    let mission_direct_admission_delay =
        mission_direct_priority_delay_for_hops(direct_priority_hops);

    let mut last_error: Option<NodeError> = None;
    let mut last_resolved_destination_hex: Option<String> = None;

    for attempt in 1..=direct_attempts {
        let resolved_destination_hex = resolve_lxmf_destination_for_send(
            state,
            requested_destination_hex,
            require_current_peer,
        )
        .await?;
        last_resolved_destination_hex = Some(resolved_destination_hex.clone());
        info!(
            "[lxmf][mission] resolved send requested_destination={} canonical_destination={} resolved_destination={} mode={:?} attempt={attempt}/{direct_attempts} require_current_peer={} saved_peer={} stored_lxmf_route={} active_relay={} relay_transport={} direct_ready={}",
            requested_destination_hex,
            canonical_requested_destination,
            resolved_destination_hex,
            send_mode,
            require_current_peer,
            is_saved_peer,
            can_try_stored_lxmf_route,
            has_active_relay,
            has_active_relay_transport,
            direct_delivery_ready,
        );
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        let rnode_direct_route =
            rnode_only_transport || destination_uses_rnode_ble_route(state, &destination).await;
        let direct_link_connect_timeout =
            rnode_direct_route.then_some(RNODE_BLE_LINK_CONNECT_TIMEOUT);
        #[cfg(not(test))]
        if !mission_direct_admission_delay.is_zero() {
            info!(
                "[lxmf][queue] deferring {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts} priority_hops={} delay_ms={}",
                send_task_class.label(),
                requested_destination_hex,
                send_mode,
                direct_priority_hops.unwrap_or(u8::MAX),
                mission_direct_admission_delay.as_millis(),
            );
            tokio::time::sleep(mission_direct_admission_delay).await;
        }
        log_send_task(
            send_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts}",
                send_task_class.label(),
                requested_destination_hex,
                send_mode,
            ),
        );
        let send_result = {
            let _permit =
                acquire_send_task_permit(&state.send_task_permits, send_task_class).await?;
            log_send_task(
                send_task_class,
                format!(
                    "[lxmf][queue] acquired {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts}",
                    send_task_class.label(),
                    requested_destination_hex,
                    send_mode,
                ),
            );
            state
                .sdk
                .send_lxmf_with_direct_attempt(
                    destination,
                    body,
                    title.clone(),
                    fields_bytes.clone(),
                    metadata.clone(),
                    direct_attempt_send_mode(send_mode),
                    Some(attempt),
                    direct_link_connect_timeout,
                    rnode_direct_route.then_some(RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES),
                    None,
                )
                .await
        };
        match send_result {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                if !report.used_propagation_node {
                    if is_saved_peer && matches!(report.method, LxmfDeliveryMethod::Direct {}) {
                        register_desired_managed_peer_link(
                            state,
                            report.resolved_destination_hex.as_str(),
                        )
                        .await;
                    }
                    clear_peer_direct_delivery_unhealthy(
                        state,
                        requested_destination_hex,
                        Some(report.resolved_destination_hex.as_str()),
                    )
                    .await;
                    record_peer_link_state(
                        state,
                        bus,
                        report.resolved_destination_hex.as_str(),
                        true,
                    )
                    .await;
                }
                return Ok(report);
            }
            Ok(report) => {
                last_resolved_destination_hex = Some(report.resolved_destination_hex.clone());
                info!(
                    "[lxmf][mission] send attempt {attempt}/{direct_attempts} failed destination={} mode={:?} outcome={:?}",
                    requested_destination_hex,
                    send_mode,
                    report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                let retriable = is_retriable_lxmf_error(&err);
                info!(
                    "[lxmf][mission] send attempt {attempt}/{direct_attempts} errored destination={} mode={:?} err={}",
                    requested_destination_hex,
                    send_mode,
                    err,
                );
                last_error = Some(err);
                if !retriable {
                    break;
                }
            }
        }

        if attempt < direct_attempts {
            log_send_task(
                send_task_class,
                format!(
                    "[lxmf][queue] sleeping before retry destination={} mode={:?} next_attempt={}/{} delay_ms={}",
                    requested_destination_hex,
                    send_mode,
                    attempt + 1,
                    direct_attempts,
                    retry_delay.as_millis(),
                ),
            );
            tokio::time::sleep(retry_delay).await;
        }
    }

    if !matches!(send_mode, SendMode::Auto {}) || !has_active_relay_transport {
        return Err(last_error.unwrap_or(NodeError::NetworkError {}));
    }

    if direct_attempts == 0 {
        info!(
            "[lxmf][mission] auto delivery using propagation without direct probe destination={} saved_peer={} stored_lxmf_route={} active_relay={} relay_transport={} direct_ready={}",
            requested_destination_hex,
            is_saved_peer,
            can_try_stored_lxmf_route,
            has_active_relay,
            has_active_relay_transport,
            direct_delivery_ready,
        );
    } else {
        if !should_try_propagation_after_direct_failure(
            send_mode,
            is_accepted_result,
            has_active_relay_transport,
            is_saved_peer,
            last_error.as_ref().is_some_and(is_retriable_lxmf_error),
        ) {
            return Err(last_error.unwrap_or(NodeError::NetworkError {}));
        }
        mark_peer_direct_delivery_unhealthy(
            state,
            requested_destination_hex,
            last_resolved_destination_hex.as_deref(),
        )
        .await;
        close_output_links_for_direct_delivery_failure(
            state,
            requested_destination_hex,
            last_resolved_destination_hex.as_deref(),
        )
        .await;
        record_peer_link_state(state, bus, requested_destination_hex, false).await;
        if let Some(target) =
            register_desired_managed_peer_link(state, requested_destination_hex).await
        {
            if let ManagedPeerReconnectStart::Started(target) = state
                .managed_peer_links
                .begin_reconnect(target.destination_hex.as_str())
                .await
            {
                spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
            }
        }
        info!(
            "[lxmf][mission] auto delivery exhausted destination={}; retrying via propagation relay",
            requested_destination_hex,
        );
    }
    let resolved_destination_hex =
        resolve_lxmf_destination_for_send(state, requested_destination_hex, false).await?;
    let destination = parse_address_hash(resolved_destination_hex.as_str())?;
    let propagation_task_class = send_task_class.direct_recovery_equivalent();
    log_send_task(
        propagation_task_class,
        format!(
            "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=fallback",
            propagation_task_class.label(),
            requested_destination_hex,
        ),
    );
    let _permit =
        acquire_send_task_permit(&state.send_task_permits, propagation_task_class).await?;
    log_send_task(
        propagation_task_class,
        format!(
            "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=fallback",
            propagation_task_class.label(),
            requested_destination_hex,
        ),
    );
    let mut report = send_lxmf_via_propagation_candidates(
        state,
        destination,
        requested_destination_hex,
        body,
        title,
        fields_bytes,
        metadata,
    )
    .await?;
    report.fallback_stage = Some(LxmfFallbackStage::AfterDirectRetryBudget {});
    Ok(report)
}

async fn send_lxmf_via_propagation_candidates(
    state: &NodeRuntimeState,
    destination: AddressHash,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
) -> Result<LxmfSendReport, NodeError> {
    let active_relay = state.active_propagation_node_hex.lock().await.clone();
    let active_relay_hex = active_relay.as_deref().unwrap_or("");
    let announces = state.messaging.lock().await.list_announces();
    let mut relay_candidates = propagation_sync_candidate_relays(
        announces.as_slice(),
        active_relay_hex,
        state.preferred_propagation_node_hex.as_deref(),
    );
    if relay_candidates.is_empty() {
        return Err(delivery_route_unavailable_error());
    }

    let mut last_error = None;
    for (index, relay_candidate) in relay_candidates.drain(..).enumerate() {
        let attempt_number = index + 1;
        info!(
            "[lxmf][mission] propagation send relay attempt relay={} attempt={}/{} destination={}",
            relay_candidate,
            attempt_number,
            PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS,
            requested_destination_hex,
        );

        match state
            .sdk
            .send_lxmf_via_propagation_relay(
                destination,
                body,
                title.clone(),
                fields_bytes.clone(),
                metadata.clone(),
                relay_candidate.clone(),
            )
            .await
        {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                return Ok(report);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={} destination={} outcome={:?}",
                    relay_candidate, requested_destination_hex, report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={} destination={} reason={}",
                    relay_candidate, requested_destination_hex, err,
                );
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or(NodeError::NetworkError {}))
}

async fn emit_received_payload(
    state: &NodeRuntimeState,
    bus: &EventBus,
    sdk: &RuntimeLxmfSdk,
    destination_hex: String,
    payload: Vec<u8>,
    fallback_fields_bytes: Option<Vec<u8>>,
    expected_lxmf: bool,
) {
    match LxmfMessage::from_wire(payload.as_slice()) {
        Ok(message) => {
            let wire_message_id_hex = LxmfWireMessage::unpack(payload.as_slice())
                .map(|wire| hex::encode(wire.message_id()))
                .ok();
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
            let sos_fields = fields_bytes.as_deref().and_then(parse_sos_fields);
            let mut sos_telemetry = sos_fields
                .as_ref()
                .and_then(|fields| fields.telemetry.clone());
            if sos_telemetry.is_none() {
                if let Some((lat, lon)) = extract_text_coordinates(body_utf8.as_str()) {
                    sos_telemetry = Some(SosDeviceTelemetryRecord {
                        lat: Some(lat),
                        lon: Some(lon),
                        alt: None,
                        speed: None,
                        course: None,
                        accuracy: None,
                        battery_percent: None,
                        battery_charging: None,
                        updated_at_ms: now_ms(),
                    });
                }
            }
            let sos_command = sos_fields
                .as_ref()
                .and_then(|fields| fields.command.clone());
            let text_sos_kind = sos_kind_from_text(body_utf8.as_str());
            let is_sos_message = sos_command.is_some() || text_sos_kind.is_some();
            let metadata = fields_bytes
                .as_deref()
                .and_then(parse_mission_sync_metadata);
            if let Some(metadata) = metadata.as_ref().filter(|_| !is_sos_message) {
                if metadata.is_mission_related() {
                    info!(
                    "[lxmf][mission] received kind={} name={} source={} destination={} event_uid={} mission_uid={} correlation={}",
                    metadata.primary_kind(),
                    metadata.primary_name().unwrap_or("-"),
                    source_hex.as_deref().unwrap_or("-"),
                    destination_hex,
                    metadata.event_uid.as_deref().unwrap_or("-"),
                    metadata.mission_uid.as_deref().unwrap_or("-"),
                    metadata.correlation_id.as_deref().unwrap_or("-"),
                );
                }
                ack_pending_lxmf_delivery(state, bus, source_hex.as_deref(), metadata).await;
                let persisted_eam = persist_received_eam_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    body_utf8.as_str(),
                    source_hex.as_deref(),
                )
                .await;
                let persisted_event = persist_received_event_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    Some(message.content.as_slice()),
                    source_hex.as_deref(),
                )
                .await;
                let persisted_telemetry = persist_received_telemetry_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                )
                .await;
                let persisted_checklist = persist_received_checklist_if_present(
                    &state.app_state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    Some(message.content.as_slice()),
                );
                send_operational_ack_if_needed(
                    state,
                    bus,
                    source_hex.as_deref(),
                    Some(metadata),
                    persisted_eam || persisted_event || persisted_telemetry || persisted_checklist,
                )
                .await;
            }
            if is_sos_message {
                let peer_hex = source_hex
                    .clone()
                    .unwrap_or_else(|| destination_hex.clone());
                let message_id_hex = wire_message_id_hex
                    .clone()
                    .unwrap_or_else(|| format!("sos-{}-{}", peer_hex, now_ms()));
                let state_kind = sos_command
                    .as_ref()
                    .map(|command| command.state)
                    .or(text_sos_kind)
                    .unwrap_or(SosMessageKind::Active {});
                let incident_id = sos_command
                    .as_ref()
                    .map(|command| command.incident_id.clone())
                    .or_else(|| {
                        matches!(state_kind, SosMessageKind::Cancelled {}).then(|| {
                            state
                                .app_state
                                .latest_active_sos_alert_for_source(peer_hex.as_str())
                                .ok()
                                .flatten()
                                .map(|alert| alert.incident_id)
                        })?
                    })
                    .unwrap_or_else(|| format!("legacy-sos-{}-{}", peer_hex, now_ms()));
                let received_at_ms = now_ms();
                let record = MessageRecord {
                    message_id_hex: message_id_hex.clone(),
                    conversation_id: conversation_id_for(peer_hex.as_str()),
                    direction: MessageDirection::Inbound {},
                    destination_hex: peer_hex.clone(),
                    source_hex: source_hex.clone(),
                    requested_destination_hex: Some(peer_hex.clone()),
                    delivery_destination_hex: Some(peer_hex.clone()),
                    recipient_identity_hex: None,
                    last_wire_message_id_hex: Some(message_id_hex.clone()),
                    title: title.clone(),
                    body_utf8: body_utf8.clone(),
                    method: MessageMethod::Direct {},
                    state: MessageState::Received {},
                    transport_state: TransportDeliveryState::TransportDelivered {},
                    application_ack_state: ApplicationAckState::NotRequired {},
                    detail: Some("sos".to_string()),
                    sent_at_ms: None,
                    received_at_ms: Some(received_at_ms),
                    updated_at_ms: received_at_ms,
                };
                upsert_message_record(state, bus, record, true).await;
                let alert = received_alert_from_sos(
                    incident_id,
                    peer_hex.clone(),
                    conversation_id_for(peer_hex.as_str()),
                    state_kind,
                    body_utf8.clone(),
                    sos_telemetry.as_ref(),
                    sos_command
                        .as_ref()
                        .and_then(|command| command.audio_id.clone()),
                    Some(message_id_hex),
                    received_at_ms,
                );
                if let Ok(invalidation) = state.app_state.upsert_sos_alert(&alert) {
                    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                }
                if let Some(location) = location_from_alert(&alert) {
                    if let Ok(invalidation) = state.app_state.upsert_sos_location(&location) {
                        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                    }
                }
                if let Some(position) = telemetry_position_from_sos(
                    peer_hex.as_str(),
                    sos_telemetry.as_ref(),
                    received_at_ms,
                ) {
                    if let Ok(invalidation) = state.app_state.record_local_telemetry_fix(&position)
                    {
                        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                    }
                }
                bus.emit(NodeEvent::SosAlertChanged { alert });
                send_operational_ack_if_needed(
                    state,
                    bus,
                    source_hex.as_deref(),
                    metadata.as_ref(),
                    true,
                )
                .await;
            } else if !metadata
                .as_ref()
                .is_some_and(MissionSyncMetadata::is_mission_related)
            {
                let peer_hex = source_hex
                    .clone()
                    .unwrap_or_else(|| destination_hex.clone());
                let message_id_hex = wire_message_id_hex
                    .clone()
                    .unwrap_or_else(|| hex::encode(destination_hex.as_bytes()));
                if !acknowledge_chat_delivery(state, bus, source_hex.as_deref(), body_utf8.as_str())
                    .await
                {
                    let record = MessageRecord {
                        message_id_hex: message_id_hex.clone(),
                        conversation_id: conversation_id_for(peer_hex.as_str()),
                        direction: MessageDirection::Inbound {},
                        destination_hex: peer_hex.clone(),
                        source_hex: source_hex.clone(),
                        requested_destination_hex: Some(peer_hex.clone()),
                        delivery_destination_hex: Some(peer_hex.clone()),
                        recipient_identity_hex: None,
                        last_wire_message_id_hex: Some(message_id_hex.clone()),
                        title,
                        body_utf8: body_utf8.clone(),
                        method: MessageMethod::Direct {},
                        state: MessageState::Received {},
                        transport_state: TransportDeliveryState::TransportDelivered {},
                        application_ack_state: ApplicationAckState::NotRequired {},
                        detail: None,
                        sent_at_ms: None,
                        received_at_ms: Some(now_ms()),
                        updated_at_ms: now_ms(),
                    };
                    upsert_message_record(state, bus, record, true).await;
                    send_chat_delivery_ack_if_needed(
                        state,
                        bus,
                        source_hex.as_deref(),
                        message_id_hex.as_str(),
                        body_utf8.as_str(),
                    )
                    .await;
                }
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
        Err(err) if expected_lxmf => {
            let prefix = hex::encode(payload.iter().take(16).copied().collect::<Vec<_>>());
            warn!(
                "[lxmf][rx] decode_failed destination={} bytes={} prefix={} reason={}",
                destination_hex,
                payload.len(),
                prefix,
                err,
            );
            bus.emit(NodeEvent::Error {
                code: "LxmfDecodeError".to_string(),
                message: format!(
                    "Failed to decode LXMF payload for destination {destination_hex}: {err}"
                ),
            });
            return;
        }
        Err(_) => {}
    }

    info!(
        "[lxmf][rx] non_lxmf_payload destination={} bytes={} prefix={}",
        destination_hex,
        payload.len(),
        hex::encode(payload.iter().take(16).copied().collect::<Vec<_>>()),
    );
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
    if !metadata.result_present && !metadata.event_present {
        return;
    }

    let Some(source_hex) = source_hex else {
        return;
    };

    let detail = metadata.ack_detail().map(ToOwned::to_owned);
    let application_ack_state = application_ack_state_for_mission_metadata(metadata);
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
            state.pending_lxmf_acknowledgements.lock().await.insert(
                tracking_key.clone(),
                PendingLxmfAcknowledgement {
                    source_hex: source_hex.to_string(),
                    detail: detail.clone(),
                    application_ack_state,
                    buffered_at_ms: now_ms(),
                },
            );
            info!(
                "[lxmf][mission] buffered acknowledgement source={} command={} correlation={} detail={}",
                source_hex,
                metadata.command_type.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
                detail.as_deref().unwrap_or("-"),
            );
        }
        return;
    };
    if !peer_destinations_equivalent(state, pending.destination_hex.as_str(), source_hex).await {
        if let Some(tracking_key) = pending
            .command_id
            .as_deref()
            .or(pending.correlation_id.as_deref())
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

    record_peer_link_state(state, bus, source_hex, true).await;
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
        application_ack_state,
        detail.clone(),
    );
    info!(
        "[lxmf][mission] acknowledged message_id={} destination={} command={} correlation={} detail={}",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
        detail.as_deref().unwrap_or("-"),
    );
}

async fn send_operational_ack_if_needed(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    metadata: Option<&MissionSyncMetadata>,
    persisted: bool,
) {
    if !persisted {
        return;
    }
    let Some(ack) = operational_ack_from_metadata(source_hex, metadata) else {
        return;
    };
    let local_lxmf_hex = {
        let destination = state.lxmf_destination.lock().await;
        address_hash_to_hex(&destination.desc.address_hash)
    };
    if ack.destination_hex == local_lxmf_hex {
        return;
    }
    const OPERATIONAL_ACK_SEND_ATTEMPTS: usize = 3;
    const OPERATIONAL_ACK_REDUNDANT_DELAY: Duration = Duration::from_millis(250);
    let fields = match build_compact_operational_ack_fields(&ack) {
        Ok(fields) => fields,
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: node_error_code(&err).to_string(),
                message: format!(
                    "operational acknowledgement build failed command={} reason={}",
                    ack.command_id, err
                ),
            });
            return;
        }
    };
    let ack_metadata = parse_mission_sync_metadata(fields.as_slice());
    let mut sent = false;
    let mut last_error: Option<NodeError> = None;
    for attempt in 1..=OPERATIONAL_ACK_SEND_ATTEMPTS {
        if attempt > 1 {
            tokio::time::sleep(OPERATIONAL_ACK_REDUNDANT_DELAY).await;
        }
        match send_lxmf_with_delivery_policy(
            state,
            bus,
            ack.destination_hex.as_str(),
            &[],
            None,
            Some(fields.clone()),
            ack_metadata.clone(),
            SendMode::Auto {},
            SendTaskClass::MissionAck,
        )
        .await
        {
            Ok(report) => {
                sent = true;
                info!(
                    "[lxmf][mission] sent received acknowledgement destination={} message_id={} command={} correlation={} type={} attempt={}/{}",
                    report.resolved_destination_hex,
                    report.message_id_hex,
                    ack.command_id,
                    ack.correlation_id.as_deref().unwrap_or("-"),
                    ack.command_type.as_deref().unwrap_or("-"),
                    attempt,
                    OPERATIONAL_ACK_SEND_ATTEMPTS,
                );
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }
    if !sent {
        if let Some(err) = last_error {
            bus.emit(NodeEvent::Error {
                code: node_error_code(&err).to_string(),
                message: format!(
                    "operational acknowledgement send failed destination={} command={} reason={}",
                    ack.destination_hex, ack.command_id, err
                ),
            });
        }
    }
}

async fn acknowledge_chat_delivery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    body_utf8: &str,
) -> bool {
    let Some(message_id_hex) = parse_chat_delivery_ack_body(body_utf8) else {
        return false;
    };
    let maybe_record = state
        .messaging
        .lock()
        .await
        .update_message_delivery_state(
            message_id_hex.as_str(),
            Some(sdkmsg::MessageState::Delivered),
            Some(sdkmsg::TransportDeliveryState::TransportDelivered),
            Some(sdkmsg::ApplicationAckState::Accepted),
            Some("chat delivery ack".to_string()),
            None,
            now_ms(),
        )
        .map(from_sdk_message_record);

    if let Some(record) = maybe_record {
        if let Some(source_hex) = source_hex {
            record_peer_link_state(state, bus, source_hex, true).await;
        }
        state.sdk.record_delivery_acknowledged(
            &record.message_id_hex,
            &record.destination_hex,
            source_hex,
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
        info!(
            "[lxmf][chat] acknowledged message_id={} source={}",
            record.message_id_hex,
            source_hex.unwrap_or("-"),
        );
    }
    true
}

async fn send_chat_delivery_ack_if_needed(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    message_id_hex: &str,
    body_utf8: &str,
) {
    if parse_chat_delivery_ack_body(body_utf8).is_some() {
        return;
    }
    let Some(source_hex) = source_hex else {
        return;
    };
    let body = chat_delivery_ack_body(message_id_hex);
    match send_lxmf_with_delivery_policy(
        state,
        bus,
        source_hex,
        body.as_bytes(),
        Some(CHAT_DELIVERY_ACK_TITLE.to_string()),
        None,
        None,
        SendMode::Auto {},
        SendTaskClass::General,
    )
    .await
    {
        Ok(report) => {
            info!(
                "[lxmf][chat] sent delivery acknowledgement destination={} message_id={} acked_message_id={}",
                report.resolved_destination_hex, report.message_id_hex, message_id_hex,
            );
        }
        Err(err) => {
            warn!(
                "[lxmf][chat] delivery acknowledgement send failed destination={} acked_message_id={} reason={}",
                source_hex, message_id_hex, err,
            );
        }
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

fn parse_hub_directory_peer_record(value: &MsgPackValue) -> Option<HubDirectoryPeerRecord> {
    let entries = msgpack_map_entries(value)?;
    Some(HubDirectoryPeerRecord {
        identity: msgpack_get_named(entries, &["identity"]).and_then(msgpack_string)?,
        destination_hash: msgpack_get_named(entries, &["destination_hash"])
            .and_then(msgpack_string)?,
        display_name: msgpack_get_named(entries, &["display_name"]).and_then(msgpack_string),
        announce_capabilities: msgpack_get_named(entries, &["announce_capabilities"])
            .and_then(msgpack_string_vec)
            .unwrap_or_default(),
        client_type: msgpack_get_named(entries, &["client_type"]).and_then(msgpack_string),
        registered_mode: msgpack_get_named(entries, &["registered_mode"]).and_then(msgpack_string),
        last_seen: msgpack_get_named(entries, &["last_seen"]).and_then(msgpack_string),
        status: msgpack_get_named(entries, &["status"]).and_then(msgpack_string),
    })
}

fn parse_hub_directory_snapshot_value(
    value: &MsgPackValue,
    received_at_ms: u64,
) -> Option<HubDirectorySnapshot> {
    let entries = msgpack_map_entries(value)?;
    let effective_connected_mode = msgpack_get_named(entries, &["effective_connected_mode"])
        .and_then(msgpack_bool)
        .unwrap_or(false);
    let items = match msgpack_get_named(entries, &["items"]) {
        Some(MsgPackValue::Array(items)) => items
            .iter()
            .filter_map(parse_hub_directory_peer_record)
            .collect(),
        _ => Vec::new(),
    };
    Some(HubDirectorySnapshot {
        effective_connected_mode,
        items,
        received_at_ms,
    })
}

enum HubDirectoryResultState {
    Accepted,
    Snapshot(HubDirectorySnapshot),
}

fn parse_hub_directory_result_state(
    value: &MsgPackValue,
    expected_command_id: &str,
    received_at_ms: u64,
) -> Option<HubDirectoryResultState> {
    let entries = msgpack_map_entries(value)?;
    let command_id = msgpack_get_named(entries, &["command_id"]).and_then(msgpack_string);
    if command_id
        .as_deref()
        .is_some_and(|value| value != expected_command_id)
    {
        return None;
    }

    let status = msgpack_get_named(entries, &["status"])
        .and_then(msgpack_string)
        .map(|value| value.to_ascii_lowercase());
    if status.as_deref() == Some("accepted") {
        return Some(HubDirectoryResultState::Accepted);
    }

    let payload = msgpack_get_named(entries, &["payload", "result", "data"]).unwrap_or(value);
    parse_hub_directory_snapshot_value(payload, received_at_ms)
        .map(HubDirectoryResultState::Snapshot)
}

async fn publish_hub_directory_snapshot(
    state: &NodeRuntimeState,
    bus: &EventBus,
    snapshot: HubDirectorySnapshot,
) {
    if let Ok(mut guard) = state.hub_directory_snapshot.lock() {
        *guard = Some(snapshot.clone());
    }
    let _ = refresh_peer_snapshot(state).await;
    state.sdk.record_hub_directory_updated(&snapshot);
    bus.emit(NodeEvent::HubDirectoryUpdated { snapshot });
}

async fn refresh_hub_directory_lxmf(
    config: &NodeConfig,
    state: &NodeRuntimeState,
) -> Result<HubDirectorySnapshot, NodeError> {
    let hub_hex = config
        .hub_identity_hash
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let hub_hex = normalize_hex_32(hub_hex).ok_or(NodeError::InvalidConfig {})?;
    let hub = parse_address_hash(&hub_hex)?;

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

    wait_for_link_active(&state.transport, &link, DEFAULT_LINK_CONNECT_TIMEOUT).await?;

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

    let command_id = format!("hub-directory-{}", now_ms());
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from(command_id.as_str()),
            ),
            (
                MsgPackValue::from("command_type"),
                MsgPackValue::from("rem.registry.peers.list"),
            ),
            (
                MsgPackValue::from("timestamp"),
                MsgPackValue::from(current_timestamp_rfc3339()),
            ),
            (
                MsgPackValue::from("source"),
                MsgPackValue::Map(vec![(
                    MsgPackValue::from("rns_identity"),
                    MsgPackValue::from(state.identity.address_hash().to_hex_string()),
                )]),
            ),
            (MsgPackValue::from("args"), MsgPackValue::Map(vec![])),
        ])]),
    )]);

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(destination);
    message.set_title_from_string("rem.registry.peers.list");
    message.fields = Some(fields);

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

        if let Some(fields) = reply.fields.as_ref() {
            match parse_hub_directory_result_state(fields, &command_id, now_ms()) {
                Some(HubDirectoryResultState::Accepted) => continue,
                Some(HubDirectoryResultState::Snapshot(snapshot)) => return Ok(snapshot),
                None => {}
            }
        }
    }
}

fn tcp_endpoint_connect_addr(endpoint: &str) -> &str {
    endpoint
        .trim()
        .strip_prefix("tcp://")
        .unwrap_or_else(|| endpoint.trim())
}

fn configured_tcp_client_endpoints(endpoints: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for endpoint in endpoints {
        let connect_addr = tcp_endpoint_connect_addr(endpoint).trim();
        if connect_addr.is_empty() || normalized.iter().any(|value| value == connect_addr) {
            continue;
        }
        normalized.push(connect_addr.to_string());
    }
    normalized
}

fn tcp_endpoint_host(connect_addr: &str) -> &str {
    connect_addr
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(connect_addr)
        .trim_matches(['[', ']'])
        .trim()
}

fn tcp_endpoint_is_loopback(connect_addr: &str) -> bool {
    let host = tcp_endpoint_host(connect_addr).to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

fn tcp_readiness_monitor_endpoints(endpoints: &[String]) -> Vec<String> {
    endpoints
        .iter()
        .filter(|endpoint| !tcp_endpoint_is_loopback(endpoint))
        .cloned()
        .collect()
}

fn tcp_data_path_unavailable_message(endpoints: &[String]) -> String {
    format!(
        "transport startup failed: no reachable Reticulum TCP interface endpoints={}",
        endpoints.join(",")
    )
}

#[cfg(target_os = "android")]
fn normalize_rnode_region(region: &str) -> &'static str {
    if region.trim().eq_ignore_ascii_case("EU868") {
        "EU868"
    } else {
        "US915"
    }
}

#[cfg(target_os = "android")]
fn rnode_lora_config(settings: &RnodeSettingsRecord) -> Result<LoraConfig, String> {
    let mut config = LoraConfig::for_region(normalize_rnode_region(&settings.region))
        .into_lora_config_option()
        .unwrap_or_else(LoraConfig::us915_default);
    match settings.profile.trim() {
        "REM-MF-URBAN-v1" => {
            config.bandwidth_hz = 250_000;
            config.spreading_factor = 9;
            config.coding_rate = 5;
        }
        "REM-LM-EXTREME-v1" => {
            config.bandwidth_hz = 125_000;
            config.spreading_factor = 11;
            config.coding_rate = 8;
        }
        "REM-LF-RURAL-v1" | _ => {
            config.bandwidth_hz = 250_000;
            config.spreading_factor = 11;
            config.coding_rate = 5;
        }
    }
    config.validate()?;
    Ok(config)
}

#[cfg(target_os = "android")]
trait IntoLoraConfigOption {
    fn into_lora_config_option(self) -> Option<LoraConfig>;
}

#[cfg(target_os = "android")]
impl IntoLoraConfigOption for Option<LoraConfig> {
    fn into_lora_config_option(self) -> Option<LoraConfig> {
        self
    }
}

#[cfg(target_os = "android")]
impl<E> IntoLoraConfigOption for Result<Option<LoraConfig>, E> {
    fn into_lora_config_option(self) -> Option<LoraConfig> {
        self.unwrap_or(None)
    }
}

#[cfg(target_os = "android")]
struct RnodeBleWiring {
    label: String,
    lora: LoraConfig,
    native: NativeRnodeBleSettings,
    kiss: RnodeBleKissConfig,
}

#[cfg(target_os = "android")]
fn rnode_ble_wiring_from_settings(
    settings: &RnodeSettingsRecord,
) -> Result<RnodeBleWiring, String> {
    let peripheral_id = settings.peripheral_id.trim().to_string();
    if peripheral_id.is_empty() {
        return Err("RNode Bluetooth is enabled but no paired device is selected.".to_string());
    }

    let lora = rnode_lora_config(settings)?;
    let label = if settings.display_name.trim().is_empty() {
        format!("rnode-ble:{peripheral_id}")
    } else {
        format!("rnode-ble:{}", settings.display_name.trim())
    };
    let native = NativeRnodeBleSettings::for_peripheral(peripheral_id)
        .with_peripheral_alias(settings.display_name.trim());
    let kiss = RnodeBleKissConfig {
        mtu: usize::from(lora.max_payload_bytes),
        max_write_len: 20,
        read_frame_timeout: RNODE_BLE_READ_FRAME_TIMEOUT,
        initial_frames: lora.probe_frames(),
        deferred_frames: lora.radio_config_frames(),
        shutdown_frames: lora.shutdown_frames(),
        ..RnodeBleKissConfig::default()
    };

    Ok(RnodeBleWiring {
        label,
        lora,
        native,
        kiss,
    })
}

#[cfg(target_os = "android")]
fn spawn_rnode_ble_interface(
    transport: Arc<Transport>,
    bus: EventBus,
    settings: RnodeSettingsRecord,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
) {
    if !settings.enabled {
        return;
    }
    let connection_mode = match RnodeConnectionMode::parse(Some(&settings.connection_mode)) {
        Ok(mode) => mode,
        Err(error) => {
            set_runtime_interface_readiness(
                &status,
                &bus,
                "rnode",
                RuntimeReadinessState::Failed,
                "RNode configuration is invalid".to_string(),
                Some(error.to_string()),
            );
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: format!("Invalid RNode connection mode: {error}"),
            });
            return;
        }
    };
    match connection_mode {
        RnodeConnectionMode::Ble => {}
        RnodeConnectionMode::BluetoothClassic => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: "RNode Bluetooth Classic/SPP is selected, but the Android SPP backend is not wired into REM yet.".to_string(),
            });
            return;
        }
        RnodeConnectionMode::Usb => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: "RNode USB is selected, but the Android USB serial transport backend is not wired into REM yet.".to_string(),
            });
            return;
        }
        RnodeConnectionMode::Tcp => {
            info!("rnode_ble: RNode TCP mode selected; skipping Android BLE interface spawn");
            return;
        }
    }
    if let Err(error) = rnode_ble_wiring_from_settings(&settings) {
        set_runtime_interface_readiness(
            &status,
            &bus,
            "rnode",
            RuntimeReadinessState::Failed,
            "RNode interface configuration failed".to_string(),
            Some(error.clone()),
        );
        bus.emit(NodeEvent::Error {
            code: "InvalidConfig".to_string(),
            message: if error.starts_with("RNode Bluetooth") {
                error
            } else {
                format!("RNode LoRa profile is invalid: {error}")
            },
        });
        return;
    }
    let peripheral_id = settings.peripheral_id.trim().to_string();

    tokio::spawn(async move {
        let active = Arc::new(AtomicBool::new(false));
        loop {
            if active.load(Ordering::Acquire) {
                tokio::time::sleep(RNODE_BLE_INTERFACE_RETRY_INTERVAL).await;
                continue;
            }

            let wiring = match rnode_ble_wiring_from_settings(&settings) {
                Ok(wiring) => wiring,
                Err(error) => {
                    set_runtime_interface_readiness(
                        &status,
                        &bus,
                        "rnode",
                        RuntimeReadinessState::Failed,
                        "RNode interface configuration failed".to_string(),
                        Some(error.clone()),
                    );
                    bus.emit(NodeEvent::Error {
                        code: "InvalidConfig".to_string(),
                        message: if error.starts_with("RNode Bluetooth") {
                            error
                        } else {
                            format!("RNode LoRa profile is invalid: {error}")
                        },
                    });
                    return;
                }
            };
            let label = wiring.label;
            let adapter =
                NativeRnodeBleKissInterface::new(label.clone(), wiring.native, wiring.kiss)
                    .with_rnode_validation(wiring.lora, Duration::from_millis(15_000))
                    .with_detection_fallback_timeout(Duration::from_millis(5_000));

            active.store(true, Ordering::Release);
            let context = transport
                .iface_manager()
                .lock()
                .await
                .new_context_with_role_and_mode(adapter, IfaceRole::Unicast, InterfaceMode::Full);
            let iface = *context.channel.address();
            let status_update = new_interface_status(iface, label.clone(), "connected");
            active_interface_registry
                .lock()
                .await
                .insert(iface, status_update.clone());
            publish_interface_registry_snapshot(
                &active_interface_registry,
                &status,
                &bus,
                Some(status_update),
            )
            .await;
            info!(
                "rnode_ble: configured label={} peripheral={} region={} profile={} iface={}",
                label, peripheral_id, settings.region, settings.profile, iface
            );
            emit_operational_notice(
                &bus,
                LogLevel::Info {},
                format!(
                    "RNode Bluetooth LoRa interface enabled: {} ({}, {})",
                    label, settings.region, settings.profile
                ),
            );

            let active_for_task = active.clone();
            let registry_for_task = active_interface_registry.clone();
            let status_for_task = status.clone();
            let bus_for_task = bus.clone();
            let label_for_task = label.clone();
            tokio::spawn(async move {
                NativeRnodeBleKissInterface::spawn(context).await;
                let removed = registry_for_task.lock().await.remove(&iface);
                if let Some(mut removed) = removed {
                    removed.state = "disconnected".to_string();
                    publish_interface_registry_snapshot(
                        &registry_for_task,
                        &status_for_task,
                        &bus_for_task,
                        Some(removed),
                    )
                    .await;
                }
                active_for_task.store(false, Ordering::Release);
                warn!(
                    "rnode_ble: stopped interface label={} iface={}; retrying",
                    label_for_task, iface
                );
            });

            tokio::time::sleep(RNODE_BLE_INTERFACE_RETRY_INTERVAL).await;
        }
    });
}

#[cfg(not(target_os = "android"))]
fn spawn_rnode_ble_interface(
    _transport: Arc<Transport>,
    bus: EventBus,
    settings: RnodeSettingsRecord,
    _active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
) {
    if !settings.enabled {
        return;
    }
    let connection_mode = match RnodeConnectionMode::parse(Some(&settings.connection_mode)) {
        Ok(mode) => mode,
        Err(error) => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: format!("Invalid RNode connection mode: {error}"),
            });
            return;
        }
    };
    if matches!(connection_mode, RnodeConnectionMode::Tcp) {
        return;
    }
    let message = match connection_mode {
            RnodeConnectionMode::Ble => {
                "RNode BLE LoRa is only available on Android builds.".to_string()
            }
            RnodeConnectionMode::BluetoothClassic => {
                "RNode Bluetooth Classic/SPP is only available after a platform SPP backend is configured.".to_string()
            }
            RnodeConnectionMode::Usb => {
                "RNode USB serial is only available after a platform USB backend is configured.".to_string()
            }
            RnodeConnectionMode::Tcp => unreachable!(),
        };
    set_runtime_interface_readiness(
        &status,
        &bus,
        "rnode",
        RuntimeReadinessState::Unsupported,
        message.clone(),
        Some(message.clone()),
    );
    bus.emit(NodeEvent::Error {
        code: "InvalidConfig".to_string(),
        message,
    });
}

async fn connect_tcp_endpoint(connect_addr: &str) -> Option<TcpStream> {
    match connect_tcp_endpoint_with_error(connect_addr).await {
        Ok(stream) => Some(stream),
        Err(error) => {
            warn!(
                "tcp_client: connect failed endpoint=<{}>: {}",
                connect_addr, error
            );
            None
        }
    }
}

async fn connect_tcp_endpoint_with_error(connect_addr: &str) -> Result<TcpStream, String> {
    let addresses = tokio::time::timeout(TCP_CLIENT_CONNECT_TIMEOUT, lookup_host(connect_addr))
        .await
        .map_err(|_| format!("DNS lookup timed out after {TCP_CLIENT_CONNECT_TIMEOUT:?}"))?
        .map_err(|error| format!("DNS lookup failed: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("DNS lookup returned no socket addresses".to_string());
    }

    let mut failures = Vec::new();
    for address in addresses {
        match tokio::time::timeout(TCP_CLIENT_CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => failures.push(format!("{address}: {error}")),
            Err(_) => failures.push(format!(
                "{address}: connect timed out after {TCP_CLIENT_CONNECT_TIMEOUT:?}"
            )),
        }
    }
    Err(format!(
        "all resolved socket addresses failed: {}",
        failures.join("; ")
    ))
}

async fn tcp_endpoint_reachable(connect_addr: &str) -> bool {
    connect_tcp_endpoint(connect_addr).await.is_some()
}

async fn any_tcp_endpoint_reachable(endpoints: &[String]) -> bool {
    for endpoint in endpoints {
        if tcp_endpoint_reachable(endpoint).await {
            return true;
        }
    }
    false
}

async fn unregister_tcp_client_endpoint(
    active_interface_registry: &ActiveInterfaceRegistry,
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    endpoint: &str,
) {
    let removed = {
        let mut registry = active_interface_registry.lock().await;
        let remove_keys = registry
            .iter()
            .filter_map(|(interface, registered)| {
                (registered.label == endpoint).then_some(*interface)
            })
            .collect::<Vec<_>>();
        remove_keys
            .into_iter()
            .filter_map(|interface| registry.remove(&interface))
            .collect::<Vec<_>>()
    };
    for mut removed in removed {
        removed.state = "disconnected".to_string();
        publish_interface_registry_snapshot(active_interface_registry, status, bus, Some(removed))
            .await;
    }
}

fn set_runtime_interface_readiness(
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    id: &str,
    state: RuntimeReadinessState,
    detail: String,
    last_error: Option<String>,
) {
    if let Ok(mut guard) = status.lock() {
        guard.set_interface_readiness(id, state, detail, last_error);
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
}

fn spawn_tcp_client_interface_manager(
    transport: Arc<Transport>,
    connect_addr: String,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    tokio::spawn(async move {
        let active = Arc::new(AtomicBool::new(false));
        loop {
            if active.load(Ordering::Acquire) {
                tokio::time::sleep(TCP_CLIENT_INTERFACE_RETRY_INTERVAL).await;
                continue;
            }

            if let Some(stream) = connect_tcp_endpoint(connect_addr.as_str()).await {
                info!(
                    "tcp_client: starting connected interface for <{}>",
                    connect_addr
                );
                active.store(true, Ordering::Release);
                let active_for_task = active.clone();
                let task_addr = connect_addr.clone();
                let registry_for_task = active_interface_registry.clone();
                let status_for_task = status.clone();
                let bus_for_task = bus.clone();
                let iface = transport.iface_manager().lock().await.spawn(
                    TcpClient::new_from_stream(connect_addr.clone(), stream),
                    move |context| async move {
                        TcpClient::spawn(context).await;
                        unregister_tcp_client_endpoint(
                            &registry_for_task,
                            &status_for_task,
                            &bus_for_task,
                            task_addr.as_str(),
                        )
                        .await;
                        active_for_task.store(false, Ordering::Release);
                        info!("tcp_client: stopped interface for <{}>", task_addr);
                    },
                );
                let status_update = new_interface_status(iface, connect_addr.clone(), "connected");
                active_interface_registry
                    .lock()
                    .await
                    .insert(iface, status_update.clone());
                publish_interface_registry_snapshot(
                    &active_interface_registry,
                    &status,
                    &bus,
                    Some(status_update),
                )
                .await;
                info!(
                    "tcp_client: connected interface endpoint=<{}> iface={}",
                    connect_addr, iface
                );
            }

            tokio::time::sleep(TCP_CLIENT_INTERFACE_RETRY_INTERVAL).await;
        }
    });
}

fn spawn_tcp_client_readiness_monitor(
    endpoints: Vec<String>,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    if endpoints.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let mut data_path_down = false;
        loop {
            let reachable = any_tcp_endpoint_reachable(endpoints.as_slice()).await;
            if reachable {
                if data_path_down {
                    info!(
                        "tcp_client: Reticulum TCP data path restored endpoints={}",
                        endpoints.join(",")
                    );
                    emit_operational_notice(
                        &bus,
                        LogLevel::Info {},
                        format!("Reticulum TCP data path restored: {}", endpoints.join(",")),
                    );
                    if let Ok(mut guard) = status.lock() {
                        guard.refresh_readiness();
                        bus.emit(NodeEvent::StatusChanged {
                            status: guard.clone(),
                        });
                    }
                }
                data_path_down = false;
            } else if !data_path_down {
                let message = tcp_data_path_unavailable_message(endpoints.as_slice());
                warn!("{}", message);
                bus.emit(NodeEvent::Error {
                    code: "NetworkError".to_string(),
                    message: message.clone(),
                });
                emit_operational_notice(
                    &bus,
                    LogLevel::Warn {},
                    format!(
                        "Reticulum TCP data path unavailable: {}",
                        endpoints.join(",")
                    ),
                );
                set_runtime_interface_readiness(
                    &status,
                    &bus,
                    "tcp",
                    RuntimeReadinessState::Failed,
                    "Configured TCP endpoints are unreachable".to_string(),
                    Some(message),
                );
                data_path_down = true;
            }

            tokio::time::sleep(TCP_CLIENT_READINESS_CHECK_INTERVAL).await;
        }
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "runtime entrypoint receives independently owned state handles and command lanes"
)]
pub async fn run_node(
    config: NodeConfig,
    identity: PrivateIdentity,
    app_state: AppStateStore,
    status: Arc<Mutex<NodeStatus>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    hub_directory_snapshot: Arc<Mutex<Option<HubDirectorySnapshot>>>,
    bus: EventBus,
    mut cmd_rx: mpsc::Receiver<Command>,
    mut priority_cmd_rx: mpsc::Receiver<Command>,
) {
    let mut transport_cfg = TransportConfig::new(config.name.clone(), &identity, config.broadcast);
    transport_cfg.set_retransmit(config.transport_node_enabled);
    if config.rnode.enabled {
        transport_cfg.set_resource_retry_interval_secs(RNODE_BLE_RESOURCE_RETRY_INTERVAL_SECS);
        transport_cfg.set_resource_retry_limit(RNODE_BLE_RESOURCE_RETRY_LIMIT);
    }

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
    let ratchet_store_path = config
        .storage_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|mut path| {
            path.push("ratchets.dat");
            path
        });

    let mut transport = Transport::new(transport_cfg);
    let receipt_message_ids =
        Arc::new(Mutex::new(HashMap::<String, ReceiptMessageTracking>::new()));
    let (receipt_tx, mut receipt_rx) = mpsc::unbounded_channel::<String>();
    transport
        .set_receipt_handler(Box::new(RuntimeReceiptBridge {
            receipt_message_ids: receipt_message_ids.clone(),
            tx: receipt_tx,
        }))
        .await;

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
    let active_interface_registry: ActiveInterfaceRegistry =
        Arc::new(TokioMutex::new(HashMap::new()));
    spawn_interface_traffic_monitor(
        transport.clone(),
        active_interface_registry.clone(),
        status.clone(),
        bus.clone(),
    );
    let tcp_client_endpoints = configured_tcp_client_endpoints(config.tcp_clients.as_slice());
    for endpoint in tcp_client_endpoints.iter().cloned() {
        spawn_tcp_client_interface_manager(
            transport.clone(),
            endpoint,
            active_interface_registry.clone(),
            status.clone(),
            bus.clone(),
        );
    }
    spawn_rnode_ble_interface(
        transport.clone(),
        bus.clone(),
        config.rnode.clone(),
        active_interface_registry.clone(),
        status.clone(),
    );

    let _legacy_app_destination_hex = app_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let lxmf_destination_hex = lxmf_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let app_destination_hex = lxmf_destination_hex.clone();

    let announce_capabilities = Arc::new(TokioMutex::new(config.announce_capabilities.clone()));
    let known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<Link>>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let connected_peers: Arc<TokioMutex<HashSet<AddressHash>>> =
        Arc::new(TokioMutex::new(HashSet::new()));
    let peer_resolution_inflight: Arc<TokioMutex<HashSet<String>>> =
        Arc::new(TokioMutex::new(HashSet::new()));
    let pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let pending_lxmf_acknowledgements: Arc<
        TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>,
    > = Arc::new(TokioMutex::new(HashMap::new()));
    let messaging = Arc::new(TokioMutex::new(sdkmsg::MessagingStore::new(
        config.stale_after_minutes,
    )));
    let active_propagation_node_hex: Arc<TokioMutex<Option<String>>> =
        Arc::new(TokioMutex::new(None));
    let propagation_sync_inflight = Arc::new(AtomicBool::new(false));
    let direct_delivery_health = DirectDeliveryHealth::default();
    let managed_peer_links = ManagedPeerLinks::default();
    let ignored_peer_destinations = Arc::new(TokioMutex::new(
        app_state
            .get_ignored_peer_destinations()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|destination| normalize_hex_32(destination.as_str()))
            .collect::<HashSet<_>>(),
    ));
    let send_task_permits = SendTaskPermits::new();
    let mission_destination_locks = MissionDestinationLocks::new();
    let projection_journal = Arc::new(RuntimeProjectionJournal::new(
        projection_journal_path(config.storage_dir.as_deref()),
        bus.clone(),
    ));
    let sdk = Arc::new(RuntimeLxmfSdk::new(
        identity.address_hash().to_hex_string(),
        SdkTransportState {
            identity: identity.clone(),
            transport: transport.clone(),
            lxmf_destination: lxmf_destination.clone(),
            known_destinations: known_destinations.clone(),
            out_links: out_links.clone(),
            active_propagation_node_hex: active_propagation_node_hex.clone(),
            ratchet_store_path,
        },
    ));

    let state = NodeRuntimeState {
        app_state,
        identity: identity.clone(),
        app_destination_hex,
        transport: transport.clone(),
        lxmf_destination: lxmf_destination.clone(),
        peer_resolution_inflight: peer_resolution_inflight.clone(),
        known_destinations: known_destinations.clone(),
        out_links: out_links.clone(),
        active_interface_registry: active_interface_registry.clone(),
        connected_peers: connected_peers.clone(),
        pending_lxmf_deliveries: pending_lxmf_deliveries.clone(),
        pending_lxmf_acknowledgements: pending_lxmf_acknowledgements.clone(),
        messaging: messaging.clone(),
        peers_snapshot: peers_snapshot.clone(),
        sync_status_snapshot: sync_status_snapshot.clone(),
        hub_directory_snapshot: hub_directory_snapshot.clone(),
        projection_journal: projection_journal.clone(),
        sdk: sdk.clone(),
        active_propagation_node_hex: active_propagation_node_hex.clone(),
        preferred_propagation_node_hex: config
            .hub_identity_hash
            .as_ref()
            .and_then(|value| normalize_hex_32(value)),
        propagation_sync_inflight: propagation_sync_inflight.clone(),
        direct_delivery_health: direct_delivery_health.clone(),
        managed_peer_links: managed_peer_links.clone(),
        ignored_peer_destinations: ignored_peer_destinations.clone(),
        send_task_permits: send_task_permits.clone(),
        mission_destination_locks: mission_destination_locks.clone(),
    };

    if let Some(snapshot) = projection_journal.load_snapshot() {
        let restored_snapshot = snapshot.pruned_for_restore();
        projection_journal.seed_snapshot(restored_snapshot.clone());
        if let Ok(mut guard) = peers_snapshot.lock() {
            *guard = restored_snapshot.peers();
        }
        if let Ok(mut guard) = sync_status_snapshot.lock() {
            *guard = restored_snapshot.sync_status();
        }
        seed_runtime_projection_snapshot(&state, &restored_snapshot).await;
    }

    if let Ok(announces) = state.app_state.list_announces() {
        let mut messaging = state.messaging.lock().await;
        for announce in announces {
            messaging.record_announce(to_sdk_announce_record(announce));
        }
    }

    let restored_saved_management = {
        let saved_peers = state.app_state.get_saved_peers().unwrap_or_default();
        let mut messaging = state.messaging.lock().await;
        restore_saved_peer_management(&mut messaging, saved_peers.as_slice())
    };

    if let Err(err) = sdk.start().await {
        bus.emit(NodeEvent::Error {
            code: "sdk_start_failed".to_string(),
            message: err.to_string(),
        });
    }

    refresh_peer_snapshot(&state).await;
    sync_auto_propagation_node(&state, &bus).await;
    if !restored_saved_management.pruned_destinations.is_empty() {
        info!(
            "[peers] pruned restored saved peers with non-rem lxmf announce evidence destinations={}",
            restored_saved_management.pruned_destinations.join(","),
        );
        for destination in &restored_saved_management.pruned_destinations {
            emit_peer_changed(&state, &bus, destination).await;
        }
    }
    if !restored_saved_management
        .route_request_destinations
        .is_empty()
    {
        info!(
            "[announce] restored saved peers route requests destinations={}",
            restored_saved_management
                .route_request_destinations
                .join(","),
        );
    }
    for target in restored_saved_management.link_targets {
        add_desired_managed_peer_link_and_schedule(&state, &bus, target, "saved-peer-restore")
            .await;
    }
    for destination_hex in restored_saved_management.route_request_destinations {
        if let Some(destination_hex) = normalize_hex_32(destination_hex.as_str()) {
            if let Ok(destination) = parse_address_hash(destination_hex.as_str()) {
                transport.request_path(&destination, None, None).await;
                spawn_managed_peer_resolution(state.clone(), bus.clone(), destination_hex);
            }
        }
    }
    let initial_sync_status = from_sdk_sync_status(state.messaging.lock().await.sync_status());
    refresh_sync_status_snapshot(&state, &initial_sync_status);

    if let Ok(mut guard) = status.lock() {
        guard.running = true;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
    spawn_tcp_client_readiness_monitor(
        tcp_readiness_monitor_endpoints(tcp_client_endpoints.as_slice()),
        status.clone(),
        bus.clone(),
    );

    // Peer freshness/relay maintenance.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                refresh_peer_snapshot(&state).await;
                sync_auto_propagation_node(&state, &bus).await;
            }
        });
    }

    // Saved peer route maintenance. Passive announces are opportunistic; keep
    // asking the transport for managed peers so late or asymmetric mesh routes
    // can still be resolved without changing global node readiness.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_ROUTE_REFRESH_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                let destinations = {
                    let messaging = state.messaging.lock().await;
                    saved_peer_destinations_needing_route_refresh(&messaging)
                };
                if !destinations.is_empty() {
                    info!(
                        "[announce] saved peer route refresh destinations={}",
                        destinations.join(","),
                    );
                }
                for destination_hex in destinations {
                    if let Ok(destination) = parse_address_hash(destination_hex.as_str()) {
                        state.transport.request_path(&destination, None, None).await;
                    }
                    spawn_managed_peer_resolution(state.clone(), bus.clone(), destination_hex);
                }
            }
        });
    }

    // Keep desired peer links warm. Fresh REM-capable LXMF delivery announces
    // add desired link targets; explicit disconnect removes them.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_LINK_MAINTENANCE_INTERVAL);
            loop {
                interval.tick().await;
                maintain_managed_peer_links_once(&state, &bus).await;
            }
        });
    }

    // Propagation receive maintenance. Relay sends are store-and-forward, so
    // receivers must poll the selected relay even when nobody taps Sync.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(AUTO_PROPAGATION_SYNC_INTERVAL);
            loop {
                interval.tick().await;
                sync_auto_propagation_node(&state, &bus).await;
                let Some(relay_hex) = state.active_propagation_node_hex.lock().await.clone() else {
                    continue;
                };
                if state
                    .propagation_sync_inflight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    continue;
                }
                let requested_at_ms = now_ms();
                info!(
                    "[sync] automatic propagation sync scheduled relay={} limit={}",
                    relay_hex, AUTO_PROPAGATION_SYNC_LIMIT
                );
                tokio::spawn(run_propagation_sync_job(
                    state.clone(),
                    bus.clone(),
                    Some(AUTO_PROPAGATION_SYNC_LIMIT),
                    requested_at_ms,
                    relay_hex,
                ));
            }
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
                    .update_message_delivery_state(
                        message_id_hex.as_str(),
                        None,
                        Some(sdkmsg::TransportDeliveryState::TransportDelivered),
                        None,
                        Some("transport receipt".to_string()),
                        None,
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
        tokio::spawn(async move {
            for delay_secs in STARTUP_ANNOUNCE_DELAYS_SECS {
                if delay_secs > 0 {
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "startup-burst",
                )
                .await;
            }
        });
    }

    {
        let transport = transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = lxmf_destination.clone();
        let announce_capabilities = announce_capabilities.clone();
        let interval_secs = effective_announce_interval_seconds(config.announce_interval_seconds);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            interval.tick().await;
            loop {
                interval.tick().await;
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "periodic",
                )
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
                        let destination_kind =
                            announce_destination_kind_from_name_hash(event.name_hash.as_slice())
                                .to_string();
                        let interface_hex = hex::encode(event.interface);
                        let received_at_ms = now_ms();
                        let sdk_announce_record = lxmf_sdk_announce_record_from_raw(
                            destination_hex.clone(),
                            identity_hex.clone(),
                            destination_kind.clone(),
                            event.app_data.as_slice(),
                            event.hops,
                            interface_hex.clone(),
                            received_at_ms,
                        );
                        let announce_record =
                            from_lxmf_sdk_announce_record(sdk_announce_record.clone());
                        let announce_class = announce_record.announce_class;
                        let app_data = announce_record.app_data.clone();
                        let is_rem_capable_lxmf_delivery = destination_kind
                            == DESTINATION_KIND_LXMF_DELIVERY
                            && app_data_has_rem_peer_capabilities(&app_data);
                        let display_name = announce_record.display_name.clone();
                        state
                            .messaging
                            .lock()
                            .await
                            .record_announce(to_compat_announce_record(&sdk_announce_record));
                        if let Err(err) = state.app_state.upsert_announce(&announce_record) {
                            bus.emit(NodeEvent::Error {
                                code: "IoError".to_string(),
                                message: format!(
                                    "failed to persist announce destination={} reason={}",
                                    destination_hex, err
                                ),
                            });
                        }
                        sdk.record_announce_received(
                            &destination_hex,
                            &identity_hex,
                            &destination_kind,
                            &announce_record.app_data,
                            event.hops,
                            &interface_hex,
                        );
                        bus.emit(NodeEvent::AnnounceReceived {
                            destination_hex: destination_hex.clone(),
                            identity_hex: identity_hex.clone(),
                            destination_kind: destination_kind.clone(),
                            announce_class,
                            app_data,
                            display_name: display_name.clone(),
                            hops: event.hops,
                            interface_hex,
                            received_at_ms,
                        });
                        if let Some(message) = operator_announce_message(
                            announce_class,
                            is_rem_capable_lxmf_delivery,
                            display_name.as_deref(),
                            destination_hex.as_str(),
                            identity_hex.as_str(),
                            event.hops,
                        ) {
                            emit_operational_notice(&bus, LogLevel::Info {}, message);
                        }
                        if destination_kind == DESTINATION_KIND_APP {
                            let lxmf_destination_hex = SingleOutputDestination::new(
                                desc.identity,
                                DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
                            )
                            .desc
                            .address_hash
                            .to_hex_string();
                            state.messaging.lock().await.record_resolution_result(
                                destination_hex.as_str(),
                                identity_hex.as_str(),
                                lxmf_destination_hex.as_str(),
                                received_at_ms,
                            );
                            emit_peer_changed(&state, &bus, &destination_hex).await;
                            emit_peer_resolved_for_destination(&state, &bus, &destination_hex)
                                .await;
                            spawn_passive_peer_resolution(
                                state.clone(),
                                bus.clone(),
                                destination_hex.clone(),
                            );
                        } else if destination_kind == DESTINATION_KIND_LXMF_DELIVERY {
                            let app_destination_hex = SingleOutputDestination::new(
                                desc.identity,
                                DestinationName::new(
                                    APP_DESTINATION_NAME.0,
                                    APP_DESTINATION_NAME.1,
                                ),
                            )
                            .desc
                            .address_hash
                            .to_hex_string();
                            debug!(
                                "[announce] derived app route from lxmf_delivery app={} lxmf={} identity={} display={} hops={}",
                                app_destination_hex,
                                destination_hex,
                                identity_hex,
                                display_name.as_deref().unwrap_or(""),
                                event.hops,
                            );
                            state.messaging.lock().await.record_resolution_result(
                                app_destination_hex.as_str(),
                                identity_hex.as_str(),
                                destination_hex.as_str(),
                                received_at_ms,
                            );
                            emit_peer_changed(&state, &bus, &destination_hex).await;
                            emit_peer_resolved_for_destination(&state, &bus, &destination_hex)
                                .await;
                            let ignored = peer_destinations_are_ignored(
                                &state,
                                [destination_hex.clone(), app_destination_hex.clone()],
                            )
                            .await;
                            if is_rem_capable_lxmf_delivery && !ignored {
                                add_desired_managed_peer_link_and_schedule(
                                    &state,
                                    &bus,
                                    ManagedPeerLinkTarget {
                                        destination_hex: destination_hex.clone(),
                                        kind: ManagedPeerLinkKind::LxmfDelivery,
                                    },
                                    "rem-lxmf-announce",
                                )
                                .await;
                            } else if is_rem_capable_lxmf_delivery {
                                debug!(
                                    "[link][maintain] destination={} status=ignored reason=rem-lxmf-announce",
                                    destination_hex,
                                );
                            }
                        }
                        let pruned_saved_destinations = {
                            let mut messaging = state.messaging.lock().await;
                            messaging.prune_saved_destinations_with_non_rem_announce_evidence()
                        };
                        if !pruned_saved_destinations.is_empty() {
                            info!(
                                "[peers] pruned saved peers with non-rem lxmf announce evidence destinations={}",
                                pruned_saved_destinations.join(",")
                            );
                            cleanup_removed_saved_destinations(
                                &state,
                                pruned_saved_destinations.as_slice(),
                            )
                            .await;
                            for destination in &pruned_saved_destinations {
                                emit_peer_changed(&state, &bus, destination).await;
                            }
                        }
                        sync_auto_propagation_node(&state, &bus).await;
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
                        let expected_lxmf = {
                            let local_lxmf_destination = state
                                .lxmf_destination
                                .lock()
                                .await
                                .desc
                                .address_hash
                                .to_hex_string();
                            destination_hex == local_lxmf_destination
                        };
                        info!(
                            "[lxmf][rx] data_event destination={} bytes={}",
                            destination_hex,
                            event.data.as_slice().len(),
                        );
                        emit_received_payload(
                            &state,
                            &bus,
                            &sdk,
                            destination_hex,
                            event.data.as_slice().to_vec(),
                            None,
                            expected_lxmf,
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
                                true,
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
                        ResourceEventKind::OutboundFailed => {
                            info!(
                                "[lxmf][events] resource outbound failed link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::OutboundCancelled => {
                            info!(
                                "[lxmf][events] resource outbound cancelled link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::InboundFailed(failure) => {
                            warn!(
                                "[lxmf][events] resource inbound failed link_id={} hash={} reason={} received_parts={} total_parts={} received_bytes={} total_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                                failure.reason,
                                failure.progress.received_parts,
                                failure.progress.total_parts,
                                failure.progress.received_bytes,
                                failure.progress.total_bytes,
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
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = now_ms();
                let mut expired = Vec::<PendingLxmfDelivery>::new();
                {
                    let mut guard = state.pending_lxmf_deliveries.lock().await;
                    let expired_keys = guard
                        .iter()
                        .filter(|(_, pending)| pending_ack_timeout_elapsed(pending, now))
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for key in expired_keys {
                        if let Some(pending) = guard.remove(&key) {
                            expired.push(pending);
                        }
                    }
                }
                for pending in expired {
                    match retry_pending_ack_timeout_via_propagation(&state, &bus, &pending).await {
                        Ok(true) => continue,
                        Ok(false) => {
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                "ack timeout",
                            );
                        }
                        Err(err) => {
                            let detail = format!("ack timeout; propagation retry failed: {err}");
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                detail.as_str(),
                            );
                        }
                    }
                }
            }
        });
    }

    // Cleanup stale buffered acknowledgements and receipt tracking.
    {
        let pending_lxmf_acknowledgements = pending_lxmf_acknowledgements.clone();
        let receipt_message_ids = receipt_message_ids.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let now = now_ms();
                let pruned_acks = {
                    let mut guard = pending_lxmf_acknowledgements.lock().await;
                    prune_expired_buffered_acknowledgements(&mut guard, now)
                };
                let pruned_receipts = if let Ok(mut guard) = receipt_message_ids.lock() {
                    prune_expired_receipt_tracking(&mut guard, now)
                } else {
                    0
                };
                if pruned_acks > 0 || pruned_receipts > 0 {
                    debug!(
                        "[runtime] pruned stale state buffered_acks={} receipt_tracking={}",
                        pruned_acks, pruned_receipts,
                    );
                }
            }
        });
    }

    // Link events.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let connected_peers = connected_peers.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.out_link_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.address_hash);
                        match event.event {
                            LinkEvent::Activated => {
                                debug!(
                                    "[link][event] kind=activated destination={} link_id={}",
                                    destination_hex,
                                    address_hash_to_hex(&event.id),
                                );
                                connected_peers.lock().await.insert(event.address_hash);
                                record_peer_link_state(&state, &bus, &destination_hex, true).await;
                            }
                            LinkEvent::Closed => {
                                debug!(
                                    "[link][event] kind=closed destination={} link_id={}",
                                    destination_hex,
                                    address_hash_to_hex(&event.id),
                                );
                                state.out_links.lock().await.remove(&event.address_hash);
                                connected_peers.lock().await.remove(&event.address_hash);
                                record_peer_link_state(&state, &bus, &destination_hex, false).await;
                                mark_peer_direct_delivery_unhealthy(
                                    &state,
                                    destination_hex.as_str(),
                                    None,
                                )
                                .await;
                                match state
                                    .managed_peer_links
                                    .begin_reconnect(destination_hex.as_str())
                                    .await
                                {
                                    ManagedPeerReconnectStart::Started(target) => {
                                        info!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-scheduled",
                                            destination_hex,
                                        );
                                        spawn_managed_peer_link_reconnect(
                                            state.clone(),
                                            bus.clone(),
                                            target,
                                        );
                                    }
                                    ManagedPeerReconnectStart::Backoff {
                                        next_retry_at_ms,
                                        last_failure_reason,
                                    } => {
                                        debug!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-deferred detail=backoff next_retry_at_ms={} last_failure={}",
                                            destination_hex,
                                            next_retry_at_ms,
                                            last_failure_reason.as_deref().unwrap_or("-"),
                                        );
                                    }
                                    ManagedPeerReconnectStart::AlreadyReconnecting => {
                                        debug!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-deferred detail=reconnecting",
                                            destination_hex,
                                        );
                                    }
                                    ManagedPeerReconnectStart::NotDesired => {}
                                }
                            }
                            LinkEvent::Data(_) => {}
                            LinkEvent::PeerIdentified(_) => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Optional periodic hub refresh.
    if matches!(
        config.hub_mode,
        HubMode::SemiAutonomous {} | HubMode::Connected {}
    ) && config.hub_refresh_interval_seconds > 0
    {
        let bus = bus.clone();
        let config = config.clone();
        let state = state.clone();
        let interval_secs = config.hub_refresh_interval_seconds;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                if let Ok(snapshot) = refresh_hub_directory_lxmf(&config, &state).await {
                    publish_hub_directory_snapshot(&state, &bus, snapshot).await;
                }
            }
        });
    }

    loop {
        let cmd = tokio::select! {
            biased;
            Some(cmd) = priority_cmd_rx.recv() => cmd,
            Some(cmd) = cmd_rx.recv() => cmd,
            else => break,
        };
        match cmd {
            Command::Stop { resp } => {
                if let Ok(mut guard) = status.lock() {
                    guard.running = false;
                    guard.refresh_readiness();
                    bus.emit(NodeEvent::StatusChanged {
                        status: guard.clone(),
                    });
                }
                let _ = resp.send(Ok(()));
                break;
            }
            Command::AnnounceNow {} => {
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "manual",
                )
                .await;
            }
            Command::SetLogLevel { level } => {
                crate::logger::NodeLogger::global().set_level(level);
            }
            Command::RequestPeerIdentity {
                destination_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                tokio::spawn(async move {
                    let result = resolve_peer_route(&state, &bus, destination_hex.as_str()).await;
                    if let Err(err) = &result {
                        state.messaging.lock().await.record_resolution_error(
                            destination_hex.as_str(),
                            Some(err.to_string()),
                        );
                        emit_peer_changed(&state, &bus, destination_hex.as_str()).await;
                    }
                    let _ = resp.send(result);
                });
            }
            Command::SetAnnounceCapabilities {
                capability_string,
                resp,
            } => {
                *announce_capabilities.lock().await = capability_string;
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "capabilities-updated",
                )
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
                    let saved_peer =
                        persist_selected_peer_destination(&state, &bus, destination_hex.as_str())
                            .await?;
                    clear_ignored_peer_destinations(&state, std::slice::from_ref(&destination_hex))
                        .await;
                    emit_peer_changed(&state, &bus, saved_peer.destination_hex.as_str()).await;
                    state.sdk.record_peer_changed(
                        saved_peer.destination_hex.as_str(),
                        PeerState::Connecting {},
                        None,
                    );
                    resolve_peer_route(&state, &bus, &destination_hex).await?;
                    let target =
                        match register_desired_managed_peer_link(&state, &destination_hex).await {
                            Some(target) => target,
                            None => {
                                let target = ManagedPeerLinkTarget {
                                    destination_hex: address_hash_to_hex(&dest),
                                    kind: ManagedPeerLinkKind::App,
                                };
                                state.managed_peer_links.add_desired(target.clone()).await;
                                target
                            }
                        };
                    let target_destination = parse_address_hash(target.destination_hex.as_str())?;
                    let desc = ensure_destination_desc(
                        &state,
                        target_destination,
                        Some(target.kind.destination_name()),
                    )
                    .await?;
                    let _link = ensure_output_link(&state, desc).await?;
                    record_peer_link_state(&state, &bus, target.destination_hex.as_str(), true)
                        .await;
                    Ok::<(), NodeError>(())
                }
                .await;
                if let Err(err) = &result {
                    state.messaging.lock().await.record_resolution_error(
                        destination_hex_copy.as_str(),
                        Some(err.to_string()),
                    );
                    emit_peer_changed(&state, &bus, &destination_hex_copy).await;
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
                    let mut destinations = vec![destination_hex.clone()];
                    if let Some(peer) = peer_for_any_destination_hex(&state, &destination_hex).await
                    {
                        destinations
                            .extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
                    }
                    destinations.sort();
                    destinations.dedup();
                    {
                        let now = now_ms();
                        let mut messaging = state.messaging.lock().await;
                        for destination in &destinations {
                            messaging.set_peer_active_link(destination.as_str(), false, now);
                        }
                    }
                    state
                        .direct_delivery_health
                        .clear(destinations.iter().map(String::as_str));
                    state
                        .managed_peer_links
                        .remove_desired(destinations.iter().map(String::as_str))
                        .await;
                    mark_peer_destinations_ignored(&state, destinations.as_slice()).await;
                    connected_peers.lock().await.remove(&dest);
                    for destination in &destinations {
                        if let Ok(destination) = parse_address_hash(destination.as_str()) {
                            connected_peers.lock().await.remove(&destination);
                            if let Some(link) = out_links.lock().await.remove(&destination) {
                                link.lock().await.close();
                            }
                        }
                    }
                    emit_peer_changed(&state, &bus, &destination_hex).await;
                    state.sdk.record_peer_changed(
                        &address_hash_to_hex(&dest),
                        PeerState::Disconnected {},
                        None,
                    );
                    sync_auto_propagation_node(&state, &bus).await;
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SetSavedPeers { peers, resp } => {
                let result = apply_saved_peer_management_projection(&state, &bus, &peers).await;
                let _ = resp.send(result);
            }
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                let transport = transport.clone();
                let metadata = fields_bytes
                    .as_deref()
                    .and_then(parse_mission_sync_metadata);
                let send_task_class = SendTaskClass::from_lxmf_request(
                    fields_bytes.is_some(),
                    metadata.as_ref(),
                    &send_mode,
                );
                log_send_task(
                    send_task_class,
                    format!(
                        "[lxmf][queue] enqueued {} send destination={} mode={:?} has_fields={}",
                        send_task_class.label(),
                        destination_hex,
                        send_mode,
                        fields_bytes.is_some(),
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let lxmf_report = if fields_bytes.is_some() {
                            Some(
                                send_lxmf_with_delivery_policy(
                                    &state,
                                    &bus,
                                    &destination_hex,
                                    &bytes,
                                    None,
                                    fields_bytes.clone(),
                                    metadata.clone(),
                                    send_mode,
                                    send_task_class,
                                )
                                .await?,
                            )
                        } else {
                            None
                        };
                        let outcome = if let Some(report) = lxmf_report.as_ref() {
                            report.outcome
                        } else {
                            log_send_task(
                                SendTaskClass::General,
                                format!(
                                    "[lxmf][queue] waiting for general send slot destination={} mode=transport-bytes",
                                    destination_hex,
                                ),
                            );
                            let _permit = acquire_send_task_permit(
                                &state.send_task_permits,
                                SendTaskClass::General,
                            )
                            .await?;
                            log_send_task(
                                SendTaskClass::General,
                                format!(
                                    "[lxmf][queue] acquired general send slot destination={} mode=transport-bytes",
                                    destination_hex,
                                ),
                            );
                            let dest = parse_address_hash(&destination_hex)?;
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
                                if metadata.is_mission_related() {
                                    info!(
                                        "[lxmf][mission] outbound kind={} name={} destination={} message_id={} event_uid={} mission_uid={} correlation={}",
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

                            let resend = build_pending_lxmf_resend(
                                report,
                                destination_hex.as_str(),
                                bytes.as_slice(),
                                None,
                                fields_bytes.clone(),
                                metadata.clone(),
                                send_mode,
                                send_task_class,
                            );
                            if let Some(registered) =
                                register_pending_lxmf_delivery(&state, report, resend, None).await
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
                                        pending,
                                        lxmf_delivery_status_for(report),
                                        None,
                                    );
                                    info!(
                                        "[lxmf][mission] sent message_id={} destination={} command={} correlation={}",
                                        pending.message_id_hex,
                                        pending.destination_hex,
                                        pending.command_type.as_deref().unwrap_or("-"),
                                        pending.correlation_id.as_deref().unwrap_or("-"),
                                    );
                                    if let Some(buffered_ack) = registered.buffered_ack {
                                        acknowledge_pending_with_buffered_ack(
                                            &state,
                                            &bus,
                                            pending,
                                            buffered_ack,
                                        )
                                        .await;
                                    }
                                } else {
                                    let failure_detail = format!("{mapped:?}");
                                    {
                                        let tracking_key = pending_tracking_key(pending);
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
                                        pending,
                                        LxmfDeliveryStatus::Failed {},
                                        Some(failure_detail.clone()),
                                    );
                                    info!(
                                        "[lxmf][mission] failed message_id={} destination={} command={} correlation={} outcome={:?}",
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
                    if let Err(err) = &result {
                        if !should_emit_global_send_bytes_error(send_task_class) {
                            info!(
                                "[lxmf][mission] propagation send exhausted destination={} reason={}",
                                destination_hex, err
                            );
                            let _ = resp.send(result);
                            return;
                        }
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "send_bytes failed destination={} reason={}",
                                destination_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
            }
            Command::SendLxmf { request, resp } => {
                let state = state.clone();
                let bus = bus.clone();
                let receipt_message_ids = receipt_message_ids.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general send destination={} mode={:?} has_fields=false",
                        request.destination_hex,
                        request.send_mode,
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let body_bytes = request.body_utf8.as_bytes().to_vec();
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            request.destination_hex.as_str(),
                            body_bytes.as_slice(),
                            request.title.clone(),
                            None,
                            None,
                            request.send_mode,
                            SendTaskClass::General,
                        )
                        .await?;
                        let method = match (report.method, report.representation) {
                            (LxmfDeliveryMethod::Propagated {}, _) => MessageMethod::Propagated {},
                            (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                MessageMethod::Opportunistic {}
                            }
                            (_, LxmfDeliveryRepresentation::Resource {}) => {
                                MessageMethod::Resource {}
                            }
                            _ => MessageMethod::Direct {},
                        };
                        let state_value = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else if matches!(
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
                        let conversation_id =
                            conversation_id_for(report.resolved_destination_hex.as_str());
                        let record = MessageRecord {
                            message_id_hex: report.message_id_hex.clone(),
                            conversation_id,
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(request.destination_hex.clone()),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: request.title.clone(),
                            body_utf8: request.body_utf8.clone(),
                            method,
                            state: state_value,
                            transport_state: transport_state_for_message_state(state_value),
                            application_ack_state: if matches!(state_value, MessageState::Failed {})
                            {
                                ApplicationAckState::Failed {}
                            } else {
                                ApplicationAckState::Waiting {}
                            },
                            detail: detail.clone(),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, record, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: to_sdk_send_request(&request),
                                message_id_hex: report.message_id_hex.clone(),
                            },
                        );
                        if let Some(receipt_hash_hex) = report.receipt_hash_hex.as_ref() {
                            if let Ok(mut guard) = receipt_message_ids.lock() {
                                guard.insert(
                                    receipt_hash_hex.clone(),
                                    ReceiptMessageTracking {
                                        message_id_hex: report.message_id_hex.clone(),
                                        recorded_at_ms: now_ms(),
                                    },
                                );
                            }
                        }
                        Ok::<String, NodeError>(report.message_id_hex)
                    }
                    .await;
                    if let Err(err) = &result {
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "send_lxmf failed destination={} reason={}",
                                request.destination_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
            }
            Command::RetryLxmf {
                message_id_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general retry message_id={}",
                        message_id_hex,
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let outbound = state
                            .messaging
                            .lock()
                            .await
                            .outbound(message_id_hex.as_str())
                            .ok_or(NodeError::InvalidConfig {})?;
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            outbound.request.destination_hex.as_str(),
                            outbound.request.body_utf8.as_bytes(),
                            outbound.request.title.clone(),
                            None,
                            None,
                            match outbound.request.effective_send_mode() {
                                sdkmsg::SendMode::Auto => SendMode::Auto {},
                                sdkmsg::SendMode::DirectOnly => SendMode::DirectOnly {},
                                sdkmsg::SendMode::PropagationOnly => SendMode::PropagationOnly {},
                            },
                            SendTaskClass::General,
                        )
                        .await?;
                        let retried_state = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else {
                            MessageState::SentDirect {}
                        };
                        let retried = MessageRecord {
                            message_id_hex: outbound.message_id_hex.clone(),
                            conversation_id: conversation_id_for(
                                report.resolved_destination_hex.as_str(),
                            ),
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(
                                outbound.request.destination_hex.clone(),
                            ),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: outbound.request.title.clone(),
                            body_utf8: outbound.request.body_utf8.clone(),
                            method: match (report.method, report.representation) {
                                (LxmfDeliveryMethod::Propagated {}, _) => {
                                    MessageMethod::Propagated {}
                                }
                                (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                    MessageMethod::Opportunistic {}
                                }
                                (_, LxmfDeliveryRepresentation::Resource {}) => {
                                    MessageMethod::Resource {}
                                }
                                _ => MessageMethod::Direct {},
                            },
                            state: retried_state,
                            transport_state: transport_state_for_message_state(retried_state),
                            application_ack_state: ApplicationAckState::Waiting {},
                            detail: Some(format!("retry of {}", outbound.message_id_hex)),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, retried, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: outbound.request,
                                message_id_hex: outbound.message_id_hex.clone(),
                            },
                        );
                        Ok::<(), NodeError>(())
                    }
                    .await;
                    if let Err(err) = &result {
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "retry_lxmf failed message_id={} reason={}",
                                message_id_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
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
                        .update_message_delivery_state(
                            message_id_hex.as_str(),
                            Some(sdkmsg::MessageState::Cancelled),
                            Some(sdkmsg::TransportDeliveryState::Cancelled),
                            Some(sdkmsg::ApplicationAckState::Failed),
                            Some("cancelled locally".to_string()),
                            None,
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
                *state.active_propagation_node_hex.lock().await = destination_hex.clone();
                let status_update = from_sdk_sync_status(
                    state
                        .messaging
                        .lock()
                        .await
                        .set_active_propagation_node(destination_hex),
                );
                if refresh_sync_status_snapshot(&state, &status_update) {
                    bus.emit(NodeEvent::SyncUpdated {
                        status: status_update,
                    });
                }
                let _ = resp.send(Ok(()));
            }
            Command::RequestLxmfSync { limit, resp } => {
                let requested_at_ms = now_ms();
                if state.propagation_sync_inflight.load(Ordering::Acquire) {
                    info!("[sync] propagation sync request ignored reason=inflight");
                    let _ = resp.send(Ok(()));
                    continue;
                }
                emit_sync_status_update(
                    &state,
                    &bus,
                    sdkmsg::SyncPhase::PathRequested,
                    requested_at_ms,
                    0,
                    Some("waiting for propagation relay selection".to_string()),
                    false,
                )
                .await;
                let Some(relay_hex) = wait_for_active_propagation_relay(&state, &bus).await else {
                    let detail = format!(
                        "no active propagation relay selected after {}s",
                        PROPAGATION_SYNC_RELAY_SELECTION_WAIT.as_secs()
                    );
                    emit_sync_status_update(
                        &state,
                        &bus,
                        sdkmsg::SyncPhase::Failed,
                        requested_at_ms,
                        0,
                        Some(detail.clone()),
                        true,
                    )
                    .await;
                    info!("[sync] propagation sync failed reason={detail}");
                    let _ = resp.send(Err(NodeError::InvalidConfig {}));
                    continue;
                };
                if state
                    .propagation_sync_inflight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    info!("[sync] propagation sync request ignored reason=inflight");
                    let _ = resp.send(Ok(()));
                    continue;
                }
                info!(
                    "[sync] propagation sync scheduled relay={} limit={}",
                    relay_hex,
                    limit
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string())
                );
                tokio::spawn(run_propagation_sync_job(
                    state.clone(),
                    bus.clone(),
                    limit,
                    requested_at_ms,
                    relay_hex,
                ));
                let _ = resp.send(Ok(()));
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
                let _ = resp.send(Ok(message_records_snapshot(
                    &state,
                    conversation_id.as_deref(),
                )
                .await));
            }
            Command::DeleteConversation {
                conversation_id,
                resp,
            } => {
                let _ = resp.send(
                    delete_conversation_records(&state, &bus, conversation_id.as_str()).await,
                );
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
                if let Err(err) = &result {
                    bus.emit(NodeEvent::Error {
                        code: node_error_code(err).to_string(),
                        message: format!("broadcast_bytes failed reason={}", err),
                    });
                }
                let _ = resp.send(result);
            }
            Command::RefreshHubDirectory { resp } => {
                let state = state.clone();
                let bus = bus.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    let result = match config.hub_mode {
                        HubMode::Autonomous {} => Err(NodeError::InvalidConfig {}),
                        HubMode::SemiAutonomous {} | HubMode::Connected {} => {
                            refresh_hub_directory_lxmf(&config, &state).await
                        }
                    }
                    .map(|snapshot| async {
                        publish_hub_directory_snapshot(&state, &bus, snapshot).await;
                    });
                    let _ = resp.send(match result {
                        Ok(publish) => {
                            publish.await;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    });
                });
            }
        }
    }

    let _ = state.sdk.shutdown().await;
    state.projection_journal.flush_now().await;

    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        guard.refresh_readiness();
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
    use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_EVENT, FIELD_RESULTS};
    use tokio::sync::oneshot;

    #[test]
    fn tcp_endpoint_connect_addr_accepts_plain_and_tcp_urls() {
        assert_eq!(
            tcp_endpoint_connect_addr("rns.beleth.net:4242"),
            "rns.beleth.net:4242"
        );
        assert_eq!(
            tcp_endpoint_connect_addr(" tcp://127.0.0.1:4242 "),
            "127.0.0.1:4242"
        );
        assert_eq!(tcp_endpoint_connect_addr(""), "");
    }

    #[test]
    fn configured_tcp_client_endpoints_trim_strip_and_deduplicate() {
        let endpoints = configured_tcp_client_endpoints(&[
            " tcp://rns.beleth.net:4242 ".to_string(),
            "rns.beleth.net:4242".to_string(),
            " ".to_string(),
            "dfw.us.g00n.cloud:6969".to_string(),
        ]);

        assert_eq!(
            endpoints,
            vec![
                "rns.beleth.net:4242".to_string(),
                "dfw.us.g00n.cloud:6969".to_string(),
            ]
        );
    }

    #[test]
    fn tcp_readiness_monitor_skips_loopback_test_relays() {
        let endpoints = tcp_readiness_monitor_endpoints(&[
            "127.0.0.1:4242".to_string(),
            "localhost:4242".to_string(),
            "[::1]:4242".to_string(),
            "rns.beleth.net:4242".to_string(),
        ]);

        assert_eq!(endpoints, vec!["rns.beleth.net:4242".to_string()]);
    }

    #[tokio::test]
    async fn active_interface_registry_removes_stopped_tcp_endpoint_entries() {
        let registry: ActiveInterfaceRegistry = Arc::new(TokioMutex::new(HashMap::from([
            (
                AddressHash::new_from_slice(&[1u8; 16]),
                new_interface_status(
                    AddressHash::new_from_slice(&[1u8; 16]),
                    "rns.beleth.net:4242".to_string(),
                    "connected",
                ),
            ),
            (
                AddressHash::new_from_slice(&[2u8; 16]),
                new_interface_status(
                    AddressHash::new_from_slice(&[2u8; 16]),
                    "rns.beleth.net:4242".to_string(),
                    "connected",
                ),
            ),
            (
                AddressHash::new_from_slice(&[3u8; 16]),
                new_interface_status(
                    AddressHash::new_from_slice(&[3u8; 16]),
                    "dfw.us.g00n.cloud:6969".to_string(),
                    "connected",
                ),
            ),
        ])));
        let status = Arc::new(Mutex::new(NodeStatus {
            readiness: crate::types::RuntimeReadinessSnapshot::default(),
            running: true,
            name: "test".to_string(),
            identity_hex: String::new(),
            app_destination_hex: String::new(),
            lxmf_destination_hex: String::new(),
            interfaces: Vec::new(),
        }));
        let bus = EventBus::new();
        let rx = bus.subscribe();

        unregister_tcp_client_endpoint(&registry, &status, &bus, "rns.beleth.net:4242").await;

        let guard = registry.lock().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(
            guard
                .get(&AddressHash::new_from_slice(&[3u8; 16]))
                .map(|status| status.label.as_str()),
            Some("dfw.us.g00n.cloud:6969"),
        );
        assert!(rx
            .try_iter()
            .any(|event| matches!(event, NodeEvent::InterfaceStatusChanged { status } if status.state == "disconnected")));
    }

    #[test]
    fn active_relay_transport_requires_non_rnode_ble_interface() {
        let rnode_only = HashMap::from([(
            AddressHash::new_from_slice(&[1u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[1u8; 16]),
                "rnode-ble:RNode 4339".to_string(),
                "connected",
            ),
        )]);
        assert!(!active_interfaces_include_relay_transport(&rnode_only));
        assert!(active_interfaces_are_rnode_ble_only(&rnode_only));
        assert!(active_interface_is_rnode_ble(
            &rnode_only,
            &AddressHash::new_from_slice(&[1u8; 16]),
        ));
        assert_eq!(link_connect_timeout(true), RNODE_BLE_LINK_CONNECT_TIMEOUT);

        let with_tcp = HashMap::from([
            (
                AddressHash::new_from_slice(&[1u8; 16]),
                new_interface_status(
                    AddressHash::new_from_slice(&[1u8; 16]),
                    "rnode-ble:RNode 4339".to_string(),
                    "connected",
                ),
            ),
            (
                AddressHash::new_from_slice(&[2u8; 16]),
                new_interface_status(
                    AddressHash::new_from_slice(&[2u8; 16]),
                    "rns.beleth.net:4242".to_string(),
                    "connected",
                ),
            ),
        ]);
        assert!(active_interfaces_include_relay_transport(&with_tcp));
        assert!(!active_interfaces_are_rnode_ble_only(&with_tcp));
        assert!(active_interface_is_rnode_ble(
            &with_tcp,
            &AddressHash::new_from_slice(&[1u8; 16]),
        ));
        assert!(!active_interface_is_rnode_ble(
            &with_tcp,
            &AddressHash::new_from_slice(&[2u8; 16]),
        ));
        assert_eq!(link_connect_timeout(false), DEFAULT_LINK_CONNECT_TIMEOUT);
        assert_eq!(link_connect_timeout(true), RNODE_BLE_LINK_CONNECT_TIMEOUT);

        let no_interfaces = HashMap::new();
        assert!(!active_interfaces_are_rnode_ble_only(&no_interfaces));
        assert!(!active_interface_is_rnode_ble(
            &no_interfaces,
            &AddressHash::new_from_slice(&[1u8; 16]),
        ));
    }

    #[test]
    fn direct_attempts_force_direct_sdk_mode() {
        assert_eq!(
            direct_attempt_send_mode(SendMode::Auto {}),
            SendMode::DirectOnly {}
        );
        assert_eq!(
            direct_attempt_send_mode(SendMode::DirectOnly {}),
            SendMode::DirectOnly {}
        );
        assert_eq!(
            direct_attempt_send_mode(SendMode::PropagationOnly {}),
            SendMode::PropagationOnly {}
        );
    }

    #[test]
    fn tcp_data_path_unavailable_message_is_readiness_classified() {
        let message = tcp_data_path_unavailable_message(&["rns.beleth.net:4242".to_string()]);

        assert!(message.contains("transport startup failed"));
        assert!(message.contains("no reachable Reticulum TCP interface"));
    }

    #[test]
    fn lxmf_delivery_announce_mapping_uses_lxmf_sdk_normalization() {
        let raw_app_data =
            encode_delivery_display_name_app_data("Alice Router").expect("encoded app data");
        let sdk_record = lxmf_sdk_announce_record_from_raw(
            "cccccccccccccccccccccccccccccccc",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            DESTINATION_KIND_LXMF_DELIVERY,
            raw_app_data.as_slice(),
            2,
            "dddddddddddddddddddddddddddddddd",
            42,
        );

        assert_eq!(sdk_record.app_data, hex::encode(raw_app_data.as_slice()));
        assert_eq!(sdk_record.display_name.as_deref(), Some("Alice Router"));

        let announce = from_lxmf_sdk_announce_record(sdk_record.clone());
        assert!(matches!(
            announce.announce_class,
            AnnounceClass::LxmfDelivery {}
        ));
        assert_eq!(announce.display_name.as_deref(), Some("Alice Router"));

        let compat = to_compat_announce_record(&sdk_record);
        assert_eq!(compat.display_name.as_deref(), Some("Alice Router"));
        assert_eq!(compat.app_data, sdk_record.app_data);
    }

    #[test]
    fn app_announce_mapping_keeps_rem_capability_policy() {
        let sdk_record = lxmf_sdk_announce_record_from_raw(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            DESTINATION_KIND_APP,
            b"R3AKT;EMergencyMessages;Telemetry;name=Bravo+Team",
            1,
            "dddddddddddddddddddddddddddddddd",
            100,
        );

        assert!(sdk_record.display_name.is_none());

        let announce = from_lxmf_sdk_announce_record(sdk_record);
        assert!(matches!(announce.announce_class, AnnounceClass::PeerApp {}));
        assert_eq!(announce.display_name.as_deref(), Some("Bravo Team"));
    }

    #[test]
    fn propagation_and_malformed_announces_keep_generic_sdk_normalization() {
        let sdk_record = lxmf_sdk_announce_record_from_raw(
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            DESTINATION_KIND_LXMF_PROPAGATION,
            &[0xff, 0xfe, 0x00],
            3,
            "dddddddddddddddddddddddddddddddd",
            200,
        );

        assert_eq!(sdk_record.app_data, "fffe00");
        assert!(sdk_record.display_name.is_none());

        let announce = from_lxmf_sdk_announce_record(sdk_record);
        assert!(matches!(
            announce.announce_class,
            AnnounceClass::PropagationNode {}
        ));
        assert_eq!(announce.app_data, "fffe00");
        assert!(announce.display_name.is_none());

        let malformed_tokens = parse_announce_metadata("fffe00").capability_tokens;
        assert!(malformed_tokens.is_empty());
    }

    #[test]
    fn announce_metadata_accepts_text_and_msgpack_layouts() {
        let text_metadata = parse_announce_metadata("R3AKT;EMergencyMessages;name=Legacy+Team");
        let text_name = text_metadata.display_name;
        let text_tokens = text_metadata.capability_tokens;
        assert_eq!(text_name.as_deref(), Some("Legacy Team"));
        assert!(text_tokens.iter().any(|token| token == "r3akt"));
        assert!(text_tokens.iter().any(|token| token == "emergencymessages"));

        let payload = MsgPackValue::Array(vec![
            MsgPackValue::from("Msgpack Team"),
            MsgPackValue::Map(vec![(
                MsgPackValue::from("caps"),
                MsgPackValue::Array(vec![
                    MsgPackValue::from("R3AKT"),
                    MsgPackValue::from("EMergencyMessages"),
                ]),
            )]),
        ]);
        let encoded = rmp_serde::to_vec(&payload).expect("msgpack");
        let msgpack_hex = hex::encode(encoded);
        let msgpack_metadata = parse_announce_metadata(msgpack_hex.as_str());
        let msgpack_name = msgpack_metadata.display_name;
        let msgpack_tokens = msgpack_metadata.capability_tokens;

        assert_eq!(msgpack_name.as_deref(), Some("Msgpack Team"));
        assert!(msgpack_tokens.iter().any(|token| token == "r3akt"));
        assert!(msgpack_tokens
            .iter()
            .any(|token| token == "emergencymessages"));
        assert!(matches!(
            classify_announce(DESTINATION_KIND_APP, msgpack_hex.as_str()),
            AnnounceClass::PeerApp {}
        ));
    }

    #[test]
    fn successful_link_marks_canonical_saved_peer_active() {
        let mut messaging = sdkmsg::MessagingStore::default();
        let now = now_ms();
        let app_destination_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let identity_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let lxmf_destination_hex = "cccccccccccccccccccccccccccccccc";

        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: app_destination_hex.to_string(),
            identity_hex: identity_hex.to_string(),
            destination_kind: "app".to_string(),
            app_data: "R3AKT,EMergencyMessages,Telemetry;name=Peer".to_string(),
            display_name: Some("Peer".to_string()),
            hops: 1,
            interface_hex: "dddddddddddddddddddddddddddddddd".to_string(),
            received_at_ms: now,
        });
        messaging.record_resolution_result(
            app_destination_hex,
            identity_hex,
            lxmf_destination_hex,
            now,
        );
        messaging.mark_peer_saved(app_destination_hex, true);

        mark_peer_active_after_successful_link(
            &mut messaging,
            lxmf_destination_hex,
            app_destination_hex,
            now,
        );

        let peer = messaging
            .list_peers()
            .into_iter()
            .find(|peer| peer.destination_hex == app_destination_hex)
            .expect("saved app peer should be listed");
        assert!(peer.active_link);
        assert_eq!(peer.state, sdkmsg::PeerState::Connected);
    }

    #[test]
    fn sos_field_telemetry_promotes_to_regular_telemetry_position() {
        let telemetry = SosDeviceTelemetryRecord {
            lat: Some(43.967_349),
            lon: Some(-66.126_159),
            alt: Some(12.0),
            speed: Some(1.4),
            course: Some(270.0),
            accuracy: Some(5.5),
            battery_percent: Some(100.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_000_000,
        };

        let position = telemetry_position_from_sos(
            "66C38067874B18B4AF15909FD86D6394",
            Some(&telemetry),
            1_700_000_050_000,
        )
        .expect("sos telemetry should become a map telemetry fix");

        assert_eq!(position.callsign, "66c38067874b18b4af15909fd86d6394");
        assert_eq!(position.lat, 43.967_349);
        assert_eq!(position.lon, -66.126_159);
        assert_eq!(position.alt, Some(12.0));
        assert_eq!(position.speed, Some(1.4));
        assert_eq!(position.course, Some(270.0));
        assert_eq!(position.accuracy, Some(5.5));
        assert_eq!(position.updated_at_ms, 1_700_000_000_000);
    }

    #[test]
    fn sos_telemetry_without_coordinates_does_not_create_map_position() {
        let telemetry = SosDeviceTelemetryRecord {
            lat: None,
            lon: None,
            alt: None,
            speed: None,
            course: None,
            accuracy: None,
            battery_percent: Some(87.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_000_000,
        };

        assert!(telemetry_position_from_sos("peer", Some(&telemetry), 42).is_none());
    }

    #[test]
    fn parse_mission_sync_metadata_extracts_command_fields() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
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
        assert!(metadata.is_mission_related());
    }

    #[test]
    fn compact_eam_fields_derive_sender_identity_and_callsign_from_lxmf_source() {
        let source_hex = "fb4c70e20cfac047b899ca2f3671b50a";
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (MsgPackValue::from("i"), MsgPackValue::from("m:eam:1")),
                (MsgPackValue::from("t"), MsgPackValue::from("M1")),
                (
                    MsgPackValue::from("a"),
                    MsgPackValue::Map(vec![
                        (MsgPackValue::from("tu"), MsgPackValue::from("blue-team")),
                        (MsgPackValue::from("ss"), MsgPackValue::from("G")),
                        (MsgPackValue::from("ca"), MsgPackValue::from("Y")),
                        (MsgPackValue::from("pr"), MsgPackValue::from("G")),
                        (MsgPackValue::from("me"), MsgPackValue::from("G")),
                        (MsgPackValue::from("mo"), MsgPackValue::from("G")),
                        (MsgPackValue::from("co"), MsgPackValue::from("Y")),
                    ]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("fields");

        let action = eam_command_action_from_fields(
            bytes.as_slice(),
            1_700_000_000_000,
            Some(source_hex),
            Some("Pixelcorvo"),
        )
        .expect("compact eam should parse");

        let EamCommandAction::Upsert(record) = action else {
            panic!("expected EAM upsert");
        };
        assert_eq!(record.callsign, "Pixelcorvo");
        assert_eq!(record.team_member_uid.as_deref(), Some(source_hex));
        assert_eq!(record.team_uid.as_deref(), Some("blue-team"));
        assert_eq!(
            record
                .source
                .as_ref()
                .map(|source| source.rns_identity.as_str()),
            Some(source_hex)
        );
        assert_eq!(
            record
                .source
                .as_ref()
                .and_then(|source| source.display_name.as_deref()),
            Some("Pixelcorvo")
        );
        assert!(record.notes.is_none());
    }

    #[test]
    fn operational_ack_is_only_built_for_inbound_commands() {
        let metadata = MissionSyncMetadata {
            command_present: true,
            command_id: Some("cmd-accepted".to_string()),
            correlation_id: Some("corr-accepted".to_string()),
            command_type: Some("mission.registry.eam.upsert".to_string()),
            ..MissionSyncMetadata::default()
        };

        let ack = operational_ack_from_metadata(
            Some("ABCDEF0123456789ABCDEF0123456789"),
            Some(&metadata),
        )
        .expect("command metadata should produce ack request");

        assert_eq!(ack.destination_hex, "abcdef0123456789abcdef0123456789");
        assert_eq!(ack.command_id, "cmd-accepted");
        assert_eq!(ack.correlation_id.as_deref(), Some("corr-accepted"));
        assert_eq!(
            ack.command_type.as_deref(),
            Some("mission.registry.eam.upsert")
        );

        let result_metadata = MissionSyncMetadata {
            result_present: true,
            command_id: Some("cmd-accepted".to_string()),
            result_status: Some("accepted".to_string()),
            ..MissionSyncMetadata::default()
        };
        assert!(operational_ack_from_metadata(
            Some("abcdef0123456789abcdef0123456789"),
            Some(&result_metadata),
        )
        .is_none());

        let missing_id = MissionSyncMetadata {
            command_present: true,
            command_type: Some("checklist.create.online".to_string()),
            ..MissionSyncMetadata::default()
        };
        assert!(operational_ack_from_metadata(
            Some("abcdef0123456789abcdef0123456789"),
            Some(&missing_id),
        )
        .is_none());
    }

    #[test]
    fn operational_ack_fields_use_existing_accepted_result_shape() {
        let ack = OperationalAck {
            destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
            command_id: "cmd-result-shape".to_string(),
            correlation_id: Some("corr-result-shape".to_string()),
            command_type: Some("checklist.task.status.set".to_string()),
        };

        let fields = build_operational_ack_fields(&ack, "0123456789abcdef0123456789abcdef")
            .expect("ack fields");
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

        assert!(metadata.result_present);
        assert!(!metadata.command_present);
        assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
        assert_eq!(metadata.command_id.as_deref(), Some("cmd-result-shape"));
        assert_eq!(
            metadata.correlation_id.as_deref(),
            Some("corr-result-shape")
        );
    }

    #[test]
    fn compact_operational_ack_fields_keep_result_tracking_metadata() {
        let ack = OperationalAck {
            destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
            command_id: "cmd-checklist-task-status-set-chk-operational-ack-task-operational-ack-abcdef01-1779627082723".to_string(),
            correlation_id: Some(
                "checklist-task-status-set-chk-operational-ack-task-operational-ack-abcdef01-1779627082723"
                    .to_string(),
            ),
            command_type: Some("checklist.task.status.set".to_string()),
        };

        let fields = build_compact_operational_ack_fields(&ack).expect("ack fields");
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

        assert!(metadata.result_present);
        assert!(!metadata.command_present);
        assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
        assert_eq!(
            metadata.command_id.as_deref(),
            Some(ack.command_id.as_str())
        );
        assert!(metadata.correlation_id.is_none());
    }

    #[test]
    fn compact_event_operational_ack_fields_use_event_uid_tracking_metadata() {
        let ack = OperationalAck {
            destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
            command_id: "log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe".to_string(),
            correlation_id: Some("log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe".to_string()),
            command_type: Some("mission.registry.log_entry.upsert".to_string()),
        };

        let fields = build_compact_operational_ack_fields(&ack).expect("ack fields");
        assert!(
            fields.len() < 32,
            "compact event ack fields were {} bytes",
            fields.len()
        );
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

        assert!(metadata.result_present);
        assert!(!metadata.command_present);
        assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
        assert_eq!(
            metadata.event_uid.as_deref(),
            Some("evt-984bfa16-cfe3-430a-a201-3294310a91fe")
        );
        assert_eq!(
            metadata.command_id.as_deref(),
            Some("log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe")
        );
        assert!(metadata.correlation_id.is_none());
    }

    #[test]
    fn accepted_result_metadata_is_identified_for_direct_ack_return() {
        let accepted = MissionSyncMetadata {
            result_present: true,
            result_status: Some("accepted".to_string()),
            ..MissionSyncMetadata::default()
        };
        assert!(is_accepted_result_metadata(Some(&accepted)));

        let command = MissionSyncMetadata {
            command_present: true,
            command_type: Some("checklist.task.status.set".to_string()),
            ..MissionSyncMetadata::default()
        };
        assert!(!is_accepted_result_metadata(Some(&command)));
    }

    #[test]
    fn mission_result_metadata_maps_application_ack_states() {
        for (status, expected) in [
            ("accepted", ApplicationAckState::Accepted {}),
            ("completed", ApplicationAckState::Completed {}),
            ("rejected", ApplicationAckState::Rejected {}),
            ("failed", ApplicationAckState::Failed {}),
        ] {
            let metadata = MissionSyncMetadata {
                result_present: true,
                result_status: Some(status.to_string()),
                ..MissionSyncMetadata::default()
            };

            assert_eq!(
                application_ack_state_for_mission_metadata(&metadata),
                expected
            );
        }
    }

    #[test]
    fn destination_send_serialization_applies_to_data_but_not_fast_lanes() {
        assert!(should_serialize_lxmf_destination_send(false, false));
        assert!(!should_serialize_lxmf_destination_send(true, false));
        assert!(!should_serialize_lxmf_destination_send(false, true));
        assert!(!should_serialize_lxmf_destination_send(true, true));
    }

    fn test_lxmf_report(
        metadata: MissionSyncMetadata,
        track_delivery_timeout: bool,
        used_propagation_node: bool,
    ) -> LxmfSendReport {
        LxmfSendReport {
            outcome: RnsSendOutcome::SentDirect,
            message_id_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            resolved_destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            metadata: Some(metadata),
            track_delivery_timeout,
            used_propagation_node,
            method: LxmfDeliveryMethod::Direct {},
            representation: LxmfDeliveryRepresentation::Packet {},
            relay_destination_hex: None,
            fallback_stage: None,
            receipt_hash_hex: None,
        }
    }

    fn test_pending_delivery(resend: Option<PendingLxmfResend>) -> PendingLxmfDelivery {
        PendingLxmfDelivery {
            message_id_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            correlation_id: Some("corr-timeout".to_string()),
            command_id: Some("cmd-timeout".to_string()),
            command_type: Some("mission.registry.eam.upsert".to_string()),
            event_uid: None,
            mission_uid: Some("mission-1".to_string()),
            method: LxmfDeliveryMethod::Direct {},
            representation: LxmfDeliveryRepresentation::Packet {},
            relay_destination_hex: None,
            fallback_stage: None,
            resend,
            sent_at_ms: now_ms(),
        }
    }

    #[test]
    fn ack_timeout_auto_command_delivery_is_eligible_for_propagation_retry() {
        let metadata = MissionSyncMetadata {
            command_present: true,
            command_id: Some("cmd-timeout".to_string()),
            correlation_id: Some("corr-timeout".to_string()),
            command_type: Some("mission.registry.eam.upsert".to_string()),
            mission_uid: Some("mission-1".to_string()),
            ..MissionSyncMetadata::default()
        };
        let report = test_lxmf_report(metadata.clone(), true, false);
        let resend = build_pending_lxmf_resend(
            &report,
            "cccccccccccccccccccccccccccccccc",
            b"body",
            None,
            Some(vec![1, 2, 3]),
            Some(metadata),
            SendMode::Auto {},
            SendTaskClass::Mission,
        )
        .expect("auto command should retain resend payload");
        let pending = test_pending_delivery(Some(resend));

        assert!(should_retry_pending_ack_timeout_via_propagation(
            &pending, true
        ));
        assert!(!should_retry_pending_ack_timeout_via_propagation(
            &pending, false
        ));
        assert!(should_retry_pending_ack_timeout_via_direct(&pending));
    }

    #[test]
    fn ack_timeout_retry_skips_results_direct_only_and_existing_propagation() {
        let command_metadata = MissionSyncMetadata {
            command_present: true,
            command_id: Some("cmd-timeout".to_string()),
            correlation_id: Some("corr-timeout".to_string()),
            command_type: Some("checklist.create.online".to_string()),
            ..MissionSyncMetadata::default()
        };
        let result_metadata = MissionSyncMetadata {
            result_present: true,
            command_id: Some("cmd-timeout".to_string()),
            correlation_id: Some("corr-timeout".to_string()),
            result_status: Some("accepted".to_string()),
            ..MissionSyncMetadata::default()
        };

        let command_report = test_lxmf_report(command_metadata.clone(), true, false);
        assert!(build_pending_lxmf_resend(
            &command_report,
            "cccccccccccccccccccccccccccccccc",
            b"body",
            None,
            Some(vec![1, 2, 3]),
            Some(command_metadata.clone()),
            SendMode::DirectOnly {},
            SendTaskClass::Mission,
        )
        .is_none());

        let propagation_report = test_lxmf_report(command_metadata.clone(), true, true);
        assert!(build_pending_lxmf_resend(
            &propagation_report,
            "cccccccccccccccccccccccccccccccc",
            b"body",
            None,
            Some(vec![1, 2, 3]),
            Some(command_metadata),
            SendMode::Auto {},
            SendTaskClass::Mission,
        )
        .is_none());

        let result_report = test_lxmf_report(result_metadata.clone(), false, false);
        assert!(build_pending_lxmf_resend(
            &result_report,
            "cccccccccccccccccccccccccccccccc",
            b"body",
            None,
            Some(vec![1, 2, 3]),
            Some(result_metadata),
            SendMode::Auto {},
            SendTaskClass::Mission,
        )
        .is_none());

        let attempted = PendingLxmfResend {
            requested_destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            body: b"body".to_vec(),
            title: None,
            fields_bytes: Some(vec![1, 2, 3]),
            metadata: MissionSyncMetadata {
                command_present: true,
                command_id: Some("cmd-timeout".to_string()),
                correlation_id: Some("corr-timeout".to_string()),
                command_type: Some("checklist.create.online".to_string()),
                ..MissionSyncMetadata::default()
            },
            send_task_class: SendTaskClass::Mission,
            original_send_mode: SendMode::Auto {},
            direct_ack_retry_attempted: true,
            propagation_fallback_attempted: true,
        };
        assert!(!should_retry_pending_ack_timeout_via_propagation(
            &test_pending_delivery(Some(attempted)),
            true,
        ));
    }

    #[test]
    fn propagated_pending_deliveries_keep_waiting_for_late_acknowledgements() {
        let now = now_ms();
        let mut direct = test_pending_delivery(None);
        direct.sent_at_ms = now.saturating_sub(DEFAULT_LXMF_ACK_TIMEOUT.as_millis() as u64);
        assert!(pending_ack_timeout_elapsed(&direct, now));

        let mut propagated = direct.clone();
        propagated.method = LxmfDeliveryMethod::Propagated {};
        propagated.relay_destination_hex = Some("cccccccccccccccccccccccccccccccc".to_string());
        assert!(!pending_ack_timeout_elapsed(&propagated, now));

        propagated.sent_at_ms = now.saturating_sub(PROPAGATED_LXMF_ACK_TIMEOUT.as_millis() as u64);
        assert!(pending_ack_timeout_elapsed(&propagated, now));
    }

    #[test]
    fn propagation_fallback_pending_deliveries_keep_waiting_for_late_acknowledgements() {
        let now = now_ms();
        let mut propagated = test_pending_delivery(Some(PendingLxmfResend {
            requested_destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            body: b"body".to_vec(),
            title: None,
            fields_bytes: Some(vec![1, 2, 3]),
            metadata: MissionSyncMetadata {
                command_present: true,
                command_id: Some("cmd-timeout".to_string()),
                correlation_id: Some("corr-timeout".to_string()),
                command_type: Some("sos.status".to_string()),
                ..MissionSyncMetadata::default()
            },
            send_task_class: SendTaskClass::Mission,
            original_send_mode: SendMode::Auto {},
            direct_ack_retry_attempted: true,
            propagation_fallback_attempted: true,
        }));
        propagated.method = LxmfDeliveryMethod::Propagated {};
        propagated.relay_destination_hex = Some("dddddddddddddddddddddddddddddddd".to_string());
        propagated.sent_at_ms = now.saturating_sub(DEFAULT_LXMF_ACK_TIMEOUT.as_millis() as u64);

        assert!(!pending_ack_timeout_elapsed(&propagated, now));
        propagated.sent_at_ms = now.saturating_sub(PROPAGATED_LXMF_ACK_TIMEOUT.as_millis() as u64);
        assert!(pending_ack_timeout_elapsed(&propagated, now));
    }

    #[test]
    fn chat_delivery_ack_body_round_trips_message_id() {
        let message_id = "482ecb36f44826e45aea88562e6ebda4a66d30575eb42557732adced08e0db7d";
        let body = chat_delivery_ack_body(message_id);

        assert_eq!(
            parse_chat_delivery_ack_body(body.as_str()),
            Some(message_id.to_string())
        );
        assert_eq!(
            parse_chat_delivery_ack_body("REM_DELIVERY_ACK:not-hex"),
            None
        );
        assert_eq!(parse_chat_delivery_ack_body("regular chat"), None);
    }

    fn propagation_announce(
        destination_hex: &str,
        hops: u8,
        received_at_ms: u64,
    ) -> sdkmsg::AnnounceRecord {
        sdkmsg::AnnounceRecord {
            destination_hex: destination_hex.to_string(),
            identity_hex: format!("id-{destination_hex}"),
            destination_kind: "lxmf_propagation".to_string(),
            app_data: String::new(),
            display_name: None,
            hops,
            interface_hex: String::new(),
            received_at_ms,
        }
    }

    #[test]
    fn propagation_auto_selection_keeps_current_equal_hop_relay_stable() {
        let current = propagation_announce("11111111111111111111111111111111", 1, 100);
        let newer_equal_hop = propagation_announce("22222222222222222222222222222222", 1, 200);
        let lower_hop = propagation_announce("33333333333333333333333333333333", 0, 300);

        let stable_choice = [&current, &newer_equal_hop]
            .into_iter()
            .min_by_key(|record| {
                propagation_candidate_sort_key(
                    record,
                    None,
                    Some("11111111111111111111111111111111"),
                )
            })
            .expect("stable relay");
        assert_eq!(stable_choice.destination_hex, current.destination_hex);

        let lower_hop_choice = [&current, &lower_hop]
            .into_iter()
            .min_by_key(|record| {
                propagation_candidate_sort_key(
                    record,
                    None,
                    Some("11111111111111111111111111111111"),
                )
            })
            .expect("lower hop relay");
        assert_eq!(lower_hop_choice.destination_hex, lower_hop.destination_hex);
    }

    #[test]
    fn propagation_auto_selection_prefers_fresh_equal_hop_relay_without_current() {
        let stale_equal_hop = propagation_announce("11111111111111111111111111111111", 1, 100);
        let fresh_equal_hop = propagation_announce("22222222222222222222222222222222", 1, 200);

        let choice = [&stale_equal_hop, &fresh_equal_hop]
            .into_iter()
            .min_by_key(|record| propagation_candidate_sort_key(record, None, None))
            .expect("fresh relay");

        assert_eq!(choice.destination_hex, fresh_equal_hop.destination_hex);
    }

    #[test]
    fn propagation_sync_candidates_include_active_then_alternate_relays() {
        let active = "11111111111111111111111111111111";
        let announces = vec![
            propagation_announce(active, 1, 200),
            propagation_announce("22222222222222222222222222222222", 1, 100),
            propagation_announce("33333333333333333333333333333333", 2, 50),
            sdkmsg::AnnounceRecord {
                destination_hex: "44444444444444444444444444444444".to_string(),
                identity_hex: "non-propagation".to_string(),
                destination_kind: "lxmf_delivery".to_string(),
                app_data: String::new(),
                display_name: None,
                hops: 0,
                interface_hex: String::new(),
                received_at_ms: 1,
            },
        ];

        let candidates = propagation_sync_candidate_relays(announces.as_slice(), active, None);

        assert_eq!(
            candidates,
            vec![
                active.to_string(),
                "22222222222222222222222222222222".to_string(),
                "33333333333333333333333333333333".to_string(),
            ]
        );
    }

    #[test]
    fn event_projection_from_trimmed_fields_uses_lxmf_body_content() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("source"),
                    MsgPackValue::Map(vec![(
                        MsgPackValue::from("rns_identity"),
                        MsgPackValue::from("identity-1"),
                    )]),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (MsgPackValue::from("entry_uid"), MsgPackValue::from("evt-1")),
                        (
                            MsgPackValue::from("mission_uid"),
                            MsgPackValue::from("mission-1"),
                        ),
                        (MsgPackValue::from("callsign"), MsgPackValue::from("Pixel")),
                    ]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let record = event_projection_from_fields(
            &bytes,
            Some(b"P01"),
            None,
            Some("Pixel"),
            1_700_000_000_000,
        )
        .expect("event projection");

        assert_eq!(record.uid, "evt-1");
        assert_eq!(record.command_id, "log-entry-evt-1");
        assert_eq!(record.command_type, "mission.registry.log_entry.upsert");
        assert_eq!(record.mission_uid, "mission-1");
        assert_eq!(record.content, "MECP/2/P01");
        assert_eq!(record.callsign, "Pixel");
        assert_eq!(record.source_identity, "identity-1");
        assert_eq!(record.source_display_name.as_deref(), Some("Pixel"));
        assert_eq!(record.keywords, Vec::<String>::new());
        assert_eq!(record.content_hashes, Vec::<String>::new());
        assert_eq!(record.topics, vec!["mission-1".to_string()]);
    }

    #[test]
    fn event_projection_from_verbose_fields_remains_compatible() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-event-1"),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("corr-event-1"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("source"),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("rns_identity"),
                            MsgPackValue::from("identity-1"),
                        ),
                        (
                            MsgPackValue::from("display_name"),
                            MsgPackValue::from("Pixel"),
                        ),
                    ]),
                ),
                (
                    MsgPackValue::from("timestamp"),
                    MsgPackValue::from("2026-05-16T21:26:30Z"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (MsgPackValue::from("entry_uid"), MsgPackValue::from("evt-1")),
                        (
                            MsgPackValue::from("mission_uid"),
                            MsgPackValue::from("mission-1"),
                        ),
                        (
                            MsgPackValue::from("content"),
                            MsgPackValue::from("MECP/2/P01 legacy"),
                        ),
                        (MsgPackValue::from("callsign"), MsgPackValue::from("Pixel")),
                        (
                            MsgPackValue::from("source_identity"),
                            MsgPackValue::from("identity-1"),
                        ),
                        (
                            MsgPackValue::from("source_display_name"),
                            MsgPackValue::from("Pixel"),
                        ),
                        (
                            MsgPackValue::from("keywords"),
                            MsgPackValue::Array(vec![MsgPackValue::from("r3akt:event-type:P")]),
                        ),
                    ]),
                ),
                (
                    MsgPackValue::from("topics"),
                    MsgPackValue::Array(vec![MsgPackValue::from("mission-1")]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let record = event_projection_from_fields(&bytes, None, None, None, 1_700_000_000_000)
            .expect("event projection");

        assert_eq!(record.uid, "evt-1");
        assert_eq!(record.command_id, "cmd-event-1");
        assert_eq!(record.content, "MECP/2/P01 legacy");
        assert_eq!(record.source_display_name.as_deref(), Some("Pixel"));
        assert_eq!(record.keywords, vec!["r3akt:event-type:P".to_string()]);
        assert_eq!(record.correlation_id.as_deref(), Some("corr-event-1"));
    }

    #[test]
    fn event_projection_from_fields_preserves_tombstone_timestamp() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-event-delete-1"),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("corr-event-delete-1"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("source"),
                    MsgPackValue::Map(vec![(
                        MsgPackValue::from("rns_identity"),
                        MsgPackValue::from("identity-1"),
                    )]),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (MsgPackValue::from("entry_uid"), MsgPackValue::from("evt-1")),
                        (
                            MsgPackValue::from("mission_uid"),
                            MsgPackValue::from("mission-1"),
                        ),
                        (
                            MsgPackValue::from("content"),
                            MsgPackValue::from("MECP/2/P01 deleted"),
                        ),
                        (MsgPackValue::from("callsign"), MsgPackValue::from("Pixel")),
                        (
                            MsgPackValue::from("source_identity"),
                            MsgPackValue::from("identity-1"),
                        ),
                        (
                            MsgPackValue::from("deleted_at_ms"),
                            MsgPackValue::from(1_700_000_050_000_u64),
                        ),
                    ]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let record = event_projection_from_fields(&bytes, None, None, None, 1_700_000_060_000)
            .expect("event");

        assert_eq!(record.uid, "evt-1");
        assert_eq!(record.deleted_at_ms, Some(1_700_000_050_000));
    }

    #[test]
    fn parse_mission_sync_metadata_extracts_result_and_event_fields() {
        let fields = MsgPackValue::Map(vec![
            (
                MsgPackValue::from(FIELD_RESULTS),
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
                MsgPackValue::from(FIELD_EVENT),
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
        assert!(metadata.is_mission_related());
    }

    #[test]
    fn incoming_timestamp_is_newer_handles_fractional_seconds() {
        assert!(incoming_timestamp_is_newer(
            Some("2026-04-22T12:00:00Z"),
            "2026-04-22T12:00:00.000000001Z"
        ));
        assert!(incoming_timestamp_is_newer(
            Some("2026-04-22T12:00:00.000000001Z"),
            "2026-04-22T12:00:00.000000002Z"
        ));
        assert!(!incoming_timestamp_is_newer(
            Some("2026-04-22T12:00:00.100000000Z"),
            "2026-04-22T12:00:00Z"
        ));
    }

    #[test]
    fn inbound_create_hydrates_newer_hidden_placeholder() {
        let hidden = hidden_placeholder_checklist_record(
            "chk-out-of-order",
            "2026-04-22T12:00:01.000000000Z",
        );

        assert!(should_apply_inbound_checklist_create(
            Some(&hidden),
            "2026-04-22T12:00:00.000000000Z",
        ));
    }

    #[test]
    fn inbound_create_keeps_non_placeholder_freshness_gate() {
        let existing = checklist_test_record(
            "2026-04-22T12:00:01.000000000Z",
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:00:01.000000000Z"),
        );

        assert!(!should_apply_inbound_checklist_create(
            Some(&existing),
            "2026-04-22T12:00:00.000000000Z",
        ));
    }

    #[test]
    fn inbound_create_sets_creator_display_name_from_command_source() {
        let timestamp = "2026-04-22T12:00:00.000000000Z";
        let mut checklist = blank_checklist_record("chk-author", timestamp, None);
        let args = Vec::<(MsgPackValue, MsgPackValue)>::new();
        let command = vec![(
            MsgPackValue::from("source"),
            MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("rns_identity"),
                    MsgPackValue::from("abcd1234"),
                ),
                (
                    MsgPackValue::from("display_name"),
                    MsgPackValue::from("Selke"),
                ),
            ]),
        )];
        let source_identity = checklist_command_source_identity(command.as_slice());

        apply_checklist_creator_from_command(
            &mut checklist,
            args.as_slice(),
            command.as_slice(),
            source_identity.as_deref(),
        );

        assert_eq!(checklist.created_by_team_member_rns_identity, "abcd1234");
        assert_eq!(
            checklist.created_by_team_member_display_name.as_deref(),
            Some("Selke")
        );
    }

    #[test]
    fn inbound_create_hydrates_tasks_from_local_template() {
        let storage_dir =
            std::env::temp_dir().join(format!("rem-runtime-template-hydration-{}", now_ms()));
        let store = AppStateStore::new(Some(
            storage_dir
                .to_str()
                .expect("temporary storage dir should be utf-8"),
        ))
        .expect("app state store");
        let mut checklist = blank_checklist_record(
            "chk-template",
            "2026-04-22T12:00:00.000000000Z",
            Some("peer-a"),
        );
        checklist.template_uid = Some("tmpl-24-hour-survival-pack".to_string());
        checklist.columns.clear();
        checklist.tasks.clear();

        hydrate_checklist_from_local_template(&store, &mut checklist);

        assert_eq!(
            checklist.template_name.as_deref(),
            Some("24 Hour Survival Pack")
        );
        assert_eq!(checklist.tasks.len(), 12);
        assert_eq!(checklist.expected_task_count, Some(12));
        assert!(!checklist.columns.is_empty());
    }

    #[test]
    fn inbound_delete_marks_existing_checklist_deleted() {
        let existing = checklist_test_record(
            "2026-04-22T12:00:00.000000000Z",
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:00:00.000000000Z"),
        );

        let deleted = checklist_delete_record_from_command(
            Some(existing),
            "chk-merge",
            "2026-04-22T12:00:01.000000000Z",
            Some("peer-delete"),
        )
        .expect("newer delete should apply");

        assert_eq!(
            deleted.deleted_at.as_deref(),
            Some("2026-04-22T12:00:01.000000000Z")
        );
        assert_eq!(
            deleted.updated_at.as_deref(),
            Some("2026-04-22T12:00:01.000000000Z")
        );
        assert_eq!(
            deleted.last_changed_by_team_member_rns_identity.as_deref(),
            Some("peer-delete")
        );
    }

    #[test]
    fn inbound_delete_ignores_stale_timestamp() {
        let existing = checklist_test_record(
            "2026-04-22T12:00:02.000000000Z",
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:00:02.000000000Z"),
        );

        assert!(checklist_delete_record_from_command(
            Some(existing),
            "chk-merge",
            "2026-04-22T12:00:01.000000000Z",
            Some("peer-delete"),
        )
        .is_none());
    }

    fn checklist_test_column(column_uid: &str) -> ChecklistColumnRecord {
        ChecklistColumnRecord {
            column_uid: column_uid.to_string(),
            column_name: column_uid.to_string(),
            display_order: 0,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: true,
            system_key: None,
        }
    }

    fn checklist_test_cell(
        task_uid: &str,
        column_uid: &str,
        value: &str,
        updated_at: &str,
    ) -> ChecklistCellRecord {
        ChecklistCellRecord {
            cell_uid: format!("{task_uid}:{column_uid}"),
            task_uid: task_uid.to_string(),
            column_uid: column_uid.to_string(),
            value: Some(value.to_string()),
            updated_at: Some(updated_at.to_string()),
            updated_by_team_member_rns_identity: Some("peer-a".to_string()),
        }
    }

    fn checklist_test_task(
        task_uid: &str,
        number: u32,
        title: &str,
        updated_at: &str,
    ) -> ChecklistTaskRecord {
        let mut task = placeholder_task_record(task_uid, updated_at);
        task.number = number;
        task.legacy_value = Some(title.to_string());
        task.cells = vec![checklist_test_cell(task_uid, "col-task", title, updated_at)];
        task
    }

    fn checklist_test_record(updated_at: &str, task: ChecklistTaskRecord) -> ChecklistRecord {
        let mut record = blank_checklist_record("chk-merge", updated_at, Some("peer-a"));
        record.mission_uid = Some("mission-alpha".to_string());
        record.template_uid = Some("template-alpha".to_string());
        record.name = "Shared Excheck".to_string();
        record.description = "Collaborative checklist".to_string();
        record.updated_at = Some(updated_at.to_string());
        record.columns = vec![checklist_test_column("col-task")];
        record.tasks = vec![task];
        normalize_checklist_record(&mut record);
        record
    }

    #[test]
    fn native_upload_snapshot_decodes_from_command_field() {
        let command = vec![(
            MsgPackValue::from("snapshot"),
            MsgPackValue::Map(vec![
                (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
                (MsgPackValue::from("name"), MsgPackValue::from("Native")),
                (
                    MsgPackValue::from("tasks"),
                    MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                        MsgPackValue::from("task_uid"),
                        MsgPackValue::from("task-1"),
                    )])]),
                ),
            ]),
        )];
        let snapshot_json =
            checklist_snapshot_json_from_command(command.as_slice()).expect("native snapshot");

        assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
        assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
    }

    #[test]
    fn native_upload_snapshot_decodes_from_msgpack_content() {
        let content = MsgPackValue::Map(vec![
            (
                MsgPackValue::from("type"),
                MsgPackValue::from("rem.checklist.snapshot.v1"),
            ),
            (
                MsgPackValue::from("checklist_uid"),
                MsgPackValue::from("chk-native"),
            ),
            (
                MsgPackValue::from("snapshot"),
                MsgPackValue::Map(vec![
                    (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
                    (MsgPackValue::from("name"), MsgPackValue::from("Native")),
                    (
                        MsgPackValue::from("tasks"),
                        MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                            MsgPackValue::from("task_uid"),
                            MsgPackValue::from("task-1"),
                        )])]),
                    ),
                ]),
            ),
        ]);
        let bytes = rmp_serde::to_vec(&content).expect("snapshot content");
        let snapshot_json =
            checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-native")
                .expect("content snapshot");

        assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
        assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
        assert!(
            checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-other").is_none()
        );
    }

    #[test]
    fn native_upload_snapshot_decodes_from_compressed_msgpack_content() {
        use std::io::Write as _;

        let snapshot = MsgPackValue::Map(vec![
            (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
            (MsgPackValue::from("name"), MsgPackValue::from("Native")),
            (
                MsgPackValue::from("tasks"),
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                    MsgPackValue::from("task_uid"),
                    MsgPackValue::from("task-1"),
                )])]),
            ),
        ]);
        let snapshot_msgpack = rmp_serde::to_vec(&snapshot).expect("snapshot msgpack");
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder
            .write_all(snapshot_msgpack.as_slice())
            .expect("write compressed snapshot");
        let compressed_snapshot = encoder.finish().expect("finish compressed snapshot");
        let content = MsgPackValue::Map(vec![
            (
                MsgPackValue::from("type"),
                MsgPackValue::from("rem.checklist.snapshot.v2"),
            ),
            (
                MsgPackValue::from("checklist_uid"),
                MsgPackValue::from("chk-native"),
            ),
            (
                MsgPackValue::from("encoding"),
                MsgPackValue::from("zlib+msgpack"),
            ),
            (
                MsgPackValue::from("snapshot"),
                MsgPackValue::Binary(compressed_snapshot),
            ),
        ]);
        let bytes = rmp_serde::to_vec(&content).expect("snapshot content");
        let snapshot_json =
            checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-native")
                .expect("compressed content snapshot");

        assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
        assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
        assert!(
            checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-other").is_none()
        );
    }

    #[test]
    fn first_status_update_can_apply_to_missing_task_placeholder() {
        let mut checklist =
            blank_checklist_record("chk-missing-task", "2026-04-22T12:00:00Z", None);
        let inserted = ensure_task_for_incoming_update(
            &mut checklist,
            "task-missing",
            "2026-04-22T12:01:00Z",
            None,
        );
        let task = find_checklist_task_mut(&mut checklist, "task-missing").expect("task inserted");

        assert!(inserted);
        assert!(
            inserted
                || incoming_timestamp_is_newer(task.updated_at.as_deref(), "2026-04-22T12:01:00Z")
        );
    }

    #[test]
    fn row_add_can_hydrate_placeholder_without_clearing_newer_status_or_cells() {
        let mut task = placeholder_task_record("task-1", "2026-04-22T12:05:00Z");
        task.user_status = ChecklistUserTaskStatus::Complete {};
        task.task_status = ChecklistTaskStatus::Complete {};
        task.completed_at = Some("2026-04-22T12:05:00Z".to_string());
        task.cells.push(ChecklistCellRecord {
            cell_uid: "task-1:col-item".to_string(),
            task_uid: "task-1".to_string(),
            column_uid: "col-item".to_string(),
            value: Some("Water".to_string()),
            updated_at: Some("2026-04-22T12:06:00Z".to_string()),
            updated_by_team_member_rns_identity: Some("peer-b".to_string()),
        });

        assert!(task_needs_row_metadata_hydration(&task));
        task.number = 1;
        task.legacy_value = Some("Water".to_string());
        task.updated_at =
            newest_timestamp(task.updated_at.as_deref(), Some("2026-04-22T12:04:00Z"))
                .map(ToString::to_string);

        assert_eq!(task.number, 1);
        assert_eq!(task.legacy_value.as_deref(), Some("Water"));
        assert!(matches!(
            task.user_status,
            ChecklistUserTaskStatus::Complete {}
        ));
        assert_eq!(task.cells.len(), 1);
        assert_eq!(task.updated_at.as_deref(), Some("2026-04-22T12:05:00Z"));
    }

    #[test]
    fn row_add_task_payload_decodes_complete_task_cells() {
        let mut task = checklist_test_task(
            "stale-task-id",
            1,
            "Secure north access",
            "2026-04-22T12:00:00Z",
        );
        task.cells.push(checklist_test_cell(
            "stale-task-id",
            "col-notes",
            "Use IR marker",
            "2026-04-22T12:00:01Z",
        ));
        let task_msgpack = rmp_serde::from_slice::<MsgPackValue>(
            rmp_serde::to_vec(&task).expect("task msgpack").as_slice(),
        )
        .expect("task value");
        let args = vec![
            (
                MsgPackValue::from("task_uid"),
                MsgPackValue::from("task-remote"),
            ),
            (MsgPackValue::from("number"), MsgPackValue::from(7_u32)),
            (MsgPackValue::from("task"), task_msgpack),
        ];

        let decoded = checklist_task_from_row_add_args(
            args.as_slice(),
            "task-remote",
            7,
            "2026-04-22T12:02:00Z",
        )
        .expect("row task");

        assert_eq!(decoded.task_uid, "task-remote");
        assert_eq!(decoded.number, 7);
        assert_eq!(decoded.updated_at.as_deref(), Some("2026-04-22T12:02:00Z"));
        assert_eq!(decoded.cells.len(), 2);
        assert!(decoded
            .cells
            .iter()
            .all(|cell| cell.task_uid == "task-remote"));
        assert_eq!(
            decoded
                .cells
                .iter()
                .find(|cell| cell.column_uid == "col-task")
                .and_then(|cell| cell.value.as_deref()),
            Some("Secure north access")
        );
    }

    #[test]
    fn inbound_complete_status_applies_even_when_cell_update_is_newer() {
        let mut task =
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
        task.cells.push(checklist_test_cell(
            "task-1",
            "col-task",
            "Existing",
            "2026-04-22T12:10:00.000000000Z",
        ));
        task.updated_at = Some("2026-04-22T12:10:00.000000000Z".to_string());

        assert!(should_apply_inbound_task_status(
            &task,
            ChecklistUserTaskStatus::Complete {},
            "2026-04-22T12:07:00.000000000Z",
            false,
        ));
    }

    #[test]
    fn inbound_pending_status_does_not_revert_newer_complete() {
        let mut task =
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
        task.user_status = ChecklistUserTaskStatus::Complete {};
        task.task_status = ChecklistTaskStatus::Complete {};
        task.completed_at = Some("2026-04-22T12:10:00.000000000Z".to_string());
        task.updated_at = Some("2026-04-22T12:10:00.000000000Z".to_string());

        assert!(!should_apply_inbound_task_status(
            &task,
            ChecklistUserTaskStatus::Pending {},
            "2026-04-22T12:07:00.000000000Z",
            false,
        ));
        assert!(should_apply_inbound_task_status(
            &task,
            ChecklistUserTaskStatus::Pending {},
            "2026-04-22T12:11:00.000000000Z",
            false,
        ));
    }

    fn checklist_status_fields(
        checklist_uid: &str,
        task_uid: Option<&str>,
        timestamp: &str,
        user_status: &str,
    ) -> Vec<u8> {
        let mut args = vec![
            (
                MsgPackValue::from("checklist_uid"),
                MsgPackValue::from(checklist_uid),
            ),
            (
                MsgPackValue::from("user_status"),
                MsgPackValue::from(user_status),
            ),
        ];
        if let Some(task_uid) = task_uid {
            args.push((MsgPackValue::from("task_uid"), MsgPackValue::from(task_uid)));
        }
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("checklist.task.status.set"),
                ),
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-status-test"),
                ),
                (
                    MsgPackValue::from("timestamp"),
                    MsgPackValue::from(timestamp),
                ),
                (MsgPackValue::from("args"), MsgPackValue::Map(args)),
            ])]),
        )]);
        rmp_serde::to_vec(&fields).expect("status fields")
    }

    fn compact_checklist_status_fields(
        checklist_uid: &str,
        task_uid: &str,
        number: Option<u32>,
        timestamp: &str,
        user_status: &str,
    ) -> Vec<u8> {
        let mut args = vec![
            (MsgPackValue::from("cl"), MsgPackValue::from(checklist_uid)),
            (MsgPackValue::from("tsk"), MsgPackValue::from(task_uid)),
            (MsgPackValue::from("us"), MsgPackValue::from(user_status)),
        ];
        if let Some(number) = number {
            args.push((MsgPackValue::from("no"), MsgPackValue::from(number)));
        }
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (MsgPackValue::from("t"), MsgPackValue::from("C6")),
                (
                    MsgPackValue::from("i"),
                    MsgPackValue::from("cmd-status-test"),
                ),
                (MsgPackValue::from("ts"), MsgPackValue::from(timestamp)),
                (MsgPackValue::from("a"), MsgPackValue::Map(args)),
            ])]),
        )]);
        rmp_serde::to_vec(&fields).expect("compact status fields")
    }

    #[test]
    fn idempotent_checklist_status_update_is_handled_for_ack() {
        let storage_dir = std::env::temp_dir().join(format!(
            "rem-runtime-checklist-status-idempotent-{}",
            now_ms()
        ));
        let store = AppStateStore::new(Some(
            storage_dir
                .to_str()
                .expect("temporary storage dir should be utf-8"),
        ))
        .expect("app state store");
        let mut task =
            checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
        task.user_status = ChecklistUserTaskStatus::Complete {};
        task.task_status = ChecklistTaskStatus::Complete {};
        task.completed_at = Some("2026-04-22T12:05:00.000000000Z".to_string());
        let checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task.clone());
        store
            .upsert_checklist(&checklist, "seed-checklist")
            .expect("seed checklist");
        let bus = EventBus::new();
        let fields = checklist_status_fields(
            "chk-merge",
            Some("task-1"),
            "2026-04-22T12:04:00.000000000Z",
            "COMPLETE",
        );

        assert!(persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(fields.as_slice()),
            None,
        ));

        let stored = store
            .get_checklist_any("chk-merge")
            .expect("stored checklist query")
            .expect("stored checklist");
        assert_eq!(
            stored.updated_at.as_deref(),
            Some("2026-04-22T12:05:00.000000000Z")
        );
        assert_eq!(
            stored.tasks[0].updated_at.as_deref(),
            Some("2026-04-22T12:05:00.000000000Z")
        );
        assert!(matches!(
            stored.tasks[0].user_status,
            ChecklistUserTaskStatus::Complete {}
        ));
    }

    #[test]
    fn compact_checklist_status_update_is_persisted() {
        let storage_dir =
            std::env::temp_dir().join(format!("rem-runtime-checklist-status-compact-{}", now_ms()));
        let store = AppStateStore::new(Some(
            storage_dir
                .to_str()
                .expect("temporary storage dir should be utf-8"),
        ))
        .expect("app state store");
        let task = checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
        let checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task);
        store
            .upsert_checklist(&checklist, "seed-checklist")
            .expect("seed checklist");
        let bus = EventBus::new();
        let fields = compact_checklist_status_fields(
            "chk-merge",
            "task-1",
            None,
            "2026-04-22T12:06:00.000000000Z",
            "COMPLETE",
        );

        assert!(persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(fields.as_slice()),
            None,
        ));

        let stored = store
            .get_checklist_any("chk-merge")
            .expect("stored checklist query")
            .expect("stored checklist");
        assert!(matches!(
            stored.tasks[0].user_status,
            ChecklistUserTaskStatus::Complete {}
        ));
        assert_eq!(
            stored.tasks[0].updated_at.as_deref(),
            Some("2026-04-22T12:06:00.000000000Z")
        );
    }

    #[test]
    fn compact_checklist_status_update_resolves_visible_row_by_number_when_task_uid_differs() {
        let storage_dir = std::env::temp_dir().join(format!(
            "rem-runtime-checklist-status-row-number-{}",
            now_ms()
        ));
        let store = AppStateStore::new(Some(
            storage_dir
                .to_str()
                .expect("temporary storage dir should be utf-8"),
        ))
        .expect("app state store");
        let task_one =
            checklist_test_task("local-task-1", 1, "First", "2026-04-22T12:05:00.000000000Z");
        let task_two = checklist_test_task(
            "local-task-2",
            2,
            "Second",
            "2026-04-22T12:05:00.000000000Z",
        );
        let mut checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task_one);
        checklist.tasks.push(task_two);
        normalize_checklist_record(&mut checklist);
        store
            .upsert_checklist(&checklist, "seed-checklist")
            .expect("seed checklist");
        let bus = EventBus::new();
        let fields = compact_checklist_status_fields(
            "chk-merge",
            "remote-task-2",
            Some(2),
            "2026-04-22T12:06:00.000000000Z",
            "COMPLETE",
        );

        assert!(persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(fields.as_slice()),
            None,
        ));

        let stored = store
            .get_checklist_any("chk-merge")
            .expect("stored checklist query")
            .expect("stored checklist");
        let first = stored
            .tasks
            .iter()
            .find(|task| task.task_uid == "local-task-1")
            .expect("first task");
        let second = stored
            .tasks
            .iter()
            .find(|task| task.task_uid == "local-task-2")
            .expect("second task");
        assert!(matches!(
            first.user_status,
            ChecklistUserTaskStatus::Pending {}
        ));
        assert!(matches!(
            second.user_status,
            ChecklistUserTaskStatus::Complete {}
        ));
        assert!(!stored
            .tasks
            .iter()
            .any(|task| task.task_uid == "remote-task-2"));
    }

    #[test]
    fn malformed_checklist_status_update_is_not_handled_for_ack() {
        let storage_dir = std::env::temp_dir().join(format!(
            "rem-runtime-checklist-status-malformed-{}",
            now_ms()
        ));
        let store = AppStateStore::new(Some(
            storage_dir
                .to_str()
                .expect("temporary storage dir should be utf-8"),
        ))
        .expect("app state store");
        let bus = EventBus::new();
        let fields = checklist_status_fields(
            "chk-merge",
            None,
            "2026-04-22T12:04:00.000000000Z",
            "COMPLETE",
        );

        assert!(!persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(fields.as_slice()),
            None,
        ));
    }

    #[test]
    fn upload_snapshot_hydrates_hidden_placeholder_even_when_snapshot_is_older() {
        let existing = hidden_placeholder_checklist_record("chk-merge", "2026-04-22T12:00:01Z");
        let mut incoming = checklist_test_record(
            "2026-04-22T12:00:00Z",
            checklist_test_task("task-1", 1, "Hydrated task", "2026-04-22T12:00:00Z"),
        );
        incoming.uploaded_at = Some("2026-04-22T12:00:00Z".to_string());

        let merged = merge_uploaded_checklist_snapshot(
            Some(existing),
            incoming,
            "2026-04-22T12:00:02Z",
            Some("peer-a"),
        )
        .expect("placeholder should hydrate");

        assert_eq!(merged.tasks.len(), 1);
        assert_eq!(
            merged.last_changed_by_team_member_rns_identity.as_deref(),
            Some("peer-a")
        );
        assert_eq!(
            merged.tasks[0].legacy_value.as_deref(),
            Some("Hydrated task")
        );
        assert!(merged.deleted_at.is_none());
    }

    #[test]
    fn upload_snapshot_preserves_newer_local_task_and_cell_state() {
        let mut local_task =
            checklist_test_task("task-1", 1, "Completed locally", "2026-04-22T12:10:00Z");
        local_task.user_status = ChecklistUserTaskStatus::Complete {};
        local_task.task_status = ChecklistTaskStatus::Complete {};
        local_task.completed_at = Some("2026-04-22T12:10:00Z".to_string());
        let local = checklist_test_record("2026-04-22T12:10:00Z", local_task);

        let mut incoming = checklist_test_record(
            "2026-04-22T12:00:00Z",
            checklist_test_task("task-1", 1, "Stale snapshot", "2026-04-22T12:00:00Z"),
        );
        incoming.uploaded_at = Some("2026-04-22T12:30:00Z".to_string());

        let merged = merge_uploaded_checklist_snapshot(
            Some(local),
            incoming,
            "2026-04-22T12:30:00Z",
            Some("peer-b"),
        )
        .expect("stale upload should merge");

        assert!(matches!(
            merged.tasks[0].user_status,
            ChecklistUserTaskStatus::Complete {}
        ));
        assert_eq!(
            merged.tasks[0]
                .cells
                .iter()
                .find(|cell| cell.column_uid == "col-task")
                .and_then(|cell| cell.value.as_deref()),
            Some("Completed locally")
        );
        assert!(merged
            .participant_rns_identities
            .iter()
            .any(|identity| identity == "peer-b"));
        assert_eq!(
            merged.last_changed_by_team_member_rns_identity.as_deref(),
            Some("peer-b")
        );
    }

    #[test]
    fn upload_snapshot_appends_missing_columns_and_tasks() {
        let local = checklist_test_record(
            "2026-04-22T12:00:00Z",
            checklist_test_task("task-1", 1, "Local task", "2026-04-22T12:00:00Z"),
        );
        let mut incoming = checklist_test_record(
            "2026-04-22T12:05:00Z",
            checklist_test_task("task-2", 2, "Incoming task", "2026-04-22T12:05:00Z"),
        );
        incoming.columns.push(checklist_test_column("col-notes"));
        incoming.tasks[0].cells.push(checklist_test_cell(
            "task-2",
            "col-notes",
            "Incoming notes",
            "2026-04-22T12:05:00Z",
        ));
        incoming.uploaded_at = Some("2026-04-22T12:05:00Z".to_string());

        let merged = merge_uploaded_checklist_snapshot(
            Some(local),
            incoming,
            "2026-04-22T12:05:00Z",
            Some("peer-b"),
        )
        .expect("upload should merge");

        assert!(merged
            .columns
            .iter()
            .any(|column| column.column_uid == "col-notes"));
        assert!(merged.tasks.iter().any(|task| task.task_uid == "task-1"));
        assert!(merged.tasks.iter().any(|task| task.task_uid == "task-2"));
    }

    #[test]
    fn upload_snapshot_preserves_newer_local_task_tombstone() {
        let mut tombstone =
            checklist_test_task("task-1", 1, "Deleted task", "2026-04-22T12:20:00Z");
        tombstone.deleted_at = Some("2026-04-22T12:20:00Z".to_string());
        let local = checklist_test_record("2026-04-22T12:20:00Z", tombstone);

        let mut incoming = checklist_test_record(
            "2026-04-22T12:10:00Z",
            checklist_test_task("task-1", 1, "Stale live task", "2026-04-22T12:10:00Z"),
        );
        incoming.uploaded_at = Some("2026-04-22T12:40:00Z".to_string());

        let merged = merge_uploaded_checklist_snapshot(
            Some(local),
            incoming,
            "2026-04-22T12:40:00Z",
            Some("peer-b"),
        )
        .expect("upload should merge");

        assert_eq!(
            merged.tasks[0].deleted_at.as_deref(),
            Some("2026-04-22T12:20:00Z")
        );
    }

    #[test]
    fn upload_snapshot_does_not_revive_newer_deleted_checklist() {
        let mut deleted = checklist_test_record(
            "2026-04-22T12:20:00Z",
            checklist_test_task("task-1", 1, "Deleted checklist", "2026-04-22T12:20:00Z"),
        );
        deleted.deleted_at = Some("2026-04-22T12:20:00Z".to_string());

        let mut incoming = checklist_test_record(
            "2026-04-22T12:10:00Z",
            checklist_test_task("task-1", 1, "Stale checklist", "2026-04-22T12:10:00Z"),
        );
        incoming.uploaded_at = Some("2026-04-22T12:40:00Z".to_string());

        assert!(merge_uploaded_checklist_snapshot(
            Some(deleted),
            incoming,
            "2026-04-22T12:40:00Z",
            Some("peer-b"),
        )
        .is_none());
    }

    #[test]
    fn parse_hub_directory_result_state_ignores_accepted_lifecycle() {
        let result = MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from("cmd-123"),
            ),
            (MsgPackValue::from("status"), MsgPackValue::from("accepted")),
        ]);

        let parsed =
            parse_hub_directory_result_state(&result, "cmd-123", 123).expect("accepted lifecycle");

        assert!(matches!(parsed, HubDirectoryResultState::Accepted));
    }

    #[test]
    fn parse_hub_directory_result_state_extracts_terminal_snapshot() {
        let result = MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from("cmd-123"),
            ),
            (
                MsgPackValue::from("status"),
                MsgPackValue::from("completed"),
            ),
            (
                MsgPackValue::from("result"),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("effective_connected_mode"),
                        MsgPackValue::from(true),
                    ),
                    (
                        MsgPackValue::from("items"),
                        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                            (
                                MsgPackValue::from("identity"),
                                MsgPackValue::from("11111111111111111111111111111111"),
                            ),
                            (
                                MsgPackValue::from("destination_hash"),
                                MsgPackValue::from("22222222222222222222222222222222"),
                            ),
                            (
                                MsgPackValue::from("display_name"),
                                MsgPackValue::from("Pixel"),
                            ),
                            (
                                MsgPackValue::from("announce_capabilities"),
                                MsgPackValue::Array(vec![
                                    MsgPackValue::from("r3akt"),
                                    MsgPackValue::from("telemetry"),
                                ]),
                            ),
                            (MsgPackValue::from("client_type"), MsgPackValue::from("rem")),
                            (
                                MsgPackValue::from("registered_mode"),
                                MsgPackValue::from("connected"),
                            ),
                            (
                                MsgPackValue::from("last_seen"),
                                MsgPackValue::from("2026-04-02T12:43:28Z"),
                            ),
                            (MsgPackValue::from("status"), MsgPackValue::from("active")),
                        ])]),
                    ),
                ]),
            ),
        ]);

        let parsed =
            parse_hub_directory_result_state(&result, "cmd-123", 456).expect("terminal result");

        let HubDirectoryResultState::Snapshot(snapshot) = parsed else {
            panic!("expected snapshot");
        };
        assert!(snapshot.effective_connected_mode);
        assert_eq!(snapshot.received_at_ms, 456);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(
            snapshot.items[0].destination_hash,
            "22222222222222222222222222222222"
        );
        assert_eq!(
            snapshot.items[0].announce_capabilities,
            vec!["r3akt".to_string(), "telemetry".to_string()]
        );
    }

    #[test]
    fn parse_mission_sync_metadata_accepts_full_rch_command_envelope() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
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
        assert!(metadata.is_mission_related());
    }

    #[test]
    fn prune_expired_buffered_acknowledgements_removes_only_stale_entries() {
        let now = now_ms();
        let mut pending = HashMap::from([
            (
                "fresh".to_string(),
                PendingLxmfAcknowledgement {
                    source_hex: "src-fresh".to_string(),
                    detail: None,
                    application_ack_state: ApplicationAckState::Accepted {},
                    buffered_at_ms: now,
                },
            ),
            (
                "stale".to_string(),
                PendingLxmfAcknowledgement {
                    source_hex: "src-stale".to_string(),
                    detail: None,
                    application_ack_state: ApplicationAckState::Accepted {},
                    buffered_at_ms: now
                        .saturating_sub(DEFAULT_BUFFERED_ACK_TTL.as_millis() as u64 + 1),
                },
            ),
        ]);

        let pruned = prune_expired_buffered_acknowledgements(&mut pending, now);

        assert_eq!(pruned, 1);
        assert!(pending.contains_key("fresh"));
        assert!(!pending.contains_key("stale"));
    }

    #[test]
    fn prune_expired_receipt_tracking_removes_only_stale_entries() {
        let now = now_ms();
        let mut tracking = HashMap::from([
            (
                "fresh".to_string(),
                ReceiptMessageTracking {
                    message_id_hex: "msg-fresh".to_string(),
                    recorded_at_ms: now,
                },
            ),
            (
                "stale".to_string(),
                ReceiptMessageTracking {
                    message_id_hex: "msg-stale".to_string(),
                    recorded_at_ms: now
                        .saturating_sub(DEFAULT_RECEIPT_TRACKING_TTL.as_millis() as u64 + 1),
                },
            ),
        ]);

        let pruned = prune_expired_receipt_tracking(&mut tracking, now);

        assert_eq!(pruned, 1);
        assert!(tracking.contains_key("fresh"));
        assert!(!tracking.contains_key("stale"));
    }

    #[tokio::test]
    async fn retry_backoff_releases_general_send_permit_before_sleep() {
        let permits = SendTaskPermits::with_limits(1, 1);
        let permits_for_retry = permits.clone();
        let (sleeping_tx, sleeping_rx) = oneshot::channel();

        tokio::spawn(async move {
            {
                let _permit = acquire_send_task_permit(&permits_for_retry, SendTaskClass::General)
                    .await
                    .expect("first attempt permit");
            }
            let _ = sleeping_tx.send(());
            tokio::time::sleep(Duration::from_millis(100)).await;
        });

        sleeping_rx.await.expect("retry task entered backoff");
        let permit = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::General),
        )
        .await
        .expect("general permit should be available during retry sleep")
        .expect("general permit acquisition should succeed");
        drop(permit);
    }

    #[tokio::test]
    async fn mission_destination_locks_serialize_same_destination() {
        let locks = MissionDestinationLocks::new();
        let first = locks
            .acquire("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .expect("first destination lock");

        let blocked = tokio::time::timeout(
            Duration::from_millis(50),
            locks.acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        )
        .await;
        assert!(
            blocked.is_err(),
            "same destination should wait for the first mission send to finish"
        );

        drop(first);
        let second = tokio::time::timeout(
            Duration::from_millis(50),
            locks.acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        )
        .await
        .expect("same destination should unblock after first send finishes")
        .expect("second destination lock");
        drop(second);
    }

    #[tokio::test]
    async fn mission_destination_locks_allow_different_destinations() {
        let locks = MissionDestinationLocks::new();
        let _first = locks
            .acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await
            .expect("first destination lock");

        let second = tokio::time::timeout(
            Duration::from_millis(50),
            locks.acquire("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        )
        .await
        .expect("different destinations should not block each other")
        .expect("second destination lock");
        drop(second);
    }

    #[tokio::test]
    async fn mission_sends_keep_reserved_capacity_when_general_pool_is_full() {
        let permits = SendTaskPermits::with_limits(1, 1);
        let _general = acquire_send_task_permit(&permits, SendTaskClass::General)
            .await
            .expect("saturate general pool");

        let mission = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::Mission),
        )
        .await
        .expect("mission permit should not wait on general pool saturation")
        .expect("mission permit acquisition should succeed");
        drop(mission);

        let blocked_general = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::General),
        )
        .await;
        assert!(
            blocked_general.is_err(),
            "general pool should remain saturated while the original permit is held"
        );
    }

    #[tokio::test]
    async fn propagation_mission_sends_do_not_block_direct_mission_capacity() {
        let permits = SendTaskPermits::with_limits(1, 1);
        let _propagation = acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation)
            .await
            .expect("saturate propagation mission pool");

        let direct = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::Mission),
        )
        .await
        .expect("direct mission permit should not wait on propagation pool saturation")
        .expect("direct mission permit acquisition should succeed");
        drop(direct);

        let blocked_propagation = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation),
        )
        .await;
        assert!(
            blocked_propagation.is_err(),
            "propagation mission pool should remain saturated while the original permit is held"
        );
    }

    #[test]
    fn direct_recovery_fallback_uses_dedicated_recovery_lane() {
        assert_eq!(
            SendTaskClass::Mission.direct_recovery_equivalent(),
            SendTaskClass::MissionRecovery
        );
        assert_eq!(
            SendTaskClass::MissionAck.direct_recovery_equivalent(),
            SendTaskClass::MissionRecovery
        );
        assert_eq!(
            SendTaskClass::MissionPropagation.direct_recovery_equivalent(),
            SendTaskClass::MissionRecovery
        );
        assert_eq!(
            SendTaskClass::MissionRecovery.direct_recovery_equivalent(),
            SendTaskClass::MissionRecovery
        );
        assert_eq!(
            SendTaskClass::General.direct_recovery_equivalent(),
            SendTaskClass::General
        );
    }

    #[test]
    fn sos_status_sends_use_dedicated_recovery_lane() {
        let metadata = MissionSyncMetadata {
            command_present: true,
            command_id: Some("sos:incident-1:active:123".to_string()),
            correlation_id: Some("incident-1".to_string()),
            command_type: Some("sos.status".to_string()),
            ..MissionSyncMetadata::default()
        };

        assert_eq!(
            SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::Auto {}),
            SendTaskClass::MissionRecovery
        );
        assert_eq!(
            SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::PropagationOnly {}),
            SendTaskClass::MissionRecovery
        );
    }

    #[test]
    fn accepted_result_sends_use_dedicated_ack_lane() {
        let metadata = MissionSyncMetadata {
            result_present: true,
            result_status: Some("accepted".to_string()),
            command_id: Some("cmd-accepted".to_string()),
            ..MissionSyncMetadata::default()
        };

        assert_eq!(
            SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::Auto {}),
            SendTaskClass::MissionAck
        );
    }

    #[tokio::test]
    async fn mission_recovery_sends_do_not_wait_on_saturated_mission_lanes() {
        let permits = SendTaskPermits::with_limits(1, 1);
        let _direct = acquire_send_task_permit(&permits, SendTaskClass::Mission)
            .await
            .expect("saturate direct mission pool");
        let _propagation = acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation)
            .await
            .expect("saturate propagation mission pool");

        let recovery = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::MissionRecovery),
        )
        .await
        .expect("recovery permit should not wait on direct or propagation saturation")
        .expect("recovery permit acquisition should succeed");

        let blocked_recovery = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::MissionRecovery),
        )
        .await;
        assert!(
            blocked_recovery.is_err(),
            "recovery pool should remain saturated while the original permit is held"
        );
        drop(recovery);
    }

    #[tokio::test]
    async fn accepted_ack_sends_do_not_wait_on_saturated_direct_mission_capacity() {
        let permits = SendTaskPermits::with_limits(1, 1);
        let _mission = acquire_send_task_permit(&permits, SendTaskClass::Mission)
            .await
            .expect("saturate direct mission pool");

        let ack = tokio::time::timeout(
            Duration::from_millis(50),
            acquire_send_task_permit(&permits, SendTaskClass::MissionAck),
        )
        .await
        .expect("accepted acknowledgement should not wait on mission pool saturation")
        .expect("accepted acknowledgement permit acquisition should succeed");
        drop(ack);
    }

    #[tokio::test]
    async fn managed_peer_links_dedupe_reconnect_and_clear_on_disconnect() {
        let links = ManagedPeerLinks::default();
        let target = ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        };

        links.add_desired(target.clone()).await;

        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(target.clone())
        );
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::AlreadyReconnecting
        );

        links.finish_reconnect(&target, Ok(())).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(target.clone())
        );

        links
            .remove_desired(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"])
            .await;
        assert_eq!(links.desired_targets().await, Vec::new());
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::NotDesired
        );
    }

    #[tokio::test]
    async fn managed_peer_links_keep_backoff_when_target_is_readded_without_new_route_evidence() {
        let links = ManagedPeerLinks::default();
        let target = ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        };

        links.add_desired(target.clone()).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(target.clone())
        );
        links
            .finish_reconnect(&target, Err("link failed".to_string()))
            .await;

        links.add_desired(target).await;

        match links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await
        {
            ManagedPeerReconnectStart::Backoff {
                last_failure_reason,
                ..
            } => assert_eq!(last_failure_reason.as_deref(), Some("link failed")),
            other => panic!("expected backoff, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fresh_rem_announce_clears_managed_link_backoff_for_new_connection_attempt() {
        let links = ManagedPeerLinks::default();
        let target = ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        };

        links.add_desired(target.clone()).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(target.clone())
        );
        links
            .finish_reconnect(&target, Err("link failed".to_string()))
            .await;
        match links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await
        {
            ManagedPeerReconnectStart::Backoff { .. } => {}
            other => panic!("expected backoff before fresh announce, got {other:?}"),
        }

        links
            .clear_failure("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await;

        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(target)
        );
    }

    #[tokio::test]
    async fn fresh_lxmf_target_replaces_app_reconnect_for_same_destination() {
        let links = ManagedPeerLinks::default();
        let app_target = ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::App,
        };
        let lxmf_target = ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        };

        links.add_desired(app_target.clone()).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(app_target.clone())
        );

        links.add_desired(lxmf_target.clone()).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(lxmf_target.clone())
        );

        links
            .finish_reconnect(&app_target, Err("app route failed".to_string()))
            .await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::AlreadyReconnecting
        );

        links.finish_reconnect(&lxmf_target, Ok(())).await;
        assert_eq!(
            links
                .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await,
            ManagedPeerReconnectStart::Started(lxmf_target)
        );
    }

    #[test]
    fn mission_delivery_failures_do_not_emit_global_send_bytes_error() {
        assert!(!should_emit_global_send_bytes_error(SendTaskClass::Mission));
        assert!(!should_emit_global_send_bytes_error(
            SendTaskClass::MissionAck
        ));
        assert!(!should_emit_global_send_bytes_error(
            SendTaskClass::MissionPropagation
        ));
        assert!(!should_emit_global_send_bytes_error(
            SendTaskClass::MissionRecovery
        ));
        assert!(should_emit_global_send_bytes_error(SendTaskClass::General));
    }

    fn send_peer(
        destination_hex: &str,
        identity_hex: Option<&str>,
        lxmf_destination_hex: Option<&str>,
        stale: bool,
        active_link: bool,
        announce_last_seen_at_ms: Option<u64>,
    ) -> sdkmsg::PeerRecord {
        sdkmsg::PeerRecord {
            destination_hex: destination_hex.to_string(),
            identity_hex: identity_hex.map(ToOwned::to_owned),
            lxmf_destination_hex: lxmf_destination_hex.map(ToOwned::to_owned),
            display_name: Some("Peer".to_string()),
            app_data: Some("R3AKT,EMergencyMessages".to_string()),
            state: if active_link {
                sdkmsg::PeerState::Connected
            } else {
                sdkmsg::PeerState::Disconnected
            },
            saved: false,
            stale,
            active_link,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: None,
            last_seen_at_ms: announce_last_seen_at_ms.unwrap_or_default(),
            announce_last_seen_at_ms,
            lxmf_last_seen_at_ms: lxmf_destination_hex.map(|_| now_ms()),
        }
    }

    #[test]
    fn direct_delivery_readiness_requires_active_link() {
        let announced_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(1),
        );
        let active_peer = send_peer(
            "dddddddddddddddddddddddddddddddd",
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            Some("ffffffffffffffffffffffffffffffff"),
            false,
            true,
            Some(1),
        );

        assert!(!sdk_peer_is_direct_delivery_ready(&announced_peer, false));
        assert!(!sdk_peer_is_direct_delivery_ready(&announced_peer, true));
        assert!(sdk_peer_is_direct_delivery_ready(&active_peer, true));
    }

    #[test]
    fn direct_delivery_rejects_fresh_route_without_active_link() {
        let mut inconsistent_connected_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(1),
        );
        inconsistent_connected_peer.state = sdkmsg::PeerState::Connected;

        assert!(!sdk_peer_is_directly_reachable(
            &inconsistent_connected_peer
        ));
        assert!(!sdk_peer_is_direct_delivery_ready(
            &inconsistent_connected_peer,
            true
        ));
    }

    #[test]
    fn direct_delivery_rejects_observed_lxmf_route_for_stale_peer() {
        let stale_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            None,
        );

        assert!(sdk_peer_has_observed_lxmf_delivery_route(&stale_peer));
        assert!(!sdk_peer_is_direct_delivery_ready(&stale_peer, true));
    }

    #[test]
    fn direct_delivery_rejects_current_app_peer_with_old_lxmf_timestamp_without_link() {
        let mut peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(now_ms()),
        );
        peer.lxmf_last_seen_at_ms =
            Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

        assert!(!sdk_peer_has_observed_lxmf_delivery_route(&peer));
        assert!(!sdk_peer_is_direct_delivery_ready(&peer, true));
    }

    #[test]
    fn direct_delivery_rejects_old_observed_lxmf_route_for_stale_peer() {
        let mut stale_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            None,
        );
        stale_peer.lxmf_last_seen_at_ms =
            Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

        assert!(!sdk_peer_has_observed_lxmf_delivery_route(&stale_peer));
        assert!(!sdk_peer_is_direct_delivery_ready(&stale_peer, true));
    }

    #[test]
    fn connected_auto_send_keeps_direct_retry_budget_even_with_relay() {
        assert_eq!(
            direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, false, None),
            LXMF_DIRECT_ATTEMPTS
        );
        assert_eq!(
            direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, true, Some(11)),
            LXMF_DIRECT_ATTEMPTS
        );
        assert_eq!(
            direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, true, Some(1)),
            LXMF_DIRECT_ATTEMPTS
        );
        assert_eq!(
            direct_attempt_budget_for_send(SendMode::Auto {}, false, true, false, false, Some(11)),
            LXMF_DIRECT_ATTEMPTS
        );
        assert_eq!(
            direct_attempt_budget_for_send(
                SendMode::DirectOnly {},
                true,
                true,
                false,
                false,
                Some(11)
            ),
            LXMF_DIRECT_ATTEMPTS
        );
    }

    #[test]
    fn announced_rem_lxmf_peers_are_managed_link_targets_without_save() {
        let announced_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(now_ms()),
        );

        assert_eq!(
            managed_peer_link_target(&announced_peer),
            Some(ManagedPeerLinkTarget {
                destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            })
        );
    }

    #[test]
    fn direct_delivery_health_blocks_and_restores_destinations_after_cooldown() {
        let health = DirectDeliveryHealth::default();
        let destinations = [
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ];

        assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 100));

        health.mark_unhealthy(destinations.iter().map(String::as_str), 200);

        assert!(!health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 150));
        assert!(!health.is_available("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 150));
        assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 201));

        health.mark_unhealthy(destinations.iter().map(String::as_str), 300);
        health.clear(destinations.iter().map(String::as_str));

        assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 250));
        assert!(health.is_available("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 250));
    }

    #[test]
    fn managed_peer_link_targets_include_saved_and_announced_lxmf_destinations() {
        let mut saved_online = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            true,
            Some(now_ms()),
        );
        saved_online.saved = true;
        let mut saved_stale = send_peer(
            "dddddddddddddddddddddddddddddddd",
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            Some("ffffffffffffffffffffffffffffffff"),
            true,
            false,
            None,
        );
        saved_stale.saved = true;
        let unsaved_online = send_peer(
            "11111111111111111111111111111111",
            Some("22222222222222222222222222222222"),
            Some("33333333333333333333333333333333"),
            false,
            true,
            Some(now_ms()),
        );

        assert_eq!(
            saved_peer_link_targets(&[saved_online, saved_stale, unsaved_online]),
            vec![
                ManagedPeerLinkTarget {
                    destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
                    kind: ManagedPeerLinkKind::LxmfDelivery,
                },
                ManagedPeerLinkTarget {
                    destination_hex: "ffffffffffffffffffffffffffffffff".to_string(),
                    kind: ManagedPeerLinkKind::LxmfDelivery,
                },
                ManagedPeerLinkTarget {
                    destination_hex: "33333333333333333333333333333333".to_string(),
                    kind: ManagedPeerLinkKind::LxmfDelivery,
                },
            ]
        );
    }

    #[test]
    fn saved_raw_lxmf_peer_without_separate_lxmf_destination_uses_lxmf_link_kind() {
        let mut saved_raw_lxmf_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            None,
            false,
            false,
            Some(now_ms()),
        );
        saved_raw_lxmf_peer.saved = true;
        saved_raw_lxmf_peer.lxmf_last_seen_at_ms = Some(now_ms());

        assert_eq!(
            managed_peer_link_target(&saved_raw_lxmf_peer),
            Some(ManagedPeerLinkTarget {
                destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            })
        );
    }

    #[test]
    fn high_hop_stale_saved_route_prefers_propagation_lane() {
        let mut stale_peer = send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            None,
        );
        stale_peer.saved = true;
        let mut current_peer = send_peer(
            "dddddddddddddddddddddddddddddddd",
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            Some("ffffffffffffffffffffffffffffffff"),
            false,
            false,
            Some(now_ms()),
        );
        current_peer.saved = true;
        let mut active_peer = send_peer(
            "11111111111111111111111111111111",
            Some("22222222222222222222222222222222"),
            Some("33333333333333333333333333333333"),
            false,
            true,
            Some(now_ms()),
        );
        active_peer.saved = true;

        assert!(saved_peer_stored_route_prefers_propagation(
            &stale_peer,
            true,
            Some(11),
        ));
        assert!(!saved_peer_stored_route_prefers_propagation(
            &stale_peer,
            true,
            Some(1),
        ));
        assert!(saved_peer_stored_route_prefers_propagation(
            &current_peer,
            true,
            Some(11),
        ));
        assert!(!saved_peer_stored_route_prefers_propagation(
            &active_peer,
            true,
            Some(11),
        ));
        assert!(!saved_peer_stored_route_prefers_propagation(
            &stale_peer,
            false,
            Some(11),
        ));
    }

    #[test]
    fn mission_direct_admission_delay_keeps_one_hop_targets_first() {
        assert_eq!(
            mission_direct_priority_delay_for_hops(Some(1)),
            Duration::ZERO
        );
        assert_eq!(
            mission_direct_priority_delay_for_hops(Some(2)),
            Duration::ZERO
        );
        assert!(
            mission_direct_priority_delay_for_hops(Some(5))
                < mission_direct_priority_delay_for_hops(Some(11))
        );
        assert_eq!(
            mission_direct_priority_delay_for_hops(Some(20)),
            MISSION_DIRECT_PRIORITY_MAX_DELAY
        );
    }

    #[test]
    fn auto_saved_peer_direct_failure_uses_propagation_when_relay_exists() {
        assert!(should_try_propagation_after_direct_failure(
            SendMode::Auto {},
            false,
            true,
            true,
            true,
        ));
        assert!(!should_try_propagation_after_direct_failure(
            SendMode::Auto {},
            false,
            true,
            true,
            false,
        ));
        assert!(!should_try_propagation_after_direct_failure(
            SendMode::DirectOnly {},
            false,
            true,
            true,
            true,
        ));
        assert!(!should_try_propagation_after_direct_failure(
            SendMode::Auto {},
            false,
            false,
            true,
            true,
        ));
        assert!(!should_try_propagation_after_direct_failure(
            SendMode::Auto {},
            false,
            true,
            false,
            true,
        ));
        assert!(!should_try_propagation_after_direct_failure(
            SendMode::Auto {},
            true,
            true,
            true,
            true,
        ));
    }

    #[test]
    fn send_destination_resolution_requires_current_peer() {
        let peers = vec![send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(1),
        )];

        assert_eq!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            )
            .expect("current app peer should resolve"),
            "cccccccccccccccccccccccccccccccc"
        );
        assert_eq!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            )
            .expect("current identity should resolve"),
            "cccccccccccccccccccccccccccccccc"
        );
        assert!(matches!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "dddddddddddddddddddddddddddddddd"
            ),
            Err(NodeError::NetworkError {})
        ));
        assert!(matches!(
            resolve_current_lxmf_destination_from_peers(peers.as_slice(), "not-a-destination"),
            Err(NodeError::InvalidConfig {})
        ));
    }

    #[test]
    fn send_destination_resolution_rejects_stale_or_unannounced_peers() {
        let peers = vec![
            send_peer(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some("cccccccccccccccccccccccccccccccc"),
                true,
                false,
                Some(1),
            ),
            send_peer(
                "dddddddddddddddddddddddddddddddd",
                Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
                Some("ffffffffffffffffffffffffffffffff"),
                false,
                false,
                None,
            ),
        ];

        assert!(matches!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            Err(NodeError::NetworkError {})
        ));
        assert!(matches!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "dddddddddddddddddddddddddddddddd"
            ),
            Err(NodeError::NetworkError {})
        ));
    }

    #[test]
    fn send_destination_resolution_uses_current_lxmf_route_for_stale_app_peer() {
        let peers = vec![
            send_peer(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some("cccccccccccccccccccccccccccccccc"),
                true,
                false,
                Some(1),
            ),
            send_peer(
                "cccccccccccccccccccccccccccccccc",
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some("cccccccccccccccccccccccccccccccc"),
                false,
                false,
                Some(2),
            ),
        ];

        assert_eq!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("current lxmf route should satisfy stale equivalent app peer"),
            "cccccccccccccccccccccccccccccccc"
        );
    }

    #[test]
    fn send_destination_resolution_rejects_stale_app_peer_without_current_route() {
        let peers = vec![
            send_peer(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some("cccccccccccccccccccccccccccccccc"),
                true,
                false,
                Some(1),
            ),
            send_peer(
                "cccccccccccccccccccccccccccccccc",
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some("cccccccccccccccccccccccccccccccc"),
                true,
                false,
                Some(2),
            ),
        ];

        assert!(matches!(
            resolve_current_lxmf_destination_from_peers(
                peers.as_slice(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            Err(NodeError::NetworkError {})
        ));
    }

    #[test]
    fn saved_peer_route_refresh_targets_saved_peers_without_known_delivery_route() {
        let mut messaging = sdkmsg::MessagingStore::new(30);
        let now = now_ms();
        messaging.mark_peer_saved("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true);
        messaging.mark_peer_saved("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", true);
        messaging.mark_peer_saved("cccccccccccccccccccccccccccccccc", false);
        messaging.record_resolution_result(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "dddddddddddddddddddddddddddddddd",
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            now,
        );

        assert_eq!(
            saved_peer_destinations_needing_route_refresh(&messaging),
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
        );
    }

    #[test]
    fn restore_saved_peer_management_marks_saved_peers_managed() {
        let mut messaging = sdkmsg::MessagingStore::new(30);
        let now = now_ms();
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            identity_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            destination_kind: "lxmf_delivery".to_string(),
            app_data: "R3AKT,EMergencyMessages,Telemetry;name=Pixel".to_string(),
            display_name: Some("Pixel".to_string()),
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: now,
        });
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            identity_hex: "dddddddddddddddddddddddddddddddd".to_string(),
            destination_kind: "lxmf_delivery".to_string(),
            app_data: "R3AKT,EMergencyMessages,Telemetry;name=Other".to_string(),
            display_name: Some("Other".to_string()),
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: now,
        });
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
            identity_hex: "ffffffffffffffffffffffffffffffff".to_string(),
            destination_kind: "lxmf_delivery".to_string(),
            app_data: "Sideband;name=NonRem".to_string(),
            display_name: Some("NonRem".to_string()),
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: now,
        });

        let restored = restore_saved_peer_management(
            &mut messaging,
            &[
                crate::types::SavedPeerRecord {
                    destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    label: Some("Pixel".to_string()),
                    saved_at_ms: now,
                    identity_hex: None,
                    lxmf_destination_hex: None,
                    app_data: None,
                    display_name: None,
                    last_route_seen_at_ms: None,
                    last_hops: None,
                },
                crate::types::SavedPeerRecord {
                    destination_hex: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                    label: Some("Pixel duplicate".to_string()),
                    saved_at_ms: now,
                    identity_hex: None,
                    lxmf_destination_hex: None,
                    app_data: None,
                    display_name: None,
                    last_route_seen_at_ms: None,
                    last_hops: None,
                },
                crate::types::SavedPeerRecord {
                    destination_hex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
                    label: Some("Non REM".to_string()),
                    saved_at_ms: now,
                    identity_hex: None,
                    lxmf_destination_hex: None,
                    app_data: None,
                    display_name: None,
                    last_route_seen_at_ms: None,
                    last_hops: None,
                },
            ],
        );

        assert_eq!(
            restored.route_request_destinations,
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
        );
        assert_eq!(
            restored.link_targets,
            vec![ManagedPeerLinkTarget {
                destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            }]
        );
        assert_eq!(
            restored.pruned_destinations,
            vec!["eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()]
        );
        let mut peers = messaging.list_peers();
        peers.sort_by(|left, right| left.destination_hex.cmp(&right.destination_hex));
        assert!(peers[0].saved);
        assert!(!peers[1].saved);
        assert!(!messaging.is_peer_saved("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
    }

    #[test]
    fn operator_announce_message_accepts_rch_hub_announces() {
        let message = operator_announce_message(
            AnnounceClass::RchHubServer {},
            false,
            Some("North Hub"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            2,
        )
        .expect("hub announce should be relevant");

        assert!(message.contains("RCH hub North Hub"));
        assert!(message.contains("dest=aaaaa..."));
        assert!(!message.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!message.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        assert!(!message.contains("id="));
    }

    #[test]
    fn operator_announce_message_accepts_rem_capable_lxmf_announces() {
        let message = operator_announce_message(
            AnnounceClass::LxmfDelivery {},
            true,
            Some("Pixel"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            1,
        )
        .expect("peer announce should be relevant");

        assert!(message.contains("[announce] Pixel"));
        assert!(!message.contains("REM peer"));
        assert!(message.contains("dest=aaaaa..."));
        assert!(!message.contains("id="));
        assert!(message.contains("hops=1"));
    }

    #[test]
    fn operator_announce_message_ignores_legacy_app_peer_announces() {
        let message = operator_announce_message(
            AnnounceClass::PeerApp {},
            false,
            Some("Pixel"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            1,
        );

        assert!(message.is_none());
    }

    #[test]
    fn effective_announce_interval_respects_reticulum_rate_limit() {
        assert_eq!(effective_announce_interval_seconds(0), 3600);
        assert_eq!(effective_announce_interval_seconds(60), 3600);
        assert_eq!(effective_announce_interval_seconds(1800), 3600);
        assert_eq!(effective_announce_interval_seconds(7200), 7200);
    }

    #[test]
    fn startup_announce_burst_leaves_reticulum_rate_limit_headroom() {
        assert_eq!(STARTUP_ANNOUNCE_DELAYS_SECS.len(), 3);
        assert_eq!(STARTUP_ANNOUNCE_DELAYS_SECS[0], 0);
    }

    #[cfg(target_os = "android")]
    #[test]
    fn rnode_ble_wiring_derives_kiss_and_native_settings_from_rem_settings() {
        let settings = RnodeSettingsRecord {
            enabled: true,
            connection_mode: RnodeConnectionMode::Ble.as_str().to_string(),
            peripheral_id: "AA:BB:CC:DD:EE:FF".to_string(),
            display_name: "Field RNode".to_string(),
            region: "EU868".to_string(),
            profile: "REM-MF-URBAN-v1".to_string(),
        };

        let wiring = rnode_ble_wiring_from_settings(&settings).expect("valid RNode wiring");

        assert_eq!(wiring.label, "rnode-ble:Field RNode");
        assert_eq!(wiring.native.peripheral_id, "AA:BB:CC:DD:EE:FF");
        assert!(wiring
            .native
            .peripheral_aliases
            .iter()
            .any(|alias| alias == "Field RNode"));
        assert_eq!(wiring.kiss.mtu, usize::from(wiring.lora.max_payload_bytes));
        assert_eq!(wiring.kiss.max_write_len, 20);
        assert_eq!(wiring.kiss.read_frame_timeout, RNODE_BLE_READ_FRAME_TIMEOUT);
        assert!(!wiring.kiss.initial_frames.is_empty());
        assert!(!wiring.kiss.deferred_frames.is_empty());
        assert!(!wiring.kiss.shutdown_frames.is_empty());
    }

    #[cfg(target_os = "android")]
    #[test]
    fn rnode_ble_wiring_falls_back_to_peripheral_label_without_display_name() {
        let settings = RnodeSettingsRecord {
            enabled: true,
            connection_mode: RnodeConnectionMode::Ble.as_str().to_string(),
            peripheral_id: "AA:BB:CC:DD:EE:FF".to_string(),
            display_name: " ".to_string(),
            region: "US915".to_string(),
            profile: "REM-LF-RURAL-v1".to_string(),
        };

        let wiring = rnode_ble_wiring_from_settings(&settings).expect("valid RNode wiring");

        assert_eq!(wiring.label, "rnode-ble:AA:BB:CC:DD:EE:FF");
        assert!(wiring.native.peripheral_aliases.is_empty());
        assert_eq!(wiring.kiss.mtu, usize::from(wiring.lora.max_payload_bytes));
        assert!(!wiring.kiss.initial_frames.is_empty());
        assert!(!wiring.kiss.deferred_frames.is_empty());
    }

    #[tokio::test]
    async fn rem_lxmf_announce_path_response_keeps_capability_app_data() {
        use reticulum::transport::destination::{DestinationAnnounce, PlainInputDestination};
        use reticulum::transport::identity::EmptyIdentity;
        use reticulum::transport::iface::{IfaceSource, RxMessage, TxMessageType};
        use reticulum::transport::packet::{
            ContextFlag, DestinationType, Header, HeaderType, PacketContext, PropagationType,
        };
        use tokio::time::timeout;

        let identity = PrivateIdentity::new_from_rand(OsRng);
        let config = TransportConfig::new("test", &identity, true);
        let mut transport = Transport::new(config);
        let mut iface_channel = transport.iface_manager().lock().await.new_channel(16);
        let app_destination = transport
            .add_destination(
                identity.clone(),
                DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
            )
            .await;
        let lxmf_destination = transport
            .add_destination(
                identity,
                DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
            )
            .await;
        let transport = Arc::new(transport);
        let capabilities = Arc::new(TokioMutex::new(
            "R3AKT,EMergencyMessages,Telemetry;name=Pixel".to_string(),
        ));

        announce_destinations(
            &transport,
            &app_destination,
            &lxmf_destination,
            &capabilities,
            "test",
        )
        .await;

        let first_announce = timeout(Duration::from_millis(200), iface_channel.tx_channel.recv())
            .await
            .expect("expected outbound announce")
            .expect("tx channel open");
        assert!(matches!(
            first_announce.tx_type,
            TxMessageType::Broadcast(None)
        ));

        let lxmf_destination_hash = lxmf_destination.lock().await.desc.address_hash;
        let mut request_data = PacketDataBuffer::new_from_slice(lxmf_destination_hash.as_slice());
        request_data.safe_write(&[0x44; 16]);
        let path_request_destination = PlainInputDestination::new(
            EmptyIdentity {},
            DestinationName::new("rnstransport", "path.request"),
        )
        .desc
        .address_hash;
        let path_request = Packet {
            header: Header {
                ifac_flag: reticulum::transport::packet::IfacFlag::Open,
                header_type: HeaderType::Type1,
                context_flag: ContextFlag::Unset,
                propagation_type: PropagationType::Broadcast,
                destination_type: DestinationType::Plain,
                packet_type: PacketType::Data,
                hops: 0,
            },
            ifac: None,
            destination: path_request_destination,
            transport: None,
            context: PacketContext::None,
            data: request_data,
        };

        iface_channel
            .rx_channel
            .send(RxMessage {
                address: iface_channel.address,
                packet: path_request,
                source: IfaceSource::None,
            })
            .await
            .expect("path request enqueued");

        let response = timeout(Duration::from_millis(500), iface_channel.tx_channel.recv())
            .await
            .expect("expected path response")
            .expect("tx channel open");
        assert!(
            matches!(response.tx_type, TxMessageType::Direct(iface) if iface == iface_channel.address)
        );
        assert_eq!(response.packet.destination, lxmf_destination_hash);
        assert_eq!(response.packet.context, PacketContext::PathResponse);

        let announce = DestinationAnnounce::validate(&response.packet).expect("path announce");
        let app_data = std::str::from_utf8(announce.app_data).expect("capabilities are text");
        assert!(app_data_has_rem_peer_capabilities(app_data));
    }

    #[test]
    fn operator_announce_message_ignores_regular_lxmf_announces() {
        let message = operator_announce_message(
            AnnounceClass::LxmfDelivery {},
            false,
            Some("LXMF Chat"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            1,
        );

        assert!(message.is_none());
    }
}
