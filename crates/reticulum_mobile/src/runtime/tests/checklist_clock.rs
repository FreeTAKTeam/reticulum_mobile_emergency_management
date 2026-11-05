#[test]
fn checklist_clock_skew_bounds_and_nested_versions() {
    let now = parse_rfc3339_sort_key("2026-11-05T12:00:00Z").unwrap().0;
    assert!(checklist_timestamp_is_acceptable(
        "2026-11-05T12:15:00Z",
        now
    ));
    assert!(!checklist_timestamp_is_acceptable(
        "2026-11-05T12:15:01Z",
        now
    ));
    assert!(!checklist_timestamp_is_acceptable(
        "9999-01-01T00:00:00Z",
        now
    ));
    assert!(!checklist_timestamp_is_acceptable(
        "9223372036854775807-01-01T00:00:00Z",
        now
    ));
    assert!(checklist_timestamp_is_acceptable(
        "2026-11-05T08:15:00-04:00",
        now
    ));
    assert!(!checklist_timestamp_is_acceptable(
        "2026-02-31T12:00:00Z",
        now
    ));
    let mut record = checklist_test_record(
        "2026-11-05T12:00:00Z",
        checklist_test_task("task-1", 1, "Task", "2026-11-05T12:00:00Z"),
    );
    assert!(checklist_snapshot_clock_is_acceptable(&record, now));
    for field in ["updated_at", "uploaded_at", "deleted_at"] {
        let mut value = serde_json::to_value(&record).unwrap();
        value[field] = serde_json::json!("9999-01-01T00:00:00Z");
        let poisoned = serde_json::from_value(value).unwrap();
        assert!(
            !checklist_snapshot_clock_is_acceptable(&poisoned, now),
            "{field}"
        );
    }
    record.tasks[0].deleted_at = Some("9999-01-01T00:00:00Z".into());
    assert!(!checklist_snapshot_clock_is_acceptable(&record, now));
    record.tasks[0].deleted_at = None;
    record.tasks[0].cells[0].updated_at = Some("9999-01-01T00:00:00Z".into());
    assert!(!checklist_snapshot_clock_is_acceptable(&record, now));
}

#[test]
fn checklist_future_versions_do_not_persist_or_block_normal_updates() {
    let storage_dir = std::env::temp_dir().join(format!("rem-clock-{}", now_ms()));
    let store = AppStateStore::new(Some(storage_dir.to_str().unwrap())).unwrap();
    let bus = EventBus::new();
    let events = bus.subscribe();
    let mut record = checklist_test_record(
        "2026-04-22T12:00:00Z",
        checklist_test_task("task-1", 1, "Task", "2026-04-22T12:00:00Z"),
    );
    record.tasks[0].deleted_at = Some("9999-01-01T00:00:00Z".into());
    assert!(!upsert_inbound_checklist(&store, &bus, &record, "future"));
    assert!(store.get_checklist_any("chk-merge").unwrap().is_none());
    assert!(events
        .try_iter()
        .any(|event| matches!(event, NodeEvent::Error { code, .. } if code == "InvalidConfig")));
    record.tasks[0].deleted_at = None;
    assert!(upsert_inbound_checklist(&store, &bus, &record, "normal"));
    let future = checklist_status_fields(
        "chk-merge",
        Some("task-1"),
        "9999-01-01T00:00:00Z",
        "COMPLETE",
    );
    assert!(!persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(&future),
        None
    ));
    let normal = checklist_status_fields(
        "chk-merge",
        Some("task-1"),
        "2026-04-22T12:00:01Z",
        "COMPLETE",
    );
    assert!(persist_received_checklist_if_present(
        &store,
        &bus,
        None,
        Some(&normal),
        None
    ));
    assert_eq!(
        store
            .get_checklist_any("chk-merge")
            .unwrap()
            .unwrap()
            .updated_at
            .as_deref(),
        Some("2026-04-22T12:00:01Z")
    );
    drop(store);
    let _ = std::fs::remove_dir_all(storage_dir);
}
