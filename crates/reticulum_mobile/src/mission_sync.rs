use rmpv::Value as MsgPackValue;
use std::borrow::ToOwned;

use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_EVENT, FIELD_RESULTS};
use crate::mission_commands::{canonical_command_type, checklist_arg_code};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MissionSyncMetadata {
    pub(crate) command_present: bool,
    pub(crate) result_present: bool,
    pub(crate) event_present: bool,
    pub(crate) correlation_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) command_type: Option<String>,
    pub(crate) result_status: Option<String>,
    pub(crate) event_type: Option<String>,
    pub(crate) event_uid: Option<String>,
    pub(crate) eam_uid: Option<String>,
    pub(crate) team_member_uid: Option<String>,
    pub(crate) team_uid: Option<String>,
    pub(crate) mission_uid: Option<String>,
    pub(crate) checklist_uid: Option<String>,
    pub(crate) task_uid: Option<String>,
    pub(crate) column_uid: Option<String>,
}

impl MissionSyncMetadata {
    pub(crate) fn tracking_key(&self) -> Option<&str> {
        self.command_id
            .as_deref()
            .or(self.correlation_id.as_deref())
    }

    pub(crate) fn primary_kind(&self) -> &'static str {
        if self.command_present {
            "command"
        } else if self.result_present {
            "result"
        } else if self.event_present {
            "event"
        } else {
            "message"
        }
    }

    pub(crate) fn primary_name(&self) -> Option<&str> {
        self.command_type
            .as_deref()
            .or(self.result_status.as_deref())
            .or(self.event_type.as_deref())
    }

    pub(crate) fn ack_detail(&self) -> Option<&str> {
        self.result_status
            .as_deref()
            .or(self.event_type.as_deref())
            .or(self.command_type.as_deref())
    }

    pub(crate) fn is_mission_related(&self) -> bool {
        self.command_present
            || self.result_present
            || self.event_present
            || self.command_id.is_some()
            || self.correlation_id.is_some()
            || self.command_type.is_some()
            || self.result_status.is_some()
            || self.event_type.is_some()
            || self.event_uid.is_some()
            || self.eam_uid.is_some()
            || self.team_member_uid.is_some()
            || self.team_uid.is_some()
            || self.mission_uid.is_some()
            || self.checklist_uid.is_some()
            || self.task_uid.is_some()
            || self.column_uid.is_some()
    }

    pub(crate) fn is_event_related(&self) -> bool {
        self.is_mission_related()
    }
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

