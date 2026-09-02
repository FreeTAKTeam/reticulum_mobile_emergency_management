#[test]
fn custom_or_empty_checklist_create_still_needs_task_rows() {
    let mut args = JsonMap::new();
    args.insert(
        "template_uid".to_string(),
        JsonValue::from("tmpl-custom-offline"),
    );
    args.insert("total_tasks".to_string(), JsonValue::from(12_u64));
    assert!(!create_template_replicates_tasks_from_template(&args));

    args.insert(
        "template_uid".to_string(),
        JsonValue::from("tmpl-72-hour-home-preparedness"),
    );
    args.insert("total_tasks".to_string(), JsonValue::from(0_u64));
    assert!(!create_template_replicates_tasks_from_template(&args));
}

#[test]
fn checklist_delete_replication_respects_local_only_flag() {
    let status = build_status_for_tests();
    let saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("saved-peer".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    };
    let peers = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    )];

    let scheduled = build_checklist_delete_replication_sends(
        &status,
        peers.as_slice(),
        &[saved_peer],
        None,
        None,
        None,
        None,
        "chk-001",
        false,
    )
    .expect("local delete should be valid");

    assert!(scheduled.is_empty());
}

#[test]
fn list_active_checklists_supports_created_at_desc() {
    let storage_dir = prepare_storage_dir("checklist-created-at-desc");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");

    node.create_online_checklist(ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-old".to_string()),
        mission_uid: Some("mission-alpha".to_string()),
        template_uid: "tmpl-evac-001".to_string(),
        name: "Older Checklist".to_string(),
        description: "Created first".to_string(),
        start_time: "2026-04-22T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some("creator-identity".to_string()),
        created_by_team_member_display_name: None,
    })
    .expect("create older checklist");
    node.create_online_checklist(ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-new".to_string()),
        mission_uid: Some("mission-alpha".to_string()),
        template_uid: "tmpl-evac-001".to_string(),
        name: "Newer Checklist".to_string(),
        description: "Created second".to_string(),
        start_time: "2026-04-22T12:05:00Z".to_string(),
        created_by_team_member_rns_identity: Some("creator-identity".to_string()),
        created_by_team_member_display_name: None,
    })
    .expect("create newer checklist");

    {
        let inner = node.inner.lock().expect("node inner");
        let mut older = inner
            .app_state
            .get_checklist_any("chk-old")
            .expect("load older checklist")
            .expect("older checklist present");
        older.created_at = Some("2026-04-22T12:00:00Z".to_string());
        older.updated_at = Some("2026-04-22T12:30:00Z".to_string());
        inner
            .app_state
            .upsert_checklist(&older, "test-created-at-desc-old")
            .expect("persist older checklist");

        let mut newer = inner
            .app_state
            .get_checklist_any("chk-new")
            .expect("load newer checklist")
            .expect("newer checklist present");
        newer.created_at = Some("2026-04-22T12:05:00Z".to_string());
        newer.updated_at = Some("2026-04-22T12:10:00Z".to_string());
        inner
            .app_state
            .upsert_checklist(&newer, "test-created-at-desc-new")
            .expect("persist newer checklist");
    }

    let items = node
        .list_active_checklists(Some(ChecklistListActiveRequest {
            search: None,
            sort_by: Some("created_at_desc".to_string()),
        }))
        .expect("list checklists");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].uid, "chk-new");
    assert_eq!(items[1].uid, "chk-old");
}
