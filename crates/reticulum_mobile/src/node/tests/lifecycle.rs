#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_state_queries_and_writes_work_before_start() {
    let _guard = test_lock().lock().await;
    let _cwd = isolate_current_dir("prestart_app_state");
    let node = Node::new().expect("node storage");

    let settings = sample_app_settings();
    let peer = sample_saved_peer();
    let payload = LegacyImportPayload {
        settings: Some(settings.clone()),
        saved_peers: vec![peer.clone()],
        eams: vec![sample_eam()],
        events: vec![sample_event()],
        messages: vec![sample_message()],
        telemetry_positions: vec![sample_position()],
    };

    node.set_app_settings(settings.clone())
        .expect("set app settings before start");
    node.set_saved_peers(vec![peer.clone()])
        .expect("set saved peers before start");
    node.import_legacy_state(payload)
        .expect("import legacy state before start");

    let persisted_settings = node
        .get_app_settings()
        .expect("get app settings")
        .expect("settings present");
    assert_eq!(persisted_settings.display_name, settings.display_name);
    assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);

    let persisted_peers = node.get_saved_peers().expect("get saved peers");
    assert_eq!(persisted_peers.len(), 1);
    assert_eq!(persisted_peers[0].destination_hex, peer.destination_hex);
    assert_eq!(persisted_peers[0].label, peer.label);
    assert!(node
        .legacy_import_completed()
        .expect("legacy import status"));
    let eams = node.get_eams().expect("get eams");
    assert_eq!(eams.len(), 1);
    assert_eq!(eams[0].callsign, "ALPHA-1");
    assert_eq!(eams[0].team_uid.as_deref(), Some("team-1"));

    let events = node.get_events().expect("get events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].uid, "event-1");
    assert_eq!(events[0].mission_uid, "mission-1");

    let telemetry_positions = node
        .get_telemetry_positions()
        .expect("get telemetry positions");
    assert_eq!(telemetry_positions.len(), 1);
    assert_eq!(telemetry_positions[0].callsign, "ALPHA-1");

    let conversations = node.list_conversations().expect("list conversations");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].conversation_id, "dest-1");
    assert_eq!(conversations[0].peer_destination_hex, "dest-1");

    let messages = node
        .list_messages(Some("dest-1".to_string()))
        .expect("list messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id_hex, "msg-1");
    assert_eq!(messages[0].conversation_id, "dest-1");
    assert_eq!(messages[0].body_utf8, "hello from pre-start");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_only_commands_still_fail_before_start() {
    let _guard = test_lock().lock().await;
    let _cwd = isolate_current_dir("prestart_runtime_commands");
    let node = Node::new().expect("node storage");

    assert!(matches!(
        node.connect_peer("ABCDEF".to_string()),
        Err(NodeError::NotRunning {})
    ));
    assert!(matches!(
        node.send_lxmf(SendLxmfRequest {
            destination_hex: "ABCDEF".to_string(),
            body_utf8: "hello".to_string(),
            title: Some("test".to_string()),
            send_mode: SendMode::Auto {},
        }),
        Err(NodeError::NotRunning {})
    ));
}

#[test]
fn lifecycle_and_cancel_commands_use_reserved_priority_capacity() {
    let priority_capacity = std::hint::black_box(PRIORITY_COMMAND_QUEUE_CAPACITY);
    assert!(priority_capacity >= 1_000);
    let storage_dir = prepare_storage_dir("priority-lifecycle-commands");
    let storage_dir_text = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(storage_dir_text.as_str())).expect("node storage");
    let (normal_tx, mut normal_rx) = mpsc::channel(4);
    let (priority_tx, mut priority_rx) = mpsc::channel(4);
    {
        let mut inner = node.inner.lock().expect("node lock");
        inner.cmd_tx = Some(normal_tx);
        inner.priority_cmd_tx = Some(priority_tx);
    }

    node.announce_now().expect("announce dispatch");
    assert!(matches!(
        priority_rx.blocking_recv(),
        Some(Command::AnnounceNow {})
    ));

    std::thread::scope(|scope| {
        let call = scope.spawn(|| {
            node.connect_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        });
        match priority_rx.blocking_recv() {
            Some(Command::ConnectPeer { resp, .. }) => {
                resp.send(Ok(())).expect("connect response");
            }
            _ => panic!("expected priority connect command"),
        }
        assert!(call.join().expect("connect call should join").is_ok());

        let call = scope.spawn(|| {
            node.disconnect_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        });
        match priority_rx.blocking_recv() {
            Some(Command::DisconnectPeer { resp, .. }) => {
                resp.send(Ok(())).expect("disconnect response");
            }
            _ => panic!("expected priority disconnect command"),
        }
        assert!(call.join().expect("disconnect call should join").is_ok());

        let call = scope.spawn(|| node.cancel_lxmf("message-id".to_string()));
        match priority_rx.blocking_recv() {
            Some(Command::CancelLxmf { resp, .. }) => {
                resp.send(Ok(())).expect("cancel response");
            }
            _ => panic!("expected priority cancel command"),
        }
        assert!(call.join().expect("cancel call should join").is_ok());
    });

    assert!(normal_rx.try_recv().is_err());
}