fn msgpack_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().map(ToOwned::to_owned),
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
        &["checklist_uid", "checklistUid", "cl"],
        &mut metadata.checklist_uid,
        false,
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
    use super::{parse_mission_sync_metadata, MissionSyncMetadata};
    use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_RESULTS};
    use rmpv::Value as MsgPackValue;

    fn metadata_from_fields(fields: MsgPackValue) -> MissionSyncMetadata {
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack fields");
        parse_mission_sync_metadata(&bytes).expect("mission sync metadata")
    }

    #[test]
    fn tracking_key_prefers_command_id_over_shared_correlation() {
        let metadata = MissionSyncMetadata {
            command_present: true,
            correlation_id: Some("incident-1".to_string()),
            command_id: Some("sos:incident-1:cancelled:1000".to_string()),
            ..MissionSyncMetadata::default()
        };

        assert_eq!(
            metadata.tracking_key(),
            Some("sos:incident-1:cancelled:1000")
        );
    }

    #[test]
    fn checklist_metadata_is_extracted_from_command_args_and_patch() {
        let metadata = metadata_from_fields(MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-checklist"),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("corr-checklist"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("checklist.update"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("checklist_uid"),
                            MsgPackValue::from("chk-001"),
                        ),
                        (
                            MsgPackValue::from("task_uid"),
                            MsgPackValue::from("task-002"),
                        ),
                        (
                            MsgPackValue::from("column_uid"),
                            MsgPackValue::from("col-task"),
                        ),
                        (
                            MsgPackValue::from("patch"),
                            MsgPackValue::Map(vec![(
                                MsgPackValue::from("mission_uid"),
                                MsgPackValue::from("mission-alpha"),
                            )]),
                        ),
                    ]),
                ),
            ])]),
        )]));

        assert_eq!(metadata.command_type.as_deref(), Some("checklist.update"));
        assert_eq!(metadata.checklist_uid.as_deref(), Some("chk-001"));
        assert_eq!(metadata.task_uid.as_deref(), Some("task-002"));
        assert_eq!(metadata.column_uid.as_deref(), Some("col-task"));
        assert_eq!(metadata.mission_uid.as_deref(), Some("mission-alpha"));
        assert!(metadata.is_mission_related());
    }

    #[test]
    fn checklist_metadata_is_extracted_from_nested_result_payload() {
        let metadata = metadata_from_fields(MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_RESULTS),
            MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-checklist-result"),
                ),
                (
                    MsgPackValue::from("status"),
                    MsgPackValue::from("completed"),
                ),
                (
                    MsgPackValue::from("result"),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("checklist_uid"),
                            MsgPackValue::from("chk-010"),
                        ),
                        (
                            MsgPackValue::from("task_uid"),
                            MsgPackValue::from("task-010"),
                        ),
                        (
                            MsgPackValue::from("column_uid"),
                            MsgPackValue::from("col-style"),
                        ),
                    ]),
                ),
            ]),
        )]));

        assert_eq!(metadata.result_status.as_deref(), Some("completed"));
        assert_eq!(metadata.checklist_uid.as_deref(), Some("chk-010"));
        assert_eq!(metadata.task_uid.as_deref(), Some("task-010"));
        assert_eq!(metadata.column_uid.as_deref(), Some("col-style"));
        assert!(metadata.is_mission_related());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mission_sync_metadata_recognizes_eam_command_lifecycle() {
        let fields = MsgPackValue::Map(vec![
            (
                MsgPackValue::from(FIELD_COMMANDS),
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("command_id"),
                        MsgPackValue::from("cmd-eam-123"),
                    ),
                    (
                        MsgPackValue::from("correlation_id"),
                        MsgPackValue::from("corr-eam-123"),
                    ),
                    (
                        MsgPackValue::from("command_type"),
                        MsgPackValue::from("mission.registry.eam.upsert"),
                    ),
                    (
                        MsgPackValue::from("args"),
                        MsgPackValue::Map(vec![
                            (MsgPackValue::from("eam_uid"), MsgPackValue::from("eam-123")),
                            (
                                MsgPackValue::from("team_member_uid"),
                                MsgPackValue::from("member-1"),
                            ),
                            (MsgPackValue::from("team_uid"), MsgPackValue::from("team-1")),
                        ]),
                    ),
                ])]),
            ),
            (
                MsgPackValue::from(FIELD_RESULTS),
                MsgPackValue::Array(vec![
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("command_id"),
                            MsgPackValue::from("cmd-eam-123"),
                        ),
                        (
                            MsgPackValue::from("correlation_id"),
                            MsgPackValue::from("corr-eam-123"),
                        ),
                        (MsgPackValue::from("status"), MsgPackValue::from("accepted")),
                    ]),
                    MsgPackValue::Map(vec![
                        (
                            MsgPackValue::from("command_id"),
                            MsgPackValue::from("cmd-eam-123"),
                        ),
                        (
                            MsgPackValue::from("correlation_id"),
                            MsgPackValue::from("corr-eam-123"),
                        ),
                        (MsgPackValue::from("status"), MsgPackValue::from("result")),
                    ]),
                ]),
            ),
            (
                MsgPackValue::from(FIELD_EVENT),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("event_type"),
                        MsgPackValue::from("mission.registry.eam.upserted"),
                    ),
                    (
                        MsgPackValue::from("payload"),
                        MsgPackValue::Map(vec![
                            (MsgPackValue::from("eam_uid"), MsgPackValue::from("eam-123")),
                            (MsgPackValue::from("team_uid"), MsgPackValue::from("team-1")),
                        ]),
                    ),
                ]),
            ),
        ]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let metadata = parse_mission_sync_metadata(&bytes).expect("metadata");

        assert!(metadata.command_present);
        assert!(metadata.result_present);
        assert!(metadata.event_present);
        assert_eq!(metadata.command_id.as_deref(), Some("cmd-eam-123"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("corr-eam-123"));
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.eam.upsert")
        );
        assert_eq!(metadata.result_status.as_deref(), Some("result"));
        assert_eq!(
            metadata.event_type.as_deref(),
            Some("mission.registry.eam.upserted")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("eam-123"));
        assert_eq!(metadata.eam_uid.as_deref(), Some("eam-123"));
        assert_eq!(metadata.team_uid.as_deref(), Some("team-1"));
        assert_eq!(metadata.team_member_uid.as_deref(), Some("member-1"));
        assert!(metadata.is_mission_related());
        assert_eq!(metadata.primary_kind(), "command");
    }

    #[test]
    fn parse_mission_sync_metadata_ignores_sos_command_envelope() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("sos_state"),
                    MsgPackValue::from("active"),
                ),
                (
                    MsgPackValue::from("incident_id"),
                    MsgPackValue::from("incident-123"),
                ),
                (
                    MsgPackValue::from("trigger_source"),
                    MsgPackValue::from("manual"),
                ),
                (MsgPackValue::from("sent_at_ms"), MsgPackValue::from(42_u64)),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let metadata = parse_mission_sync_metadata(&bytes);

        assert!(metadata.is_none());
    }
}
