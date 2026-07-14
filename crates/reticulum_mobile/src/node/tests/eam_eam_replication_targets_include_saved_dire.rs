#[test]
fn eam_replication_targets_include_saved_direct_peer_without_lxmf_snapshot() {
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
        label: Some("saved-direct".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let peer = PeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        identity_hex: Some("identity-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        lxmf_destination_hex: None,
        display_name: Some("peer-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
        state: crate::types::PeerState::Connected {},
        saved: true,
        stale: false,
        active_link: true,
        hub_derived: false,
        last_resolution_error: None,
        last_resolution_attempt_at_ms: Some(now_ms()),
        last_seen_at_ms: now_ms(),
        announce_last_seen_at_ms: Some(now_ms()),
        lxmf_last_seen_at_ms: None,
    };

    let targets = build_mission_replication_targets(&status, &[peer], &[saved_peer], None);

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}

#[test]
fn eam_replication_targets_keep_direct_peer_when_relay_is_active_before_lxmf_route_is_known() {
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
        label: Some("saved-direct".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );
    peer.lxmf_destination_hex = None;
    peer.lxmf_last_seen_at_ms = None;

    let targets = build_mission_replication_targets(
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
fn eam_replication_targets_keep_direct_peer_before_lxmf_announce_is_current() {
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
    peer.lxmf_last_seen_at_ms = None;

    let targets = build_mission_replication_targets(
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
fn eam_replication_targets_use_propagation_for_saved_stored_route_without_active_link() {
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
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn eam_replication_targets_skip_saved_peer_without_current_peer() {
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

    let targets = build_mission_replication_targets(&status, &[], &[saved_peer], None);

    assert!(targets.is_empty());
}

#[test]
fn eam_replication_targets_use_saved_lxmf_profile_without_current_peer() {
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

    let targets = build_mission_replication_targets(
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