#[test]
fn start_is_idempotent_for_equivalent_config() {
    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let _guard = rt.block_on(test_lock().lock());
    let _cwd = isolate_current_dir("start_idempotent");
    let relay = rt.block_on(TcpRelayHandle::start());
    let storage_dir = prepare_storage_dir("start_idempotent");
    let config = build_config(
        "start-idempotent",
        storage_dir.as_path(),
        relay.address().as_str(),
    );
    let node = Node::new().expect("node storage");

    node.start(config.clone()).expect("initial start");
    node.start(config.clone())
        .expect("repeat start with same config");
    wait_until_running(&node);
    node.announce_now()
        .expect("runtime command stays available after idempotent start");

    let status = node.get_status();
    assert!(status.running);
    assert_eq!(status.name, config.name);

    node.stop().expect("stop idempotent node");
    rt.block_on(relay.shutdown());
}

#[test]
fn start_restarts_when_config_changes_while_running() {
    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let _guard = rt.block_on(test_lock().lock());
    let _cwd = isolate_current_dir("start_restart_changed");
    let relay = rt.block_on(TcpRelayHandle::start());
    let storage_dir = prepare_storage_dir("start_restart_changed");
    let node = Node::new().expect("node storage");
    let config = build_config(
        "start-restart",
        storage_dir.as_path(),
        relay.address().as_str(),
    );
    let mut changed_config = config.clone();
    changed_config.name = "start-restart-updated".to_string();
    changed_config.announce_interval_seconds = 2;

    node.start(config).expect("initial start");
    node.start(changed_config.clone())
        .expect("start with changed config while running");
    wait_until_running(&node);
    node.announce_now()
        .expect("runtime command stays available after config restart");

    let status = node.get_status();
    assert!(status.running);
    assert_eq!(status.name, changed_config.name);

    node.stop().expect("stop restarted node");
    rt.block_on(relay.shutdown());
}

#[test]
fn restart_while_running_keeps_runtime_available() {
    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let _guard = rt.block_on(test_lock().lock());
    let _cwd = isolate_current_dir("restart_running");
    let relay = rt.block_on(TcpRelayHandle::start());
    let storage_dir = prepare_storage_dir("restart_running");
    let config = build_config(
        "restart-running",
        storage_dir.as_path(),
        relay.address().as_str(),
    );
    let node = Node::new().expect("node storage");

    node.start(config.clone()).expect("initial start");
    node.restart(config).expect("restart while running");
    wait_until_running(&node);
    node.announce_now()
        .expect("runtime command stays available after restart");

    let status = node.get_status();
    assert!(status.running);

    node.stop().expect("stop restarted node");
    rt.block_on(relay.shutdown());
}

