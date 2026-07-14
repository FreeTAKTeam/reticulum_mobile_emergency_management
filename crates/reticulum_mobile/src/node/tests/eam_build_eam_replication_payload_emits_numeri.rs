#[test]
fn build_eam_replication_payload_emits_numeric_lxmf_command_field() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Pixel".to_string(),
        identity_hex: "11111111111111111111111111111111".to_string(),
        app_destination_hex: "22222222222222222222222222222222".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let record = build_eam();
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };

    let (_, fields) =
        build_eam_replication_payload(&status, &record, &target).expect("eam fields");
    let metadata = parse_mission_sync_metadata(&fields).expect("mission metadata");

    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.eam.upsert")
    );
    assert!(metadata.eam_uid.is_none());
    assert!(metadata.team_uid.is_none());
    assert!(metadata.team_member_uid.is_none());
}

#[test]
fn eam_replication_payload_uses_compact_envelope_like_events() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod,
    };
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
    let mut record = build_eam();
    record.eam_uid = Some("eam-6ef80799-29b1-4de5-b711-3896b9a55161".to_string());
    record.team_member_uid = Some("fb4c70e20cfac047b899ca2f3671b50a".to_string());
    record.team_uid = Some("blue-team".to_string());
    record.notes = None;
    let target = MissionReplicationTarget {
        app_destination_hex: "e3f1c3f63adef0c0d771ddff8b0eeed5".to_string(),
        send_mode: SendMode::Auto {},
    };

    let (body, fields) =
        build_eam_replication_payload(&status, &record, &target).expect("eam payload");
    let field_text = String::from_utf8_lossy(&fields);
    let field_value: MsgPackValue = rmp_serde::from_slice(fields.as_slice()).expect("fields");
    let field_entries = field_value.as_map().expect("field map");
    let commands = field_entries
        .iter()
        .find(|(key, _)| key.as_i64() == Some(FIELD_COMMANDS))
        .and_then(|(_, value)| value.as_array())
        .expect("command field");
    let command = commands
        .first()
        .and_then(MsgPackValue::as_map)
        .expect("command map");
    let has_command_source = command.iter().any(|(key, _)| key.as_str() == Some("s"));

    assert_eq!(body.as_slice(), b"E|POCO|GYGGGY");
    assert!(
        fields.len() <= 24,
        "compact EAM fields should stay small, fields bytes={}",
        fields.len()
    );
    assert!(
        !has_command_source,
        "compact EAM fields should derive source from the LXMF message source"
    );
    assert!(
        command.iter().all(|(key, _)| key.as_str() != Some("a")),
        "compact EAM fields should keep payload data in the body"
    );
    for verbose in [
        "command_type",
        "security_status",
        "capability_status",
        "preparedness_status",
        "team_member_uid",
        "team_uid",
        "callsign",
        "e3f1",
    ] {
        assert!(
            !field_text.contains(verbose),
            "compact EAM fields should not contain verbose token {verbose}"
        );
    }

    let source = hex::decode(status.lxmf_destination_hex.as_str()).expect("source hex");
    let target_hex = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(target_hex.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(body.as_slice());
    message.fields = Some(rmp_serde::from_slice(fields.as_slice()).expect("fields msgpack"));
    let identity = PrivateIdentity::new_from_name("eam-replication-budget");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");
    let receiver = PrivateIdentity::new_from_name("eam-replication-budget-rx");
    let receiver = crate::runtime::lxmf_private_identity(&receiver).expect("receiver");
    let propagation_stamp = [0_u8; 32];
    let (propagated, _) = lxmf::message::WireMessage::unpack(wire.as_slice())
        .expect("wire unpack")
        .pack_propagation_with_options_and_rng(
            receiver.as_identity(),
            2.0,
            Some(propagation_stamp.as_slice()),
            rand_core::OsRng,
        )
        .expect("propagation pack");
    assert!(
        propagated.len() <= 360,
        "compact EAM stamped propagation should have budget headroom, bytes={}",
        propagated.len()
    );
    assert!(
        wire.len() <= 140,
        "RNode direct EAM wire bytes={} budget=140",
        wire.len()
    );

    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(decision.representation, LxmfMessageMethod::Packet);

    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("mission metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.eam.upsert")
    );
    assert!(metadata.eam_uid.is_none());
    assert!(metadata.team_uid.is_none());
    assert!(metadata.team_member_uid.is_none());
    assert_eq!(metadata.command_id.as_deref(), Some("m"));
}

