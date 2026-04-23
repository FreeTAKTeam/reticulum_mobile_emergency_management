use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::announce_compat::{
    display_name_from_delivery_app_data, encode_delivery_display_name_app_data,
};
use crate::lxmf_fields::FIELD_COMMANDS;
use crate::messaging_compat as sdkmsg;
use crate::mission_sync::{parse_mission_sync_metadata, MissionSyncMetadata};
use crate::sos::{location_from_alert, received_alert_from_sos};
use crate::sos_fields::{extract_text_coordinates, looks_like_sos_text, parse_sos_fields};
use crossbeam_channel as cb;
use fs_err as fs;
#[cfg(feature = "legacy-lxmf-runtime")]
use log::error;
use log::{debug, info};
use lxmf::message::Message as LxmfMessage;
use lxmf::message::WireMessage as LxmfWireMessage;
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
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex as TokioMutex, OwnedSemaphorePermit, Semaphore};

#[path = "runtime_projection.rs"]
mod runtime_projection;

use crate::app_state::{
    canonicalize_chat_message, checklist_task_status_for, find_checklist_task_mut,
    normalize_checklist_record, normalize_optional_string, AppStateStore,
};
use crate::event_bus::EventBus;
use crate::sdk_bridge::{RuntimeLxmfSdk, SdkTransportState};
use crate::types::{
    AnnounceClass, AnnounceRecord, ChecklistCellRecord, ChecklistColumnRecord, ChecklistColumnType,
    ChecklistRecord, ChecklistSyncState, ChecklistTaskRecord, ChecklistTaskStatus,
    ChecklistUserTaskStatus, ConversationRecord, EamProjectionRecord, EamSourceRecord,
    EventProjectionRecord, HubDirectoryPeerRecord, HubDirectorySnapshot, HubMode, LogLevel,
    LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfDeliveryUpdate,
    LxmfFallbackStage, MessageDirection, MessageMethod, MessageRecord, MessageState, NodeConfig,
    NodeError, NodeEvent, NodeStatus, OperationalNotice, PeerChange, PeerRecord, PeerState,
    ProjectionScope, SendLxmfRequest, SendMode, SendOutcome, SosDeviceTelemetryRecord,
    SosMessageKind, SyncPhase, SyncStatus, TelemetryPositionRecord,
};

use self::runtime_projection::RuntimeProjectionJournal;

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const LXMF_PROPAGATION_NAME: (&str, &str) = ("lxmf", "propagation");
const PASSIVE_PEER_RESOLUTION_MIN_INTERVAL_MS: u64 = 10_000;
const RCH_SERVER_FEATURE_CAPABILITIES: [&str; 5] = [
    "topic_broker",
    "group_chat",
    "attachments",
    "telemetry_relay",
    "tak_bridge",
];

const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);
const DEFAULT_LXMF_ACK_TIMEOUT: Duration = Duration::from_secs(90);
const DEFAULT_BUFFERED_ACK_TTL: Duration = Duration::from_secs(5 * 60);
const DEFAULT_RECEIPT_TRACKING_TTL: Duration = Duration::from_secs(10 * 60);
const SEND_TASK_CONCURRENCY_LIMIT: usize = 8;
const MISSION_SEND_TASK_RESERVED_LIMIT: usize = 2;
const GENERAL_SEND_TASK_CONCURRENCY_LIMIT: usize =
    SEND_TASK_CONCURRENCY_LIMIT - MISSION_SEND_TASK_RESERVED_LIMIT;
const DEFAULT_EAM_GROUP_NAME: &str = "YELLOW";

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
    Upsert(EamProjectionRecord),
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
    if envelope.command_type != "mission.registry.eam.upsert" {
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
        return Some(EamCommandAction::Upsert(projection));
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
    Some(EamCommandAction::Upsert(record))
}

