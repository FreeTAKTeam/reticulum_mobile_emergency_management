#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_eam_updates_with_same_callsign_replicate_latest_projection() {
    const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("eam_repeated_updates").await;

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
            circle_tier: CircleTier::Inner {},
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm repeated eam link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm repeated eam link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm repeated eam link")
    })
    .expect("node b received repeated eam warmup message");

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

    let first_record = EamProjectionRecord {
        callsign: "Pippo".to_string(),
        group_name: "Yellow".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Green".to_string(),
        preparedness_status: "Yellow".to_string(),
        medical_status: "Unknown".to_string(),
        mobility_status: "Unknown".to_string(),
        comms_status: "Unknown".to_string(),
        notes: Some("first native eam".to_string()),
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        eam_uid: Some("eam-repeated-native".to_string()),
        team_member_uid: Some(node_a_status.app_destination_hex.clone()),
        team_uid: Some(TEAM_UID_YELLOW.to_string()),
        reported_at: Some("2026-03-27T15:00:00Z".to_string()),
        reported_by: Some(node_a_status.name.clone()),
        overall_status: Some("Yellow".to_string()),
        confidence: Some(0.9),
        ttl_seconds: Some(3600),
        source: Some(EamSourceRecord {
            rns_identity: node_a_status.identity_hex.clone(),
            display_name: Some(node_a_status.name.clone()),
        }),
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(now_ms()),
        last_synced_at_ms: None,
    };

    node_a
        .upsert_eam(first_record.clone())
        .expect("upsert initial eam");

    let first_received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    let first_received = loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| eam.callsign == first_record.callsign && eam.deleted_at_ms.is_none());
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < first_received_deadline,
            "node b never persisted initial eam update"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert_eq!(first_received.callsign, first_record.callsign);

    let mut second_record = first_record.clone();
    second_record.preparedness_status = "Red".to_string();
    second_record.notes = Some("second native eam".to_string());
    second_record.updated_at_ms = first_received.updated_at_ms.saturating_add(1);
    second_record.overall_status = Some("Red".to_string());

    node_a
        .upsert_eam(second_record.clone())
        .expect("upsert repeated eam");

    let second_received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| {
                eam.callsign == second_record.callsign
                    && eam.preparedness_status == second_record.preparedness_status
            });
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < second_received_deadline,
            "node b never persisted repeated eam update"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(received.callsign, second_record.callsign);
    assert!(received.updated_at_ms >= second_record.updated_at_ms);
    assert_eq!(
        received.preparedness_status,
        second_record.preparedness_status
    );
    assert!(received.notes.is_none());

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn eam_replication_targets_only_include_intentional_peers() {
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

    let targets = build_mission_replication_targets(
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
fn eam_route_priority_puts_low_hop_live_peers_before_high_hop_saved_routes() {
    let peers = vec![
        build_peer_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "4d30bc17c71d98436558ff63da188f2a",
            true,
            false,
            false,
        ),
        build_peer_record(
            "a3e80acf291d9f57ee455493946b3763",
            "182af0c5afb2ab7176406539c83676dc",
            true,
            false,
            false,
        ),
        build_peer_record(
            "0e252632c57fa999a15c4a05c0d1bae2",
            "a1c8126d7cb806e6bde086d582b6cb0d",
            true,
            false,
            false,
        ),
        build_peer_record(
            "5ea3149a58998316f6c8200a006e14c2",
            "34eb6bec21defd52736eaca1adff75eb",
            true,
            false,
            false,
        ),
        build_peer_record(
            "a133b8b1fe137f92210a048efded46db",
            "1080d475b77274d7fdabaed81878d916",
            true,
            false,
            false,
        ),
    ];
    let announces = vec![
        build_announce_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "app",
            AnnounceClass::PeerApp {},
            11,
            100,
        ),
        build_announce_record(
            "a3e80acf291d9f57ee455493946b3763",
            "app",
            AnnounceClass::PeerApp {},
            10,
            110,
        ),
        build_announce_record(
            "0e252632c57fa999a15c4a05c0d1bae2",
            "app",
            AnnounceClass::PeerApp {},
            1,
            90,
        ),
        build_announce_record(
            "5ea3149a58998316f6c8200a006e14c2",
            "app",
            AnnounceClass::PeerApp {},
            1,
            80,
        ),
        build_announce_record(
            "1080d475b77274d7fdabaed81878d916",
            "lxmf_delivery",
            AnnounceClass::LxmfDelivery {},
            7,
            120,
        ),
    ];
    let route_hops = announce_route_hops(announces.as_slice());
    let mut targets = vec![
        MissionReplicationTarget {
            app_destination_hex: "3457d5ba744e89bbae543c5ff9c679fb".to_string(),
            send_mode: SendMode::Auto {},
        },
        MissionReplicationTarget {
            app_destination_hex: "a3e80acf291d9f57ee455493946b3763".to_string(),
            send_mode: SendMode::Auto {},
        },
        MissionReplicationTarget {
            app_destination_hex: "0e252632c57fa999a15c4a05c0d1bae2".to_string(),
            send_mode: SendMode::Auto {},
        },
        MissionReplicationTarget {
            app_destination_hex: "5ea3149a58998316f6c8200a006e14c2".to_string(),
            send_mode: SendMode::Auto {},
        },
        MissionReplicationTarget {
            app_destination_hex: "a133b8b1fe137f92210a048efded46db".to_string(),
            send_mode: SendMode::Auto {},
        },
    ];

    prioritize_replication_targets_by_route_hops(
        targets.as_mut_slice(),
        peers.as_slice(),
        &route_hops,
    );

    assert_eq!(
        targets
            .iter()
            .map(|target| target.app_destination_hex.as_str())
            .collect::<Vec<_>>(),
        vec![
            "0e252632c57fa999a15c4a05c0d1bae2",
            "5ea3149a58998316f6c8200a006e14c2",
            "a133b8b1fe137f92210a048efded46db",
            "a3e80acf291d9f57ee455493946b3763",
            "3457d5ba744e89bbae543c5ff9c679fb",
        ]
    );
}

#[test]
fn eam_replication_targets_use_propagation_for_saved_stored_route_without_discovered_peer() {
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
        circle_tier: CircleTier::Inner {},
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

    let targets = build_mission_replication_targets(
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

#[test]
fn eam_replication_targets_include_saved_reachable_peer_without_active_link_when_no_relay() {
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
        circle_tier: CircleTier::Inner {},
    };
    let peers = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        false,
    )];

    let targets =
        build_mission_replication_targets(&status, peers.as_slice(), &[saved_peer], None);

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}
