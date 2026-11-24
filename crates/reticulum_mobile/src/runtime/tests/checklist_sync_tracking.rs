fn checklist_upload_update(
    command_id: &str,
    status: LxmfDeliveryStatus,
    ack: ApplicationAckState,
) -> LxmfDeliveryUpdate {
    LxmfDeliveryUpdate {
        message_id_hex: "b".repeat(64),
        destination_hex: "a".repeat(32),
        source_hex: Some("a".repeat(32)),
        correlation_id: Some(command_id.to_string()),
        command_id: Some(command_id.to_string()),
        command_type: Some("checklist.upload".to_string()),
        event_uid: None,
        mission_uid: None,
        status,
        transport_state: TransportDeliveryState::TransportDelivered {},
        application_ack_state: ack,
        method: LxmfDeliveryMethod::Direct {},
        representation: LxmfDeliveryRepresentation::Resource {},
        relay_destination_hex: None,
        fallback_stage: None,
        detail: None,
        sent_at_ms: now_ms(),
        updated_at_ms: now_ms(),
    }
}

#[test]
fn checklist_becomes_synced_only_after_accepted_upload_ack() {
    let dir = std::env::temp_dir().join(format!("rem-checklist-sync-{}", now_ms()));
    let store = AppStateStore::new(Some(dir.to_str().unwrap())).unwrap();
    let mut record = checklist_test_record(
        "2026-04-22T12:00:00Z",
        checklist_test_task("task-1", 1, "Task", "2026-04-22T12:00:00Z"),
    );
    record.sync_state = ChecklistSyncState::UploadPending {};
    store.upsert_checklist(&record, "fixture").unwrap();
    let command_id = crate::app_state::checklist_upload_command_id("chk-merge");
    let sync_state = || store.get_checklist_any("chk-merge").unwrap().unwrap().sync_state;

    // Transport delivery, rejection, or another command's ack do not count as synced.
    for update in [
        checklist_upload_update(&command_id, LxmfDeliveryStatus::Delivered {}, ApplicationAckState::Waiting {}),
        checklist_upload_update(&command_id, LxmfDeliveryStatus::Acknowledged {}, ApplicationAckState::Rejected {}),
        checklist_upload_update("cmd-chk-merge", LxmfDeliveryStatus::Acknowledged {}, ApplicationAckState::Accepted {}),
    ] {
        assert!(store.record_checklist_delivery(&update).unwrap().is_empty());
        assert!(matches!(sync_state(), ChecklistSyncState::UploadPending {}));
    }

    let accepted = checklist_upload_update(&command_id, LxmfDeliveryStatus::Acknowledged {}, ApplicationAckState::Accepted {});
    assert!(!store.record_checklist_delivery(&accepted).unwrap().is_empty());
    assert!(matches!(sync_state(), ChecklistSyncState::Synced {}));
    // A duplicate ack (retry or second peer) is a no-op.
    assert!(store.record_checklist_delivery(&accepted).unwrap().is_empty());
    let stored = store.get_checklist_any("chk-merge").unwrap().unwrap();
    assert_eq!(stored.tasks.len(), record.tasks.len());
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}
