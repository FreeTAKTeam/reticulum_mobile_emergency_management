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

#[test]
fn compact_checklist_commands_derive_stable_tracking_ids() {
    let commands = |command: MsgPackValue| {
        metadata_from_fields(MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![command]),
        )]))
    };
    let create = commands(MsgPackValue::Array(vec![
        MsgPackValue::from(1_u64),
        MsgPackValue::from("chk-001"),
        MsgPackValue::from("mission-alpha"),
        MsgPackValue::from("tmpl-001"),
        MsgPackValue::from("Evac"),
    ]));
    assert_eq!(create.tracking_key(), Some("cmd-chk-001"));

    let status = |done: bool| {
        commands(MsgPackValue::Map(vec![
            (MsgPackValue::from("t"), MsgPackValue::from("checklist.task.status.set")),
            (MsgPackValue::from("cl"), MsgPackValue::from("chk-001")),
            (MsgPackValue::from("no"), MsgPackValue::from(3_u64)),
            (MsgPackValue::from("x"), MsgPackValue::from(done)),
        ]))
    };
    assert_eq!(status(true).tracking_key(), Some("st-chk-001-n3-1"));
    assert_eq!(status(true).tracking_key(), status(true).tracking_key());
    assert_ne!(status(true).tracking_key(), status(false).tracking_key());

    let row_add = commands(MsgPackValue::Map(vec![
        (MsgPackValue::from("t"), MsgPackValue::from("checklist.task.row.add")),
        (MsgPackValue::from("cl"), MsgPackValue::from("chk-001")),
        (MsgPackValue::from("tsk"), MsgPackValue::from("task-9")),
        (MsgPackValue::from("no"), MsgPackValue::from(9_u64)),
    ]));
    assert!(row_add.command_present);
    assert_eq!(row_add.tracking_key(), Some("ra-chk-001-task-9"));
}
