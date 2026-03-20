use rmpv::Value as MsgPackValue;
use std::borrow::ToOwned;

const LXMF_FIELD_COMMANDS: i64 = 0x09;
const LXMF_FIELD_RESULTS: i64 = 0x0A;
const LXMF_FIELD_EVENT: i64 = 0x0D;

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
}

impl MissionSyncMetadata {
    pub(crate) fn tracking_key(&self) -> Option<&str> {
        self.correlation_id
            .as_deref()
            .or(self.command_id.as_deref())
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
        MsgPackValue::String(value) => value.as_str().map(ToOwned::to_owned),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone()).ok(),
        _ => None,
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

fn parse_command_envelope(envelope: &MsgPackValue, metadata: &mut MissionSyncMetadata) {
    let MsgPackValue::Map(map) = envelope else {
        return;
    };
    metadata.command_present = true;
    let entries = map.as_slice();
    parse_string_field(entries, &["command_id"], &mut metadata.command_id, false);
    parse_string_field(entries, &["correlation_id"], &mut metadata.correlation_id, false);
    parse_string_field(entries, &["command_type"], &mut metadata.command_type, false);
    if let Some(args) = msgpack_get_named(entries, &["args"]) {
        if let Some(args_entries) = msgpack_map_entries(args) {
            parse_string_field(
                args_entries,
                &["eam_uid", "event_uid", "entry_uid", "entryUid", "uid"],
                &mut metadata.event_uid,
                false,
            );
            parse_string_field(
                args_entries,
                &["eam_uid", "uid"],
                &mut metadata.eam_uid,
                false,
            );
            parse_string_field(
                args_entries,
                &["team_member_uid", "teamMemberUid", "subject_id", "subjectId"],
                &mut metadata.team_member_uid,
                false,
            );
            parse_string_field(
                args_entries,
                &["team_uid", "teamUid", "team_id", "teamId"],
                &mut metadata.team_uid,
                false,
            );
            parse_string_field(
                args_entries,
                &["mission_uid", "missionUid", "uid"],
                &mut metadata.mission_uid,
                false,
            );
        }
    }
}

fn parse_result_envelope(envelope: &MsgPackValue, metadata: &mut MissionSyncMetadata) {
    let MsgPackValue::Map(map) = envelope else {
        return;
    };
    metadata.result_present = true;
    let entries = map.as_slice();
    parse_string_field(entries, &["command_id"], &mut metadata.command_id, false);
    parse_string_field(entries, &["correlation_id"], &mut metadata.correlation_id, false);
    parse_string_field(entries, &["status"], &mut metadata.result_status, true);
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

    if let Some(payload) = msgpack_get_named(entries, &["payload"]) {
        if let Some(payload_entries) = msgpack_map_entries(payload) {
            parse_string_field(
                payload_entries,
                &["eam_uid", "event_uid", "entry_uid", "entryUid", "uid"],
                &mut metadata.event_uid,
                false,
            );
            parse_string_field(
                payload_entries,
                &["eam_uid"],
                &mut metadata.eam_uid,
                false,
            );
            parse_string_field(
                payload_entries,
                &["team_member_uid", "teamMemberUid", "subject_id", "subjectId"],
                &mut metadata.team_member_uid,
                false,
            );
            parse_string_field(
                payload_entries,
                &["team_uid", "teamUid", "team_id", "teamId"],
                &mut metadata.team_uid,
                false,
            );
            parse_string_field(
                payload_entries,
                &["mission_uid", "missionUid", "uid"],
                &mut metadata.mission_uid,
                false,
            );
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
        if let Some(commands) = msgpack_get_indexed(entries, LXMF_FIELD_COMMANDS) {
            parse_envelope_tree(commands, &mut metadata, parse_command_envelope);
        }
        if let Some(results) = msgpack_get_indexed(entries, LXMF_FIELD_RESULTS) {
            parse_envelope_tree(results, &mut metadata, parse_result_envelope);
        }
        if let Some(events) = msgpack_get_indexed(entries, LXMF_FIELD_EVENT) {
            parse_envelope_tree(events, &mut metadata, parse_event_envelope);
        }
    }

    if metadata.is_mission_related() {
        if metadata.event_uid.is_none() {
            metadata.event_uid = metadata.eam_uid.clone();
        }
        return Some(metadata);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mission_sync_metadata_recognizes_eam_command_lifecycle() {
        let fields = MsgPackValue::Map(vec![
            (
                MsgPackValue::from(LXMF_FIELD_COMMANDS),
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
                            (
                                MsgPackValue::from("eam_uid"),
                                MsgPackValue::from("eam-123"),
                            ),
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
                MsgPackValue::from(LXMF_FIELD_RESULTS),
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
                MsgPackValue::from(LXMF_FIELD_EVENT),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("event_type"),
                        MsgPackValue::from("mission.registry.eam.upserted"),
                    ),
                    (
                        MsgPackValue::from("payload"),
                        MsgPackValue::Map(vec![
                            (MsgPackValue::from("eam_uid"), MsgPackValue::from("eam-123")),
                            (
                                MsgPackValue::from("team_uid"),
                                MsgPackValue::from("team-1"),
                            ),
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
}