fn eam_command_action_from_fields(
    fields_bytes: &[u8],
    received_at_ms: u64,
) -> Option<EamCommandAction> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type =
            msgpack_get_named(command_map, &["command_type"]).and_then(msgpack_string)?;
        if command_type == "mission.registry.eam.delete" {
            let args = msgpack_get_named(command_map, &["args"]).and_then(msgpack_map_entries)?;
            let callsign = msgpack_get_named(args, &["callsign"]).and_then(msgpack_string)?;
            if callsign.trim().is_empty() {
                return None;
            }
            let deleted_at_ms = msgpack_get_named(args, &["deleted_at_ms"])
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
        let args = msgpack_get_named(command_map, &["args"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(args, &["source"]).and_then(msgpack_map_entries);
        let mut record = EamProjectionRecord {
            callsign: msgpack_get_named(args, &["callsign"]).and_then(msgpack_string)?,
            group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
            security_status: msgpack_get_named(args, &["security_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            capability_status: msgpack_get_named(args, &["capability_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            preparedness_status: msgpack_get_named(args, &["preparedness_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            medical_status: msgpack_get_named(args, &["medical_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            mobility_status: msgpack_get_named(args, &["mobility_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            comms_status: msgpack_get_named(args, &["comms_status"])
                .and_then(msgpack_string)
                .unwrap_or_else(|| "Unknown".to_string()),
            notes: msgpack_get_named(args, &["notes"]).and_then(msgpack_string),
            updated_at_ms: received_at_ms,
            deleted_at_ms: None,
            eam_uid: msgpack_get_named(args, &["eam_uid"]).and_then(msgpack_string),
            team_member_uid: msgpack_get_named(args, &["team_member_uid"]).and_then(msgpack_string),
            team_uid: msgpack_get_named(args, &["team_uid"]).and_then(msgpack_string),
            reported_at: msgpack_get_named(args, &["reported_at"]).and_then(msgpack_string),
            reported_by: msgpack_get_named(args, &["reported_by"]).and_then(msgpack_string),
            overall_status: None,
            confidence: msgpack_get_named(args, &["confidence"]).and_then(msgpack_f64),
            ttl_seconds: msgpack_get_named(args, &["ttl_seconds"]).and_then(msgpack_u64),
            source: source.map(|source_map| EamSourceRecord {
                rns_identity: msgpack_get_named(source_map, &["rns_identity"])
                    .and_then(msgpack_string)
                    .unwrap_or_default(),
                display_name: msgpack_get_named(source_map, &["display_name"])
                    .and_then(msgpack_string),
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
        return Some(EamCommandAction::Upsert(record));
    }

    None
}

async fn persist_received_eam_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    body_utf8: &str,
) {
    let received_at_ms = now_ms();
    let parsed_from_fields =
        fields_bytes.and_then(|value| eam_command_action_from_fields(value, received_at_ms));
    if metadata.is_none() && parsed_from_fields.is_none() {
        return;
    }
    if !metadata
        .and_then(|value| value.command_type.as_deref())
        .is_some_and(|value| {
            value == "mission.registry.eam.upsert" || value == "mission.registry.eam.delete"
        })
        && parsed_from_fields.is_none()
    {
        return;
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
        return;
    };

    match action {
        EamCommandAction::Upsert(record) => match state.app_state.upsert_eam(&record) {
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
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to persist inbound eam callsign={} reason={}",
                        record.callsign, err
                    ),
                });
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
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to delete inbound eam callsign={} reason={}",
                        callsign, err
                    ),
                });
            }
        },
    }
}

fn event_projection_from_fields(
    fields_bytes: &[u8],
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
        let command_type =
            msgpack_get_named(command_map, &["command_type"]).and_then(msgpack_string)?;
        if command_type != "mission.registry.log_entry.upsert" {
            continue;
        }
        let args = msgpack_get_named(command_map, &["args"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(command_map, &["source"]).and_then(msgpack_map_entries);
        let uid = msgpack_get_named(args, &["entry_uid"]).and_then(msgpack_string)?;
        let mission_uid = msgpack_get_named(args, &["mission_uid"]).and_then(msgpack_string)?;
        let content = msgpack_get_named(args, &["content"]).and_then(msgpack_string)?;
        let callsign = msgpack_get_named(args, &["callsign"]).and_then(msgpack_string)?;
        let timestamp = msgpack_get_named(command_map, &["timestamp"])
            .and_then(msgpack_string)
            .or_else(|| msgpack_get_named(args, &["server_time"]).and_then(msgpack_string))
            .or_else(|| msgpack_get_named(args, &["client_time"]).and_then(msgpack_string))?;
        let command_id =
            msgpack_get_named(command_map, &["command_id"]).and_then(msgpack_string)?;
        let source_identity = msgpack_get_named(args, &["source_identity"])
            .and_then(msgpack_string)
            .or_else(|| {
                source.and_then(|source_map| {
                    msgpack_get_named(source_map, &["rns_identity"]).and_then(msgpack_string)
                })
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
        let topics = msgpack_get_named(command_map, &["topics"])
            .and_then(msgpack_string_vec)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| vec![mission_uid.clone()]);
        return Some(EventProjectionRecord {
            uid,
            command_id,
            source_identity,
            source_display_name: msgpack_get_named(args, &["source_display_name"])
                .and_then(msgpack_string)
                .or_else(|| {
                    source.and_then(|source_map| {
                        msgpack_get_named(source_map, &["display_name"]).and_then(msgpack_string)
                    })
                }),
            timestamp,
            command_type,
            mission_uid,
            content,
            callsign,
            server_time: msgpack_get_named(args, &["server_time"]).and_then(msgpack_string),
            client_time: msgpack_get_named(args, &["client_time"]).and_then(msgpack_string),
            keywords: msgpack_get_named(args, &["keywords"])
                .and_then(msgpack_string_vec)
                .unwrap_or_default(),
            content_hashes: msgpack_get_named(args, &["content_hashes"])
                .and_then(msgpack_string_vec)
                .unwrap_or_default(),
            updated_at_ms: received_at_ms,
            deleted_at_ms: None,
            correlation_id: msgpack_get_named(command_map, &["correlation_id"])
                .and_then(msgpack_string),
            topics,
        });
    }

    None
}

async fn persist_received_event_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
) {
    let parsed_from_fields =
        fields_bytes.and_then(|value| event_projection_from_fields(value, now_ms()));
    if metadata.is_none() && parsed_from_fields.is_none() {
        return;
    }
    if !metadata
        .and_then(|value| value.command_type.as_deref())
        .is_some_and(|value| value == "mission.registry.log_entry.upsert")
        && parsed_from_fields.is_none()
    {
        return;
    }

    let Some(record) = parsed_from_fields else {
        return;
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
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound event uid={} reason={}",
                    record.uid, err
                ),
            });
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
    let source = msgpack_get_named(command_map, &["source"]).and_then(msgpack_map_entries)?;
    msgpack_get_named(source, &["rns_identity"]).and_then(msgpack_string)
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
    state: &NodeRuntimeState,
    bus: &EventBus,
    checklist: &ChecklistRecord,
    reason: &str,
) {
    match state.app_state.upsert_checklist(checklist, reason) {
        Ok(invalidations) => emit_checklist_invalidations(bus, invalidations),
        Err(err) => bus.emit(NodeEvent::Error {
            code: "IoError".to_string(),
            message: format!(
                "failed to persist inbound checklist uid={} reason={reason} error={err}",
                checklist.uid
            ),
        }),
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
        updated_at: Some(timestamp.to_string()),
        deleted_at: None,
        uploaded_at: None,
        participant_rns_identities: source_identity
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
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
    incoming.participant_rns_identities = merge_uploaded_participants(
        Vec::new(),
        incoming.participant_rns_identities,
        source_identity,
    );
    incoming.sync_state = ChecklistSyncState::Synced {};
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
    merged.feed_publications =
        merge_uploaded_feed_publications(existing.feed_publications, incoming.feed_publications);
    normalize_checklist_record(&mut merged);
    Some(merged)
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

fn persist_received_checklist_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    _metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
) {
    let Some(fields_bytes) = fields_bytes else {
        return;
    };
    let fields = match rmp_serde::from_slice::<MsgPackValue>(fields_bytes) {
        Ok(value) => value,
        Err(_) => return,
    };
    let Some(field_entries) = msgpack_map_entries(&fields) else {
        return;
    };
    let Some(commands) = msgpack_get_indexed(field_entries, FIELD_COMMANDS) else {
        return;
    };
    let MsgPackValue::Array(command_entries) = commands else {
        return;
    };

    for command in command_entries {
        let Some(command_map) = msgpack_map_entries(command) else {
            continue;
        };
        let Some(command_type) =
            msgpack_get_named(command_map, &["command_type"]).and_then(msgpack_string)
        else {
            continue;
        };
        if !command_type.starts_with("checklist.") {
            continue;
        }
        let timestamp = msgpack_get_named(command_map, &["timestamp"])
            .and_then(msgpack_string)
            .unwrap_or_else(current_timestamp_rfc3339);
        let source_identity = checklist_command_source_identity(command_map);
        let Some(args) = msgpack_get_named(command_map, &["args"]).and_then(msgpack_map_entries)
        else {
            continue;
        };

        match command_type.as_str() {
            "checklist.create.online" => {
                let checklist_uid = msgpack_get_named(args, &["checklist_uid"])
                    .and_then(msgpack_string)
                    .or_else(|| {
                        msgpack_get_named(command_map, &["command_id"])
                            .and_then(msgpack_string)
                            .map(|value| value.trim_start_matches("cmd-").to_string())
                    });
                let Some(checklist_uid) = checklist_uid else {
                    continue;
                };
                let Some(mission_uid) =
                    msgpack_get_named(args, &["mission_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(template_uid) =
                    msgpack_get_named(args, &["template_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(name) = msgpack_get_named(args, &["name"]).and_then(msgpack_string) else {
                    continue;
                };
                let description = msgpack_get_named(args, &["description"])
                    .and_then(msgpack_string)
                    .unwrap_or_default();
                let start_time = msgpack_get_named(args, &["start_time"]).and_then(msgpack_string);
                let existing = match state.app_state.get_checklist_any(checklist_uid.as_str()) {
                    Ok(value) => value,
                    Err(_) => None,
                };
                if existing.as_ref().is_some_and(|record| {
                    !incoming_timestamp_is_newer(record.updated_at.as_deref(), timestamp.as_str())
                        || record.deleted_at.as_deref().is_some_and(|deleted_at| {
                            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                        })
                }) {
                    continue;
                }
                let reused_placeholder = existing
                    .as_ref()
                    .is_some_and(is_hidden_placeholder_checklist);
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
                checklist.updated_at = Some(timestamp.clone());
                checklist.deleted_at = None;
                if reused_placeholder || checklist.created_at.is_none() {
                    checklist.created_at = Some(timestamp.clone());
                }
                if checklist
                    .created_by_team_member_rns_identity
                    .trim()
                    .is_empty()
                {
                    checklist.created_by_team_member_rns_identity =
                        source_identity.unwrap_or_default();
                }
                if let Some(source_identity) = checklist_command_source_identity(command_map) {
                    if !checklist
                        .participant_rns_identities
                        .iter()
                        .any(|value| value == &source_identity)
                    {
                        checklist.participant_rns_identities.push(source_identity);
                    }
                }
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-create");
            }
            "checklist.upload" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(snapshot_json) =
                    msgpack_get_named(command_map, &["snapshot_json"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Ok(mut checklist) =
                    serde_json::from_str::<ChecklistRecord>(snapshot_json.as_str())
                else {
                    continue;
                };
                checklist.uid = checklist_uid.clone();
                let existing = match state.app_state.get_checklist_any(checklist_uid.as_str()) {
                    Ok(value) => value,
                    Err(_) => None,
                };
                let Some(checklist) = merge_uploaded_checklist_snapshot(
                    existing,
                    checklist,
                    timestamp.as_str(),
                    source_identity.as_deref(),
                ) else {
                    continue;
                };
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-upload");
            }
            "checklist.update" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                let Some(patch) = msgpack_get_named(args, &["patch"]).and_then(msgpack_map_entries)
                else {
                    continue;
                };
                if let Some(value) =
                    msgpack_get_named(patch, &["mission_uid"]).and_then(msgpack_string)
                {
                    checklist.mission_uid = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) =
                    msgpack_get_named(patch, &["template_uid"]).and_then(msgpack_string)
                {
                    checklist.template_uid = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) = msgpack_get_named(patch, &["name"]).and_then(msgpack_string) {
                    checklist.name = value.trim().to_string();
                }
                if let Some(value) =
                    msgpack_get_named(patch, &["description"]).and_then(msgpack_string)
                {
                    checklist.description = value.trim().to_string();
                }
                if let Some(value) =
                    msgpack_get_named(patch, &["start_time"]).and_then(msgpack_string)
                {
                    checklist.start_time = normalize_optional_string(Some(value.as_str()));
                }
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-update");
            }
            "checklist.delete" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let existing = state
                    .app_state
                    .get_checklist_any(checklist_uid.as_str())
                    .ok()
                    .flatten();
                if existing.as_ref().is_some_and(|checklist| {
                    !incoming_timestamp_is_newer(
                        checklist.updated_at.as_deref(),
                        timestamp.as_str(),
                    ) || checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
                        !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                    })
                }) {
                    continue;
                }
                let mut checklist = existing.unwrap_or_else(|| {
                    blank_checklist_record(checklist_uid.as_str(), timestamp.as_str(), None)
                });
                checklist.deleted_at = Some(timestamp.clone());
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-delete");
            }
            "checklist.task.row.add" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_named(args, &["task_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(number) = msgpack_get_named(args, &["number"]).and_then(msgpack_u64)
                else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                    if !incoming_timestamp_is_newer(task.updated_at.as_deref(), timestamp.as_str())
                        || task.deleted_at.as_deref().is_some_and(|deleted_at| {
                            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
                        })
                    {
                        continue;
                    }
                }
                let due_relative_minutes = msgpack_get_named(args, &["due_relative_minutes"])
                    .and_then(msgpack_u64)
                    .map(|value| value as u32);
                let legacy_value =
                    msgpack_get_named(args, &["legacy_value"]).and_then(msgpack_string);
                if let Some(task) = checklist
                    .tasks
                    .iter_mut()
                    .find(|task| task.task_uid == task_uid)
                {
                    task.number = number as u32;
                    task.custom_status = None;
                    task.due_relative_minutes = due_relative_minutes;
                    task.due_dtg = None;
                    task.notes = None;
                    task.row_background_color = None;
                    task.line_break_enabled = false;
                    task.legacy_value = legacy_value;
                    task.user_status = ChecklistUserTaskStatus::Pending {};
                    task.task_status = ChecklistTaskStatus::Pending {};
                    task.is_late = false;
                    task.completed_at = None;
                    task.completed_by_team_member_rns_identity = None;
                    task.deleted_at = None;
                    task.cells = blank_task_cells(checklist.columns.as_slice(), task_uid.as_str());
                    task.updated_at = Some(timestamp.clone());
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
                        due_dtg: None,
                        notes: None,
                        row_background_color: None,
                        line_break_enabled: false,
                        completed_at: None,
                        completed_by_team_member_rns_identity: None,
                        legacy_value,
                        cells,
                    });
                }
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-task-row-add");
            }
            "checklist.task.row.delete" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_named(args, &["task_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let existing = state
                    .app_state
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
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(
                    state,
                    bus,
                    &checklist,
                    "checklist-received-task-row-delete",
                );
            }
            "checklist.task.status.set" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_named(args, &["task_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
                    checklist.tasks.push(placeholder_task_record(
                        task_uid.as_str(),
                        timestamp.as_str(),
                    ));
                }
                let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
                    continue;
                };
                if !incoming_timestamp_is_newer(task.updated_at.as_deref(), timestamp.as_str()) {
                    continue;
                }
                let user_status = match msgpack_get_named(args, &["user_status"])
                    .and_then(msgpack_string)
                    .as_deref()
                {
                    Some("COMPLETE") => ChecklistUserTaskStatus::Complete {},
                    _ => ChecklistUserTaskStatus::Pending {},
                };
                task.user_status = user_status;
                task.task_status = checklist_task_status_for(task.user_status, task.is_late);
                task.updated_at = Some(timestamp.clone());
                if task.task_status.is_complete() {
                    task.completed_at = Some(timestamp.clone());
                    task.completed_by_team_member_rns_identity =
                        msgpack_get_named(args, &["changed_by_team_member_rns_identity"])
                            .and_then(msgpack_string)
                            .or_else(|| source_identity.clone());
                } else {
                    task.completed_at = None;
                    task.completed_by_team_member_rns_identity = None;
                }
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-task-status");
            }
            "checklist.task.row.style.set" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_named(args, &["task_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
                    checklist.tasks.push(placeholder_task_record(
                        task_uid.as_str(),
                        timestamp.as_str(),
                    ));
                }
                let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
                    continue;
                };
                if !incoming_timestamp_is_newer(task.updated_at.as_deref(), timestamp.as_str()) {
                    continue;
                }
                if let Some(value) =
                    msgpack_get_named(args, &["row_background_color"]).and_then(msgpack_string)
                {
                    task.row_background_color = normalize_optional_string(Some(value.as_str()));
                }
                if let Some(value) =
                    msgpack_get_named(args, &["line_break_enabled"]).and_then(msgpack_bool)
                {
                    task.line_break_enabled = value;
                }
                task.updated_at = Some(timestamp.clone());
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(
                    state,
                    bus,
                    &checklist,
                    "checklist-received-task-row-style",
                );
            }
            "checklist.task.cell.set" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(task_uid) =
                    msgpack_get_named(args, &["task_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(column_uid) =
                    msgpack_get_named(args, &["column_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(value) = msgpack_get_named(args, &["value"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                        msgpack_get_named(args, &["updated_by_team_member_rns_identity"])
                            .and_then(msgpack_string)
                            .or_else(|| source_identity.clone());
                } else {
                    task.cells.push(ChecklistCellRecord {
                        cell_uid: format!("{}:{column_uid}", task.task_uid),
                        task_uid: task.task_uid.clone(),
                        column_uid: column_uid.clone(),
                        value: Some(value),
                        updated_at: Some(timestamp.clone()),
                        updated_by_team_member_rns_identity: msgpack_get_named(
                            args,
                            &["updated_by_team_member_rns_identity"],
                        )
                        .and_then(msgpack_string)
                        .or_else(|| source_identity.clone()),
                    });
                }
                task.updated_at = Some(timestamp.clone());
                checklist.updated_at = Some(timestamp.clone());
                normalize_checklist_record(&mut checklist);
                upsert_inbound_checklist(state, bus, &checklist, "checklist-received-task-cell");
            }
            "checklist.join" => {
                let Some(checklist_uid) =
                    msgpack_get_named(args, &["checklist_uid"]).and_then(msgpack_string)
                else {
                    continue;
                };
                let Some(source_identity) = source_identity.clone() else {
                    continue;
                };
                let mut checklist = state
                    .app_state
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
                    checklist.participant_rns_identities.push(source_identity);
                    checklist.updated_at = Some(timestamp.clone());
                    normalize_checklist_record(&mut checklist);
                    upsert_inbound_checklist(state, bus, &checklist, "checklist-received-join");
                }
            }
            _ => {}
        }
    }
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
    command_id: String,
    source: MissionWireSource,
    timestamp: String,
    command_type: String,
    args: T,
    correlation_id: Option<String>,
    topics: Vec<String>,
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

fn msgpack_get_indexed<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    key: i64,
) -> Option<&'a MsgPackValue> {
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

