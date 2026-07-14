#[test]
fn inbound_delete_marks_existing_checklist_deleted() {
    let existing = checklist_test_record(
        "2026-04-22T12:00:00.000000000Z",
        checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:00:00.000000000Z"),
    );

    let deleted = checklist_delete_record_from_command(
        Some(existing),
        "chk-merge",
        "2026-04-22T12:00:01.000000000Z",
        Some("peer-delete"),
    )
    .expect("newer delete should apply");

    assert_eq!(
        deleted.deleted_at.as_deref(),
        Some("2026-04-22T12:00:01.000000000Z")
    );
    assert_eq!(
        deleted.updated_at.as_deref(),
        Some("2026-04-22T12:00:01.000000000Z")
    );
    assert_eq!(
        deleted.last_changed_by_team_member_rns_identity.as_deref(),
        Some("peer-delete")
    );
}

#[test]
fn native_upload_snapshot_decodes_from_command_field() {
    let command = vec![(
        MsgPackValue::from("snapshot"),
        MsgPackValue::Map(vec![
            (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
            (MsgPackValue::from("name"), MsgPackValue::from("Native")),
            (
                MsgPackValue::from("tasks"),
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                    MsgPackValue::from("task_uid"),
                    MsgPackValue::from("task-1"),
                )])]),
            ),
        ]),
    )];
    let snapshot_json =
        checklist_snapshot_json_from_command(command.as_slice()).expect("native snapshot");

    assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
    assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
}

#[test]
fn native_upload_snapshot_decodes_from_msgpack_content() {
    let content = MsgPackValue::Map(vec![
        (
            MsgPackValue::from("type"),
            MsgPackValue::from("rem.checklist.snapshot.v1"),
        ),
        (
            MsgPackValue::from("checklist_uid"),
            MsgPackValue::from("chk-native"),
        ),
        (
            MsgPackValue::from("snapshot"),
            MsgPackValue::Map(vec![
                (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
                (MsgPackValue::from("name"), MsgPackValue::from("Native")),
                (
                    MsgPackValue::from("tasks"),
                    MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                        MsgPackValue::from("task_uid"),
                        MsgPackValue::from("task-1"),
                    )])]),
                ),
            ]),
        ),
    ]);
    let bytes = rmp_serde::to_vec(&content).expect("snapshot content");
    let snapshot_json =
        checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-native")
            .expect("content snapshot");

    assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
    assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
    assert!(
        checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-other").is_none()
    );
}

#[test]
fn native_upload_snapshot_decodes_from_compressed_msgpack_content() {
    use std::io::Write as _;

    let snapshot = MsgPackValue::Map(vec![
        (MsgPackValue::from("uid"), MsgPackValue::from("chk-native")),
        (MsgPackValue::from("name"), MsgPackValue::from("Native")),
        (
            MsgPackValue::from("tasks"),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                MsgPackValue::from("task_uid"),
                MsgPackValue::from("task-1"),
            )])]),
        ),
    ]);
    let snapshot_msgpack = rmp_serde::to_vec(&snapshot).expect("snapshot msgpack");
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(snapshot_msgpack.as_slice())
        .expect("write compressed snapshot");
    let compressed_snapshot = encoder.finish().expect("finish compressed snapshot");
    let content = MsgPackValue::Map(vec![
        (
            MsgPackValue::from("type"),
            MsgPackValue::from("rem.checklist.snapshot.v2"),
        ),
        (
            MsgPackValue::from("checklist_uid"),
            MsgPackValue::from("chk-native"),
        ),
        (
            MsgPackValue::from("encoding"),
            MsgPackValue::from("zlib+msgpack"),
        ),
        (
            MsgPackValue::from("snapshot"),
            MsgPackValue::Binary(compressed_snapshot),
        ),
    ]);
    let bytes = rmp_serde::to_vec(&content).expect("snapshot content");
    let snapshot_json =
        checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-native")
            .expect("compressed content snapshot");

    assert!(snapshot_json.contains("\"uid\":\"chk-native\""));
    assert!(snapshot_json.contains("\"task_uid\":\"task-1\""));
    assert!(
        checklist_snapshot_json_from_content(Some(bytes.as_slice()), "chk-other").is_none()
    );
}

#[test]
fn first_status_update_can_apply_to_missing_task_placeholder() {
    let mut checklist =
        blank_checklist_record("chk-missing-task", "2026-04-22T12:00:00Z", None);
    let inserted = ensure_task_for_incoming_update(
        &mut checklist,
        "task-missing",
        "2026-04-22T12:01:00Z",
        None,
    );
    let task = find_checklist_task_mut(&mut checklist, "task-missing").expect("task inserted");

    assert!(inserted);
    assert!(
        inserted
            || incoming_timestamp_is_newer(task.updated_at.as_deref(), "2026-04-22T12:01:00Z")
    );
}

