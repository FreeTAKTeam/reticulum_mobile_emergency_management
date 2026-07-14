#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delete_event_replicates_to_native_peer_projection() {
    const EVENT_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("event_delete_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: node_b_status.app_destination_hex.clone(),
            label: Some("peer-b".to_string()),
            saved_at_ms: now_ms(),
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm event delete link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm event delete link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm event delete link")
    })
    .expect("node b received event delete warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.saved && has_known_lxmf_route(&peer));
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became mission-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let record = EventProjectionRecord {
        uid: "evt-delete-native".to_string(),
        command_id: "cmd-evt-delete-native".to_string(),
        source_identity: node_a_status.identity_hex.clone(),
        source_display_name: Some(node_a_status.name.clone()),
        timestamp: "2026-03-25T17:05:00Z".to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: "r3akt-default-mission".to_string(),
        content: "Native deleted event".to_string(),
        callsign: node_a_status.name.clone(),
        server_time: Some("2026-03-25T17:05:00Z".to_string()),
        client_time: Some("2026-03-25T17:05:00Z".to_string()),
        keywords: vec!["r3akt:event-type:Incident".to_string()],
        content_hashes: vec![],
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        correlation_id: Some("corr-evt-delete-native".to_string()),
        topics: vec!["r3akt-default-mission".to_string(), "Default".to_string()],
    };

    let upsert_ack_subscription = node_a.subscribe_events();
    node_a
        .upsert_event(record.clone())
        .expect("upsert local event");

    let received_deadline = Instant::now() + EVENT_REPLICATION_TIMEOUT;
    loop {
        let received = node_b
            .get_events()
            .expect("get events")
            .into_iter()
            .find(|event| event.uid == record.uid && event.deleted_at_ms.is_none());
        if received.is_some() {
            break;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted replicated event before delete"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let upsert_command_id = format!("log-entry-{}", record.uid);
    wait_for_operational_ack(
        &upsert_ack_subscription,
        upsert_command_id.as_str(),
        "mission.registry.log_entry.upsert",
    );

    let deleted_at_ms = now_ms();
    node_a
        .delete_event(record.uid.clone(), deleted_at_ms)
        .expect("delete local event");

    let delete_deadline = Instant::now() + EVENT_REPLICATION_TIMEOUT;
    let deleted = loop {
        let deleted = node_b
            .get_events()
            .expect("get events")
            .into_iter()
            .find(|event| {
                event.uid == record.uid && event.deleted_at_ms == Some(deleted_at_ms)
            });
        if let Some(deleted) = deleted {
            break deleted;
        }
        assert!(
            Instant::now() < delete_deadline,
            "node b never persisted replicated event delete"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(deleted.uid, record.uid);
    assert_eq!(deleted.deleted_at_ms, Some(deleted_at_ms));

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn event_replication_targets_only_include_intentional_peers() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let saved_peer = build_saved_peer();
    let peers = vec![
        build_peer_record(
            saved_peer.destination_hex.as_str(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        ),
        build_peer_record(
            "cccccccccccccccccccccccccccccccc",
            "dddddddddddddddddddddddddddddddd",
            false,
            true,
            true,
        ),
        build_peer_record(
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "ffffffffffffffffffffffffffffffff",
            false,
            false,
            false,
        ),
        build_peer_record(
            "99999999999999999999999999999999",
            "12121212121212121212121212121212",
            false,
            true,
            false,
        ),
    ];

    let targets = build_event_replication_targets(
        &status,
        peers.as_slice(),
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}

#[test]
fn event_replication_targets_try_saved_reachable_peer_without_active_link_when_no_relay() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("saved-connected".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let peers = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        false,
    )];

    let targets =
        build_event_replication_targets(&status, peers.as_slice(), &[saved_peer], None);

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}

#[test]
fn event_replication_targets_use_relay_for_fresh_route_without_active_link_when_relay_exists() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("saved-connected".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let peers = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        false,
    )];

    let targets = build_event_replication_targets(
        &status,
        peers.as_slice(),
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn event_replication_targets_try_current_app_peer_with_old_lxmf_timestamp_before_relay() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    peer.lxmf_last_seen_at_ms =
        Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

    assert!(!peer_has_observed_lxmf_delivery_route(&peer));
    assert!(!peer_is_mission_direct_delivery_ready(&peer, true));

    let targets = build_event_replication_targets(
        &status,
        &[peer],
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn event_replication_targets_use_direct_link_then_relay_for_saved_stored_routes() {
    let status = build_status_for_tests();
    let direct_saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("Pixel".to_string()),
        saved_at_ms: 1,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let relay_saved_peer = SavedPeerRecord {
        destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        label: Some("RelayOnly".to_string()),
        saved_at_ms: 2,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let direct_peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "11111111111111111111111111111111",
        true,
        true,
        true,
    );
    let mut relay_peer = build_peer_record(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "22222222222222222222222222222222",
        true,
        false,
        false,
    );
    relay_peer.stale = true;
    relay_peer.announce_last_seen_at_ms = None;
    relay_peer.lxmf_last_seen_at_ms = None;

    let targets = build_event_replication_targets(
        &status,
        &[relay_peer, direct_peer],
        &[direct_saved_peer, relay_saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
    assert_eq!(
        targets[1].app_destination_hex,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(targets[1].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn event_replication_targets_use_propagation_for_saved_stored_route_without_discovered_peer() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let saved_peer = SavedPeerRecord {
        destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        label: Some("saved-relay".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let mut saved_relay_peer = build_peer_record(
        "cccccccccccccccccccccccccccccccc",
        "dddddddddddddddddddddddddddddddd",
        true,
        false,
        false,
    );
    saved_relay_peer.stale = true;
    saved_relay_peer.announce_last_seen_at_ms = None;
    saved_relay_peer.lxmf_last_seen_at_ms = None;
    let mut unsaved_relay_peer = build_peer_record(
        "cccccccccccccccccccccccccccccccc",
        "dddddddddddddddddddddddddddddddd",
        false,
        false,
        false,
    );
    unsaved_relay_peer.stale = true;
    unsaved_relay_peer.announce_last_seen_at_ms = None;
    unsaved_relay_peer.lxmf_last_seen_at_ms = None;
    let peers = vec![
        build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            false,
            true,
            true,
        ),
        saved_relay_peer,
        unsaved_relay_peer,
        build_peer_record(
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "ffffffffffffffffffffffffffffffff",
            false,
            true,
            true,
        ),
    ];

    let targets = build_event_replication_targets(
        &status,
        peers.as_slice(),
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "cccccccccccccccccccccccccccccccc"
    );
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}
