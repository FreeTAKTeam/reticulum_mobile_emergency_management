#[test]
fn telemetry_replication_payload_stays_under_direct_packet_budget() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod,
    };
    use reticulum::transport::identity::PrivateIdentity;

    let target = MissionReplicationTarget {
        app_destination_hex: "e3f1c3f63adef0c0d771ddff8b0eeed5".to_string(),
        send_mode: SendMode::Auto {},
    };
    let position = TelemetryPositionRecord {
        callsign: "NoemiPix".to_string(),
        lat: 44.6488,
        lon: -63.5752,
        alt: Some(12.0),
        course: Some(45.0),
        speed: Some(3.5),
        accuracy: Some(5.0),
        updated_at_ms: 1_779_756_701_317,
    };
    let (body, fields) =
        build_telemetry_replication_payload(&position, &target).expect("telemetry payload");
    let source = hex::decode("fb4c70e20cfac047b899ca2f3671b50a").expect("source hex");
    let target_hex = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(target_hex.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(body.as_slice());
    message.fields = Some(rmp_serde::from_slice(fields.as_slice()).expect("fields msgpack"));
    let identity = PrivateIdentity::new_from_name("telemetry-replication-budget");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");

    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(decision.representation, LxmfMessageMethod::Packet);
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("telemetry metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.telemetry.upsert")
    );
}

fn build_message() -> MessageRecord {
    MessageRecord {
        message_id_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        conversation_id: "conversation-1".to_string(),
        direction: MessageDirection::Outbound {},
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        source_hex: Some("cccccccccccccccccccccccccccccccc".to_string()),
        requested_destination_hex: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        delivery_destination_hex: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        title: Some("check-in".to_string()),
        body_utf8: "Hello world".to_string(),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Queued {},
        transport_state: TransportDeliveryState::Queued {},
        application_ack_state: ApplicationAckState::Waiting {},
        detail: None,
        sent_at_ms: Some(1_700_000_000_300),
        received_at_ms: None,
        updated_at_ms: 1_700_000_000_300,
    }
}

fn build_telemetry() -> TelemetryPositionRecord {
    TelemetryPositionRecord {
        callsign: "POCO".to_string(),
        lat: 44.6488,
        lon: -63.5752,
        alt: Some(12.0),
        course: Some(45.0),
        speed: Some(3.5),
        accuracy: Some(5.0),
        updated_at_ms: 1_700_000_000_400,
    }
}

#[test]
fn latest_sos_telemetry_reads_updated_snapshot() {
    let store = Arc::new(Mutex::new(Some(SosDeviceTelemetryRecord {
        lat: None,
        lon: None,
        alt: None,
        speed: None,
        course: None,
        accuracy: None,
        battery_percent: Some(52.0),
        battery_charging: Some(false),
        updated_at_ms: 1_700_000_000_000,
    })));

    assert_eq!(
        latest_sos_telemetry(&store).and_then(|value| value.lat),
        None
    );

    *store.lock().expect("telemetry lock") = Some(SosDeviceTelemetryRecord {
        lat: Some(44.6488),
        lon: Some(-63.5752),
        alt: None,
        speed: None,
        course: None,
        accuracy: Some(8.0),
        battery_percent: Some(53.0),
        battery_charging: Some(false),
        updated_at_ms: 1_700_000_003_000,
    });

    let telemetry = latest_sos_telemetry(&store).expect("latest telemetry");
    assert_eq!(telemetry.lat, Some(44.6488));
    assert_eq!(telemetry.lon, Some(-63.5752));
    assert_eq!(telemetry.battery_percent, Some(53.0));
}

fn sample_app_settings() -> AppSettingsRecord {
    AppSettingsRecord {
        display_name: "Alpha".to_string(),
        auto_connect_saved: true,
        announce_capabilities: "mission,eam".to_string(),
        tcp_clients: vec!["tcp://127.0.0.1:4242".to_string()],
        broadcast: true,
        transport_node_enabled: true,
        announce_interval_seconds: 30,
        telemetry: TelemetrySettingsRecord {
            enabled: true,
            publish_interval_seconds: 15,
            accuracy_threshold_meters: Some(8.5),
            stale_after_minutes: 5,
            expire_after_minutes: 30,
        },
        hub: HubSettingsRecord {
            mode: HubMode::Autonomous {},
            identity_hash: String::new(),
            api_base_url: String::new(),
            api_key: String::new(),
            refresh_interval_seconds: 0,
        },
        teams: crate::types::TeamSettingsRecord::default(),
        checklists: crate::types::ChecklistSettingsRecord::default(),
        rnode: crate::types::RnodeSettingsRecord::default(),
        community: crate::types::CommunitySettingsRecord::default(),
        power: crate::types::PowerPolicyRecord::default(),
    }
}

fn sample_saved_peer() -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: "A1B2C3D4".to_string(),
        label: Some("Bravo".to_string()),
        saved_at_ms: 1,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    }
}

