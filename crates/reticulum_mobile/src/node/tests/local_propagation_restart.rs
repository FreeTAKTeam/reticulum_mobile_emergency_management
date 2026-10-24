#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn known_offline_chat_survives_restart_with_an_active_unrelated_interface() {
    use crate::types::LocalPropagationState;
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("offline-key-restart").await;
    let destination = node_b.get_status().lxmf_destination_hex;
    let (mut config_a, config_b) = (
        node_a.inner.lock().unwrap().active_config.clone().unwrap(),
        node_b.inner.lock().unwrap().active_config.clone().unwrap(),
    );
    let storage_a = config_a.storage_dir.clone().unwrap();
    let storage_b = config_b.storage_dir.clone().unwrap();
    let db =
        rusqlite::Connection::open(Path::new(&storage_a).join("local-propagation.db")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let learned: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM announce_identities WHERE peer=?1)",
                [&destination],
                |row| row.get(0),
            )
            .unwrap();
        if learned {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "verified announce key must be persisted"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
    let relay = TcpRelayHandle::start().await;
    config_a.tcp_clients = vec![relay.address()];
    let node_a = Node::with_storage_dir(Some(&storage_a)).unwrap();
    node_a.start(config_a.to_config()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while node_a.get_status().interfaces.is_empty() {
        assert!(
            Instant::now() < deadline,
            "unrelated interface must be active"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let started = Instant::now();
    let id = node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: destination.clone(),
            body_utf8: "hello world after restart".to_owned(),
            title: None,
            send_mode: SendMode::Auto {},
        })
        .expect("unreachable known recipient enters custody");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "no path-resolution timeout"
    );
    let stored = node_a.list_local_propagation().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].message_id_hex, id);
    assert_eq!(stored[0].state, LocalPropagationState::StoredLocally);
    assert!(stored[0].encrypted_bytes > 0);
    stop_node(node_a).await;
    let node_a = Node::with_storage_dir(Some(&storage_a)).unwrap();
    node_a.start(config_a.to_config()).unwrap();
    assert_eq!(
        node_a.list_local_propagation().unwrap()[0].message_id_hex,
        id
    );
    let node_b = Node::with_storage_dir(Some(&storage_b)).unwrap();
    let mut config_b = config_b;
    config_b.tcp_clients = vec![relay.address()];
    let events = node_b.subscribe_events();
    node_b.start(config_b.to_config()).unwrap();
    node_b.announce_now().unwrap();
    wait_for_event(&events, TEST_TIMEOUT, |event| matches!(event,
        NodeEvent::MessageReceived {message} if message.body_utf8 == "hello world after restart" && message.message_id_hex == id
    )).expect("recipient receives the same persisted message after reconnection");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let stored = node_a.list_local_propagation().unwrap();
        if stored[0].state == LocalPropagationState::Delivered {
            assert_eq!(stored[0].message_id_hex, id);
            assert_eq!(stored[0].encrypted_bytes, 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "proof must retire the local copy"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
