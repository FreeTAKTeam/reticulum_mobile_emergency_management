use rmpv::Value as MsgPackValue;

use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_EVENT, FIELD_RESULTS};
use crate::mission_commands::{canonical_command_type, checklist_arg_code};
use crate::msgpack_values::{
    msgpack_get_indexed, msgpack_get_named, msgpack_hex_or_string, msgpack_map_entries,
    msgpack_string,
};

include!("mission_sync/metadata.rs");

fn msgpack_get_checklist_arg<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    key: &str,
) -> Option<&'a MsgPackValue> {
    if let Some(code) = checklist_arg_code(key) {
        msgpack_get_named(entries, &[key, code])
    } else {
        msgpack_get_named(entries, &[key])
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

fn msgpack_checklist_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().map(|value| format!("chk-{value}")),
        _ => msgpack_string(value),
    }
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
            Some("r3akt-default-mission".to_string())
        }
        _ => msgpack_string(value),
    }
}

fn set_if_none(slot: &mut Option<String>, value: Option<String>) {
    if slot.is_none() {
        *slot = value;
    }
}

fn parse_string_field(
    entries: &[(MsgPackValue, MsgPackValue)],
    keys: &[&str],
    slot: &mut Option<String>,
    overwrite: bool,
) {
    let value = msgpack_get_named(entries, keys).and_then(msgpack_string);
    if overwrite {
        if value.is_some() {
            *slot = value;
        }
    } else {
        set_if_none(slot, value);
    }
}

fn parse_event_uid_field(
    entries: &[(MsgPackValue, MsgPackValue)],
    keys: &[&str],
    slot: &mut Option<String>,
) {
    set_if_none(
        slot,
        keys.iter()
            .find_map(|key| msgpack_get_named(entries, &[*key]).and_then(msgpack_event_uid)),
    );
}

fn parse_mission_uid_field(
    entries: &[(MsgPackValue, MsgPackValue)],
    keys: &[&str],
    slot: &mut Option<String>,
) {
    set_if_none(
        slot,
        keys.iter()
            .find_map(|key| msgpack_get_named(entries, &[*key]).and_then(msgpack_mission_uid)),
    );
}

fn parse_identifier_fields(
    entries: &[(MsgPackValue, MsgPackValue)],
    metadata: &mut MissionSyncMetadata,
) {
    parse_event_uid_field(
        entries,
        &["eam_uid", "event_uid", "entry_uid", "entryUid", "uid", "u"],
        &mut metadata.event_uid,
    );
    set_if_none(
        &mut metadata.eam_uid,
        ["eam_uid", "uid", "u"]
            .iter()
            .find_map(|key| msgpack_get_named(entries, &[*key]).and_then(msgpack_eam_uid)),
    );
    set_if_none(
        &mut metadata.team_member_uid,
        [
            "team_member_uid",
            "teamMemberUid",
            "subject_id",
            "subjectId",
            "tm",
        ]
        .iter()
        .find_map(|key| msgpack_get_named(entries, &[*key]).and_then(msgpack_hex_or_string)),
    );
    parse_string_field(
        entries,
        &["team_uid", "teamUid", "team_id", "teamId", "tu"],
        &mut metadata.team_uid,
        false,
    );
    parse_mission_uid_field(
        entries,
        &["mission_uid", "missionUid", "uid", "m"],
        &mut metadata.mission_uid,
    );
    parse_string_field(
        entries,
        &["checklist_uid", "checklistUid"],
        &mut metadata.checklist_uid,
        false,
    );
    set_if_none(
        &mut metadata.checklist_uid,
        msgpack_get_named(entries, &["cl"]).and_then(msgpack_checklist_uid),
    );
    parse_string_field(
        entries,
        &["task_uid", "taskUid", "tsk"],
        &mut metadata.task_uid,
        false,
    );
    parse_string_field(
        entries,
        &["column_uid", "columnUid", "col"],
        &mut metadata.column_uid,
        false,
    );
}

