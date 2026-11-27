#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_checklist_syncs_full_snapshot_and_is_synced_only_after_ack() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("checklist_snapshot_initial_sync").await;
    let status_b = node_b.get_status();
    let mut peer_b = build_peer_record(
        &status_b.app_destination_hex,
        &status_b.lxmf_destination_hex,
        true,
        false,
        false,
    );
    peer_b.identity_hex = Some(status_b.identity_hex.clone());
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: peer_b.destination_hex.clone(),
            label: None,
            saved_at_ms: now_ms(),
            identity_hex: peer_b.identity_hex.clone(),
            lxmf_destination_hex: peer_b.lxmf_destination_hex.clone(),
            app_data: peer_b.app_data.clone(),
            display_name: None,
            last_route_seen_at_ms: Some(peer_b.last_seen_at_ms),
            last_hops: None,
            circle_tier: CircleTier::Inner {},
        }])
        .expect("save node b");

    // A CSV template only node A has: node B cannot hydrate anything from its own templates.
    let template = node_a
        .import_checklist_template_csv(ChecklistTemplateImportCsvRequest {
            template_uid: Some("tmpl-only-on-a".to_string()),
            name: "Custom preparedness".to_string(),
            description: Some("CSV import".to_string()),
            csv_text: "Task,CompletedDTG,Description,Owner,Radio Channel\nTorch,+1 hour,Portable light,Logistics,1\nRadio,+01:30,Receive alerts,Comms,7\n".to_string(),
            source_filename: Some("custom.csv".to_string()),
        })
        .expect("import csv template");
    node_a
        .create_checklist_from_template(ChecklistCreateFromTemplateRequest {
            checklist_uid: Some("chk-initial-sync".to_string()),
            mission_uid: Some("mission-initial-sync".to_string()),
            template_uid: template.uid.clone(),
            name: "Initial sync".to_string(),
            description: "Full snapshot".to_string(),
            start_time: "2099-04-23T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: None,
            created_by_team_member_display_name: None,
        })
        .expect("create checklist");
    let sent = node_a
        .get_checklist("chk-initial-sync".to_string())
        .expect("get checklist")
        .expect("local checklist");
    assert_eq!(sent.sync_state, crate::types::ChecklistSyncState::UploadPending {});

    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let received = node_b
            .get_checklist("chk-initial-sync".to_string())
            .expect("get checklist");
        let synced = node_a
            .get_checklist("chk-initial-sync".to_string())
            .expect("get checklist")
            .is_some_and(|record| record.sync_state == crate::types::ChecklistSyncState::Synced {});
        let has_received = received.is_some();
        if let Some(received) = received.filter(|_| synced) {
            assert_eq!(received.description, "Full snapshot");
            assert_eq!(received.columns.len(), sent.columns.len());
            assert_eq!(received.tasks.len(), sent.tasks.len());
            for (got, want) in received.tasks.iter().zip(sent.tasks.iter()) {
                assert_eq!(got.task_uid, want.task_uid);
                assert_eq!(got.cells.len(), want.cells.len());
                assert_eq!(got.due_relative_minutes, want.due_relative_minutes);
            }
            break;
        }
        assert!(
            Instant::now() < deadline,
            "initial sync incomplete: node b has checklist={} node a synced={synced}",
            has_received
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
