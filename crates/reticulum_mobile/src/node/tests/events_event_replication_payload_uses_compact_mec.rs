#[test]
fn event_replication_payload_uses_compact_mecp_envelope_with_compatible_metadata() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Pixel".to_string(),
        identity_hex: "11111111111111111111111111111111".to_string(),
        app_destination_hex: "22222222222222222222222222222222".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let record = EventProjectionRecord {
        uid: "evt-a9f9c462-c439-425a-879d-6d13f13a3b86".to_string(),
        command_id:
            "log-entry-evt-a9f9c462-c439-425a-879d-6d13f13a3b86-a25c9832-4ff1-4d6d-bd70-290b7add090c"
                .to_string(),
        source_identity: "e6fcf8f02e290ed46f88a729460dfffc".to_string(),
        source_display_name: Some("Pixel".to_string()),
        timestamp: "2026-05-16T21:26:30.243Z".to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: "r3akt-default-mission".to_string(),
        content: "MECP/2/P01 stranded near bridge".to_string(),
        callsign: "Pixel".to_string(),
        server_time: Some("2026-05-16T21:26:30.243Z".to_string()),
        client_time: Some("2026-05-16T21:26:30.243Z".to_string()),
        keywords: vec!["r3akt:event-type:P".to_string()],
        content_hashes: vec!["hash-1".to_string()],
        updated_at_ms: 1_778_966_790_243,
        deleted_at_ms: None,
        correlation_id: Some("event-upsert-legacy".to_string()),
        topics: vec!["r3akt-default-mission".to_string(), "Default".to_string()],
    };
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };

    let (body, fields) =
        build_event_replication_payload(&status, &record, &target).expect("event payload");

    assert_eq!(body.as_slice(), b"P01 stranded near bridge");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.log_entry.upsert")
    );
    assert_eq!(
        metadata.command_id.as_deref(),
        Some("log-entry-evt-a9f9c462-c439-425a-879d-6d13f13a3b86")
    );
    assert_eq!(metadata.event_uid.as_deref(), Some(record.uid.as_str()));
    assert_eq!(
        metadata.mission_uid.as_deref(),
        Some(record.mission_uid.as_str())
    );
    let packed_fields =
        rmp_serde::from_slice::<MsgPackValue>(fields.as_slice()).expect("fields msgpack");
    let MsgPackValue::Map(field_entries) = packed_fields else {
        panic!("fields should be a map");
    };
    let commands = field_entries
        .iter()
        .find(|(key, _)| key.as_i64() == Some(FIELD_COMMANDS))
        .and_then(|(_, value)| value.as_array())
        .expect("command array");
    let command = commands[0].as_map().expect("command map");
    let args = command
        .iter()
        .find(|(key, _)| key.as_str() == Some("a"))
        .and_then(|(_, value)| value.as_map())
        .expect("command args");
    assert!(
        args.iter().all(|(key, _)| key.as_str() != Some("kw")),
        "RNode compact event fields should omit keyword adornments"
    );
    assert!(
        args.iter().all(|(key, _)| key.as_str() != Some("cs")),
        "RNode compact event fields should rely on LXMF source display fallback"
    );
    assert!(
        command.iter().all(|(key, _)| key.as_str() != Some("t")),
        "RNode compact event fields should infer the log-entry command type"
    );
    assert!(
        !fields
            .windows("r3akt:event-type:P".len())
            .any(|window| window == "r3akt:event-type:P".as_bytes()),
        "compact event fields should not carry verbose event keyword"
    );
    assert!(
        !fields
            .windows("hash-1".len())
            .any(|window| window == "hash-1".as_bytes()),
        "RNode compact event fields should omit content hashes"
    );
    assert!(
        !fields
            .windows("Pixel".len())
            .any(|window| window == "Pixel".as_bytes()),
        "compact event fields should omit display name and use peer projection fallback"
    );
}