fn parse_command_envelope(envelope: &MsgPackValue, metadata: &mut MissionSyncMetadata) {
    let MsgPackValue::Map(map) = envelope else {
        return;
    };
    let entries = map.as_slice();
    let args_entries = msgpack_get_named(entries, &["args", "a"]).and_then(msgpack_map_entries);
    let has_compact_event_args = args_entries.is_some_and(|args| {
        ["entry_uid", "event_uid", "u"].iter().any(|key| {
            msgpack_get_named(args, &[*key])
                .and_then(msgpack_event_uid)
                .is_some()
        })
    });
    let has_command_markers = msgpack_get_named(entries, &["command_id", "i"]).is_some()
        || msgpack_get_named(entries, &["correlation_id", "c"]).is_some()
        || msgpack_get_named(entries, &["command_type", "t"]).is_some()
        || has_compact_event_args;
    if !has_command_markers {
        return;
    }
    metadata.command_present = true;
    parse_string_field(
        entries,
        &["command_id", "i"],
        &mut metadata.command_id,
        false,
    );
    parse_string_field(
        entries,
        &["correlation_id", "c"],
        &mut metadata.correlation_id,
        false,
    );
    parse_string_field(
        entries,
        &["command_type", "t"],
        &mut metadata.command_type,
        false,
    );
    parse_identifier_fields(entries, metadata);
    if let Some(args_entries) = args_entries {
        parse_identifier_fields(args_entries, metadata);
        if let Some(uid) = metadata.event_uid.as_deref() {
            if let Some(command_id) = msgpack_get_named(args_entries, &["ci"])
                .and_then(|value| event_command_id_from_tail(uid, value))
            {
                metadata.command_id = Some(command_id);
            }
        }
        if metadata.correlation_id.is_none() {
            metadata.correlation_id = metadata.command_id.clone();
        }
        if let Some(patch) = msgpack_get_checklist_arg(args_entries, "patch") {
            if let Some(patch_entries) = msgpack_map_entries(patch) {
                parse_identifier_fields(patch_entries, metadata);
            }
        }
    }
}

fn parse_positional_command_envelope(values: &[MsgPackValue], metadata: &mut MissionSyncMetadata) {
    let Some(command_type) = values.first().and_then(|value| match value {
        MsgPackValue::Integer(value) if value.as_u64() == Some(1) => {
            Some("checklist.create.online".to_string())
        }
        value => {
            msgpack_string(value).map(|value| canonical_command_type(value.as_str()).to_string())
        }
    }) else {
        return;
    };
    if command_type != "checklist.create.online" || values.len() < 5 {
        return;
    }
    metadata.command_present = true;
    metadata.command_type = Some(command_type);
    set_if_none(
        &mut metadata.checklist_uid,
        values.get(1).and_then(msgpack_checklist_uid),
    );
    let Some(mission_uid) = values.get(2) else {
        return;
    };
    parse_mission_uid_field(
        &[(MsgPackValue::from("m"), mission_uid.clone())],
        &["m"],
        &mut metadata.mission_uid,
    );
}

fn parse_result_envelope(envelope: &MsgPackValue, metadata: &mut MissionSyncMetadata) {
    let MsgPackValue::Map(map) = envelope else {
        return;
    };
    metadata.result_present = true;
    let entries = map.as_slice();
    parse_string_field(
        entries,
        &["command_id", "i"],
        &mut metadata.command_id,
        false,
    );
    parse_string_field(
        entries,
        &["correlation_id", "c"],
        &mut metadata.correlation_id,
        false,
    );
    parse_string_field(entries, &["status", "s"], &mut metadata.result_status, true);
    parse_identifier_fields(entries, metadata);
    for key in ["result", "payload", "args"] {
        if let Some(value) = msgpack_get_named(entries, &[key]) {
            if let Some(nested_entries) = msgpack_map_entries(value) {
                parse_identifier_fields(nested_entries, metadata);
            }
        }
    }
}

