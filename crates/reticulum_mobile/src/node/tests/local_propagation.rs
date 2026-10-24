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
        build_mission_replication_targets(&status, &[], std::slice::from_ref(&saved), None),
        build_event_replication_targets(&status, &[], std::slice::from_ref(&saved), None),
        build_mission_replication_targets(
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
    assert!(build_mission_replication_targets(
        &status,
        std::slice::from_ref(&peer),
        std::slice::from_ref(&saved),
        None
    )
    .is_empty());
    assert!(build_event_replication_targets(&status, &[peer], &[saved], None).is_empty());
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
    relay.shutdown().await;
}
