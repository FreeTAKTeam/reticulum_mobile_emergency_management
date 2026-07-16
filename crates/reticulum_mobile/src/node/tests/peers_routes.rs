#[test]
fn effective_hub_mode_uses_server_connected_override() {
    let snapshot = HubDirectorySnapshot {
        effective_connected_mode: true,
        items: Vec::new(),
        received_at_ms: 123,
    };

    assert!(matches!(
        effective_hub_mode(HubMode::SemiAutonomous {}, Some(&snapshot)),
        HubMode::Connected {}
    ));
    assert!(matches!(
        effective_hub_mode(HubMode::SemiAutonomous {}, None),
        HubMode::SemiAutonomous {}
    ));
}

#[test]
fn semi_autonomous_replication_targets_use_current_hub_directory_peers() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "abababababababababababababababab",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        false,
        true,
        true,
    )];
    let snapshot = HubDirectorySnapshot {
        effective_connected_mode: false,
        items: vec![crate::types::HubDirectoryPeerRecord {
            identity: "78787878787878787878787878787878".to_string(),
            destination_hash: "abababababababababababababababab".to_string(),
            display_name: Some("Pixel".to_string()),
            announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
            client_type: Some("rem".to_string()),
            registered_mode: Some("semi_autonomous".to_string()),
            last_seen: Some("2026-04-02T12:43:28Z".to_string()),
            status: Some("active".to_string()),
        }],
        received_at_ms: 456,
    };

    let targets = build_runtime_mission_replication_targets(
        &status,
        peers.as_slice(),
        &[],
        None,
        Some(&config),
        Some(&snapshot),
    )
    .expect("semi-autonomous targets");

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "abababababababababababababababab"
    );
    assert!(matches!(targets[0].send_mode, SendMode::Auto {}));
}

#[test]
fn semi_autonomous_replication_targets_fail_closed_without_hub_directory() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "abababababababababababababababab",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        true,
        true,
        true,
    )];

    let mission_targets = build_runtime_mission_replication_targets(
        &status,
        peers.as_slice(),
        &[],
        None,
        Some(&config),
        None,
    )
    .expect("mission targets");
    let event_targets = build_runtime_event_replication_targets(
        &status,
        peers.as_slice(),
        &[],
        None,
        Some(&config),
        None,
    )
    .expect("event targets");

    assert!(mission_targets.is_empty());
    assert!(event_targets.is_empty());
}

#[test]
fn connected_replication_targets_route_to_selected_hub_without_current_peer() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );

    let mission_targets =
        build_runtime_mission_replication_targets(&status, &[], &[], None, Some(&config), None)
            .expect("connected mission targets");
    let event_targets =
        build_runtime_event_replication_targets(&status, &[], &[], None, Some(&config), None)
            .expect("connected event targets");

    assert_eq!(mission_targets.len(), 1);
    assert_eq!(
        mission_targets[0].app_destination_hex,
        "56565656565656565656565656565656"
    );
    assert!(matches!(
        mission_targets[0].send_mode,
        SendMode::PropagationOnly {}
    ));
    assert_eq!(event_targets.len(), 1);
    assert_eq!(
        event_targets[0].app_destination_hex,
        "56565656565656565656565656565656"
    );
    assert!(matches!(
        event_targets[0].send_mode,
        SendMode::PropagationOnly {}
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_chat_message_is_received_by_peer() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("chat").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let body = "chat: hello from node a";
    let subscription = node_b.subscribe_events();
    let message_id = node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: body.to_string(),
            title: Some("chat".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("send chat message");
    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == body)
    })
    .expect("node b received chat message");

    assert_packet_received(event, &node_a_status.lxmf_destination_hex, body, None);
    assert!(!message_id.is_empty());
    let persisted_messages = node_b.list_messages(None).expect("persisted messages");
    assert!(
        persisted_messages
            .iter()
            .any(|message| message.body_utf8 == body
                && message.conversation_id
                    == node_a_status.lxmf_destination_hex.to_ascii_lowercase()),
        "received LXMF chat should be persisted in the canonical peer thread"
    );

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_emergency_message_to_app_destination_is_received_as_mission_packet() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("emergency_app_destination").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let body = "emergency: request medevac";
    let fields = mission_command_fields(
        "cmd-eam-app-123",
        "corr-eam-app-123",
        "mission.registry.eam.upsert",
        vec![
            ("eam_uid", MsgPackValue::from("eam-123")),
            ("team_member_uid", MsgPackValue::from("member-1")),
            ("team_uid", MsgPackValue::from("team-1")),
            ("mission_uid", MsgPackValue::from("mission-1")),
        ],
    );
    let subscription = node_b.subscribe_events();
    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            body.as_bytes().to_vec(),
            Some(fields.clone()),
            SendMode::Auto {},
        )
        .expect("send emergency packet via app destination");

    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
    })
    .expect("node b received emergency packet via app destination");

    assert_packet_received(
        event,
        &node_a_status.lxmf_destination_hex,
        body,
        Some(fields.as_slice()),
    );

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_peer_establishes_active_link_without_message_send() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("connect_peer_link").await;

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

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.saved && peer.active_link);
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never established an active link from connect_peer"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn mission_replication_targets_prioritize_current_saved_peers_before_stale_stored_routes() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "poco".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let stale_saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("stale".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let connected_saved_peer = SavedPeerRecord {
        destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        label: Some("pixel".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let mut stale_peer = build_peer_record(
        stale_saved_peer.destination_hex.as_str(),
        "cccccccccccccccccccccccccccccccc",
        true,
        false,
        false,
    );
    stale_peer.stale = true;
    stale_peer.last_seen_at_ms = 0;
    stale_peer.announce_last_seen_at_ms = None;
    stale_peer.lxmf_last_seen_at_ms = None;
    let connected_peer = build_peer_record(
        connected_saved_peer.destination_hex.as_str(),
        "dddddddddddddddddddddddddddddddd",
        true,
        true,
        true,
    );

    let targets = build_mission_replication_targets(
        &status,
        &[stale_peer, connected_peer],
        &[stale_saved_peer, connected_saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets[0].app_destination_hex,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
    assert_eq!(
        targets[1].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[1].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn replication_targets_skip_saved_peer_without_mission_capabilities() {
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
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    peer.app_data = Some("Telemetry".to_string());

    let mission_targets = build_mission_replication_targets(
        &status,
        &[peer.clone()],
        std::slice::from_ref(&saved_peer),
        Some("99999999999999999999999999999999"),
    );
    let event_targets = build_event_replication_targets(
        &status,
        &[peer],
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert!(mission_targets.is_empty());
    assert!(event_targets.is_empty());
}
