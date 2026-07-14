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
