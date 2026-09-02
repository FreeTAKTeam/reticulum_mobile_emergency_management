#[test]
fn ignored_peer_destinations_persist_and_can_be_cleared() {
    let storage_dir = test_storage_dir("ignored-peers");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let destinations = vec![
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
    ];

    store
        .add_ignored_peer_destinations(destinations.as_slice())
        .expect("add ignored peers");
    store
        .add_ignored_peer_destinations(&["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()])
        .expect("dedupe ignored peer");

    let restored = AppStateStore::new(storage_dir.to_str())
        .expect("restored store")
        .get_ignored_peer_destinations()
        .expect("ignored peers");
    assert_eq!(restored.len(), 2);
    assert!(restored.contains(&"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()));
    assert!(restored.contains(&"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()));

    store
        .remove_ignored_peer_destinations(&["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()])
        .expect("remove ignored peer");
    let remaining = store
        .get_ignored_peer_destinations()
        .expect("remaining ignored peers");
    assert_eq!(remaining, vec!["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]);
}

#[test]
fn upsert_saved_peer_persists_selected_lxmf_route() {
    let storage_dir = test_storage_dir("saved-peer-upsert");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");

    store
        .upsert_saved_peer(&SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("Original".to_string()),
            saved_at_ms: 1,
            identity_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
            lxmf_destination_hex: Some("cccccccccccccccccccccccccccccccc".to_string()),
            app_data: Some("R3AKT,EMergencyMessages".to_string()),
            display_name: Some("Peer".to_string()),
            last_route_seen_at_ms: Some(42),
            last_hops: Some(2),
            circle_tier: CircleTier::Outer {},
        })
        .expect("insert saved peer");
    store
        .upsert_saved_peer(&SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("Updated".to_string()),
            saved_at_ms: 2,
            identity_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
            lxmf_destination_hex: Some("dddddddddddddddddddddddddddddddd".to_string()),
            app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
            display_name: Some("Updated Peer".to_string()),
            last_route_seen_at_ms: Some(84),
            last_hops: Some(1),
            circle_tier: CircleTier::Outer {},
        })
        .expect("update saved peer");

    let restored = AppStateStore::new(storage_dir.to_str())
        .expect("reopen store")
        .get_saved_peers()
        .expect("saved peers");

    assert_eq!(restored.len(), 1);
    assert_eq!(
        restored[0].destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        restored[0].lxmf_destination_hex.as_deref(),
        Some("dddddddddddddddddddddddddddddddd")
    );
    assert_eq!(restored[0].label.as_deref(), Some("Updated"));
    assert_eq!(restored[0].last_route_seen_at_ms, Some(84));
    assert_eq!(restored[0].circle_tier, CircleTier::Outer {});
}

#[test]
fn legacy_saved_peers_migrate_once_into_local_yellow() {
    let storage_dir = test_storage_dir("local-yellow-migration");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let mut settings = app_settings_with_due_step(30);
    settings.teams.local_teams.clear();
    settings.teams.local_teams_initialized = false;
    store.set_app_settings(&settings).expect("settings");
    store
        .set_saved_peers(&[SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: Some("Family".to_string()),
            saved_at_ms: 1,
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
            circle_tier: CircleTier::Inner {},
        }])
        .expect("saved peer");

    let migrated = store
        .get_app_settings()
        .expect("get settings")
        .expect("settings exist");
    assert!(migrated.teams.local_teams_initialized);
    assert_eq!(migrated.teams.local_teams.len(), 1);
    assert_eq!(migrated.teams.local_teams[0].team_uid, YELLOW_TEAM_UID);
    assert_eq!(
        migrated.teams.local_teams[0].member_destinations,
        vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
    );
}

fn message(
    id: &str,
    conversation_id: &str,
    direction: MessageDirection,
    destination_hex: &str,
    source_hex: Option<&str>,
    updated_at_ms: u64,
) -> MessageRecord {
    MessageRecord {
        message_id_hex: id.to_string(),
        conversation_id: conversation_id.to_string(),
        direction,
        destination_hex: destination_hex.to_string(),
        source_hex: source_hex.map(str::to_string),
        requested_destination_hex: Some(destination_hex.to_string()),
        delivery_destination_hex: Some(destination_hex.to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some(id.to_string()),
        title: Some("chat".to_string()),
        body_utf8: format!("body {id}"),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Received {},
        transport_state: TransportDeliveryState::TransportDelivered {},
        application_ack_state: ApplicationAckState::NotRequired {},
        detail: None,
        sent_at_ms: Some(updated_at_ms),
        received_at_ms: None,
        updated_at_ms,
    }
}

fn announce(
    destination_hex: &str,
    display_name: Option<&str>,
    received_at_ms: u64,
) -> AnnounceRecord {
    AnnounceRecord {
        destination_hex: destination_hex.to_string(),
        identity_hex: "11112222333344445555666677778888".to_string(),
        destination_kind: "app".to_string(),
        announce_class: AnnounceClass::PeerApp {},
        app_data: "R3AKT,EMergencyMessages".to_string(),
        display_name: display_name.map(str::to_string),
        hops: 2,
        interface_hex: "aabbccdd".to_string(),
        received_at_ms,
    }
}

fn sos_alert(
    incident_id: &str,
    source_hex: &str,
    conversation_id: &str,
    active: bool,
    updated_at_ms: u64,
) -> SosAlertRecord {
    SosAlertRecord {
        incident_id: incident_id.to_string(),
        source_hex: source_hex.to_string(),
        conversation_id: conversation_id.to_string(),
        state: if active {
            SosMessageKind::Active {}
        } else {
            SosMessageKind::Cancelled {}
        },
        active,
        body_utf8: "SOS".to_string(),
        lat: Some(44.6488),
        lon: Some(-63.5752),
        battery_percent: None,
        audio_id: None,
        message_id_hex: Some(format!("message-{incident_id}")),
        received_at_ms: updated_at_ms,
        updated_at_ms,
    }
}

fn sos_location(incident_id: &str, source_hex: &str, recorded_at_ms: u64) -> SosLocationRecord {
    SosLocationRecord {
        incident_id: incident_id.to_string(),
        source_hex: source_hex.to_string(),
        lat: 44.6488,
        lon: -63.5752,
        alt: None,
        accuracy: None,
        battery_percent: None,
        recorded_at_ms,
    }
}

fn checklist(uid: &str) -> ChecklistRecord {
    ChecklistRecord {
        uid: uid.to_string(),
        mission_uid: Some("mission-alpha".to_string()),
        template_uid: Some("tmpl-alpha".to_string()),
        template_version: Some(1),
        template_name: Some("Alpha Template".to_string()),
        name: "Alpha Checklist".to_string(),
        description: "Shared alpha checklist".to_string(),
        start_time: Some("2099-04-22T12:00:00Z".to_string()),
        mode: ChecklistMode::Online {},
        sync_state: crate::types::ChecklistSyncState::Synced {},
        origin_type: ChecklistOriginType::RchTemplate {},
        checklist_status: ChecklistTaskStatus::Pending {},
        created_at: Some("2026-04-22T12:00:00Z".to_string()),
        created_by_team_member_rns_identity: "abcd1234".to_string(),
        created_by_team_member_display_name: Some("Alpha Operator".to_string()),
        updated_at: Some("2026-04-22T12:00:00Z".to_string()),
        last_changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        deleted_at: None,
        uploaded_at: None,
        participant_rns_identities: vec!["abcd1234".to_string()],
        expected_task_count: Some(1),
        progress_percent: 0.0,
        counts: ChecklistStatusCounts {
            pending_count: 1,
            late_count: 0,
            complete_count: 0,
        },
        columns: vec![
            ChecklistColumnRecord {
                column_uid: "col-due".to_string(),
                column_name: "Due".to_string(),
                display_order: 0,
                column_type: ChecklistColumnType::RelativeTime {},
                column_editable: false,
                background_color: None,
                text_color: None,
                is_removable: false,
                system_key: Some(ChecklistSystemColumnKey::DueRelativeDtg {}),
            },
            ChecklistColumnRecord {
                column_uid: "col-task".to_string(),
                column_name: "Task".to_string(),
                display_order: 1,
                column_type: ChecklistColumnType::ShortString {},
                column_editable: true,
                background_color: None,
                text_color: None,
                is_removable: true,
                system_key: None,
            },
        ],
        tasks: vec![ChecklistTaskRecord {
            task_uid: "task-1".to_string(),
            number: 1,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: None,
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: Some(15),
            due_dtg: None,
            notes: None,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some("Check in".to_string()),
            cells: vec![ChecklistCellRecord {
                cell_uid: "task-1:col-task".to_string(),
                task_uid: "task-1".to_string(),
                column_uid: "col-task".to_string(),
                value: Some("Check in".to_string()),
                updated_at: None,
                updated_by_team_member_rns_identity: None,
            }],
        }],
        feed_publications: Vec::new(),
    }
}

#[test]
fn latest_announce_is_one_row_per_destination_and_preserves_display_name() {
    let storage_dir = test_storage_dir("latest-announce");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

    store
        .upsert_announce(&announce(
            "aaaabbbbccccddddeeeeffff00001111",
            Some("Alpha Peer"),
            1_000,
        ))
        .expect("insert announce");
    store
        .upsert_announce(&announce("aaaabbbbccccddddeeeeffff00001111", None, 2_000))
        .expect("update announce");
    store
        .upsert_announce(&announce(
            "aaaabbbbccccddddeeeeffff00001111",
            Some("Stale Name"),
            1_500,
        ))
        .expect("ignore older announce");

    let announces = store.list_announces().expect("list announces");
    assert_eq!(announces.len(), 1);
    assert_eq!(
        announces[0].destination_hex,
        "aaaabbbbccccddddeeeeffff00001111"
    );
    assert_eq!(announces[0].received_at_ms, 2_000);
    assert_eq!(announces[0].display_name.as_deref(), Some("Alpha Peer"));
}

#[test]
fn announces_are_available_after_store_restart() {
    let storage_dir = test_storage_dir("restart-announces");
    let storage_dir_str = storage_dir.to_string_lossy();
    let store = AppStateStore::new(Some(storage_dir_str.as_ref())).expect("create store");
    store
        .upsert_announce(&announce(
            "bbbbccccddddeeeeffff000011112222",
            Some("Offline Peer"),
            3_000,
        ))
        .expect("insert announce");
    drop(store);

    let restarted = AppStateStore::new(Some(storage_dir_str.as_ref())).expect("reopen store");
    let announces = restarted.list_announces().expect("list announces");
    assert_eq!(announces.len(), 1);
    assert_eq!(
        announces[0].destination_hex,
        "bbbbccccddddeeeeffff000011112222"
    );
    assert_eq!(announces[0].display_name.as_deref(), Some("Offline Peer"));
    assert_eq!(announces[0].received_at_ms, 3_000);
}

#[test]
fn peer_identity_aliases_fold_existing_split_threads() {
    let storage_dir = test_storage_dir("identity-alias-thread");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let outbound = message(
        "outbound-alias",
        "app-thread",
        MessageDirection::Outbound {},
        "APPDEST",
        Some("LOCAL"),
        10,
    );
    let inbound = message(
        "inbound-alias",
        "lxmf-thread",
        MessageDirection::Inbound {},
        "LOCAL",
        Some("LXMFDest"),
        20,
    );

    store.upsert_message(&outbound).expect("persist outbound");
    store.upsert_message(&inbound).expect("persist inbound");
    assert_eq!(
        store
            .list_conversations()
            .expect("list before aliases")
            .len(),
        2
    );

    let mut resolver = ConversationPeerResolver::default();
    resolver.insert(
        vec!["APPDEST".to_string(), "LXMFDest".to_string()],
        "IDENTITY".to_string(),
        "LXMFDest".to_string(),
        Some("Poco".to_string()),
    );

    let conversations = store
        .list_conversations_resolved(&resolver)
        .expect("list after aliases");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].conversation_id, "identity");
    assert_eq!(conversations[0].peer_destination_hex, "lxmfdest");
    assert_eq!(conversations[0].peer_display_name.as_deref(), Some("Poco"));

    let messages = store
        .list_messages_resolved(Some("APPDEST"), &resolver)
        .expect("list canonical messages");
    assert_eq!(messages.len(), 2);
    assert!(messages
        .iter()
        .all(|message| message.conversation_id == "identity"));
}
#[test]
fn hub_team_directories_are_durable_and_scoped_by_hub_identity() {
    let storage_dir = test_storage_dir("hub-team-directory-scope");
    let hub_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let hub_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let mut snapshot = crate::types::HubDirectorySnapshot::yellow_only(123);
    snapshot.hub_identity_hash = Some(hub_a.to_string());
    snapshot.items.push(crate::types::HubDirectoryPeerRecord {
        identity: "11111111111111111111111111111111".to_string(),
        destination_hash: "22222222222222222222222222222222".to_string(),
        display_name: Some("Yellow peer".to_string()),
        announce_capabilities: vec!["r3akt".to_string(), "emergencymessages".to_string()],
        client_type: Some("rem".to_string()),
        registered_mode: Some("semi_autonomous".to_string()),
        last_seen: None,
        status: Some("offline".to_string()),
    });

    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    store
        .set_hub_directory(hub_a, &snapshot)
        .expect("persist hub A directory");
    assert!(store.get_hub_directory(hub_b).expect("read hub B").is_none());
    drop(store);

    let restored = AppStateStore::new(storage_dir.to_str()).expect("restored store");
    let restored_snapshot = restored
        .get_hub_directory(hub_a)
        .expect("read hub A")
        .expect("hub A snapshot");
    assert_eq!(restored_snapshot.hub_identity_hash.as_deref(), Some(hub_a));
    assert_eq!(restored_snapshot.items.len(), 1);
    assert!(restored.get_hub_directory(hub_b).expect("read hub B").is_none());

    std::fs::remove_dir_all(storage_dir).expect("cleanup storage");
}