fn sample_eam() -> EamProjectionRecord {
    EamProjectionRecord {
        callsign: "ALPHA-1".to_string(),
        group_name: "Operations".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Ready".to_string(),
        preparedness_status: "Ready".to_string(),
        medical_status: "Ready".to_string(),
        mobility_status: "Ready".to_string(),
        comms_status: "Ready".to_string(),
        notes: Some("pre-start import".to_string()),
        updated_at_ms: 1,
        deleted_at_ms: None,
        eam_uid: Some("eam-1".to_string()),
        team_member_uid: Some("member-1".to_string()),
        team_uid: Some("team-1".to_string()),
        reported_at: None,
        reported_by: None,
        overall_status: Some("Green".to_string()),
        confidence: Some(1.0),
        ttl_seconds: Some(3600),
        source: None,
        sync_state: Some("Synced".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(1),
        last_synced_at_ms: Some(1),
    }
}

fn sample_event() -> EventProjectionRecord {
    EventProjectionRecord {
        uid: "event-1".to_string(),
        command_id: "command-1".to_string(),
        source_identity: "identity-1".to_string(),
        source_display_name: Some("Alpha".to_string()),
        timestamp: "2026-03-25T00:00:00Z".to_string(),
        command_type: "event".to_string(),
        mission_uid: "mission-1".to_string(),
        content: "status update".to_string(),
        callsign: "ALPHA-1".to_string(),
        server_time: None,
        client_time: None,
        keywords: vec!["status".to_string()],
        content_hashes: vec!["hash-1".to_string()],
        updated_at_ms: 1,
        deleted_at_ms: None,
        correlation_id: Some("corr-1".to_string()),
        topics: vec!["mission".to_string()],
    }
}

fn sample_message() -> MessageRecord {
    MessageRecord {
        message_id_hex: "msg-1".to_string(),
        conversation_id: "conversation-1".to_string(),
        direction: MessageDirection::Outbound {},
        destination_hex: "DEST-1".to_string(),
        source_hex: None,
        requested_destination_hex: Some("DEST-1".to_string()),
        delivery_destination_hex: Some("DEST-1".to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("msg-1".to_string()),
        title: Some("Hello".to_string()),
        body_utf8: "hello from pre-start".to_string(),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Queued {},
        transport_state: TransportDeliveryState::Queued {},
        application_ack_state: ApplicationAckState::Waiting {},
        detail: Some("queued".to_string()),
        sent_at_ms: Some(1),
        received_at_ms: None,
        updated_at_ms: 1,
    }
}

fn sample_position() -> TelemetryPositionRecord {
    TelemetryPositionRecord {
        callsign: "ALPHA-1".to_string(),
        lat: 44.0,
        lon: -63.0,
        alt: Some(12.0),
        course: Some(90.0),
        speed: Some(3.0),
        accuracy: Some(5.0),
        updated_at_ms: 1,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_telemetry_is_received_as_mission_packet() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let body = "telemetry: position sample";
    let fields = mission_command_fields(
        "cmd-telemetry-123",
        "corr-telemetry-123",
        "mission.registry.telemetry.upsert",
        vec![
            ("event_uid", MsgPackValue::from("telemetry-123")),
            ("team_member_uid", MsgPackValue::from("member-1")),
            ("team_uid", MsgPackValue::from("team-1")),
            ("mission_uid", MsgPackValue::from("mission-1")),
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
        .expect("send telemetry packet");

    let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
    })
    .expect("node b received telemetry packet");

    assert_packet_received(
        event,
        &node_a_status.lxmf_destination_hex,
        body,
        Some(fields.as_slice()),
    );
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("telemetry metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.telemetry.upsert")
    );
    assert_eq!(metadata.event_uid.as_deref(), Some("telemetry-123"));

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_built_telemetry_replication_payload_is_persisted_by_receiver() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry_payload_projection").await;

    let node_b_status = node_b.get_status();
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

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn record_local_telemetry_fix_replicates_to_native_peer_projection() {
    const TELEMETRY_REPLICATION_TIMEOUT: Duration = Duration::from_secs(300);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("telemetry_projection").await;

    let node_b_status = node_b.get_status();
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm telemetry link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm telemetry link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm telemetry link")
    })
    .expect("node b received telemetry warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.active_link);
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became telemetry-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

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

    node_a
        .record_local_telemetry_fix(position.clone())
        .expect("record local telemetry");

    let received = tokio::time::timeout(TELEMETRY_REPLICATION_TIMEOUT, async {
        loop {
            if let Some(received) = node_b
                .get_telemetry_positions()
                .expect("get telemetry")
                .into_iter()
                .find(|entry| entry.callsign == position.callsign)
            {
                break received;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await;

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;

    let received = received.expect("telemetry replication timed out after 300 seconds");
    assert_eq!(received.callsign, position.callsign);
    assert_eq!(received.lat, position.lat);
    assert_eq!(received.lon, position.lon);
    assert_eq!(received.updated_at_ms, position.updated_at_ms);
}
