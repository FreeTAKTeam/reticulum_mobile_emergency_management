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
