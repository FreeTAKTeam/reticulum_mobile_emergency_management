#[test]
fn saved_offline_mission_recipients_remain_targets_without_a_relay() {
    let status = build_status_for_tests();
    let mut peer = build_peer_record(&"aa".repeat(16), &"bb".repeat(16), true, false, false);
    peer.stale = true;
    peer.last_seen_at_ms = 0;
    peer.announce_last_seen_at_ms = None;
    peer.lxmf_last_seen_at_ms = None;
    let saved = SavedPeerRecord {
        destination_hex: peer.destination_hex.clone(),
        lxmf_destination_hex: peer.lxmf_destination_hex.clone(),
        identity_hex: peer.identity_hex.clone(),
        label: None,
        saved_at_ms: 1,
        app_data: peer.app_data.clone(),
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    };
    for targets in [
        mission_targets_with_offline(&status, &[], std::slice::from_ref(&saved), None),
        event_targets_with_offline(&status, &[], std::slice::from_ref(&saved), None),
        mission_targets_with_offline(
            &status,
            std::slice::from_ref(&peer),
            std::slice::from_ref(&saved),
            None,
        ),
    ] {
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].app_destination_hex, "bb".repeat(16));
        assert_eq!(targets[0].send_mode, SendMode::Auto {});
    }
    peer.app_data = Some("LXMF".to_owned());
    assert!(mission_targets_with_offline(
        &status,
        std::slice::from_ref(&peer),
        std::slice::from_ref(&saved),
        None
    )
    .is_empty());
    assert!(event_targets_with_offline(&status, &[peer], &[saved], None).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rejected_result_retires_custody_without_claiming_acceptance() {
    use crate::lxmf_fields::{FIELD_COMMANDS, FIELD_RESULTS};
    use crate::types::LocalPropagationState;
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("custody-rejection").await;
    let a = node_a.get_status();
    let b = node_b.get_status();
    let mut peer = node_a
        .list_peers()
        .unwrap()
        .into_iter()
        .find(|peer| peer.lxmf_destination_hex.as_deref() == Some(&b.lxmf_destination_hex))
        .unwrap();
    peer.saved = true;
    node_a
        .set_saved_peers(saved_telemetry_fixtures(&[peer]))
        .unwrap();
    let key = "custody-rejection-command";
    let fields = rmp_serde::to_vec(&MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Map(vec![
            (MsgPackValue::from("command_id"), MsgPackValue::from(key)),
            (
                MsgPackValue::from("command_type"),
                MsgPackValue::from("mission.registry.unsupported"),
            ),
        ]),
    )]))
    .unwrap();
    let endpoint = relay.address();
    relay.shutdown().await;
    wait_no_interfaces(&node_a).await;
    let events = node_a.subscribe_events();
    node_a
        .send_bytes_with_class(
            b.lxmf_destination_hex.clone(),
            b"custody rejection regression".to_vec(),
            Some(fields),
            SendMode::PropagationOnly {},
            OutboundTrafficClass::Event {},
        )
        .unwrap();
    let stored = node_a.list_local_propagation().unwrap();
    assert_eq!(stored.len(), 1);
    assert_ne!(stored[0].state, LocalPropagationState::Acknowledged);
    let relay = TcpRelayHandle::start_at(&endpoint).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let result_fields = rmp_serde::to_vec(&MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(vec![
            (MsgPackValue::from("command_id"), MsgPackValue::from(key)),
            (MsgPackValue::from("status"), MsgPackValue::from("rejected")),
        ]),
    )]))
    .unwrap();
    node_b
        .send_bytes(
            a.lxmf_destination_hex,
            b"rejected".to_vec(),
            Some(result_fields),
            SendMode::DirectOnly {},
        )
        .unwrap();
    wait_for_event(&events, TEST_TIMEOUT, |event| {
        matches!(event,
        NodeEvent::LxmfDelivery { update } if update.command_id.as_deref() == Some(key)
            && matches!(update.application_ack_state, ApplicationAckState::Rejected {}))
    })
    .expect("authenticated rejection stays rejected");
    let final_status = node_a.list_local_propagation().unwrap();
    assert_eq!(final_status[0].state, LocalPropagationState::Delivered);
    assert_eq!(final_status[0].encrypted_bytes, 0);
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chat_history_survives_rejected_local_admission() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("custody-history").await;
    let destination = node_b.get_status().lxmf_destination_hex;
    let storage = node_a
        .inner
        .lock()
        .unwrap()
        .active_config
        .as_ref()
        .unwrap()
        .storage_dir
        .clone()
        .unwrap();
    let conn =
        rusqlite::Connection::open(Path::new(&storage).join("local-propagation.db")).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER reject_admission BEFORE INSERT ON propagation_owned
        BEGIN SELECT RAISE(ABORT,'injected admission failure'); END;",
    )
    .unwrap();
    relay.shutdown().await;
    wait_no_interfaces(&node_a).await;
    let result = node_a.send_lxmf(SendLxmfRequest {
        destination_hex: destination,
        body_utf8: "Keep this attempted chat after a crash".to_owned(),
        title: Some("History regression".to_owned()),
        send_mode: SendMode::PropagationOnly {},
    });
    assert!(matches!(result, Err(NodeError::IoError {})));
    assert!(node_a.list_local_propagation().unwrap().is_empty());
    let persisted = node_a
        .inner
        .lock()
        .unwrap()
        .app_state
        .list_messages(None)
        .unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(
        persisted[0].body_utf8,
        "Keep this attempted chat after a crash"
    );
    assert_eq!(persisted[0].title.as_deref(), Some("History regression"));
    assert_eq!(persisted[0].state, MessageState::Failed {});
    assert_eq!(
        persisted[0].detail.as_deref(),
        Some("Local storage admission interrupted")
    );
    conn.execute_batch("DROP TRIGGER reject_admission").unwrap();
    drop(conn);
    stop_node(node_a).await;
    stop_node(node_b).await;
}