fn msgpack_string_vec(value: &MsgPackValue) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(entries.iter().filter_map(msgpack_string).collect())
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

async fn announce_destinations(
    transport: &Arc<Transport>,
    app_destination: &Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
    lxmf_destination: &Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<String>>,
    reason: &str,
) {
    let caps = announce_capabilities.lock().await.clone();
    let app_hex = app_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let lxmf_hex = lxmf_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let delivery_app_data = delivery_display_name_app_data(caps.as_str());
    info!(
        "[announce] sending reason={} app={} lxmf={}",
        reason, app_hex, lxmf_hex,
    );
    transport
        .send_announce(app_destination, Some(caps.as_bytes()))
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    transport
        .send_announce(lxmf_destination, delivery_app_data.as_deref())
        .await;
}

fn delivery_display_name_app_data(capability_string: &str) -> Option<Vec<u8>> {
    capability_string
        .split(';')
        .map(str::trim)
        .find_map(|token| token.strip_prefix("name="))
        .and_then(encode_delivery_display_name_app_data)
}

fn announce_destination_kind_from_name_hash(name_hash: &[u8]) -> &'static str {
    let app_name = DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1);
    if name_hash == app_name.as_name_hash_slice() {
        return "app";
    }

    let lxmf_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    if name_hash == lxmf_name.as_name_hash_slice() {
        return "lxmf_delivery";
    }

    let propagation_name = DestinationName::new(LXMF_PROPAGATION_NAME.0, LXMF_PROPAGATION_NAME.1);
    if name_hash == propagation_name.as_name_hash_slice() {
        return "lxmf_propagation";
    }

    "other"
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