#[test]
fn eam_delete_payload_uses_compact_stable_command_ids_like_events() {
    let target = MissionReplicationTarget {
        app_destination_hex: "e3f1c3f63adef0c0d771ddff8b0eeed5".to_string(),
        send_mode: SendMode::Auto {},
    };

    let (_body, fields) =
        build_eam_delete_replication_payload("NoemiPix", 1_779_756_701_317, &target)
            .expect("eam delete payload");
    let field_text = String::from_utf8_lossy(&fields);

    assert!(
        !field_text.contains("e3f1"),
        "delete payload should not carry a destination-specific command suffix"
    );
    assert!(field_text.contains("md:noemipix:mplx6rnp"));
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("delete metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("mission.registry.eam.delete")
    );
}

#[test]
fn populate_eam_defaults_uses_local_app_hash_and_team_color_hash() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Pixel".to_string(),
        identity_hex: "11111111111111111111111111111111".to_string(),
        app_destination_hex: "22222222222222222222222222222222".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let mut record = build_eam();
    record.group_name = "blue".to_string();
    record.team_member_uid = None;
    record.team_uid = None;
    record.reported_by = None;
    record.source = None;
    record.overall_status = None;

    let normalized = populate_eam_defaults(&status, &record);

    assert_eq!(normalized.group_name, "BLUE");
    assert_eq!(
        normalized.team_member_uid.as_deref(),
        Some("22222222222222222222222222222222")
    );
    assert_eq!(normalized.team_uid.as_deref(), Some(TEAM_UID_BLUE));
    assert_eq!(normalized.reported_by.as_deref(), Some("Pixel"));
    assert_eq!(
        normalized
            .source
            .as_ref()
            .map(|source| source.rns_identity.as_str()),
        Some("11111111111111111111111111111111")
    );
    assert_eq!(normalized.overall_status.as_deref(), Some("Yellow"));
}

fn build_event() -> EventProjectionRecord {
    EventProjectionRecord {
        uid: "evt-1".to_string(),
        command_id: "cmd-1".to_string(),
        source_identity: "identity-1".to_string(),
        source_display_name: Some("Atlas-1".to_string()),
        timestamp: "2026-03-25T00:00:00Z".to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: "mission-1".to_string(),
        content: "Economy Crash".to_string(),
        callsign: "Atlas-1".to_string(),
        server_time: None,
        client_time: None,
        keywords: vec!["economy".to_string()],
        content_hashes: vec!["hash-1".to_string()],
        updated_at_ms: 1_700_000_000_200,
        deleted_at_ms: None,
        correlation_id: Some("corr-1".to_string()),
        topics: vec!["mission-1".to_string()],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_built_eam_replication_payload_is_persisted_by_receiver() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("eam_payload_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let record = EamProjectionRecord {
        callsign: "Pixel".to_string(),
        group_name: "Blue".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Yellow".to_string(),
        preparedness_status: "Green".to_string(),
        medical_status: "Green".to_string(),
        mobility_status: "Green".to_string(),
        comms_status: "Yellow".to_string(),
        notes: Some("native eam replication".to_string()),
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        eam_uid: Some("eam-upsert-native".to_string()),
        team_member_uid: Some("member-1".to_string()),
        team_uid: Some("team-1".to_string()),
        reported_at: Some("2026-03-25T16:30:00Z".to_string()),
        reported_by: Some(node_a_status.name.clone()),
        overall_status: Some("Yellow".to_string()),
        confidence: Some(0.8),
        ttl_seconds: Some(3600),
        source: Some(EamSourceRecord {
            rns_identity: node_a_status.identity_hex.clone(),
            display_name: Some(node_a_status.name.clone()),
        }),
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(now_ms()),
        last_synced_at_ms: None,
    };
    let target = MissionReplicationTarget {
        app_destination_hex: node_b_status.app_destination_hex.clone(),
        send_mode: SendMode::Auto {},
    };
    let (body, fields) =
        build_eam_replication_payload(&node_a_status, &record, &target).expect("eam payload");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("eam metadata");
    let command_id = metadata.command_id.clone().expect("eam command id");
    let command_type = metadata.command_type.clone().expect("eam command type");
    let ack_subscription = node_a.subscribe_events();

    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            body,
            Some(fields),
            SendMode::Auto {},
        )
        .expect("send eam replication payload");

    let received_deadline = Instant::now() + TEST_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| eam.callsign == record.callsign);
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted direct eam replication payload"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert!(received.eam_uid.is_none());
    assert!(received.team_uid.is_none());
    assert_eq!(
        received.team_member_uid.as_deref(),
        Some(node_a_status.lxmf_destination_hex.as_str())
    );
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
