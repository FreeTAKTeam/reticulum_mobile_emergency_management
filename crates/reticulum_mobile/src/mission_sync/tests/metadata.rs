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