#[test]
fn row_add_can_hydrate_placeholder_without_clearing_newer_status_or_cells() {
    let mut task = placeholder_task_record("task-1", "2026-04-22T12:05:00Z");
    task.user_status = ChecklistUserTaskStatus::Complete {};
    task.task_status = ChecklistTaskStatus::Complete {};
    task.completed_at = Some("2026-04-22T12:05:00Z".to_string());
    task.cells.push(ChecklistCellRecord {
        cell_uid: "task-1:col-item".to_string(),
        task_uid: "task-1".to_string(),
        column_uid: "col-item".to_string(),
        value: Some("Water".to_string()),
        updated_at: Some("2026-04-22T12:06:00Z".to_string()),
        updated_by_team_member_rns_identity: Some("peer-b".to_string()),
    });

    assert!(task_needs_row_metadata_hydration(&task));
    task.number = 1;
    task.legacy_value = Some("Water".to_string());
    task.updated_at =
        newest_timestamp(task.updated_at.as_deref(), Some("2026-04-22T12:04:00Z"))
            .map(ToString::to_string);

    assert_eq!(task.number, 1);
    assert_eq!(task.legacy_value.as_deref(), Some("Water"));
    assert!(matches!(
        task.user_status,
        ChecklistUserTaskStatus::Complete {}
    ));
    assert_eq!(task.cells.len(), 1);
    assert_eq!(task.updated_at.as_deref(), Some("2026-04-22T12:05:00Z"));
}

#[test]
fn row_add_task_payload_decodes_complete_task_cells() {
    let mut task = checklist_test_task(
        "stale-task-id",
        1,
        "Secure north access",
        "2026-04-22T12:00:00Z",
    );
    task.cells.push(checklist_test_cell(
        "stale-task-id",
        "col-notes",
        "Use IR marker",
        "2026-04-22T12:00:01Z",
    ));
    let task_msgpack = rmp_serde::from_slice::<MsgPackValue>(
        rmp_serde::to_vec(&task).expect("task msgpack").as_slice(),
    )
    .expect("task value");
    let args = vec![
        (
            MsgPackValue::from("task_uid"),
            MsgPackValue::from("task-remote"),
        ),
        (MsgPackValue::from("number"), MsgPackValue::from(7_u32)),
        (MsgPackValue::from("task"), task_msgpack),
    ];

    let decoded = checklist_task_from_row_add_args(
        args.as_slice(),
        "task-remote",
        7,
        "2026-04-22T12:02:00Z",
    )
    .expect("row task");

    assert_eq!(decoded.task_uid, "task-remote");
    assert_eq!(decoded.number, 7);
    assert_eq!(decoded.updated_at.as_deref(), Some("2026-04-22T12:02:00Z"));
    assert_eq!(decoded.cells.len(), 2);
    assert!(decoded
        .cells
        .iter()
        .all(|cell| cell.task_uid == "task-remote"));
    assert_eq!(
        decoded
            .cells
            .iter()
            .find(|cell| cell.column_uid == "col-task")
            .and_then(|cell| cell.value.as_deref()),
        Some("Secure north access")
    );
}

#[test]
fn inbound_complete_status_applies_even_when_cell_update_is_newer() {
    let mut task =
        checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
    task.cells.push(checklist_test_cell(
        "task-1",
        "col-task",
        "Existing",
        "2026-04-22T12:10:00.000000000Z",
    ));
    task.updated_at = Some("2026-04-22T12:10:00.000000000Z".to_string());

    assert!(should_apply_inbound_task_status(
        &task,
        ChecklistUserTaskStatus::Complete {},
        "2026-04-22T12:07:00.000000000Z",
        false,
    ));
}

#[test]
fn idempotent_checklist_status_update_is_handled_for_ack() {
    let storage_dir = std::env::temp_dir().join(format!(
        "rem-runtime-checklist-status-idempotent-{}",
        now_ms()
    ));
    let store = AppStateStore::new(Some(
        storage_dir
            .to_str()
            .expect("temporary storage dir should be utf-8"),
    ))
    .expect("app state store");
    let mut task =
        checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
    task.user_status = ChecklistUserTaskStatus::Complete {};
    task.task_status = ChecklistTaskStatus::Complete {};
    task.completed_at = Some("2026-04-22T12:05:00.000000000Z".to_string());
    let checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task.clone());
    store
        .upsert_checklist(&checklist, "seed-checklist")
        .expect("seed checklist");
    let bus = EventBus::new();
    let fields = checklist_status_fields(
        "chk-merge",
        Some("task-1"),
        "2026-04-22T12:04:00.000000000Z",
        "COMPLETE",
    );

    assert!(persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(fields.as_slice()),
        None,
    ));

    let stored = store
        .get_checklist_any("chk-merge")
        .expect("stored checklist query")
        .expect("stored checklist");
    assert_eq!(
        stored.updated_at.as_deref(),
        Some("2026-04-22T12:05:00.000000000Z")
    );
    assert_eq!(
        stored.tasks[0].updated_at.as_deref(),
        Some("2026-04-22T12:05:00.000000000Z")
    );
    assert!(matches!(
        stored.tasks[0].user_status,
        ChecklistUserTaskStatus::Complete {}
    ));
}

