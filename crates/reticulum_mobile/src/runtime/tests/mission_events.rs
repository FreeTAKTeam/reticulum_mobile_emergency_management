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
