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
