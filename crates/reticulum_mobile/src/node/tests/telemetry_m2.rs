fn saved_telemetry_fixtures(peers: &[PeerRecord]) -> Vec<SavedPeerRecord> {
    peers
        .iter()
        .filter(|peer| peer.saved)
        .map(|peer| SavedPeerRecord {
            destination_hex: peer.destination_hex.clone(),
            label: None,
            saved_at_ms: now_ms(),
            identity_hex: peer.identity_hex.clone(),
            lxmf_destination_hex: peer.lxmf_destination_hex.clone(),
            app_data: peer.app_data.clone(),
            display_name: peer.display_name.clone(),
            last_route_seen_at_ms: Some(peer.last_seen_at_ms),
            last_hops: None,
            circle_tier: CircleTier::Inner {},
        })
        .collect()
}

#[test]
fn telemetry_m2_saved_only_and_cold_live_targets_preserve_trust() {
    let status = build_status_for_tests();
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        false,
    );
    peer.active_link = false;
    peer.state = crate::types::PeerState::Disconnected {};
    peer.stale = false;
    peer.announce_last_seen_at_ms = Some(now_ms());
    let saved = saved_telemetry_fixtures(&[peer.clone()]);
    let mut snapshot = HubDirectorySnapshot::yellow_only(now_ms());
    snapshot.local_teams = vec![crate::types::LocalTeamRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        member_destinations: vec![peer.destination_hex.clone()],
    }];
    let mut delivery_alias = peer.clone();
    delivery_alias.destination_hex = peer.lxmf_destination_hex.clone().unwrap();
    for live in [vec![], vec![peer.clone()], vec![delivery_alias]] {
        let targets = build_runtime_telemetry_destinations(
            &status,
            &live,
            &saved,
            None,
            None,
            Some(&snapshot),
        )
        .unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(Some(&targets[0].app_destination_hex), peer.lxmf_destination_hex.as_ref());
        assert!(matches!(targets[0].send_mode, SendMode::Auto {}));
    }
    let mut revoked = peer.clone();
    revoked.app_data = Some("r3akt".to_string());
    assert!(build_runtime_telemetry_destinations(
        &status,
        &[revoked],
        &saved,
        None,
        None,
        Some(&snapshot)
    )
    .unwrap()
    .is_empty());
    assert!(build_runtime_telemetry_destinations(
        &status,
        &[peer.clone()],
        &[],
        None,
        None,
        Some(&snapshot)
    )
    .unwrap()
    .is_empty());
    snapshot.local_teams[0].member_destinations.clear();
    assert!(build_runtime_telemetry_destinations(
        &status,
        &[peer],
        &saved,
        None,
        None,
        Some(&snapshot)
    )
    .unwrap()
    .is_empty());
}

fn publication_test_node(name: &str) -> (Node, mpsc::Receiver<Command>, PeerRecord) {
    let node = Node::with_storage_dir(prepare_storage_dir(name).to_str()).unwrap();
    let peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    node.set_saved_peers(saved_telemetry_fixtures(&[peer.clone()]))
        .unwrap();
    set_test_team(&node, vec![peer.destination_hex.clone()]);
    let (tx, rx) = mpsc::channel(16);
    {
        let mut inner = node.inner.lock().unwrap();
        *inner.status.lock().unwrap() = build_status_for_tests();
        *inner.peers_snapshot.lock().unwrap() = vec![peer.clone()];
        inner.cmd_tx = Some(tx);
    }
    (node, rx, peer)
}