fn decode_percent_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
                let byte = u8::from_str_radix(hex, 16).ok()?;
                decoded.push(byte);
                index += 3;
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            value => {
                decoded.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn normalize_display_name(value: &str) -> Option<String> {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(64).collect())
    }
}

fn announce_display_name_from_msgpack_value(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().and_then(normalize_display_name),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone())
            .ok()
            .as_deref()
            .and_then(normalize_display_name),
        _ => None,
    }
}

fn parse_announce_payload_msgpack(bytes: &[u8]) -> Option<MsgPackValue> {
    rmp_serde::from_slice::<MsgPackValue>(bytes).ok()
}

fn extract_msgpack_announce_display_name(value: &MsgPackValue) -> Option<String> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    entries
        .first()
        .and_then(announce_display_name_from_msgpack_value)
}

fn extract_msgpack_capability_tokens(value: &MsgPackValue) -> Vec<String> {
    match value {
        MsgPackValue::Map(entries) => entries
            .iter()
            .find_map(|(key, value)| {
                if matches!(key, MsgPackValue::String(actual) if actual.as_str() == Some("caps") || actual.as_str() == Some("announce_capabilities")) {
                    Some(match value {
                        MsgPackValue::Array(items) => items
                            .iter()
                            .filter_map(msgpack_string)
                            .map(|token| token.to_ascii_lowercase())
                            .collect(),
                        _ => Vec::new(),
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default(),
        MsgPackValue::Array(entries) => entries
            .iter()
            .find_map(|entry| match entry {
                MsgPackValue::Map(_) => Some(extract_msgpack_capability_tokens(entry)),
                MsgPackValue::Binary(bytes) => {
                    parse_announce_payload_msgpack(bytes).map(|nested| extract_msgpack_capability_tokens(&nested))
                }
                _ => None,
            })
            .unwrap_or_default(),
        MsgPackValue::Binary(bytes) => parse_announce_payload_msgpack(bytes)
            .map(|nested| extract_msgpack_capability_tokens(&nested))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn announce_metadata_from_app_data(app_data: &str) -> (Option<String>, Vec<String>) {
    let display_name = app_data
        .split(|ch: char| ch == ',' || ch == ';')
        .map(str::trim)
        .find_map(|token| token.strip_prefix("name="))
        .and_then(decode_percent_component)
        .as_deref()
        .and_then(normalize_display_name);
    let text_tokens = parse_capability_tokens(app_data);
    if display_name.is_some() || !text_tokens.is_empty() {
        return (display_name, text_tokens);
    }

    let Some(bytes) = hex::decode(app_data).ok() else {
        return (None, Vec::new());
    };
    let Some(payload) = parse_announce_payload_msgpack(bytes.as_slice()) else {
        return (None, Vec::new());
    };
    (
        extract_msgpack_announce_display_name(&payload),
        extract_msgpack_capability_tokens(&payload),
    )
}

fn classify_announce(destination_kind: &str, app_data: &str) -> AnnounceClass {
    let (_, tokens) = announce_metadata_from_app_data(app_data);
    if tokens.iter().any(|token| token == "r3akt")
        && RCH_SERVER_FEATURE_CAPABILITIES
            .iter()
            .any(|capability| tokens.iter().any(|token| token == capability))
    {
        return AnnounceClass::RchHubServer {};
    }

    match destination_kind {
        "lxmf_propagation" => AnnounceClass::PropagationNode {},
        "lxmf_delivery" => AnnounceClass::LxmfDelivery {},
        _ => {
            if tokens.iter().any(|token| token == "r3akt")
                && tokens.iter().any(|token| token == "emergencymessages")
            {
                return AnnounceClass::PeerApp {};
            }
            AnnounceClass::Other {}
        }
    }
}

fn announce_class_is_operator_relevant(class: AnnounceClass) -> bool {
    matches!(
        class,
        AnnounceClass::PeerApp {} | AnnounceClass::RchHubServer {}
    )
}

fn operator_label(display_name: Option<&str>, fallback_hex: &str) -> String {
    display_name
        .and_then(normalize_display_name)
        .unwrap_or_else(|| fallback_hex.to_ascii_lowercase())
}

fn operator_announce_message(
    announce_class: AnnounceClass,
    display_name: Option<&str>,
    destination_hex: &str,
    identity_hex: &str,
    hops: u8,
) -> Option<String> {
    if !announce_class_is_operator_relevant(announce_class) {
        return None;
    }

    let subject = operator_label(display_name, destination_hex);
    let prefix = match announce_class {
        AnnounceClass::RchHubServer {} => "RCH hub",
        AnnounceClass::PeerApp {} => "REM peer",
        _ => return None,
    };
    Some(format!(
        "[announce] {prefix} {subject} destination={} identity={} hops={hops}.",
        destination_hex.to_ascii_lowercase(),
        identity_hex.to_ascii_lowercase(),
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
    let (parsed_display_name, _) = announce_metadata_from_app_data(&record.app_data);
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
struct PendingLxmfDelivery {
    message_id_hex: String,
    destination_hex: String,
    correlation_id: Option<String>,
    command_id: Option<String>,
    command_type: Option<String>,
    event_uid: Option<String>,
    eam_uid: Option<String>,
    team_member_uid: Option<String>,
    team_uid: Option<String>,
    mission_uid: Option<String>,
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    relay_destination_hex: Option<String>,
    fallback_stage: Option<LxmfFallbackStage>,
    sent_at_ms: u64,
}

#[derive(Debug, Clone)]
struct PendingLxmfAcknowledgement {
    source_hex: String,
    detail: Option<String>,
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
    pub(crate) used_resource: bool,
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
    General,
}

impl SendTaskClass {
    fn from_metadata(metadata: Option<&MissionSyncMetadata>) -> Self {
        if metadata.is_some_and(MissionSyncMetadata::is_mission_related) {
            Self::Mission
        } else {
            Self::General
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mission => "mission",
            Self::General => "general",
        }
    }
}

#[derive(Clone)]
struct SendTaskPermits {
    general: Arc<Semaphore>,
    mission: Arc<Semaphore>,
}

impl SendTaskPermits {
    fn new() -> Self {
        Self {
            general: Arc::new(Semaphore::new(GENERAL_SEND_TASK_CONCURRENCY_LIMIT)),
            mission: Arc::new(Semaphore::new(MISSION_SEND_TASK_RESERVED_LIMIT)),
        }
    }

    #[cfg(test)]
    fn with_limits(general: usize, mission: usize) -> Self {
        Self {
            general: Arc::new(Semaphore::new(general)),
            mission: Arc::new(Semaphore::new(mission)),
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
            SendTaskClass::General => self
                .general
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| NodeError::InternalError {}),
        }
    }
}

fn log_send_task(class: SendTaskClass, message: String) {
    match class {
        SendTaskClass::Mission => info!("{message}"),
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

async fn connected_destination_hexes(state: &NodeRuntimeState) -> Vec<String> {
    state
        .connected_peers
        .lock()
        .await
        .iter()
        .map(address_hash_to_hex)
        .collect::<Vec<_>>()
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

fn projection_journal_path(storage_dir: Option<&str>) -> Option<PathBuf> {
    storage_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|dir| PathBuf::from(dir).join("runtime_projection.json"))
}

fn seed_peer_announces(messaging: &mut sdkmsg::MessagingStore, peer: &PeerRecord) {
    let Some(identity_hex) = peer
        .identity_hex
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let app_received_at_ms = peer
        .announce_last_seen_at_ms
        .unwrap_or(peer.last_seen_at_ms);
    let lxmf_received_at_ms = peer.lxmf_last_seen_at_ms.unwrap_or(app_received_at_ms);
    let display_name = peer.display_name.clone();
    let app_data = peer.app_data.clone().unwrap_or_default();

    messaging.record_announce(sdkmsg::AnnounceRecord {
        destination_hex: peer.destination_hex.clone(),
        identity_hex: identity_hex.to_string(),
        destination_kind: "app".to_string(),
        app_data: app_data.clone(),
        display_name: display_name.clone(),
        hops: 0,
        interface_hex: String::new(),
        received_at_ms: app_received_at_ms,
    });

    if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.clone() {
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: lxmf_destination_hex,
            identity_hex: identity_hex.to_string(),
            destination_kind: "lxmf_delivery".to_string(),
            app_data,
            display_name,
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: lxmf_received_at_ms,
        });
    }

    messaging.mark_peer_saved(peer.destination_hex.as_str(), peer.saved);
    messaging.set_peer_active_link(
        peer.destination_hex.as_str(),
        peer.active_link,
        peer.last_seen_at_ms,
    );
    messaging.record_resolution_attempt(
        peer.destination_hex.as_str(),
        peer.last_resolution_attempt_at_ms
            .unwrap_or(peer.last_seen_at_ms),
    );
    messaging.record_resolution_error(
        peer.destination_hex.as_str(),
        peer.last_resolution_error.clone(),
    );
}

fn restore_saved_peer_management(
    messaging: &mut sdkmsg::MessagingStore,
    saved_peers: &[crate::types::SavedPeerRecord],
) -> Vec<String> {
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
        restored_destinations.push(destination_hex);
    }
    restored_destinations
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
    // Only saved peers survive restart. Unsaved discovered peers must be rebuilt
    // from fresh announces after startup instead of being revived from cache.
    for peer in snapshot.restored_peers() {
        seed_peer_announces(&mut messaging, &peer);
    }
    for message in snapshot.messages() {
        messaging.upsert_message(to_sdk_message_record(message));
    }
}

fn sdk_peer_is_directly_reachable(peer: &sdkmsg::PeerRecord) -> bool {
    peer.active_link || matches!(peer.state, sdkmsg::PeerState::Connected)
}

async fn saved_peer_prefers_propagation(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    has_active_relay: bool,
) -> bool {
    if !has_active_relay {
        return false;
    }

    let normalized_destination = requested_destination_hex.to_ascii_lowercase();
    let canonical_destination =
        canonical_app_destination_hex(state, normalized_destination.as_str()).await;
    let saved_peers = match state.app_state.get_saved_peers() {
        Ok(saved_peers) => saved_peers,
        Err(_) => return false,
    };
    let is_saved = saved_peers
        .iter()
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .any(|destination_hex| {
            destination_hex == canonical_destination || destination_hex == normalized_destination
        });
    if !is_saved {
        return false;
    }

    let Some(peer) = peer_for_any_destination_hex(state, canonical_destination.as_str()).await
    else {
        return true;
    };
    !sdk_peer_is_directly_reachable(&peer)
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

async fn resolve_lxmf_destination_hex(state: &NodeRuntimeState, destination_hex: &str) -> String {
    let normalized_destination = destination_hex.to_ascii_lowercase();
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
) -> (u8, u8, u64, String) {
    let preferred_rank = if preferred_destination_hex.is_some_and(|preferred| {
        preferred == announce.destination_hex || preferred == announce.identity_hex
    }) {
        0
    } else {
        1
    };
    (
        preferred_rank,
        announce.hops,
        u64::MAX - announce.received_at_ms,
        announce.destination_hex.clone(),
    )
}

async fn sync_auto_propagation_node(state: &NodeRuntimeState, bus: &EventBus) {
    let announces = {
        let messaging = state.messaging.lock().await;
        messaging.list_announces()
    };
    let desired_destination = announces
        .iter()
        .filter(|record| record.destination_kind == "lxmf_propagation")
        .min_by_key(|record| {
            propagation_candidate_sort_key(record, state.preferred_propagation_node_hex.as_deref())
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
        messaging.record_resolution_error(destination_hex, None);
    }
    emit_peer_changed(state, bus, destination_hex).await;

    state.transport.request_path(&destination, None, None).await;
    let desc = ensure_destination_desc(state, destination, None).await?;
    let identity_hex = desc.identity.address_hash.to_hex_string();
    let lxmf_desc = SingleOutputDestination::new(
        desc.identity.clone(),
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
    emit_peer_changed(state, bus, destination_hex).await;
    emit_peer_resolved_for_destination(state, bus, destination_hex).await;
    sync_auto_propagation_node(state, bus).await;
    Ok(())
}

fn spawn_managed_peer_resolution(state: NodeRuntimeState, bus: EventBus, destination_hex: String) {
    tokio::spawn(async move {
        let retry_delays_secs = [0_u64, 3, 8, 15, 30];
        for delay_secs in retry_delays_secs {
            if delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }

            let should_retry = {
                let messaging = state.messaging.lock().await;
                if !messaging.is_peer_saved(destination_hex.as_str()) {
                    false
                } else {
                    messaging
                        .peer_by_destination(destination_hex.as_str())
                        .is_none_or(|peer| !sdk_peer_is_directly_reachable(&peer))
                }
            };

            if !should_retry {
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
                return;
            }
        }
    });
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
    lxmf_destination: Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
    connected_peers: Arc<TokioMutex<HashSet<AddressHash>>>,
    peer_resolution_inflight: Arc<TokioMutex<HashSet<String>>>,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    out_links:
        Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>>,
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
    send_task_permits: SendTaskPermits,
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

async fn ensure_output_link(
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
async fn ensure_lxmf_output_link(
    state: &NodeRuntimeState,
    desc: DestinationDesc,
) -> Result<Arc<TokioMutex<reticulum::destination::link::Link>>, NodeError> {
    ensure_output_link(state, desc).await
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
        .filter(|metadata| metadata.is_mission_related())
    {
        info!(
            "[lxmf][mission] attempting send requested_destination={} resolved_destination={} kind={} name={} message_id={} event_uid={} mission_uid={} correlation={}",
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
        eam_uid: metadata.eam_uid.clone(),
        team_member_uid: metadata.team_member_uid.clone(),
        team_uid: metadata.team_uid.clone(),
        mission_uid: metadata.mission_uid.clone(),
        method: report.method,
        representation: report.representation,
        relay_destination_hex: report.relay_destination_hex.clone(),
        fallback_stage: report.fallback_stage,
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

async fn send_lxmf_with_delivery_policy(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    send_mode: SendMode,
    send_task_class: SendTaskClass,
) -> Result<LxmfSendReport, NodeError> {
    const DIRECT_ATTEMPTS: usize = 5;
    const RETRY_DELAY: Duration = Duration::from_secs(10);
    let has_active_relay = has_active_propagation_relay(state).await;
    let prefer_propagation = matches!(send_mode, SendMode::Auto {})
        && saved_peer_prefers_propagation(state, requested_destination_hex, has_active_relay).await;

    if matches!(send_mode, SendMode::PropagationOnly {}) || prefer_propagation {
        if prefer_propagation {
            info!(
                "[lxmf][mission] saved peer {} is not directly reachable; using propagation relay",
                requested_destination_hex,
            );
        }
        let resolved_destination_hex =
            resolve_lxmf_destination_hex(state, requested_destination_hex).await;
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        log_send_task(
            send_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                send_task_class.label(),
                requested_destination_hex,
            ),
        );
        let _permit = acquire_send_task_permit(&state.send_task_permits, send_task_class).await?;
        log_send_task(
            send_task_class,
            format!(
                "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                send_task_class.label(),
                requested_destination_hex,
            ),
        );
        return state
            .sdk
            .send_lxmf(
                destination,
                body,
                title,
                fields_bytes,
                metadata,
                SendMode::PropagationOnly {},
            )
            .await;
    }

    let mut last_error: Option<NodeError> = None;

    for attempt in 1..=DIRECT_ATTEMPTS {
        let resolved_destination_hex =
            resolve_lxmf_destination_hex(state, requested_destination_hex).await;
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        log_send_task(
            send_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode={:?} attempt={attempt}/{DIRECT_ATTEMPTS}",
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
                    "[lxmf][queue] acquired {} send slot destination={} mode={:?} attempt={attempt}/{DIRECT_ATTEMPTS}",
                    send_task_class.label(),
                    requested_destination_hex,
                    send_mode,
                ),
            );
            state
                .sdk
                .send_lxmf(
                    destination,
                    body,
                    title.clone(),
                    fields_bytes.clone(),
                    metadata.clone(),
                    send_mode,
                )
                .await
        };
        match send_result {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                return Ok(report);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] send attempt {attempt}/{DIRECT_ATTEMPTS} failed destination={} mode={:?} outcome={:?}",
                    requested_destination_hex,
                    send_mode,
                    report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                let retriable = is_retriable_lxmf_error(&err);
                info!(
                    "[lxmf][mission] send attempt {attempt}/{DIRECT_ATTEMPTS} errored destination={} mode={:?} err={}",
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

        if attempt < DIRECT_ATTEMPTS {
            log_send_task(
                send_task_class,
                format!(
                    "[lxmf][queue] sleeping before retry destination={} mode={:?} next_attempt={}/{} delay_ms={}",
                    requested_destination_hex,
                    send_mode,
                    attempt + 1,
                    DIRECT_ATTEMPTS,
                    RETRY_DELAY.as_millis(),
                ),
            );
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    if !matches!(send_mode, SendMode::Auto {}) || !has_active_propagation_relay(state).await {
        return Err(last_error.unwrap_or(NodeError::NetworkError {}));
    }

    info!(
        "[lxmf][mission] auto delivery exhausted destination={}; retrying via propagation relay",
        requested_destination_hex,
    );
    let resolved_destination_hex =
        resolve_lxmf_destination_hex(state, requested_destination_hex).await;
    let destination = parse_address_hash(resolved_destination_hex.as_str())?;
    log_send_task(
        send_task_class,
        format!(
            "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=fallback",
            send_task_class.label(),
            requested_destination_hex,
        ),
    );
    let _permit = acquire_send_task_permit(&state.send_task_permits, send_task_class).await?;
    log_send_task(
        send_task_class,
        format!(
            "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=fallback",
            send_task_class.label(),
            requested_destination_hex,
        ),
    );
    let mut report = state
        .sdk
        .send_lxmf(
            destination,
            body,
            title,
            fields_bytes,
            metadata,
            SendMode::PropagationOnly {},
        )
        .await?;
    report.fallback_stage = Some(LxmfFallbackStage::AfterDirectRetryBudget {});
    Ok(report)
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
        let is_sos_message = sos_command.is_some() || looks_like_sos_text(body_utf8.as_str());
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
            ack_pending_lxmf_delivery(state, bus, source_hex.as_deref(), &metadata).await;
            persist_received_eam_if_present(
                state,
                bus,
                Some(metadata),
                fields_bytes.as_deref(),
                body_utf8.as_str(),
            )
            .await;
            persist_received_event_if_present(state, bus, Some(metadata), fields_bytes.as_deref())
                .await;
            persist_received_checklist_if_present(
                state,
                bus,
                Some(metadata),
                fields_bytes.as_deref(),
            );
        }
        if is_sos_message {
            let peer_hex = source_hex
                .clone()
                .unwrap_or_else(|| destination_hex.clone());
            let message_id_hex = LxmfWireMessage::unpack(payload.as_slice())
                .map(|wire| hex::encode(wire.message_id()))
                .unwrap_or_else(|_| format!("sos-{}-{}", peer_hex, now_ms()));
            let state_kind = sos_command
                .as_ref()
                .map(|command| command.state)
                .unwrap_or(SosMessageKind::Active {});
            let incident_id = sos_command
                .as_ref()
                .map(|command| command.incident_id.clone())
                .unwrap_or_else(|| format!("legacy-sos-{}-{}", peer_hex, now_ms()));
            let received_at_ms = now_ms();
            let record = MessageRecord {
                message_id_hex: message_id_hex.clone(),
                conversation_id: conversation_id_for(peer_hex.as_str()),
                direction: MessageDirection::Inbound {},
                destination_hex: peer_hex.clone(),
                source_hex: source_hex.clone(),
                title: title.clone(),
                body_utf8: body_utf8.clone(),
                method: MessageMethod::Direct {},
                state: MessageState::Received {},
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
                if let Ok(invalidation) = state.app_state.record_local_telemetry_fix(&position) {
                    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                }
            }
            bus.emit(NodeEvent::SosAlertChanged { alert });
        } else if !metadata
            .as_ref()
            .is_some_and(MissionSyncMetadata::is_mission_related)
        {
            let peer_hex = source_hex
                .clone()
                .unwrap_or_else(|| destination_hex.clone());
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

    let detail = metadata.ack_detail().map(ToOwned::to_owned);
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
        "[lxmf][mission] acknowledged message_id={} destination={} command={} correlation={} detail={}",
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
    let app_destination_hex = app_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();

    let announce_capabilities = Arc::new(TokioMutex::new(config.announce_capabilities.clone()));
    let known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let out_links: Arc<
        TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>,
    > = Arc::new(TokioMutex::new(HashMap::new()));
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
    let send_task_permits = SendTaskPermits::new();
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
        connected_peers: connected_peers.clone(),
        peer_resolution_inflight: peer_resolution_inflight.clone(),
        known_destinations: known_destinations.clone(),
        out_links: out_links.clone(),
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
        send_task_permits: send_task_permits.clone(),
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

    let restored_saved_destinations = {
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
    for destination_hex in restored_saved_destinations {
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
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }

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
        tokio::spawn(async move {
            for delay_secs in [0_u64, 2, 5, 12] {
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
        let interval_secs = config.announce_interval_seconds.max(1);
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
                        let app_data_bytes = event.app_data.as_slice().to_vec();
                        let app_data = String::from_utf8(app_data_bytes.clone())
                            .unwrap_or_else(|_| hex::encode(app_data_bytes.as_slice()));
                        let (parsed_display_name, _) = announce_metadata_from_app_data(&app_data);
                        let display_name = if destination_kind == "lxmf_delivery" {
                            display_name_from_delivery_app_data(app_data_bytes.as_slice())
                                .or(parsed_display_name)
                        } else {
                            parsed_display_name
                        };
                        let announce_class = classify_announce(&destination_kind, &app_data);
                        let interface_hex = hex::encode(event.interface);
                        let received_at_ms = now_ms();
                        state
                            .messaging
                            .lock()
                            .await
                            .record_announce(to_sdk_announce_record(AnnounceRecord {
                                destination_hex: destination_hex.clone(),
                                identity_hex: identity_hex.clone(),
                                destination_kind: destination_kind.clone(),
                                announce_class,
                                app_data: app_data.clone(),
                                display_name: display_name.clone(),
                                hops: event.hops,
                                interface_hex: interface_hex.clone(),
                                received_at_ms,
                            }));
                        sdk.record_announce_received(
                            &destination_hex,
                            &identity_hex,
                            &destination_kind,
                            &app_data,
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
                            display_name.as_deref(),
                            destination_hex.as_str(),
                            identity_hex.as_str(),
                            event.hops,
                        ) {
                            emit_operational_notice(&bus, LogLevel::Info {}, message);
                        }
                        if destination_kind == "app" {
                            let lxmf_destination_hex = SingleOutputDestination::new(
                                desc.identity.clone(),
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
                        } else if destination_kind == "lxmf_delivery" {
                            let app_destination_hex = state
                                .messaging
                                .lock()
                                .await
                                .app_destination_for_identity(identity_hex.as_str());
                            if let Some(app_destination_hex) = app_destination_hex {
                                emit_peer_changed(&state, &bus, &app_destination_hex).await;
                                emit_peer_resolved_for_destination(
                                    &state,
                                    &bus,
                                    &app_destination_hex,
                                )
                                .await;
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
                        "[lxmf][mission] timed out message_id={} destination={} command={} correlation={}",
                        pending.message_id_hex,
                        pending.destination_hex,
                        pending.command_type.as_deref().unwrap_or("-"),
                        pending.correlation_id.as_deref().unwrap_or("-"),
                    );
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
        let sdk = sdk.clone();
        let connected_peers = connected_peers.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.out_link_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.address_hash);
                        let canonical_destination_hex =
                            canonical_app_destination_hex(&state, &destination_hex).await;
                        match event.event {
                            LinkEvent::Activated => {
                                connected_peers.lock().await.insert(event.address_hash);
                                state.messaging.lock().await.set_peer_active_link(
                                    &destination_hex,
                                    true,
                                    now_ms(),
                                );
                                let state_name = state
                                    .messaging
                                    .lock()
                                    .await
                                    .peer_change_for_destination(&canonical_destination_hex)
                                    .map(from_sdk_peer_change);
                                if let Some(change) = state_name {
                                    sdk.record_peer_changed(
                                        &change.destination_hex,
                                        change.state,
                                        change.last_error.as_deref(),
                                    );
                                }
                                emit_peer_changed(&state, &bus, &canonical_destination_hex).await;
                                sync_auto_propagation_node(&state, &bus).await;
                            }
                            LinkEvent::Closed => {
                                connected_peers.lock().await.remove(&event.address_hash);
                                state.messaging.lock().await.set_peer_active_link(
                                    &destination_hex,
                                    false,
                                    now_ms(),
                                );
                                let state_name = state
                                    .messaging
                                    .lock()
                                    .await
                                    .peer_change_for_destination(&canonical_destination_hex)
                                    .map(from_sdk_peer_change);
                                if let Some(change) = state_name {
                                    sdk.record_peer_changed(
                                        &change.destination_hex,
                                        change.state,
                                        change.last_error.as_deref(),
                                    );
                                }
                                emit_peer_changed(&state, &bus, &canonical_destination_hex).await;
                                sync_auto_propagation_node(&state, &bus).await;
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
                    state
                        .messaging
                        .lock()
                        .await
                        .mark_peer_saved(&destination_hex, true);
                    emit_peer_changed(&state, &bus, &destination_hex).await;
                    state
                        .sdk
                        .record_peer_changed(&destination_hex, PeerState::Connecting {}, None);
                    resolve_peer_route(&state, &bus, &destination_hex).await?;
                    let desc = ensure_destination_desc(&state, dest, None).await?;
                    let _link = ensure_output_link(&state, desc).await?;
                    sync_auto_propagation_node(&state, &bus).await;
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
                    state
                        .messaging
                        .lock()
                        .await
                        .mark_peer_saved(&destination_hex, false);
                    connected_peers.lock().await.remove(&dest);
                    // Clean up any stale link from older builds if present.
                    if let Some(link) = out_links.lock().await.remove(&dest) {
                        link.lock().await.close();
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
                let send_task_class = if fields_bytes.is_some() {
                    SendTaskClass::from_metadata(metadata.as_ref())
                } else {
                    SendTaskClass::General
                };
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

                            if let Some(registered) = register_pending_lxmf_delivery(&state, report).await {
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
                                        let tracking_key = pending
                                            .correlation_id
                                            .as_deref()
                                            .or(pending.command_id.as_deref())
                                            .map(ToOwned::to_owned);
                                        if peer_destinations_equivalent(
                                            &state,
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
                                                &bus,
                                                pending,
                                                Some(buffered_ack.source_hex.clone()),
                                                LxmfDeliveryStatus::Acknowledged {},
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
                            detail: Some(format!("retry of {}", outbound.message_id_hex)),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, retried, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: outbound.request,
                                message_id_hex: report.message_id_hex.clone(),
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
                let status_update = from_sdk_sync_status(
                    state.messaging.lock().await.update_sync_status(|status| {
                        status.phase = sdkmsg::SyncPhase::Idle;
                        status.requested_at_ms = Some(requested_at_ms);
                        status.completed_at_ms = Some(now_ms());
                        status.messages_received = 0;
                        status.detail = None;
                    }),
                );
                if refresh_sync_status_snapshot(&state, &status_update) {
                    bus.emit(NodeEvent::SyncUpdated {
                        status: status_update,
                    });
                }
                if let Some(value) = limit {
                    info!(
                        "[sync] propagation sync request ignored in mobile runtime requested_limit={value}"
                    );
                } else {
                    info!("[sync] propagation sync request ignored in mobile runtime");
                }
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
                    buffered_at_ms: now,
                },
            ),
            (
                "stale".to_string(),
                PendingLxmfAcknowledgement {
                    source_hex: "src-stale".to_string(),
                    detail: None,
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

    #[test]
    fn restore_saved_peer_management_marks_saved_peers_managed() {
        let mut messaging = sdkmsg::MessagingStore::new(30);
        let now = now_ms();
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            identity_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            destination_kind: "app".to_string(),
            app_data: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            display_name: Some("Pixel".to_string()),
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: now,
        });
        messaging.record_announce(sdkmsg::AnnounceRecord {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            identity_hex: "dddddddddddddddddddddddddddddddd".to_string(),
            destination_kind: "app".to_string(),
            app_data: "R3AKT,EMergencyMessages,Telemetry".to_string(),
            display_name: Some("Other".to_string()),
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
                },
                crate::types::SavedPeerRecord {
                    destination_hex: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                    label: Some("Pixel duplicate".to_string()),
                    saved_at_ms: now,
                },
            ],
        );

        assert_eq!(
            restored,
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
        );
        let mut peers = messaging.list_peers();
        peers.sort_by(|left, right| left.destination_hex.cmp(&right.destination_hex));
        assert!(peers[0].saved);
        assert!(!peers[1].saved);
    }

    #[test]
    fn operator_announce_message_accepts_rch_hub_announces() {
        let message = operator_announce_message(
            AnnounceClass::RchHubServer {},
            Some("North Hub"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            2,
        )
        .expect("hub announce should be relevant");

        assert!(message.contains("RCH hub North Hub"));
        assert!(message.contains("destination=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(message.contains("identity=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    }

    #[test]
    fn operator_announce_message_accepts_rem_peer_announces() {
        let message = operator_announce_message(
            AnnounceClass::PeerApp {},
            Some("Pixel"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            1,
        )
        .expect("peer announce should be relevant");

        assert!(message.contains("REM peer Pixel"));
        assert!(message.contains("hops=1"));
    }

    #[test]
    fn operator_announce_message_ignores_regular_lxmf_announces() {
        let message = operator_announce_message(
            AnnounceClass::LxmfDelivery {},
            Some("LXMF Chat"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            1,
        );

        assert!(message.is_none());
    }
}
