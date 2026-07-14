#[test]
fn connected_telemetry_destinations_route_only_to_current_hub() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "56565656565656565656565656565656",
        "abababababababababababababababab",
        true,
        true,
        true,
    )];

    let destinations = build_runtime_telemetry_destinations(
        &status,
        peers.as_slice(),
        None,
        Some(&config),
        None,
    )
    .expect("connected telemetry destinations");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "56565656565656565656565656565656"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn connected_telemetry_destinations_route_to_selected_hub_without_current_peer() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );

    let destinations =
        build_runtime_telemetry_destinations(&status, &[], None, Some(&config), None)
            .expect("connected telemetry destinations");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "56565656565656565656565656565656"
    );
    assert!(matches!(
        destinations[0].send_mode,
        SendMode::PropagationOnly {}
    ));
}

#[test]
fn connected_telemetry_destinations_require_selected_hub() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Connected {}, None);

    let err = build_runtime_telemetry_destinations(&status, &[], None, Some(&config), None)
        .expect_err("connected telemetry should require a hub");

    assert!(matches!(err, NodeError::InvalidConfig {}));
}

#[test]
fn semi_autonomous_telemetry_destinations_use_hub_snapshot() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "abababababababababababababababab",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    )];
    let snapshot = HubDirectorySnapshot {
        effective_connected_mode: false,
        items: vec![
            crate::types::HubDirectoryPeerRecord {
                identity: "78787878787878787878787878787878".to_string(),
                destination_hash: "abababababababababababababababab".to_string(),
                display_name: Some("Pixel".to_string()),
                announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
                client_type: Some("rem".to_string()),
                registered_mode: Some("semi_autonomous".to_string()),
                last_seen: Some("2026-04-02T12:43:28Z".to_string()),
                status: Some("active".to_string()),
            },
            crate::types::HubDirectoryPeerRecord {
                identity: "89898989898989898989898989898989".to_string(),
                destination_hash: "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd".to_string(),
                display_name: Some("NoTelemetry".to_string()),
                announce_capabilities: vec!["r3akt".to_string()],
                client_type: Some("rem".to_string()),
                registered_mode: Some("semi_autonomous".to_string()),
                last_seen: Some("2026-04-02T12:43:28Z".to_string()),
                status: Some("active".to_string()),
            },
        ],
        received_at_ms: 123,
    };

    let destinations = build_runtime_telemetry_destinations(
        &status,
        peers.as_slice(),
        None,
        Some(&config),
        Some(&snapshot),
    )
    .expect("semi-autonomous telemetry destinations");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "abababababababababababababababab"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn semi_autonomous_telemetry_destinations_use_propagation_for_unsaved_snapshot_peers() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "abababababababababababababababab",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        true,
        true,
    )];
    let snapshot = HubDirectorySnapshot {
        effective_connected_mode: false,
        items: vec![crate::types::HubDirectoryPeerRecord {
            identity: "78787878787878787878787878787878".to_string(),
            destination_hash: "abababababababababababababababab".to_string(),
            display_name: Some("S8".to_string()),
            announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
            client_type: Some("rem".to_string()),
            registered_mode: Some("semi_autonomous".to_string()),
            last_seen: Some("2026-04-02T12:43:28Z".to_string()),
            status: Some("active".to_string()),
        }],
        received_at_ms: 123,
    };

    let destinations = build_runtime_telemetry_destinations(
        &status,
        peers.as_slice(),
        Some("56565656565656565656565656565656"),
        Some(&config),
        Some(&snapshot),
    )
    .expect("semi-autonomous telemetry destinations");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "abababababababababababababababab"
    );
    assert!(matches!(
        destinations[0].send_mode,
        SendMode::PropagationOnly {}
    ));
}

