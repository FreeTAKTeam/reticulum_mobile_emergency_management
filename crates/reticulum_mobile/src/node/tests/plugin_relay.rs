fn enable_test_communications_relay(node: &Node) -> (String, u64) {
    let id = "org.example.relay".to_string();
    let plugin = serde_json::from_value(json!({
        "pluginId":id,"displayName":"Relay test","version":"0.1.0","apiMajor":1,"apiMinor":5,
        "packageName":"org.example.relay","serviceClassName":".RelayService",
        "publisherFingerprint":"ab".repeat(32),"declaredCapabilities":{"communicationsRelay":true,"eventsPublish":true,"lxmfSend":true}
    }))
    .unwrap();
    node.sync_discovered_plugins(vec![plugin]).unwrap();
    node.approve_plugin_publisher(&id, None).unwrap();
    node.grant_plugin_capabilities(
        &id,
        PluginCapabilityRecord {
            communications_relay: true,
            events_publish: true,
            lxmf_send: true,
            ..Default::default()
        },
    )
    .unwrap();
    node.set_plugin_enabled(&id, true).unwrap();
    let cursor = node.poll_plugin_relay(&id, None, 16).unwrap()["cursor"]
        .as_u64()
        .unwrap();
    (id, cursor)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_built_telemetry_replication_payload_is_persisted_by_receiver() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry_payload_projection").await;

    let node_b_status = node_b.get_status();
    let (relay_plugin, relay_cursor) = enable_test_communications_relay(&node_b);
    let position = TelemetryPositionRecord {
        callsign: "PixelManualMonitor".to_string(),
        lat: 43.9674,
        lon: -66.1261,
        alt: Some(10.0),
        course: Some(0.0),
        speed: Some(0.0),
        accuracy: Some(5.0),
        updated_at_ms: now_ms(),
    };
    let target = MissionReplicationTarget {
        app_destination_hex: node_b_status.app_destination_hex.clone(),
        send_mode: SendMode::Auto {},
    };
    let (body, fields) =
        build_telemetry_replication_payload(&position, &target).expect("telemetry payload");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("telemetry metadata");
    let command_id = metadata.command_id.clone().expect("telemetry command id");
    let command_type = metadata
        .command_type
        .clone()
        .expect("telemetry command type");
    let ack_subscription = node_a.subscribe_events();

    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            body,
            Some(fields),
            SendMode::Auto {},
        )
        .expect("send telemetry replication payload");

    let received_deadline = Instant::now() + TEST_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_telemetry_positions()
            .expect("get telemetry")
            .into_iter()
            .find(|entry| entry.callsign == position.callsign);
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted direct telemetry replication payload"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(received.callsign, position.callsign);
    assert_eq!(received.lat, position.lat);
    assert_eq!(received.lon, position.lon);
    assert_eq!(received.accuracy, position.accuracy);
    let ack = wait_for_operational_ack(&ack_subscription, &command_id, &command_type);
    assert_eq!(
        ack.source_hex.as_deref(),
        Some(node_b_status.lxmf_destination_hex.as_str())
    );
    assert_eq!(ack.destination_hex, node_b_status.lxmf_destination_hex);

    let forwarded = node_b
        .poll_plugin_relay(&relay_plugin, Some(relay_cursor), 16)
        .unwrap();
    assert!(forwarded["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["kind"] == "position"
            && item["payload"]["lat"] == position.lat
            && item["source"] == node_a.get_status().lxmf_destination_hex));
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plugin_relay_addressed_chat_uses_normal_runtime_history_and_scoped_status() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("relay-addressed-chat").await;
    let (plugin, _) = enable_test_communications_relay(&node_b);
    let source = node_a.get_status().lxmf_destination_hex;
    let local = node_b.get_status().lxmf_destination_hex;
    let input = RelayChatInput {
        id: "a".repeat(32),
        source_hex: source.clone(),
        destination_hex: local.clone(),
        text: "Addressed Mesh chat".to_string(),
        radio_gid: "001122334455".to_string(),
    };
    node_b
        .accept_relay_chat(&plugin, input.clone(), false, false)
        .unwrap();
    node_b
        .accept_relay_chat(&plugin, input.clone(), false, false)
        .unwrap();
    let messages = node_b.list_messages(Some(source.clone())).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].body_utf8, "Addressed Mesh chat");
    assert_eq!(messages[0].destination_hex, local);
    assert_eq!(
        messages[0].detail.as_deref(),
        Some("external-relay:gotenna:001122334455:unverified")
    );
    let mut invalid = input.clone();
    invalid.destination_hex.clear();
    assert!(node_b
        .accept_relay_chat(&plugin, invalid, false, false)
        .is_err());
    let sent = RelayChatInput {
        id: "b".repeat(32),
        source_hex: "f".repeat(32),
        destination_hex: source.clone(),
        ..input.clone()
    };
    node_b
        .accept_relay_chat(&plugin, sent.clone(), true, false)
        .unwrap();
    assert!(node_b
        .list_messages(Some(source.clone()))
        .unwrap()
        .iter()
        .any(|message| message
            .detail
            .as_deref()
            .is_some_and(|detail| detail.ends_with(":radio-attempt-unconfirmed"))));
    node_b.accept_relay_chat(&plugin, sent, true, true).unwrap();
    let messages = node_b.list_messages(Some(source)).unwrap();
    assert_eq!(messages.len(), 2);
    let outbound = messages
        .iter()
        .find(|item| matches!(item.direction, MessageDirection::Outbound {}))
        .unwrap();
    assert_eq!(outbound.source_hex.as_deref(), Some(local.as_str()));
    assert!(outbound
        .detail
        .as_deref()
        .unwrap()
        .ends_with("radio-accepted-unconfirmed"));
    node_b
        .grant_plugin_capabilities(&plugin, Default::default())
        .unwrap();
    assert!(node_b
        .accept_relay_chat(&plugin, input, false, false)
        .is_err());
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plugin_relay_events_use_mecp_codes_and_do_not_echo_imports() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("relay-event-code").await;
    let (plugin, cursor) = enable_test_communications_relay(&node_b);
    let mut event = build_event();
    event.source_identity = node_b.get_status().identity_hex;
    event.content = "MECP/2/P01 #A1 detail".to_string();
    node_b.upsert_event(event).unwrap();
    let page = node_b.poll_plugin_relay(&plugin, Some(cursor), 16).unwrap();
    assert!(page["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["kind"] == "event" && item["payload"]["code"] == "MECP/2/P01"));
    let head = page["cursor"].as_u64().unwrap();
    let mut remote = build_event();
    remote.uid = "remote-relay-event".to_string();
    remote.source_identity = node_a.get_status().identity_hex;
    remote.content = "MECP/0/B01".to_string();
    let target = MissionReplicationTarget {
        app_destination_hex: node_b.get_status().app_destination_hex,
        send_mode: SendMode::Auto {},
    };
    let (body, fields) =
        build_event_replication_payload(&node_a.get_status(), &remote, &target).unwrap();
    node_a
        .send_bytes(
            target.app_destination_hex,
            body,
            Some(fields),
            SendMode::Auto {},
        )
        .unwrap();
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let page = node_b.poll_plugin_relay(&plugin, Some(head), 16).unwrap();
        if page["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "event" && item["payload"]["code"] == "MECP/0/B01")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Remote event code never reached the Mesh feed"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let head = node_b.poll_plugin_relay(&plugin, None, 16).unwrap()["cursor"]
        .as_u64()
        .unwrap();
    let input = RelayChatInput {
        id: "c".repeat(32),
        source_hex: "d".repeat(32),
        destination_hex: "-".to_string(),
        text: "MECP/1/W02".to_string(),
        radio_gid: "001122334455".to_string(),
    };
    node_b.accept_relay_event(&plugin, input.clone()).unwrap();
    node_b.accept_relay_event(&plugin, input.clone()).unwrap();
    let another_source = RelayChatInput {
        source_hex: "e".repeat(32),
        ..input
    };
    node_b.accept_relay_event(&plugin, another_source).unwrap();
    let events = node_b.get_events().unwrap();
    let imported = events
        .iter()
        .filter(|event| event.uid.starts_with("gotenna-"))
        .collect::<Vec<_>>();
    assert_eq!(imported.len(), 2);
    assert_eq!(imported[0].content, "MECP/1/W02");
    assert_eq!(
        imported[0].source_identity,
        node_b.get_status().identity_hex
    );
    assert!(
        node_b.poll_plugin_relay(&plugin, Some(head), 16).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plugin_relay_nonlocal_chat_uses_carried_recipient_and_never_confirms_an_unknown_attempt() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("relay-forward-chat").await;
    let (plugin, _) = enable_test_communications_relay(&node_b);
    let input = RelayChatInput {
        id: "f".repeat(32),
        source_hex: "c".repeat(32),
        destination_hex: node_a.get_status().lxmf_destination_hex,
        text: "Forward to this person".to_string(),
        radio_gid: "001122334455".to_string(),
    };
    let unknown = RelayChatInput {
        id: "e".repeat(32),
        ..input.clone()
    };
    let unknown_id = relay_chat_id(&plugin, &unknown);
    node_b
        .with_app_state(|store, _| store.claim_relay_delivery(&plugin, &unknown_id))
        .unwrap();
    assert!(matches!(
        node_b.accept_relay_chat(&plugin, unknown, false, false),
        Err(NodeError::Timeout {})
    ));
    let subscription = node_a.subscribe_events();
    let accepted = node_b
        .accept_relay_chat(&plugin, input.clone(), false, false)
        .unwrap();
    assert!(accepted["forwardedMessageId"].is_string());
    wait_for_event(&subscription, TEST_TIMEOUT, |event| matches!(event, NodeEvent::MessageReceived {message} if message.body_utf8.contains("Forward to this person"))).unwrap();
    let duplicate = node_b
        .accept_relay_chat(&plugin, input, false, false)
        .unwrap();
    assert_eq!(
        duplicate["forwardedMessageId"],
        accepted["forwardedMessageId"]
    );
    let received = node_a.list_messages(None).unwrap();
    let forwarded = received
        .iter()
        .filter(|item| item.body_utf8.contains("Forward to this person"))
        .collect::<Vec<_>>();
    assert_eq!(forwarded.len(), 1);
    assert_eq!(
        forwarded[0].source_hex.as_deref(),
        Some(node_b.get_status().lxmf_destination_hex.as_str())
    );
    assert!(forwarded[0].body_utf8.starts_with("[goTenna:"));
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
