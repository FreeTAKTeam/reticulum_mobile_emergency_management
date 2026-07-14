#[test]
fn checklist_participant_targets_use_propagation_for_current_unsaved_source_when_relay_exists()
{
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        false,
        false,
    );
    peer.lxmf_last_seen_at_ms = None;
    let mut targets = build_mission_replication_targets(
        &status,
        &[peer.clone()],
        &[],
        Some("99999999999999999999999999999999"),
    );

    append_checklist_participant_replication_targets(
        &status,
        &[peer],
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            status.identity_hex.clone(),
            "not-a-destination".to_string(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        ],
        Some("99999999999999999999999999999999"),
        &mut targets,
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn checklist_participant_targets_skip_source_without_current_peer() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let mut targets = Vec::new();

    append_checklist_participant_replication_targets(
        &status,
        &[],
        &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()],
        Some("99999999999999999999999999999999"),
        &mut targets,
    );

    assert!(targets.is_empty());
}

#[test]
fn checklist_participant_targets_keep_direct_return_path_when_relay_is_active_before_lxmf_route_is_known(
) {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        true,
        true,
    );
    peer.lxmf_destination_hex = None;
    peer.lxmf_last_seen_at_ms = None;
    let mut targets = Vec::new();

    append_checklist_participant_replication_targets(
        &status,
        &[peer],
        &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()],
        Some("99999999999999999999999999999999"),
        &mut targets,
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
}

#[test]
fn checklist_participant_targets_include_current_participants_when_direct_target_exists() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "pixel".to_string(),
        identity_hex: "22222222222222222222222222222222".to_string(),
        app_destination_hex: "11111111111111111111111111111111".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let direct_target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let mut relay_peer = build_peer_record(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccc",
        false,
        false,
        false,
    );
    relay_peer.lxmf_last_seen_at_ms = None;
    let mut targets = vec![direct_target];

    append_checklist_participant_replication_targets(
        &status,
        &[relay_peer],
        &["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()],
        Some("99999999999999999999999999999999"),
        &mut targets,
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
fn checklist_participant_targets_do_not_override_connected_hub_mode() {
    let connected = build_config_fingerprint_for_tests(HubMode::Connected {}, None);

    assert!(!should_include_checklist_participant_targets(
        Some(&connected),
        None
    ));
}

#[test]
fn join_checklist_updates_local_participants_immediately() {
    let storage_dir = prepare_storage_dir("checklist-join-local");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");
    {
        let inner = node.inner.lock().expect("node inner");
        let mut status = inner.status.lock().expect("status");
        status.identity_hex = "joiner-identity".to_string();
    }

    node.create_online_checklist(ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-join".to_string()),
        mission_uid: Some("mission-alpha".to_string()),
        template_uid: "tmpl-evac-001".to_string(),
        name: "Mission Alpha Evac".to_string(),
        description: "Shared run for Alpha".to_string(),
        start_time: "2026-04-22T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some("creator-identity".to_string()),
        created_by_team_member_display_name: None,
    })
    .expect("create checklist");

    node.join_checklist("chk-join".to_string())
        .expect("join checklist");

    let checklist = node
        .get_checklist("chk-join".to_string())
        .expect("get checklist")
        .expect("checklist exists");
    assert!(checklist
        .participant_rns_identities
        .iter()
        .any(|value| value == "creator-identity"));
    assert!(checklist
        .participant_rns_identities
        .iter()
        .any(|value| value == "joiner-identity"));
    assert_eq!(
        checklist
            .last_changed_by_team_member_rns_identity
            .as_deref(),
        Some("joiner-identity")
    );
}