#[test]
fn event_replication_payload_stays_under_direct_and_propagation_packet_budgets() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod, WireMessage as LxmfWireMessage,
    };
    use rand_core::OsRng;
    use reticulum::transport::destination::{DestinationName, SingleOutputDestination};
    use reticulum::transport::identity::PrivateIdentity;

    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Pixelcorvo".to_string(),
        identity_hex: "e6fcf8f02e290ed46f88a729460dfffc".to_string(),
        app_destination_hex: "fb4c70e20cfac047b899ca2f3671b50a".to_string(),
        lxmf_destination_hex: "fb4c70e20cfac047b899ca2f3671b50a".to_string(),
        interfaces: Vec::new(),
    };
    let receiver = PrivateIdentity::new_from_name("event-replication-propagation-budget-peer");
    let receiver_lxmf = SingleOutputDestination::new(
        *receiver.as_identity(),
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    );
    let target = MissionReplicationTarget {
        app_destination_hex: receiver_lxmf.desc.address_hash.to_hex_string(),
        send_mode: SendMode::Auto {},
    };
    let record = EventProjectionRecord {
        uid: "evt-a3a1c275-1d90-41a6-b7ed-f8f9cf0b8fc1".to_string(),
        command_id: "log-entry-evt-a3a1c275-1d90-41a6-b7ed-f8f9cf0b8fc1-d5898636-72e6-4af5-b94b-120ffaed1e49".to_string(),
        source_identity: status.identity_hex.clone(),
        source_display_name: Some(status.name.clone()),
        timestamp: "2026-05-25T20:57:42.166000000Z".to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: "r3akt-default-mission".to_string(),
        content: "MECP/2/P01".to_string(),
        callsign: status.name.clone(),
        server_time: Some("2026-05-25T20:57:42.166000000Z".to_string()),
        client_time: Some("2026-05-25T20:57:42.166000000Z".to_string()),
        keywords: vec!["r3akt:event-type:P".to_string()],
        content_hashes: vec![],
        updated_at_ms: 1_779_753_342_166,
        deleted_at_ms: None,
        correlation_id: Some(
            "event-upsert-evt-a3a1c275-1d90-41a6-b7ed-f8f9cf0b8fc1-e3f1c3f6-1779753342166"
                .to_string(),
        ),
        topics: vec!["r3akt-default-mission".to_string(), "Default".to_string()],
    };
    let (body, fields) =
        build_event_replication_payload(&status, &record, &target).expect("event payload");
    let source = hex::decode(status.lxmf_destination_hex.as_str()).expect("source hex");
    let target_hex = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(target_hex.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(body.as_slice());
    message.fields = Some(rmp_serde::from_slice(fields.as_slice()).expect("fields msgpack"));
    let identity = PrivateIdentity::new_from_name("event-replication-budget");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");

    assert!(fields.len() <= 56, "fields bytes={}", fields.len());
    assert!(
        wire.len() <= 145,
        "RNode direct event wire bytes={} budget=145",
        wire.len()
    );
    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(decision.representation, LxmfMessageMethod::Packet);
    let propagation_stamp = [0u8; 32];
    let (propagated, _) = LxmfWireMessage::unpack(wire.as_slice())
        .expect("wire message")
        .pack_propagation_with_options_and_rng(
            &lxmf::identity::Identity::new_from_slices(
                receiver.as_identity().public_key_bytes(),
                receiver.as_identity().verifying_key_bytes(),
            ),
            1_779_753_342.166,
            Some(propagation_stamp.as_slice()),
            OsRng,
        )
        .expect("propagation envelope");
    assert!(
        propagated.len() <= 360,
        "propagated event bytes={} relay_plaintext_budget=360 lxmf_max={}",
        propagated.len(),
        reticulum::transport::packet::LXMF_MAX_PAYLOAD
    );
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("event metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.log_entry.upsert")
    );
    assert_eq!(
        metadata.event_uid.as_deref(),
        Some("evt-a3a1c275-1d90-41a6-b7ed-f8f9cf0b8fc1")
    );
    assert_eq!(
        metadata.mission_uid.as_deref(),
        Some(DEFAULT_R3AKT_MISSION_UID)
    );
    assert_eq!(
        metadata.command_id.as_deref(),
        Some("log-entry-evt-a3a1c275-1d90-41a6-b7ed-f8f9cf0b8fc1")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_event_is_received_as_mission_packet() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("event").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let body = "event: checkpoint reached";
    let fields = mission_event_fields(
        "mission.registry.log_entry.upserted",
        "event-123",
        vec![
            ("entry_uid", MsgPackValue::from("event-123")),
            ("mission_uid", MsgPackValue::from("mission-1")),
            ("content", MsgPackValue::from("Checkpoint reached")),
        ],
    );
    let subscription = node_b.subscribe_events();
    node_a
        .send_bytes(
            node_b_status.lxmf_destination_hex.clone(),
            body.as_bytes().to_vec(),
            Some(fields.clone()),
            SendMode::Auto {},
        )
        .expect("send event packet");

    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
    })
    .expect("node b received event packet");

    assert_packet_received(
        event,
        &node_a_status.lxmf_destination_hex,
        body,
        Some(fields.as_slice()),
    );
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("event metadata");
    assert_eq!(
        metadata.event_type.as_deref(),
        Some("mission.registry.log_entry.upserted")
    );
    assert_eq!(metadata.event_uid.as_deref(), Some("event-123"));

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_built_event_replication_payload_acknowledges_after_persistence() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("event_payload_ack").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let mut record = build_event();
    record.uid = "evt-operational-ack".to_string();
    record.command_id = "cmd-event-operational-ack".to_string();
    record.correlation_id = Some("corr-event-operational-ack".to_string());
    record.source_identity = node_a_status.identity_hex.clone();
    record.source_display_name = Some(node_a_status.name.clone());
    record.callsign = node_a_status.name.clone();
    record.content = "Operational ACK event".to_string();
    let target = MissionReplicationTarget {
        app_destination_hex: node_b_status.app_destination_hex.clone(),
        send_mode: SendMode::Auto {},
    };
    let (body, fields) = build_event_replication_payload(&node_a_status, &record, &target)
        .expect("event payload");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("event metadata");
    let command_id = metadata.command_id.clone().expect("event command id");
    let command_type = metadata.command_type.clone().expect("event command type");
    let ack_subscription = node_a.subscribe_events();

    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            body,
            Some(fields),
            SendMode::Auto {},
        )
        .expect("send event replication payload");

    let received_deadline = Instant::now() + TEST_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_events()
            .expect("get events")
            .into_iter()
            .find(|event| event.uid == record.uid);
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted direct event replication payload"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(received.content, record.content);
    assert_eq!(received.command_id, "log-entry-evt-operational-ack");
    let ack = wait_for_operational_ack(&ack_subscription, &command_id, &command_type);
    assert_eq!(
        ack.source_hex.as_deref(),
        Some(node_b_status.lxmf_destination_hex.as_str())
    );
    assert_eq!(ack.destination_hex, node_b_status.lxmf_destination_hex);

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn targeted_upsert_event_replicates_to_native_peer_projection() {
    const EVENT_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("event_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: node_b_status.app_destination_hex.clone(),
            label: Some("peer-b".to_string()),
            saved_at_ms: now_ms(),
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
            circle_tier: CircleTier::Inner {},
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm event link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm event link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm event link")
    })
    .expect("node b received warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.saved && has_known_lxmf_route(&peer));
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became mission-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let replication_targets = build_event_replication_targets(
        &node_a.get_status(),
        node_a.list_peers().expect("list peers").as_slice(),
        node_a.get_saved_peers().expect("saved peers").as_slice(),
        node_a
            .get_lxmf_sync_status()
            .expect("sync status")
            .active_propagation_node_hex
            .as_deref(),
    );
    assert_eq!(
        replication_targets.len(),
        1,
        "expected one event replication target"
    );
    assert_eq!(
        replication_targets[0].app_destination_hex,
        node_b_status.app_destination_hex
    );

    let record = EventProjectionRecord {
        uid: "evt-upsert-native".to_string(),
        command_id: "cmd-evt-upsert-native".to_string(),
        source_identity: node_a_status.identity_hex.clone(),
        source_display_name: Some(node_a_status.name.clone()),
        timestamp: "2026-03-25T16:50:00Z".to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: "r3akt-default-mission".to_string(),
        content: "MECP/2/H01 Bolle".to_string(),
        callsign: node_a_status.name.clone(),
        server_time: Some("2026-03-25T16:50:00Z".to_string()),
        client_time: Some("2026-03-25T16:50:00Z".to_string()),
        keywords: vec!["r3akt:event-type:Incident".to_string()],
        content_hashes: vec![],
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        correlation_id: Some("corr-evt-upsert-native".to_string()),
        topics: vec!["r3akt-default-mission".to_string(), "Default".to_string()],
    };

    node_a
        .upsert_event_to_destination(
            record.clone(),
            node_b_status.app_destination_hex.clone(),
        )
        .expect("upsert local event for the selected peer");

    let received_deadline = Instant::now() + EVENT_REPLICATION_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_events()
            .expect("get events")
            .into_iter()
            .find(|event| event.uid == record.uid);
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted replicated event"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(received.uid, record.uid);
    assert_eq!(received.command_type, "mission.registry.log_entry.upsert");
    assert_eq!(received.mission_uid, record.mission_uid);
    assert_eq!(received.content, record.content);
    assert_eq!(received.callsign, node_a_status.name);
    assert_eq!(received.source_identity, node_a_status.lxmf_destination_hex);

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