#[test]
fn compact_checklist_status_update_is_persisted() {
    let storage_dir =
        std::env::temp_dir().join(format!("rem-runtime-checklist-status-compact-{}", now_ms()));
    let store = AppStateStore::new(Some(
        storage_dir
            .to_str()
            .expect("temporary storage dir should be utf-8"),
    ))
    .expect("app state store");
    let task = checklist_test_task("task-1", 1, "Existing", "2026-04-22T12:05:00.000000000Z");
    let checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task);
    store
        .upsert_checklist(&checklist, "seed-checklist")
        .expect("seed checklist");
    let bus = EventBus::new();
    let fields = compact_checklist_status_fields(
        "chk-merge",
        "task-1",
        None,
        "2026-04-22T12:06:00.000000000Z",
        "COMPLETE",
    );

    assert!(persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(fields.as_slice()),
        None,
    ));

    let stored = store
        .get_checklist_any("chk-merge")
        .expect("stored checklist query")
        .expect("stored checklist");
    assert!(matches!(
        stored.tasks[0].user_status,
        ChecklistUserTaskStatus::Complete {}
    ));
    assert_eq!(
        stored.tasks[0].updated_at.as_deref(),
        Some("2026-04-22T12:06:00.000000000Z")
    );
}

#[test]
fn compact_checklist_status_update_resolves_visible_row_by_number_when_task_uid_differs() {
    let storage_dir = std::env::temp_dir().join(format!(
        "rem-runtime-checklist-status-row-number-{}",
        now_ms()
    ));
    let store = AppStateStore::new(Some(
        storage_dir
            .to_str()
            .expect("temporary storage dir should be utf-8"),
    ))
    .expect("app state store");
    let task_one =
        checklist_test_task("local-task-1", 1, "First", "2026-04-22T12:05:00.000000000Z");
    let task_two = checklist_test_task(
        "local-task-2",
        2,
        "Second",
        "2026-04-22T12:05:00.000000000Z",
    );
    let mut checklist = checklist_test_record("2026-04-22T12:05:00.000000000Z", task_one);
    checklist.tasks.push(task_two);
    normalize_checklist_record(&mut checklist);
    store
        .upsert_checklist(&checklist, "seed-checklist")
        .expect("seed checklist");
    let bus = EventBus::new();
    let fields = compact_checklist_status_fields(
        "chk-merge",
        "remote-task-2",
        Some(2),
        "2026-04-22T12:06:00.000000000Z",
        "COMPLETE",
    );

    assert!(persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(fields.as_slice()),
        None,
    ));

    let stored = store
        .get_checklist_any("chk-merge")
        .expect("stored checklist query")
        .expect("stored checklist");
    let first = stored
        .tasks
        .iter()
        .find(|task| task.task_uid == "local-task-1")
        .expect("first task");
    let second = stored
        .tasks
        .iter()
        .find(|task| task.task_uid == "local-task-2")
        .expect("second task");
    assert!(matches!(
        first.user_status,
        ChecklistUserTaskStatus::Pending {}
    ));
    assert!(matches!(
        second.user_status,
        ChecklistUserTaskStatus::Complete {}
    ));
    assert!(!stored
        .tasks
        .iter()
        .any(|task| task.task_uid == "remote-task-2"));
}

#[test]
fn malformed_checklist_status_update_is_not_handled_for_ack() {
    let storage_dir = std::env::temp_dir().join(format!(
        "rem-runtime-checklist-status-malformed-{}",
        now_ms()
    ));
    let store = AppStateStore::new(Some(
        storage_dir
            .to_str()
            .expect("temporary storage dir should be utf-8"),
    ))
    .expect("app state store");
    let bus = EventBus::new();
    let fields = checklist_status_fields(
        "chk-merge",
        None,
        "2026-04-22T12:04:00.000000000Z",
        "COMPLETE",
    );

    assert!(!persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(fields.as_slice()),
        None,
    ));
}
