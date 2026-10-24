// Relay custody is retired only when every operation in an understood envelope was saved.
fn received_mission_is_durable(
    fields_bytes: Option<&[u8]>,
    eam: bool,
    event: bool,
    telemetry: bool,
    checklist: bool,
    marker: bool,
) -> bool {
    let Some(fields_bytes) = fields_bytes else {
        return false;
    };
    let Ok(fields) = rmp_serde::from_slice::<MsgPackValue>(fields_bytes) else {
        return false;
    };
    let Some(fields) = msgpack_map_entries(&fields) else {
        return false;
    };
    if msgpack_get_indexed(fields, FIELD_RESULTS).is_some() {
        return false;
    }
    let commands = match msgpack_get_indexed(fields, FIELD_COMMANDS) {
        None => &[][..],
        Some(MsgPackValue::Array(commands)) => commands.as_slice(),
        Some(_) => return false,
    };
    let mut command_types = Vec::with_capacity(commands.len());
    for command in commands {
        let singleton = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![command.clone()]),
        )]);
        let metadata = rmp_serde::to_vec(&singleton)
            .ok()
            .and_then(|bytes| parse_mission_sync_metadata(&bytes));
        let Some(command_type) = metadata.and_then(|metadata| metadata.command_type) else {
            return false;
        };
        command_types.push(command_type);
    }
    if let Some(events) = msgpack_get_indexed(fields, crate::lxmf_fields::FIELD_EVENT) {
        let events = match events {
            MsgPackValue::Array(events) => events.as_slice(),
            event => std::slice::from_ref(event),
        };
        return marker
            && !events.is_empty()
            && events.iter().all(|event| {
                msgpack_value_to_json(event).is_some_and(|event| {
                    event.get("payload").is_some()
                        && event
                            .get("event_type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(supported_marker_receive_event)
                })
            })
            && command_types.iter().all(|command| {
                matches!(
                    command.as_str(),
                    "mission.marker.create"
                        | "mission.marker.patch"
                        | "mission.marker.position.patch"
                        | "mission.marker.delete"
                )
            });
    }
    if !command_types.is_empty()
        && command_types
            .iter()
            .all(|command| command.starts_with("checklist."))
    {
        return checklist;
    }
    // Existing single-operation parsers do not prove persistence of mixed or repeated batches.
    let [command] = command_types.as_slice() else {
        return false;
    };
    match command.as_str() {
        "mission.registry.eam.upsert" | "mission.registry.eam.delete" => eam,
        "mission.registry.log_entry.upsert" => event,
        "mission.registry.telemetry.upsert" => telemetry,
        _ => false,
    }
}

fn received_fields_are_plain_chat(fields_bytes: Option<&[u8]>) -> bool {
    let Some(bytes) = fields_bytes else {
        return true;
    };
    let Ok(fields) = rmp_serde::from_slice::<MsgPackValue>(bytes) else {
        return false;
    };
    let Some(fields) = msgpack_map_entries(&fields) else {
        return false;
    };
    [
        FIELD_COMMANDS,
        FIELD_RESULTS,
        crate::lxmf_fields::FIELD_EVENT,
    ]
    .iter()
    .all(|key| msgpack_get_indexed(fields, *key).is_none())
}

fn supported_marker_receive_event(event_type: &str) -> bool {
    event_type == r3akt_map_core::replica::DELETE_EVENT
        || r3akt_map_core::replica::UPSERT_EVENTS.contains(&event_type)
}