#[test]
fn stop_clears_running_state_and_commands_fail_after_stop() {
    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let _guard = rt.block_on(test_lock().lock());
    let _cwd = isolate_current_dir("stop_after_idle");
    let relay = rt.block_on(TcpRelayHandle::start());
    let storage_dir = prepare_storage_dir("stop_after_idle");
    let config = build_config(
        "stop-after-idle",
        storage_dir.as_path(),
        relay.address().as_str(),
    );
    let node = Node::new().expect("node storage");

    node.start(config).expect("initial start");
    wait_until_running(&node);
    std::thread::sleep(Duration::from_millis(100));

    node.stop().expect("first stop succeeds");
    node.stop().expect("second stop remains idempotent");

    let status = node.get_status();
    assert!(!status.running);
    assert!(matches!(node.announce_now(), Err(NodeError::NotRunning {})));
    assert!(matches!(
        node.request_peer_identity("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        Err(NodeError::NotRunning {})
    ));

    rt.block_on(relay.shutdown());
}

#[test]
fn pre_start_app_state_queries_use_initialized_storage() {
    let storage_dir = prepare_storage_dir("pre_start_app_state");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");

    let settings = build_app_settings();
    let saved_peer = build_saved_peer();
    let eam = build_eam();
    let event = build_event();
    let message = build_message();
    let telemetry = build_telemetry();

    assert!(!node
        .legacy_import_completed()
        .expect("legacy import completed before import"));

    node.import_legacy_state(LegacyImportPayload {
        settings: Some(settings.clone()),
        saved_peers: vec![saved_peer.clone()],
        eams: vec![eam.clone()],
        events: vec![event.clone()],
        messages: vec![message.clone()],
        telemetry_positions: vec![telemetry.clone()],
    })
    .expect("import legacy state");

    assert!(node
        .legacy_import_completed()
        .expect("legacy import completed after import"));
    let persisted_settings = node
        .get_app_settings()
        .expect("app settings")
        .expect("settings present");
    assert_eq!(persisted_settings.display_name, settings.display_name);
    assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);

    let persisted_saved_peers = node.get_saved_peers().expect("saved peers");
    assert_eq!(persisted_saved_peers.len(), 1);
    assert_eq!(
        persisted_saved_peers[0].destination_hex,
        saved_peer.destination_hex
    );
    assert_eq!(persisted_saved_peers[0].label, saved_peer.label);

    let persisted_eams = node.get_eams().expect("eams");
    assert_eq!(persisted_eams.len(), 1);
    assert_eq!(persisted_eams[0].callsign, eam.callsign);
    assert_eq!(persisted_eams[0].team_uid, eam.team_uid);

    let persisted_events = node.get_events().expect("events");
    assert_eq!(persisted_events.len(), 1);
    assert_eq!(persisted_events[0].uid, event.uid);
    assert_eq!(persisted_events[0].mission_uid, event.mission_uid);

    let persisted_messages = node.list_messages(None).expect("messages");
    assert_eq!(persisted_messages.len(), 1);
    assert_eq!(persisted_messages[0].message_id_hex, message.message_id_hex);
    assert_eq!(
        persisted_messages[0].conversation_id,
        message.destination_hex.to_ascii_lowercase()
    );

    let conversations = node.list_conversations().expect("conversations");
    assert_eq!(conversations.len(), 1);
    assert_eq!(
        conversations[0].conversation_id,
        message.destination_hex.to_ascii_lowercase()
    );
    let persisted_telemetry = node.get_telemetry_positions().expect("telemetry");
    assert_eq!(persisted_telemetry.len(), 1);
    assert_eq!(persisted_telemetry[0].callsign, telemetry.callsign);
}

#[test]
fn start_reuses_pre_initialized_storage_directory() {
    let storage_dir = prepare_storage_dir("pre_start_storage_reuse");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");
    let settings = build_app_settings();

    node.set_app_settings(settings.clone())
        .expect("persist settings before start");
    node.initialize_storage(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("reinitialize same storage dir");

    let persisted_settings = node
        .get_app_settings()
        .expect("settings after reinitialize")
        .expect("settings present after reinitialize");
    assert_eq!(persisted_settings.display_name, settings.display_name);
    assert_eq!(persisted_settings.tcp_clients, settings.tcp_clients);
}

#[test]
fn runtime_commands_still_fail_before_start() {
    let storage_dir = prepare_storage_dir("runtime_not_running");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");

    assert!(matches!(
        node.connect_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        Err(NodeError::NotRunning {})
    ));
    assert!(matches!(
        node.request_peer_identity("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        Err(NodeError::NotRunning {})
    ));
    assert!(matches!(node.announce_now(), Err(NodeError::NotRunning {})));
    assert!(matches!(
        node.send_lxmf(SendLxmfRequest {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            body_utf8: "hello".to_string(),
            title: None,
            send_mode: SendMode::Auto {},
        }),
        Err(NodeError::NotRunning {})
    ));
}