#[test]
fn semi_autonomous_telemetry_destinations_fall_back_without_selected_hub() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::SemiAutonomous {}, None);
    let peers = vec![
        build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        ),
        build_peer_record(
            "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
            "efefefefefefefefefefefefefefefef",
            true,
            true,
            false,
        ),
    ];

    let destinations = build_runtime_telemetry_destinations(
        &status,
        peers.as_slice(),
        None,
        Some(&config),
        None,
    )
    .expect("semi-autonomous fallback telemetry destinations");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn telemetry_targets_keep_direct_peer_before_lxmf_announce_is_current() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    peer.lxmf_last_seen_at_ms = None;
    let peers = vec![peer];

    let destinations = build_runtime_telemetry_destinations(
        &status,
        peers.as_slice(),
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn telemetry_targets_use_direct_for_saved_peers_when_relay_is_active() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );

    let destinations = build_runtime_telemetry_destinations(
        &status,
        &[peer],
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn telemetry_targets_include_direct_and_propagation_fanout() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let direct_peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    let mut relay_peer = build_peer_record(
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        "efefefefefefefefefefefefefefefef",
        true,
        false,
        false,
    );
    relay_peer.lxmf_last_seen_at_ms = None;

    let destinations = build_runtime_telemetry_destinations(
        &status,
        &[direct_peer, relay_peer],
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert_eq!(destinations.len(), 2);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
    assert_eq!(
        destinations[1].app_destination_hex,
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
    );
    assert!(matches!(
        destinations[1].send_mode,
        SendMode::PropagationOnly {}
    ));
}

#[test]
fn telemetry_targets_skip_stale_peers_without_relay() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let mut stale_peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    stale_peer.stale = true;

    let destinations =
        build_runtime_telemetry_destinations(&status, &[stale_peer], None, Some(&config), None)
            .expect("telemetry fallback targets");

    assert!(destinations.is_empty());
}

#[test]
fn telemetry_targets_keep_direct_peer_when_relay_is_active_before_lxmf_route_is_known() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        true,
        true,
    );
    peer.lxmf_destination_hex = None;
    peer.lxmf_last_seen_at_ms = None;

    let destinations = build_runtime_telemetry_destinations(
        &status,
        &[peer],
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(destinations[0].send_mode, SendMode::Auto {}));
}

#[test]
fn telemetry_targets_skip_unsaved_discovered_peers_when_relay_is_active() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        false,
        false,
    );
    peer.lxmf_last_seen_at_ms = None;

    let destinations = build_runtime_telemetry_destinations(
        &status,
        &[peer],
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert!(destinations.is_empty());
}

#[test]
fn telemetry_targets_use_relay_for_fresh_route_without_active_link_when_relay_exists() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None);
    let peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        false,
    );

    let destinations = build_runtime_telemetry_destinations(
        &status,
        &[peer],
        Some("56565656565656565656565656565656"),
        Some(&config),
        None,
    )
    .expect("telemetry fallback targets");

    assert_eq!(destinations.len(), 1);
    assert_eq!(
        destinations[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(matches!(
        destinations[0].send_mode,
        SendMode::PropagationOnly {}
    ));
}

fn build_eam() -> EamProjectionRecord {
    EamProjectionRecord {
        callsign: "POCO".to_string(),
        group_name: "Blue".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Yellow".to_string(),
        preparedness_status: "Green".to_string(),
        medical_status: "Green".to_string(),
        mobility_status: "Green".to_string(),
        comms_status: "Yellow".to_string(),
        notes: Some("pre-start eam".to_string()),
        updated_at_ms: 1_700_000_000_100,
        deleted_at_ms: None,
        eam_uid: Some("eam-1".to_string()),
        team_member_uid: Some("member-1".to_string()),
        team_uid: Some("team-1".to_string()),
        reported_at: Some("2026-03-25T00:00:00Z".to_string()),
        reported_by: Some("Atlas-1".to_string()),
        overall_status: Some("Yellow".to_string()),
        confidence: Some(0.9),
        ttl_seconds: Some(3600),
        source: Some(EamSourceRecord {
            rns_identity: "identity-1".to_string(),
            display_name: Some("Atlas-1".to_string()),
        }),
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(1_700_000_000_100),
        last_synced_at_ms: None,
    }
}
