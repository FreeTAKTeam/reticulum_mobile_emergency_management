#[test]
fn event_route_priority_puts_low_hop_live_peers_before_high_hop_saved_routes() {
    let status = build_status_for_tests();
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
    ];
    let saved_peers = peers
        .iter()
        .map(|peer| SavedPeerRecord {
            destination_hex: peer.destination_hex.clone(),
            label: peer.display_name.clone(),
            saved_at_ms: 1_700_000_000_000,
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
        })
        .collect::<Vec<_>>();
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
    ];
    let route_hops = announce_route_hops(announces.as_slice());
    let mut targets = build_event_replication_targets(
        &status,
        peers.as_slice(),
        saved_peers.as_slice(),
        Some("99999999999999999999999999999999"),
    );

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
            "a3e80acf291d9f57ee455493946b3763",
            "3457d5ba744e89bbae543c5ff9c679fb",
        ]
    );
}

#[test]
fn event_replication_targets_include_saved_active_link_without_lxmf_route() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    peer.lxmf_destination_hex = None;
    peer.lxmf_last_seen_at_ms = None;

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
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}

#[test]
fn event_replication_targets_use_propagation_for_saved_stored_route_without_active_link() {
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
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;
    peer.lxmf_last_seen_at_ms = None;
    let peers = vec![peer];

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
fn event_replication_targets_use_propagation_for_stale_saved_peer_with_stored_lxmf_route() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;
    peer.lxmf_last_seen_at_ms = None;

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
fn event_replication_targets_use_propagation_for_observed_lxmf_route_without_active_link() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;

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
fn event_replication_targets_use_propagation_for_stored_route_when_observed_route_is_old() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;
    peer.lxmf_last_seen_at_ms =
        Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

    assert!(!peer_has_observed_lxmf_delivery_route(&peer));
    assert!(!peer_is_direct_delivery_ready(&peer));
    assert!(saved_peer_can_try_stored_lxmf_route(&peer, true));

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
fn event_replication_targets_skip_saved_peer_without_current_peer() {
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

    let targets = build_event_replication_targets(&status, &[], &[saved_peer], None);

    assert!(targets.is_empty());
}

#[test]
fn event_replication_targets_use_saved_lxmf_profile_without_current_peer() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let saved_peer = build_saved_peer_with_lxmf_route(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    let targets = build_event_replication_targets(
        &status,
        &[],
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