async fn wait_no_interfaces(node: &Node) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !node.get_status().interfaces.is_empty() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "fixture network must be offline"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn online_two_way_chat_never_writes_local_custody() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) =
        start_node_pair_with_profile("online-custody-isolation", true).await;
    let mut connections = Vec::new();
    for node in [&node_a, &node_b] {
        let storage = node
            .inner
            .lock()
            .unwrap()
            .active_config
            .as_ref()
            .unwrap()
            .storage_dir
            .clone()
            .unwrap();
        let db =
            rusqlite::Connection::open(Path::new(&storage).join("local-propagation.db")).unwrap();
        db.execute_batch("CREATE TRIGGER reject_online_custody BEFORE INSERT ON propagation_owned BEGIN SELECT RAISE(ABORT,'online traffic entered custody'); END;").unwrap();
        db.execute_batch("CREATE TRIGGER reject_recipient_cache BEFORE INSERT ON announce_identities BEGIN SELECT RAISE(ABORT,'cache unavailable'); END;").unwrap();
        connections.push(db);
    }
    node_a.announce_now().unwrap();
    node_b.announce_now().unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    for (sender, receiver, body) in [
        (&node_a, &node_b, "online a to b"),
        (&node_b, &node_a, "online b to a"),
    ] {
        let subscription = receiver.subscribe_events();
        let id = sender
            .send_lxmf(SendLxmfRequest {
                destination_hex: receiver.get_status().lxmf_destination_hex,
                body_utf8: body.to_owned(),
                title: None,
                send_mode: SendMode::Auto {},
            })
            .expect(
                "baseline online send succeeds even if custody storage rejects every admission",
            );
        wait_for_event(&subscription, TEST_TIMEOUT, |event| matches!(event,
            NodeEvent::MessageReceived { message } if message.body_utf8 == body && message.message_id_hex == id
        )).expect("recipient durably received the same wire");
        assert!(sender.list_local_propagation().unwrap().is_empty());
    }
    stop_node(node_a).await;
    stop_node(node_b).await;
    drop(connections);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unavailable_custody_storage_does_not_stop_the_normal_node() {
    let _guard = test_lock().lock().await;
    let storage = prepare_storage_dir("custody-unavailable-startup");
    std::fs::write(
        storage.join("local-propagation.db"),
        b"not a sqlite database",
    )
    .unwrap();
    let node = Node::with_storage_dir(storage.to_str()).unwrap();
    node.start(build_config("custody-unavailable", &storage, ""))
        .unwrap();
    assert!(matches!(
        node.list_local_propagation(),
        Err(NodeError::IoError {})
    ));
    assert!(node.get_status().running);
    node.announce_now()
        .expect("normal command lane remains usable");
    stop_node(node).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_interface_chat_is_stored_promptly_and_expires_in_power_saver() {
    use crate::types::LocalPropagationState;
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("offline-fast-admission").await;
    let destination = node_b.get_status().lxmf_destination_hex;
    let received = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: destination.clone(),
            body_utf8: "warm link".to_owned(),
            title: None,
            send_mode: SendMode::DirectOnly {},
        })
        .unwrap();
    wait_for_event(&received, TEST_TIMEOUT, |event| {
        matches!(event,
            NodeEvent::MessageReceived {message} if message.body_utf8 == "warm link"
        )
    })
    .expect("fixture has an established Link");
    relay.shutdown().await;
    wait_no_interfaces(&node_a).await;
    let started = Instant::now();
    let id = node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: destination,
            body_utf8: "offline custody expiry".to_owned(),
            title: None,
            send_mode: SendMode::Auto {},
        })
        .expect("validated offline recipient is stored");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "no futile baseline attempts"
    );
    let stored = node_a.list_local_propagation().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].message_id_hex, id);
    assert_eq!(stored[0].state, LocalPropagationState::StoredLocally);
    assert!(stored[0].encrypted_bytes > 0);
    let (storage, app_state) = {
        let inner = node_a.inner.lock().unwrap();
        inner.power_saver_tx.send_replace(true);
        (
            inner
                .active_config
                .as_ref()
                .unwrap()
                .storage_dir
                .clone()
                .unwrap(),
            inner.app_state.clone(),
        )
    };
    let events = node_a.subscribe_events();
    let conn =
        rusqlite::Connection::open(Path::new(&storage).join("local-propagation.db")).unwrap();
    conn.execute(
        "UPDATE propagation_owned SET expires_at=0 WHERE message_id=?1",
        [&id],
    )
    .unwrap();
    // Do not query custody: that would itself publish the pending expiry notice.
    wait_for_event(&events, Duration::from_secs(10), |event| matches!(event,
        NodeEvent::LocalPropagationUpdated {entry} if entry.message_id_hex == id && entry.state == LocalPropagationState::Expired
    )).expect("background maintenance publishes final expiry during saver");
    let message = app_state.message_for_wire(&id).unwrap().unwrap();
    assert_eq!(message.state, MessageState::Failed {});
    assert_eq!(message.detail.as_deref(), Some("Expired"));
    assert_eq!(
        conn.query_row(
            "SELECT encrypted_bytes FROM propagation_owned WHERE message_id=?1",
            [&id],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    stop_node(node_a).await;
    stop_node(node_b).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_chat_without_validated_recipient_returns_typed_error() {
    let _guard = test_lock().lock().await;
    let storage = prepare_storage_dir("offline-unresolved-recipient");
    let node = Node::with_storage_dir(storage.to_str()).unwrap();
    let mut config = build_config("offline-unresolved", &storage, "");
    config.tcp_clients.clear();
    node.start(config).unwrap();
    let result = node.send_lxmf(SendLxmfRequest {
        destination_hex: "ee".repeat(16),
        body_utf8: "cannot encrypt unknown recipient".to_owned(),
        title: None,
        send_mode: SendMode::Auto {},
    });
    assert!(
        matches!(result, Err(NodeError::UnresolvedRecipient {})),
        "{result:?}"
    );
    assert!(node.list_local_propagation().unwrap().is_empty());
    stop_node(node).await;
}

include!("local_propagation_restart.rs");