#[cfg(test)]
mod receive_persistence_tests {
    use super::*;
    fn store(name: &str) -> (std::path::PathBuf, AppStateStore) {
        let dir = std::env::temp_dir().join(format!(
            "rem-receive-proof-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let store = AppStateStore::new(dir.to_str()).expect("store");
        (dir, store)
    }
    fn fields(key: i64, entries: Vec<MsgPackValue>) -> Vec<u8> {
        rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(key),
            MsgPackValue::Array(entries),
        )]))
        .expect("fields")
    }
    fn commands(types: &[&str]) -> Vec<u8> {
        rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(
                types
                    .iter()
                    .map(|kind| {
                        MsgPackValue::Map(vec![(
                            MsgPackValue::from("command_type"),
                            MsgPackValue::from(*kind),
                        )])
                    })
                    .collect(),
            ),
        )]))
        .expect("fields")
    }
    #[test]
    fn only_proven_complete_envelopes_retire_relay_custody() {
        let single = commands(&["mission.registry.telemetry.upsert"]);
        assert!(received_mission_is_durable(
            Some(&single),
            false,
            false,
            true,
            false,
            false
        ));
        assert!(!received_mission_is_durable(
            Some(&single),
            true,
            true,
            false,
            true,
            true
        ));
        let mixed = commands(&[
            "mission.registry.telemetry.upsert",
            "mission.registry.eam.upsert",
        ]);
        assert!(!received_mission_is_durable(
            Some(&mixed),
            true,
            true,
            true,
            true,
            true
        ));
        let repeated = commands(&[
            "mission.registry.telemetry.upsert",
            "mission.registry.telemetry.upsert",
        ]);
        assert!(!received_mission_is_durable(
            Some(&repeated),
            true,
            true,
            true,
            true,
            true
        ));
        assert!(!received_mission_is_durable(
            Some(&[0xc1]),
            true,
            true,
            true,
            true,
            true
        ));
    }

    #[test]
    fn ignored_semantic_fields_cannot_be_confirmed_as_plain_chat() {
        assert!(received_fields_are_plain_chat(None));
        for entries in [vec![], vec![MsgPackValue::Nil]] {
            let bytes = fields(FIELD_COMMANDS, entries);
            assert!(!received_fields_are_plain_chat(Some(&bytes)));
        }
        assert!(!received_fields_are_plain_chat(Some(&[0xc1])));
        let deletion = commands(&["mission.registry.eam.delete"]);
        assert!(received_mission_is_durable(
            Some(&deletion),
            true,
            false,
            false,
            false,
            false
        ));
    }

    #[test]
    fn partial_checklist_batch_retries_after_the_first_operation_was_saved() {
        let (dir, store) = store("checklist");
        let bus = EventBus::new();
        let deletion = |uid: Option<&str>| {
            MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("checklist.delete"),
                ),
                (
                    MsgPackValue::from("timestamp"),
                    MsgPackValue::from("2026-10-23T12:00:00Z"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(
                        uid.map(|uid| {
                            vec![(MsgPackValue::from("checklist_uid"), MsgPackValue::from(uid))]
                        })
                        .unwrap_or_default(),
                    ),
                ),
            ])
        };
        let partial = fields(
            FIELD_COMMANDS,
            vec![deletion(Some("chk-proof-a")), deletion(None)],
        );
        assert!(!persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(&partial),
            None
        ));
        assert!(store
            .get_checklist_any("chk-proof-a")
            .expect("query")
            .is_some());
        let corrected = fields(
            FIELD_COMMANDS,
            vec![deletion(Some("chk-proof-a")), deletion(Some("chk-proof-b"))],
        );
        assert!(persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(&corrected),
            None
        ));
        assert!(persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(&corrected),
            None
        ));
        std::fs::remove_dir_all(&dir).expect("remove fixture storage");
        assert!(!persist_received_checklist_if_present(
            &store,
            &bus,
            None,
            Some(&corrected),
            None
        ));
    }

    #[test]
    fn marker_confirmation_rejects_dropped_or_failed_events_and_accepts_replay() {
        let (dir, store) = store("marker");
        let bus = EventBus::new();
        let metadata = MissionSyncMetadata {
            event_present: true,
            ..Default::default()
        };
        let event = |payload: Option<MsgPackValue>| {
            let mut entries = vec![(
                MsgPackValue::from("event_type"),
                MsgPackValue::from(r3akt_map_core::replica::DELETE_EVENT),
            )];
            if let Some(payload) = payload {
                entries.push((MsgPackValue::from("payload"), payload));
            }
            MsgPackValue::Map(entries)
        };
        let valid = event(Some(MsgPackValue::Map(vec![(
            MsgPackValue::from("object_destination_hash"),
            MsgPackValue::from("0123456789abcdef0123456789abcdef"),
        )])));
        let missing_payload = fields(
            crate::lxmf_fields::FIELD_EVENT,
            vec![valid.clone(), event(None)],
        );
        let applied = persist_received_map_marker_if_present(
            &store,
            &bus,
            Some(&metadata),
            Some(&missing_payload),
        );
        assert!(applied, "existing parser drops the malformed sibling");
        assert!(!received_mission_is_durable(
            Some(&missing_payload),
            false,
            false,
            false,
            false,
            applied
        ));
        let rejected = fields(
            crate::lxmf_fields::FIELD_EVENT,
            vec![valid.clone(), event(Some(MsgPackValue::Map(vec![])))],
        );
        assert!(!persist_received_map_marker_if_present(
            &store,
            &bus,
            Some(&metadata),
            Some(&rejected)
        ));
        let replay = fields(crate::lxmf_fields::FIELD_EVENT, vec![valid.clone(), valid]);
        let saved =
            persist_received_map_marker_if_present(&store, &bus, Some(&metadata), Some(&replay));
        assert!(received_mission_is_durable(
            Some(&replay),
            false,
            false,
            false,
            false,
            saved
        ));
        std::fs::remove_dir_all(&dir).expect("remove fixture storage");
        assert!(!persist_received_map_marker_if_present(
            &store,
            &bus,
            Some(&metadata),
            Some(&replay)
        ));
    }
}
