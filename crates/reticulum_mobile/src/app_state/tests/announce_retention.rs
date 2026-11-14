#[test]
fn store_restart_keeps_only_most_recent_announces() {
    let storage_dir = test_storage_dir("announce-retention");
    let storage_dir_str = storage_dir.to_string_lossy();
    let store = AppStateStore::new(Some(storage_dir_str.as_ref())).expect("create store");
    for index in 0..=ANNOUNCE_RETENTION_LIMIT {
        store
            .upsert_announce(&announce(&format!("{index:032x}"), None, index as u64))
            .expect("insert announce");
    }
    drop(store);

    let restarted = AppStateStore::new(Some(storage_dir_str.as_ref())).expect("reopen store");
    let announces = restarted.list_announces().expect("list announces");
    assert_eq!(announces.len(), ANNOUNCE_RETENTION_LIMIT);
    assert!(announces.iter().all(|record| record.received_at_ms > 0));
}
