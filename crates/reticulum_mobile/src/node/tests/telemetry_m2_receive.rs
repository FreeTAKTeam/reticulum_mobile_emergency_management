#[test]
fn telemetry_m2_replayed_older_fix_cannot_regress_receiver_projection() {
    let store = AppStateStore::new(prepare_storage_dir("telemetry-m2-monotonic").to_str()).unwrap();
    let mut fix = build_telemetry();
    fix.updated_at_ms = 200;
    store.record_local_telemetry_fix(&fix).unwrap();
    let mut old = fix.clone();
    old.updated_at_ms = 100;
    old.lat = 1.0;
    store.record_local_telemetry_fix(&old).unwrap();
    store.record_local_telemetry_fix(&fix).unwrap();
    let stored = store.get_telemetry_positions().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0], fix);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn telemetry_m2_rejection_is_correlated_and_diagnostic_contains_no_position() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry-m2-reject").await;
    let subscription = node_a.subscribe_events();
    let diagnostics = node_b.subscribe_events();
    let fields = mission_command_fields(
        "telemetry-reject",
        "telemetry-reject-correlation",
        "mission.registry.telemetry.upsert",
        vec![
            ("callsign", MsgPackValue::from("PrivateCallsign")),
            ("lon", MsgPackValue::from(-63.987654321)),
        ],
    );
    node_a
        .send_bytes(
            node_b.get_status().lxmf_destination_hex,
            b"T".to_vec(),
            Some(fields),
            SendMode::Auto {},
        )
        .unwrap();
    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| matches!(event, NodeEvent::LxmfDelivery { update } if update.command_id.as_deref() == Some("telemetry-reject") && matches!(update.application_ack_state, ApplicationAckState::Rejected {}))).expect("correlated rejection");
    let NodeEvent::LxmfDelivery { update } = event else {
        unreachable!()
    };
    assert_eq!(
        update.source_hex.as_deref(),
        Some(node_b.get_status().lxmf_destination_hex.as_str())
    );
    let diagnostic = wait_for_event(&diagnostics, TEST_TIMEOUT, |event| matches!(event, NodeEvent::Error { code, .. } if code == "TelemetryReceiveMissingLatitude")).unwrap();
    let NodeEvent::Error { message, .. } = diagnostic else {
        unreachable!()
    };
    assert!(!message.contains("PrivateCallsign"));
    assert!(!message.contains("63.987"));
    let snapshot = node_b.telemetry_publication_json().unwrap();
    assert_eq!(
        snapshot["last_receive_failure"]["reason"],
        "MissingLatitude"
    );
    assert_eq!(
        snapshot["last_receive_failure"]["command_id"],
        "telemetry-reject"
    );
    assert!(node_b.get_telemetry_positions().unwrap().is_empty());
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn telemetry_m2_partial_fanout_failure_keeps_other_destination_pending() {
    let (node, mut commands, first_peer) = publication_test_node("telemetry-m2-fanout");
    let second_peer = build_peer_record(
        "cccccccccccccccccccccccccccccccc",
        "dddddddddddddddddddddddddddddddd",
        true,
        false,
        false,
    );
    let app_state = node.inner.lock().unwrap().app_state.clone();
    app_state
        .set_saved_peers(&saved_telemetry_fixtures(&[
            first_peer.clone(),
            second_peer.clone(),
        ]))
        .unwrap();
    // Update the authoritative team and its projection before attaching the test command lane.
    let mut settings = app_state.get_app_settings().unwrap().unwrap();
    settings.teams.local_teams[0]
        .member_destinations
        .push(second_peer.destination_hex.clone());
    app_state.set_app_settings(&settings).unwrap();
    {
        let inner = node.inner.lock().unwrap();
        *inner.hub_directory_snapshot.lock().unwrap() = Some(HubDirectorySnapshot {
            local_teams: settings.teams.local_teams.clone(),
            ..HubDirectorySnapshot::yellow_only(now_ms())
        });
        *inner.peers_snapshot.lock().unwrap() = vec![first_peer, second_peer];
        inner
            .status
            .lock()
            .unwrap()
            .interfaces
            .push(crate::types::InterfaceStatusRecord {
                interface_hex: "e".repeat(32),
                label: "rnode-ble:test".to_string(),
                kind: "RNode".to_string(),
                state: "ready".to_string(),
                last_error: None,
                rx_packets: 0,
                rx_bytes: 0,
                last_activity_ms: 0,
            });
    }
    let mut fix = build_telemetry();
    fix.updated_at_ms = now_ms();
    node.record_local_telemetry_fix(fix).unwrap();
    let mut inflight = HashMap::new();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let Command::SendBytes { resp, .. } = commands.try_recv().unwrap() else {
        panic!("first send");
    };
    let second = commands.try_recv().unwrap();
    resp.send(Err(NodeError::NetworkError {})).unwrap();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let rows = app_state.telemetry_publication().unwrap().recipients;
    assert_eq!(
        rows.iter()
            .filter(|row| row.state == PublicationState::Failed)
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.state == PublicationState::QueuedForDelivery)
            .count(),
        1
    );
    drop(second);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn telemetry_m2_persistence_failure_has_typed_diagnostic_and_rejected_result() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry-m2-io-reject").await;
    let diagnostics = node_b.subscribe_events();
    let sender = node_a.subscribe_events();
    let storage = PathBuf::from(
        node_b
            .inner
            .lock()
            .unwrap()
            .active_config
            .as_ref()
            .unwrap()
            .storage_dir
            .as_ref()
            .unwrap(),
    );
    let db = storage.join("app_state.db");
    let backup = storage.join("app_state.db.telemetry-test-backup");
    std::fs::rename(&db, &backup).unwrap();
    std::fs::create_dir(&db).unwrap();
    let fields = mission_command_fields(
        "telemetry-io-reject",
        "telemetry-io-correlation",
        "mission.registry.telemetry.upsert",
        vec![
            ("callsign", MsgPackValue::from("IO-test")),
            ("lat", MsgPackValue::from(1.0)),
            ("lon", MsgPackValue::from(2.0)),
        ],
    );
    let send = node_a.send_bytes(
        node_b.get_status().lxmf_destination_hex,
        b"T".to_vec(),
        Some(fields),
        SendMode::Auto {},
    );
    let rejected = wait_for_event(
        &sender,
        TEST_TIMEOUT,
        |event| matches!(event, NodeEvent::LxmfDelivery { update } if update.command_id.as_deref() == Some("telemetry-io-reject") && matches!(update.application_ack_state, ApplicationAckState::Rejected {})),
    );
    let diagnostic = wait_for_event(
        &diagnostics,
        TEST_TIMEOUT,
        |event| matches!(event, NodeEvent::Error { code, .. } if code == "TelemetryReceivePersistenceFailure"),
    );
    std::fs::remove_dir(&db).unwrap();
    std::fs::rename(&backup, &db).unwrap();
    send.unwrap();
    assert!(rejected.is_some());
    assert!(diagnostic.is_some());
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[test]
fn telemetry_m2_retry_authorization_rechecks_current_fix_and_contact() {
    let (node, mut commands, peer) = publication_test_node("telemetry-m2-retry-guard");
    let app_state = node.inner.lock().unwrap().app_state.clone();
    let mut fix = build_telemetry();
    fix.updated_at_ms = now_ms();
    node.record_local_telemetry_fix(fix.clone()).unwrap();
    {
        let inner = node.inner.lock().unwrap();
        inner
            .status
            .lock()
            .unwrap()
            .interfaces
            .push(crate::types::InterfaceStatusRecord {
                interface_hex: "e".repeat(32),
                label: "rnode-ble:test".to_string(),
                kind: "RNode".to_string(),
                state: "ready".to_string(),
                last_error: None,
                rx_packets: 0,
                rx_bytes: 0,
                last_activity_ms: 0,
            });
    }
    let mut inflight = HashMap::new();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let Command::SendBytes {
        fields_bytes, resp, ..
    } = commands.try_recv().unwrap()
    else {
        panic!("telemetry send");
    };
    let command = parse_mission_sync_metadata(&fields_bytes.unwrap())
        .unwrap()
        .command_id
        .unwrap();
    let status = node.get_status();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None).to_config();
    let snapshot = node
        .inner
        .lock()
        .unwrap()
        .hub_directory_snapshot
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    assert!(telemetry_delivery_is_current(
        &app_state,
        &status,
        &config,
        &[peer.clone()],
        Some(&snapshot),
        (&peer.destination_hex, &command)
    )
    .unwrap());
    // Same peer/interface, newly available propagation route must release a failed direct attempt.
    resp.send(Err(NodeError::NetworkError {})).unwrap();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    node.inner
        .lock()
        .unwrap()
        .sync_status_snapshot
        .lock()
        .unwrap()
        .active_propagation_node_hex = Some("f".repeat(32));
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let retry = commands.try_recv().unwrap();
    assert!(matches!(
        retry,
        Command::SendBytes {
            send_mode: SendMode::PropagationOnly {},
            ..
        }
    ));
    app_state.set_saved_peers(&[]).unwrap();
    assert!(!telemetry_delivery_is_current(
        &app_state,
        &status,
        &config,
        &[peer.clone()],
        Some(&snapshot),
        (&peer.destination_hex, &command)
    )
    .unwrap());
    app_state
        .set_saved_peers(&saved_telemetry_fixtures(&[peer.clone()]))
        .unwrap();
    fix.updated_at_ms += 1;
    node.record_local_telemetry_fix(fix).unwrap();
    assert!(!telemetry_delivery_is_current(
        &app_state,
        &status,
        &config,
        &[peer.clone()],
        Some(&snapshot),
        (&peer.destination_hex, &command)
    )
    .unwrap());
    node.delete_local_telemetry("POCO".to_string()).unwrap();
    assert!(!telemetry_delivery_is_current(
        &app_state,
        &status,
        &config,
        &[peer.clone()],
        Some(&snapshot),
        (&peer.destination_hex, &command)
    )
    .unwrap());
    drop(retry);
}
