const POISON: &str = "9999-01-01T00:00:00Z";
const LEGIT_START: &str = "2099-01-01T00:00:00Z";
const LEGIT_DUE: &str = "2099-01-01T00:30:00Z";

/// Pre-1.6.1 fixture: far-future versions on the checklist, a task, a cell and a task tombstone.
fn poisoned_checklist_fixture() -> ChecklistRecord {
    let mut record = checklist_test_record(
        "2026-04-22T12:00:00Z",
        checklist_test_task("task-1", 1, "Task", "2026-04-22T12:00:00Z"),
    );
    let mut deleted = checklist_test_task("task-2", 2, "Gone", "2026-04-22T11:00:00Z");
    deleted.deleted_at = Some(POISON.into());
    record.tasks.push(deleted);
    record.updated_at = Some(POISON.into());
    record.tasks[0].updated_at = Some(POISON.into());
    record.tasks[0].cells[0].updated_at = Some(POISON.into());
    // Legitimate future schedule: due_dtg is derived from start_time + due_relative_minutes.
    record.start_time = Some(LEGIT_START.into());
    record.tasks[0].due_relative_minutes = Some(30);
    record
}

fn poisoned_store(label: &str) -> (AppStateStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rem-clock-repair-{label}-{}", now_ms()));
    let store = AppStateStore::new(Some(dir.to_str().unwrap())).unwrap();
    // Written straight to storage, as pre-1.6.1 builds did without ingress validation.
    store
        .upsert_checklist(&poisoned_checklist_fixture(), "fixture")
        .unwrap();
    (store, dir)
}

fn now_seconds() -> i64 {
    crate::numeric::u64_to_i64_saturating(now_ms() / 1_000)
}

#[test]
fn checklist_clock_repair_unblocks_edits_and_keeps_data() {
    let (store, dir) = poisoned_store("unblock");
    let bus = EventBus::new();
    let edit = checklist_status_fields("chk-merge", Some("task-1"), "2026-04-22T12:00:01Z", "COMPLETE");
    let poisoned = store.get_checklist_any("chk-merge").unwrap();
    assert!(checklist_delete_record_from_command(poisoned, "chk-merge", "2026-04-22T12:00:01Z", None).is_none());
    assert!(!persist_received_checklist_if_present(&store, &bus, None, Some(&edit), None));

    let outcomes = store.repair_future_checklist_clocks(now_seconds()).unwrap();
    let [crate::app_state::ChecklistClockRepairOutcome::Repaired { anchor, fields, .. }] = outcomes.as_slice() else {
        panic!("unexpected outcomes {outcomes:?}");
    };
    assert_eq!(anchor, "2026-04-22T12:00:00Z");
    assert_eq!(fields.len(), 4);
    let repaired = store.get_checklist_any("chk-merge").unwrap().unwrap();
    assert_eq!(repaired.start_time.as_deref(), Some(LEGIT_START));
    assert_eq!(repaired.tasks[0].due_dtg.as_deref(), Some(LEGIT_DUE));
    assert_eq!(repaired.tasks[1].deleted_at.as_deref(), Some(anchor.as_str()));
    assert_eq!(repaired.tasks[0].cells[0].value, poisoned_checklist_fixture().tasks[0].cells[0].value);
    // Idempotent: a second run (e.g. after restart) changes nothing.
    assert!(store.repair_future_checklist_clocks(now_seconds()).unwrap().is_empty());
    assert_eq!(serde_json::to_value(store.get_checklist_any("chk-merge").unwrap().unwrap()).unwrap(), serde_json::to_value(&repaired).unwrap());

    let backup: String = rusqlite::Connection::open(dir.join("app_state.db"))
        .unwrap()
        .query_row("SELECT original_json FROM checklist_clock_repairs WHERE uid = 'chk-merge'", [], |row| row.get(0))
        .unwrap();
    assert!(backup.contains(POISON));

    assert!(checklist_delete_record_from_command(Some(repaired), "chk-merge", "2026-04-22T12:00:01Z", None).is_some());
    assert!(persist_received_checklist_if_present(&store, &bus, None, Some(&edit), None));
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn checklist_clock_repair_converges_and_rejects_poisoned_replay() {
    let (peer_a, dir_a) = poisoned_store("peer-a");
    let (peer_b, dir_b) = poisoned_store("peer-b");
    peer_a.repair_future_checklist_clocks(now_seconds()).unwrap();
    peer_b.repair_future_checklist_clocks(now_seconds() + 120).unwrap();
    let a = peer_a.get_checklist_any("chk-merge").unwrap().unwrap();
    assert_eq!(serde_json::to_value(&a).unwrap(), serde_json::to_value(peer_b.get_checklist_any("chk-merge").unwrap().unwrap()).unwrap());

    let bus = EventBus::new();
    assert!(!upsert_inbound_checklist(&peer_a, &bus, &poisoned_checklist_fixture(), "replay"));
    // Duplicate/reordered replay of the repaired snapshot is a no-op merge.
    assert!(upsert_inbound_checklist(&peer_a, &bus, &a, "replay"));
    assert_eq!(peer_a.get_checklist_any("chk-merge").unwrap().unwrap().updated_at, a.updated_at);
    for (store, dir) in [(peer_a, dir_a), (peer_b, dir_b)] {
        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn checklist_clock_repair_reports_ambiguous_and_distrusts_old_clock() {
    let (store, dir) = poisoned_store("ambiguous");
    // A device clock before the 1.6.1 release would mark everything as future: do nothing.
    assert!(store.repair_future_checklist_clocks(0).unwrap().is_empty());
    let mut record = poisoned_checklist_fixture();
    record.uploaded_at = Some("not-a-time".into());
    store.upsert_checklist(&record, "fixture").unwrap();
    let outcomes = store.repair_future_checklist_clocks(now_seconds()).unwrap();
    assert!(matches!(outcomes.as_slice(), [crate::app_state::ChecklistClockRepairOutcome::Skipped { .. }]));
    assert_eq!(store.get_checklist_any("chk-merge").unwrap().unwrap().updated_at.as_deref(), Some(POISON));
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}