#[test]
fn telemetry_m2_coalesces_before_runtime_and_survives_reconstruction() {
    let (node, mut commands, peer) = publication_test_node("telemetry-m2-coalesce");
    {
        let inner = node.inner.lock().unwrap();
        inner.status.lock().unwrap().running = false;
        inner.peers_snapshot.lock().unwrap().clear();
        let mut saved = inner.app_state.get_saved_peers().unwrap();
        saved[0].last_route_seen_at_ms = None;
        inner.app_state.set_saved_peers(&saved).unwrap();
    }
    let mut position = build_telemetry();
    position.updated_at_ms = now_ms();
    node.record_local_telemetry_fix(position.clone()).unwrap();
    let mut inflight = HashMap::new();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    assert!(commands.try_recv().is_err());
    position.updated_at_ms += 1;
    position.lat += 0.01;
    node.record_local_telemetry_fix(position.clone()).unwrap();
    {
        let inner = node.inner.lock().unwrap();
        let persisted = inner.app_state.clone().telemetry_publication().unwrap();
        assert_eq!(persisted.fix.unwrap().lat, position.lat);
        assert_eq!(persisted.recipients.len(), 1);
        assert_eq!(
            persisted.recipients[0].state,
            PublicationState::QueuedNoRoute
        );
        assert!(inner.peers_snapshot.lock().unwrap().is_empty());
        inner.status.lock().unwrap().running = true;
        *inner.peers_snapshot.lock().unwrap() = vec![peer.clone()];
        inner
            .status
            .lock()
            .unwrap()
            .interfaces
            .push(crate::types::InterfaceStatusRecord {
                interface_hex: "c".repeat(32),
                label: "rnode-ble:test".to_string(),
                kind: "RNode".to_string(),
                state: "ready".to_string(),
                last_error: None,
                rx_packets: 0,
                rx_bytes: 0,
                last_activity_ms: 0,
            });
    }
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let Command::SendBytes {
        destination_hex,
        bytes,
        fields_bytes,
        resp,
        ..
    } = commands.try_recv().unwrap()
    else {
        panic!("telemetry send expected");
    };
    assert_eq!(Some(destination_hex), peer.lxmf_destination_hex.clone());
    assert_eq!(bytes, b"T");
    let fields = fields_bytes.unwrap();
    let metadata = parse_mission_sync_metadata(&fields).unwrap();
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.telemetry.upsert")
    );
    let first_command = metadata.command_id.unwrap();
    resp.send(Err(NodeError::Timeout {})).unwrap();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let publication = node
        .inner
        .lock()
        .unwrap()
        .app_state
        .telemetry_publication()
        .unwrap();
    assert_eq!(publication.recipients[0].state, PublicationState::TimedOut);
    assert_eq!(publication.recipients[0].command_id, first_command);
    assert!(commands.try_recv().is_err());
    {
        let inner = node.inner.lock().unwrap();
        inner.app_state.set_saved_peers(&[]).unwrap();
    }
    assert!(matches!(
        node.queue_bytes_with_class(
            peer.destination_hex,
            b"T".to_vec(),
            None,
            SendMode::Auto {},
            OutboundTrafficClass::Telemetry {}
        ),
        Err(NodeError::InvalidConfig {})
    ));
    node.flush_pending_telemetry(&mut inflight).unwrap();
    assert!(node
        .inner
        .lock()
        .unwrap()
        .app_state
        .telemetry_publication()
        .unwrap()
        .recipients
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn telemetry_m2_cold_start_first_workflow_persists_and_application_acks() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair_with_profile("telemetry-m2-first", true).await;
    for node in [&node_a, &node_b] {
        let interfaces = node.get_status().interfaces;
        assert!(!interfaces.is_empty());
        assert!(interfaces.iter().all(|interface| interface.kind == "rnode_ble"));
    }
    let status_b = node_b.get_status();
    let mut saved = saved_telemetry_fixtures(&[build_peer_record(
        &status_b.app_destination_hex,
        &status_b.lxmf_destination_hex,
        true,
        false,
        false,
    )]);
    saved[0].identity_hex = Some(status_b.identity_hex.clone());
    node_a.set_saved_peers(saved).unwrap();
    assert!(
        node_a
            .list_peers()
            .unwrap()
            .iter()
            .all(|peer| !peer.active_link),
        "fixture must not pre-warm a link"
    );
    let subscription = node_a.subscribe_events();
    let mut position = build_telemetry();
    position.updated_at_ms = now_ms();
    node_a.record_local_telemetry_fix(position.clone()).unwrap();
    let event = wait_for_event(&subscription, Duration::from_secs(75), |event| matches!(event, NodeEvent::LxmfDelivery { update } if update.command_type.as_deref() == Some("mission.registry.telemetry.upsert") && matches!(update.application_ack_state, ApplicationAckState::Accepted {}))).expect("correlated application acceptance");
    let NodeEvent::LxmfDelivery { update } = event else {
        unreachable!()
    };
    assert_eq!(
        update.representation,
        crate::types::LxmfDeliveryRepresentation::Resource {}
    );
    assert!(!update.message_id_hex.is_empty());
    assert!(update.command_id.is_some());
    assert_eq!(update.destination_hex, status_b.lxmf_destination_hex);
    let received = node_b
        .get_telemetry_positions()
        .unwrap()
        .into_iter()
        .find(|fix| fix.callsign == position.callsign)
        .unwrap();
    assert_eq!(received.updated_at_ms, position.updated_at_ms);
    assert_eq!(received.lat, position.lat);
    assert_eq!(
        node_a
            .inner
            .lock()
            .unwrap()
            .app_state
            .telemetry_publication()
            .unwrap()
            .recipients[0]
            .state,
        PublicationState::Acknowledged
    );
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn telemetry_m2_zero_targets_is_local_and_old_ack_cannot_accept_new_fix() {
    let (node, _commands, _peer) = publication_test_node("telemetry-m2-outcomes");
    let mut fix = build_telemetry();
    fix.updated_at_ms = now_ms();
    node.record_local_telemetry_fix(fix.clone()).unwrap();
    let mut inflight = HashMap::new();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let app_state = node.inner.lock().unwrap().app_state.clone();
    app_state
        .update_telemetry_publication(|publication| {
            publication.recipients[0].command_id = "current".to_string()
        })
        .unwrap();
    let mut update = crate::types::LxmfDeliveryUpdate {
        message_id_hex: "b".repeat(64),
        destination_hex: "a".repeat(32),
        source_hex: None,
        command_id: Some("older-fix".to_string()),
        correlation_id: Some("older-fix".to_string()),
        command_type: Some("mission.registry.telemetry.upsert".to_string()),
        event_uid: None,
        mission_uid: None,
        status: crate::types::LxmfDeliveryStatus::Acknowledged {},
        transport_state: TransportDeliveryState::TransportDelivered {},
        application_ack_state: ApplicationAckState::Accepted {},
        method: crate::types::LxmfDeliveryMethod::Direct {},
        representation: crate::types::LxmfDeliveryRepresentation::Packet {},
        relay_destination_hex: None,
        fallback_stage: None,
        detail: None,
        sent_at_ms: now_ms(),
        updated_at_ms: now_ms(),
    };
    app_state.record_telemetry_delivery(&update).unwrap();
    assert_eq!(
        app_state.telemetry_publication().unwrap().recipients[0].state,
        PublicationState::QueuedForDelivery
    );
    update.command_id = Some("current".to_string());
    update.status = crate::types::LxmfDeliveryStatus::Delivered {};
    update.application_ack_state = ApplicationAckState::Waiting {};
    app_state.record_telemetry_delivery(&update).unwrap();
    assert_eq!(
        app_state.telemetry_publication().unwrap().recipients[0].state,
        PublicationState::TransportDelivered
    );
    update.status = crate::types::LxmfDeliveryStatus::Acknowledged {};
    update.application_ack_state = ApplicationAckState::Rejected {};
    app_state.record_telemetry_delivery(&update).unwrap();
    assert_eq!(
        app_state.telemetry_publication().unwrap().recipients[0].state,
        PublicationState::Rejected
    );
    update.status = crate::types::LxmfDeliveryStatus::Delivered {};
    update.application_ack_state = ApplicationAckState::Waiting {};
    app_state.record_telemetry_delivery(&update).unwrap();
    assert_eq!(
        app_state.telemetry_publication().unwrap().recipients[0].state,
        PublicationState::Rejected
    );
    app_state.set_saved_peers(&[]).unwrap();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let snapshot = node.telemetry_publication_json().unwrap();
    assert_eq!(snapshot["state"], "StoredLocally");
    assert!(snapshot["recipients"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn telemetry_m2_pending_fix_survives_node_restart_without_warmup() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair_with_profile("telemetry-m2-restart", true).await;
    let config = node_a
        .inner
        .lock()
        .unwrap()
        .active_config
        .as_ref()
        .unwrap()
        .to_config();
    let status_b = node_b.get_status();
    let mut saved = saved_telemetry_fixtures(&[build_peer_record(
        &status_b.app_destination_hex,
        &status_b.lxmf_destination_hex,
        true,
        false,
        false,
    )]);
    saved[0].identity_hex = Some(status_b.identity_hex.clone());
    node_a.set_saved_peers(saved).unwrap();
    let initial_events = node_a.subscribe_events();
    let mut position = build_telemetry();
    position.updated_at_ms = now_ms();
    node_a.record_local_telemetry_fix(position.clone()).unwrap();
    wait_for_event(&initial_events, Duration::from_secs(75), |event| matches!(event,
        NodeEvent::LxmfDelivery { update }
            if update.command_type.as_deref() == Some("mission.registry.telemetry.upsert")
                && matches!(update.application_ack_state, ApplicationAckState::Accepted {})
    )).expect("initial telemetry before losing the runtime link");
    stop_node(node_a).await;
    let node_a = Node::with_storage_dir(config.storage_dir.as_deref()).unwrap();
    assert!(node_a.list_peers().unwrap().is_empty());
    assert!(node_a.get_status().interfaces.is_empty());
    position.updated_at_ms += 1;
    position.lat += 0.01;
    node_a.record_local_telemetry_fix(position.clone()).unwrap();
    position.updated_at_ms += 1;
    position.lat += 0.01;
    node_a.record_local_telemetry_fix(position.clone()).unwrap();
    drop(node_a);
    let node_a = Node::with_storage_dir(config.storage_dir.as_deref()).unwrap();
    let pending = node_a.inner.lock().unwrap().app_state.telemetry_publication().unwrap();
    assert_eq!(pending.fix, Some(position.clone()));
    assert_eq!(pending.recipients[0].state, PublicationState::QueuedNoRoute);
    assert!(node_a.list_peers().unwrap().is_empty());
    let subscription = node_a.subscribe_events();
    node_a.start(config).unwrap();
    let event = wait_for_event(&subscription, Duration::from_secs(75), |event| matches!(event, NodeEvent::LxmfDelivery { update } if update.command_type.as_deref() == Some("mission.registry.telemetry.upsert") && matches!(update.application_ack_state, ApplicationAckState::Accepted {}))).expect("pending telemetry application acceptance after restart");
    let NodeEvent::LxmfDelivery { update } = event else {
        unreachable!()
    };
    assert_eq!(
        update.representation,
        crate::types::LxmfDeliveryRepresentation::Resource {}
    );
    assert!(node_b.get_telemetry_positions().unwrap().iter().any(|fix| {
        fix.callsign == position.callsign && fix.lat == position.lat
            && fix.updated_at_ms == position.updated_at_ms
    }));
    assert_eq!(
        node_b
            .get_telemetry_positions()
            .unwrap()
            .iter()
            .filter(|fix| fix.callsign == position.callsign)
            .count(),
        1
    );
    assert_eq!(
        node_a
            .inner
            .lock()
            .unwrap()
            .app_state
            .telemetry_publication()
            .unwrap()
            .recipients[0]
            .state,
        PublicationState::Acknowledged
    );
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
