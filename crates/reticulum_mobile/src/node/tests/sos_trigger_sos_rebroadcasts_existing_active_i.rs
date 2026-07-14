#[test]
fn trigger_sos_rebroadcasts_existing_active_incident() {
    let storage_dir = prepare_storage_dir("sos-active-rebroadcast");
    let storage_dir_text = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(storage_dir_text.as_str())).expect("node storage");
    let (tx, mut rx) = mpsc::channel(4);
    let mut settings = default_sos_settings();
    settings.enabled = true;
    settings.countdown_seconds = 0;
    settings.include_location = false;
    let existing = active_status(
        "incident-existing".to_string(),
        SosTriggerSource::Manual {},
        1_700_000_000_000,
    );

    {
        let mut inner = node.inner.lock().expect("node lock");
        inner
            .app_state
            .set_sos_settings(&settings)
            .expect("persist sos settings");
        inner
            .app_state
            .set_sos_status(&existing, "test-active")
            .expect("persist active status");
        inner
            .app_state
            .set_saved_peers(&[build_saved_peer()])
            .expect("persist saved peer");
        *inner.status.lock().expect("status lock") = build_status_for_tests();
        *inner.peers_snapshot.lock().expect("peers lock") = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        )];
        inner.cmd_tx = Some(tx);
    }

    let status = node
        .trigger_sos(SosTriggerSource::Manual {})
        .expect("rebroadcast active sos");
    assert!(matches!(status.state, SosState::Active {}));
    assert_eq!(status.incident_id.as_deref(), Some("incident-existing"));

    let command = rx.try_recv().expect("expected sos send command");
    if let Command::SendBytes {
        destination_hex,
        bytes,
        fields_bytes,
        send_mode,
        ..
    } = command
    {
        assert_eq!(destination_hex, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(matches!(send_mode, SendMode::Auto {}));
        let body = String::from_utf8(bytes).expect("sos body utf8");
        assert!(body.starts_with("SOS! I need help."));
        let parsed =
            crate::sos_fields::parse_sos_fields(fields_bytes.as_deref().expect("sos fields"))
                .expect("parsed sos fields");
        let command = parsed.command.expect("sos command field");
        assert_eq!(command.incident_id, "incident-existing");
        assert!(matches!(command.state, SosMessageKind::Update {}));
        assert!(matches!(
            command.trigger_source,
            SosTriggerSource::Manual {}
        ));
    } else {
        panic!("expected SendBytes command");
    }
}

#[test]
fn trigger_sos_uses_priority_command_lane_when_available() {
    let storage_dir = prepare_storage_dir("sos-priority-command-lane");
    let storage_dir_text = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(storage_dir_text.as_str())).expect("node storage");
    let (normal_tx, mut normal_rx) = mpsc::channel(4);
    let (priority_tx, mut priority_rx) = mpsc::channel(4);
    let mut settings = default_sos_settings();
    settings.enabled = true;
    settings.countdown_seconds = 0;
    settings.include_location = false;

    {
        let mut inner = node.inner.lock().expect("node lock");
        inner
            .app_state
            .set_sos_settings(&settings)
            .expect("persist sos settings");
        inner
            .app_state
            .set_saved_peers(&[build_saved_peer()])
            .expect("persist saved peer");
        *inner.status.lock().expect("status lock") = build_status_for_tests();
        *inner.peers_snapshot.lock().expect("peers lock") = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        )];
        inner.cmd_tx = Some(normal_tx);
        inner.priority_cmd_tx = Some(priority_tx);
    }

    let status = node
        .trigger_sos(SosTriggerSource::Manual {})
        .expect("trigger sos");
    assert!(matches!(status.state, SosState::Active {}));

    let command = priority_rx
        .try_recv()
        .expect("expected sos send command on priority lane");
    assert!(matches!(command, Command::SendBytes { .. }));
    assert!(normal_rx.try_recv().is_err());
}

