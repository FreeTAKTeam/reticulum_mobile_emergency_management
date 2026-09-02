#[test]
fn sos_send_failure_updates_persisted_message_for_retry() {
    let storage_dir = prepare_storage_dir("sos-send-failure");
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

    node.trigger_sos(SosTriggerSource::Manual {})
        .expect("trigger sos");
    let command = rx.try_recv().expect("expected sos send command");
    let Command::SendBytes { resp, .. } = command else {
        panic!("expected SendBytes command");
    };
    resp.send(Err(NodeError::NetworkError {}))
        .expect("fail sos send");

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let messages = node
            .inner
            .lock()
            .expect("node lock")
            .app_state
            .list_messages(None)
            .expect("messages");
        if messages.iter().any(|message| {
            matches!(message.traffic_class, OutboundTrafficClass::Sos {})
                && matches!(message.state, MessageState::Failed {})
                && matches!(message.transport_state, TransportDeliveryState::Failed {})
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "SOS failure was not persisted"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
