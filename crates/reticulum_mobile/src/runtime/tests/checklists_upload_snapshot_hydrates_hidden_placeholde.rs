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
        merged.last_changed_by_team_member_rns_identity.as_deref(),
        Some("peer-a")
    );
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
    assert_eq!(
        merged.last_changed_by_team_member_rns_identity.as_deref(),
        Some("peer-b")
    );
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
fn parse_hub_directory_result_state_extracts_terminal_snapshot() {
    let result = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from("cmd-123"),
            ),
            (
                MsgPackValue::from("status"),
                MsgPackValue::from("result"),
            ),
            (
                MsgPackValue::from("result"),
                MsgPackValue::Map(vec![
                    (
                        MsgPackValue::from("scope"),
                        MsgPackValue::from("shared_teams"),
                    ),
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
                                    MsgPackValue::from("emergencymessages"),
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
        ]),
    )]);

    let parsed =
        parse_hub_directory_result_state(&result, "cmd-123", 456).expect("terminal result");

    let Some(HubDirectoryResultState::Snapshot(snapshot)) = parsed else {
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
        vec![
            "r3akt".to_string(),
            "emergencymessages".to_string(),
            "telemetry".to_string()
        ]
    );
}