fn parse_event_envelope(envelope: &MsgPackValue, metadata: &mut MissionSyncMetadata) {
    let MsgPackValue::Map(map) = envelope else {
        return;
    };
    metadata.event_present = true;
    let entries = map.as_slice();
    parse_string_field(entries, &["event_type"], &mut metadata.event_type, true);
    parse_string_field(
        entries,
        &["event_id", "eam_uid", "entry_uid", "entryUid", "uid"],
        &mut metadata.event_uid,
        false,
    );
    parse_identifier_fields(entries, metadata);

    if let Some(payload) = msgpack_get_named(entries, &["payload"]) {
        if let Some(payload_entries) = msgpack_map_entries(payload) {
            parse_identifier_fields(payload_entries, metadata);
        }
    }
}

fn parse_envelope_tree(
    envelope: &MsgPackValue,
    metadata: &mut MissionSyncMetadata,
    parser: fn(&MsgPackValue, &mut MissionSyncMetadata),
) {
    match envelope {
        MsgPackValue::Array(entries) => {
            parse_positional_command_envelope(entries.as_slice(), metadata);
            for entry in entries {
                parse_envelope_tree(entry, metadata, parser);
            }
        }
        MsgPackValue::Map(_) => parser(envelope, metadata),
        _ => {}
    }
}

pub(crate) fn parse_mission_sync_metadata(fields_bytes: &[u8]) -> Option<MissionSyncMetadata> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let mut metadata = MissionSyncMetadata::default();

    if let Some(entries) = msgpack_map_entries(&fields) {
        if let Some(commands) = msgpack_get_indexed(entries, FIELD_COMMANDS) {
            parse_envelope_tree(commands, &mut metadata, parse_command_envelope);
        }
        if let Some(results) = msgpack_get_indexed(entries, FIELD_RESULTS) {
            parse_envelope_tree(results, &mut metadata, parse_result_envelope);
        }
        if let Some(events) = msgpack_get_indexed(entries, FIELD_EVENT) {
            parse_envelope_tree(events, &mut metadata, parse_event_envelope);
        }
    }

    if metadata.is_mission_related() {
        if let Some(command_type) = metadata.command_type.as_deref() {
            metadata.command_type = Some(canonical_command_type(command_type).to_string());
        }
        if metadata.command_type.is_none()
            && metadata.command_present
            && metadata.event_uid.is_some()
            && metadata.checklist_uid.is_none()
        {
            metadata.command_type = Some("mission.registry.log_entry.upsert".to_string());
        }
        if metadata.command_type.as_deref() == Some("mission.registry.log_entry.upsert")
            && metadata.event_uid.is_some()
            && metadata.mission_uid.is_none()
        {
            metadata.mission_uid = Some("r3akt-default-mission".to_string());
        }
        if metadata.command_type.as_deref() == Some("mission.registry.log_entry.upsert")
            && metadata.command_id.is_none()
        {
            if let Some(uid) = metadata.event_uid.as_deref() {
                metadata.command_id = Some(format!("log-entry-{uid}"));
                if metadata.correlation_id.is_none() {
                    metadata.correlation_id = metadata.command_id.clone();
                }
            }
        }
        if metadata.result_present
            && metadata.command_id.is_none()
            && metadata.event_uid.is_some()
            && metadata.checklist_uid.is_none()
        {
            if let Some(uid) = metadata.event_uid.as_deref() {
                metadata.command_id = Some(format!("log-entry-{uid}"));
            }
        }
        if metadata.result_status.as_deref() == Some("a") {
            metadata.result_status = Some("accepted".to_string());
        }
        if metadata.result_present
            && metadata.result_status.is_none()
            && metadata.event_uid.is_some()
            && metadata
                .command_id
                .as_deref()
                .is_some_and(|value| value.starts_with("log-entry-"))
        {
            metadata.result_status = Some("accepted".to_string());
        }
        if matches!(
            metadata.command_type.as_deref(),
            Some("mission.registry.eam.upsert" | "mission.registry.eam.delete")
        ) && metadata.eam_uid.is_some()
        {
            metadata.event_uid = metadata.eam_uid.clone();
        }
        if metadata.event_uid.is_none() {
            metadata.event_uid = metadata.eam_uid.clone();
        }
        return Some(metadata);
    }

    None
}

#[cfg(test)]
mod checklist_tests {
    include!("mission_sync/tests/checklists.rs");
}

#[cfg(test)]
mod tests {
    include!("mission_sync/tests/metadata.rs");
}