#[test]
fn sos_targets_all_current_saved_peers_without_hub_narrowing() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    );
    let peers = vec![
        build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        ),
        build_peer_record(
            "cccccccccccccccccccccccccccccccc",
            "dddddddddddddddddddddddddddddddd",
            true,
            true,
            true,
        ),
    ];

    let regular_targets = build_runtime_mission_replication_targets(
        &status,
        peers.as_slice(),
        &[
            build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            build_saved_peer_for("cccccccccccccccccccccccccccccccc"),
        ],
        None,
        Some(&config),
        None,
    )
    .expect("regular mission targets");
    assert_eq!(regular_targets.len(), 1);

    let saved_peers = [
        build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        build_saved_peer_for("cccccccccccccccccccccccccccccccc"),
    ];
    let sos_targets =
        build_sos_replication_targets(&status, peers.as_slice(), &saved_peers, None);

    let destinations = sos_targets
        .iter()
        .map(|target| target.app_destination_hex.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(destinations.len(), 2);
    assert!(destinations.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    assert!(destinations.contains("cccccccccccccccccccccccccccccccc"));
    assert!(sos_targets
        .iter()
        .all(|target| matches!(target.send_mode, SendMode::Auto {})));
}

#[test]
fn trigger_sos_fans_out_to_all_current_saved_peers() {
    let storage_dir = prepare_storage_dir("sos-multi-peer-fanout");
    let storage_dir_text = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(storage_dir_text.as_str())).expect("node storage");
    let (tx, mut rx) = mpsc::channel(4);
    let mut settings = default_sos_settings();
    settings.enabled = true;
    settings.countdown_seconds = 0;
    settings.include_location = false;

    {
        let mut inner = node.inner.lock().expect("node lock");
        inner
            .app_state
            .set_sos_settings(&settings)
            .expect("persist sos settings");
        inner
            .app_state
            .set_saved_peers(&[
                build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                build_saved_peer_for("cccccccccccccccccccccccccccccccc"),
            ])
            .expect("persist saved peers");
        *inner.status.lock().expect("status lock") = build_status_for_tests();
        *inner.peers_snapshot.lock().expect("peers lock") = vec![
            build_peer_record(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                true,
                true,
                true,
            ),
            build_peer_record(
                "cccccccccccccccccccccccccccccccc",
                "dddddddddddddddddddddddddddddddd",
                true,
                true,
                true,
            ),
        ];
        inner.active_config = Some(build_config_fingerprint_for_tests(
            HubMode::Connected {},
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ));
        inner.cmd_tx = Some(tx);
    }

    let status = node
        .trigger_sos(SosTriggerSource::Manual {})
        .expect("trigger sos");
    assert!(matches!(status.state, SosState::Active {}));

    let mut destinations = HashSet::new();
    for _ in 0..2 {
        let command = rx.try_recv().expect("expected sos send command");
        if let Command::SendBytes {
            destination_hex,
            fields_bytes,
            send_mode,
            ..
        } = command
        {
            destinations.insert(destination_hex);
            assert!(fields_bytes.is_some());
            assert!(matches!(send_mode, SendMode::Auto {}));
        } else {
            panic!("expected SendBytes command");
        }
    }
    assert_eq!(destinations.len(), 2);
    assert!(destinations.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    assert!(destinations.contains("cccccccccccccccccccccccccccccccc"));
    assert!(rx.try_recv().is_err());
}

#[test]
fn sos_targets_use_persisted_saved_peers_when_snapshot_saved_flag_is_stale() {
    let status = build_status_for_tests();
    let peers = vec![
        build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            false,
        ),
        build_peer_record(
            "cccccccccccccccccccccccccccccccc",
            "dddddddddddddddddddddddddddddddd",
            true,
            true,
            false,
        ),
    ];
    let saved_peers = [
        build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        build_saved_peer_for("cccccccccccccccccccccccccccccccc"),
    ];

    let targets = build_sos_replication_targets(&status, peers.as_slice(), &saved_peers, None);

    let destinations = targets
        .iter()
        .map(|target| target.app_destination_hex.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        destinations,
        vec![
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "cccccccccccccccccccccccccccccccc"
        ]
    );
    assert!(targets
        .iter()
        .all(|target| matches!(target.send_mode, SendMode::Auto {})));
}

#[test]
fn sos_route_priority_omits_stale_saved_route_without_relay() {
    let status = build_status_for_tests();
    let mut stale_saved_peer = build_peer_record(
        "11111111111111111111111111111111",
        "22222222222222222222222222222222",
        true,
        false,
        false,
    );
    stale_saved_peer.stale = true;
    stale_saved_peer.announce_last_seen_at_ms = None;

    let peers = vec![
        stale_saved_peer,
        build_peer_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "4d30bc17c71d98436558ff63da188f2a",
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
    ];
    let saved_peers = peers
        .iter()
        .map(|peer| build_saved_peer_for(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    let announces = vec![
        build_announce_record(
            "11111111111111111111111111111111",
            "app",
            AnnounceClass::PeerApp {},
            1,
            80,
        ),
        build_announce_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "app",
            AnnounceClass::PeerApp {},
            3,
            100,
        ),
        build_announce_record(
            "0e252632c57fa999a15c4a05c0d1bae2",
            "app",
            AnnounceClass::PeerApp {},
            2,
            90,
        ),
    ];
    let route_hops = announce_route_hops(announces.as_slice());
    let mut targets =
        build_sos_replication_targets(&status, peers.as_slice(), saved_peers.as_slice(), None);

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
            "3457d5ba744e89bbae543c5ff9c679fb",
        ]
    );
}

#[test]
fn sos_targets_use_propagation_for_saved_stale_stored_route_when_relay_exists() {
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

    let targets = build_sos_replication_targets(
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
fn sos_targets_skip_saved_stale_stored_route_without_relay() {
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

    let targets = build_sos_replication_targets(&status, &[peer], &[saved_peer], None);

    assert!(targets.is_empty());
}

#[test]
fn sos_targets_keep_active_direct_link_on_auto() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    );

    let targets = build_sos_replication_targets(
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
