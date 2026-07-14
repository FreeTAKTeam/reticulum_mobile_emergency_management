#[test]
fn checklist_upload_snapshot_uses_compressed_msgpack_content_not_command_fields() {
    let status = NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Pixel".to_string(),
        identity_hex: "11111111111111111111111111111111".to_string(),
        app_destination_hex: "22222222222222222222222222222222".to_string(),
        lxmf_destination_hex: "33333333333333333333333333333333".to_string(),
        interfaces: Vec::new(),
    };
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let snapshot_json =
        r#"{"uid":"chk-native","name":"Native","tasks":[{"task_uid":"task-1","number":1}]}"#;
    let args = checklist_uid_args_json("chk-native");

    let (body, fields) = build_checklist_replication_payload_with_snapshot(
        &status,
        &target,
        "checklist.upload",
        &args,
        Some("cmd-upload"),
        snapshot_json,
    )
    .expect("upload payload");
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields.as_slice()).expect("fields");
    let MsgPackValue::Map(field_entries) = fields else {
        panic!("fields should be a map");
    };
    let commands = field_entries
        .iter()
        .find(|(key, _)| key.as_i64() == Some(FIELD_COMMANDS))
        .and_then(|(_, value)| value.as_array())
        .expect("commands");
    let command = commands[0].as_map().expect("command map");

    assert!(!command
        .iter()
        .any(|(key, _)| key.as_str() == Some("snapshot")));
    let content =
        rmp_serde::from_slice::<MsgPackValue>(body.as_slice()).expect("msgpack snapshot body");
    let MsgPackValue::Map(content_entries) = content else {
        panic!("snapshot body should be a map");
    };
    assert!(content_entries
        .iter()
        .any(|(key, value)| key.as_str() == Some("type")
            && value.as_str() == Some("rem.checklist.snapshot.v2")));
    assert!(content_entries.iter().any(|(key, value)| {
        key.as_str() == Some("encoding") && value.as_str() == Some("zlib+msgpack")
    }));
    assert!(content_entries
        .iter()
        .any(|(key, value)| key.as_str() == Some("snapshot")
            && matches!(value, MsgPackValue::Binary(_))));
}

#[test]
fn compact_checklist_create_payload_stays_packet_sized() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod,
    };
    use reticulum::transport::identity::PrivateIdentity;

    let checklist = ChecklistRecord {
        uid: "chk-1780000000000".to_string(),
        mission_uid: Some("LORA".to_string()),
        template_uid: None,
        template_version: None,
        template_name: None,
        name: "LoRaChk0927".to_string(),
        description: String::new(),
        start_time: None,
        mode: crate::types::ChecklistMode::Online {},
        sync_state: crate::types::ChecklistSyncState::Synced {},
        origin_type: crate::types::ChecklistOriginType::RchTemplate {},
        checklist_status: crate::types::ChecklistTaskStatus::Pending {},
        created_at: Some("2026-04-23T12:00:00Z".to_string()),
        created_by_team_member_rns_identity: "peer-a".to_string(),
        created_by_team_member_display_name: Some("Peer A".to_string()),
        updated_at: Some("2026-04-23T12:00:00Z".to_string()),
        last_changed_by_team_member_rns_identity: Some("peer-a".to_string()),
        deleted_at: None,
        uploaded_at: Some("2026-04-23T12:00:00Z".to_string()),
        participant_rns_identities: vec!["peer-a".to_string()],
        expected_task_count: Some(1),
        progress_percent: 0.0,
        counts: crate::types::ChecklistStatusCounts {
            pending_count: 1,
            late_count: 0,
            complete_count: 0,
        },
        columns: vec![crate::types::ChecklistColumnRecord {
            column_uid: "col-item".to_string(),
            column_name: "Item".to_string(),
            display_order: 0,
            column_type: crate::types::ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: true,
            system_key: None,
        }],
        tasks: vec![crate::types::ChecklistTaskRecord {
            task_uid: "task-1".to_string(),
            number: 1,
            user_status: crate::types::ChecklistUserTaskStatus::Pending {},
            task_status: crate::types::ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some("2026-04-23T12:00:00Z".to_string()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: None,
            due_dtg: None,
            notes: None,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some("Water".to_string()),
            cells: vec![crate::types::ChecklistCellRecord {
                cell_uid: "task-1:col-item".to_string(),
                task_uid: "task-1".to_string(),
                column_uid: "col-item".to_string(),
                value: Some("Water".to_string()),
                updated_at: None,
                updated_by_team_member_rns_identity: None,
            }],
        }],
        feed_publications: Vec::new(),
    };
    let create_request = ChecklistCreateOnlineRequest {
        checklist_uid: Some(checklist.uid.clone()),
        mission_uid: checklist.mission_uid.clone(),
        template_uid: "tmpl-vehicle-emergency-preparedness".to_string(),
        name: checklist.name.clone(),
        description: checklist.description.clone(),
        start_time: "2026-04-23T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some(
            checklist.created_by_team_member_rns_identity.clone(),
        ),
        created_by_team_member_display_name: checklist
            .created_by_team_member_display_name
            .clone(),
    };
    let mut snapshot_args =
        checklist_create_online_args_json(&create_request).expect("create args");
    append_checklist_create_snapshot_args(&mut snapshot_args, &checklist)
        .expect("append create snapshot");
    assert_eq!(
        snapshot_args
            .get("checklist_uid")
            .and_then(JsonValue::as_str),
        Some("chk-1780000000000")
    );
    assert!(snapshot_args.get("columns").is_none());
    assert_eq!(
        snapshot_args.get("total_tasks").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        snapshot_args
            .get("created_by_team_member_display_name")
            .and_then(JsonValue::as_str),
        Some("Peer A")
    );
    assert!(snapshot_args.get("tasks").is_none());
    assert!(snapshot_args.get("counts").is_none());
    assert!(snapshot_args.get("progress_percent").is_none());

    let status = build_status_for_tests();
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let create_args = compact_checklist_create_online_args_json(
        &create_request,
        checklist.expected_task_count,
    )
    .expect("compact create args");
    assert!(create_args.get("description").is_none());
    assert!(create_args.get("start_time").is_none());
    let (create_body, create_fields) = build_checklist_replication_payload_with_command_id(
        &status,
        &target,
        "checklist.create.online",
        &create_args,
        Some("cmd-chk-1780000000000"),
    )
    .expect("create payload");
    assert_eq!(String::from_utf8_lossy(create_body.as_slice()), "C1");

    let source = hex::decode(status.lxmf_destination_hex.as_str()).expect("source hex");
    let destination = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(destination.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(create_body.as_slice());
    message.fields = Some(rmp_serde::from_slice(create_fields.as_slice()).expect("fields"));
    let identity = PrivateIdentity::new_from_name("compact-checklist-create");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");
    const RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES: usize = 145;
    assert!(
        wire.len() <= RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES,
        "compact checklist create should fit RNode direct packet budget, body={} fields={} wire={} budget={}",
        create_body.len(),
        create_fields.len(),
        wire.len(),
        RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES
    );
    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(
        decision.representation,
        LxmfMessageMethod::Packet,
        "compact checklist create should avoid resource mode, body={} fields={} wire={}",
        create_body.len(),
        create_fields.len(),
        wire.len()
    );
}

#[test]
fn built_in_checklist_create_replicates_template_tasks() {
    let request = ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-1780000000000".to_string()),
        mission_uid: Some("LORA".to_string()),
        template_uid: "tmpl-72-hour-home-preparedness".to_string(),
        name: "LoRaChk".to_string(),
        description: String::new(),
        start_time: "2026-04-23T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some("peer-a".to_string()),
        created_by_team_member_display_name: Some("Peer A".to_string()),
    };
    let create_args =
        compact_checklist_create_online_args_json(&request, Some(12)).expect("create args");

    assert!(create_template_replicates_tasks_from_template(&create_args));
}

#[test]
fn checklist_cell_subject_includes_task_and_column() {
    let task_one_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
        checklist_uid: "chk-hydrate".to_string(),
        task_uid: "task-1".to_string(),
        column_uid: "col-description".to_string(),
        value: "Reliable ignition source".to_string(),
        updated_by_team_member_rns_identity: None,
    });
    let task_two_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
        checklist_uid: "chk-hydrate".to_string(),
        task_uid: "task-2".to_string(),
        column_uid: "col-description".to_string(),
        value: "Hands-free lighting".to_string(),
        updated_by_team_member_rns_identity: None,
    });

    let task_one_subject = checklist_subject_token("checklist.task.cell.set", &task_one_args);
    let task_two_subject = checklist_subject_token("checklist.task.cell.set", &task_two_args);

    assert_eq!(task_one_subject, "chk-hydrate-task-1-col-description");
    assert_eq!(task_two_subject, "chk-hydrate-task-2-col-description");
    assert_ne!(task_one_subject, task_two_subject);
}

async fn start_node_pair(test_name: &str) -> (TcpRelayHandle, Node, Node) {
    let relay = TcpRelayHandle::start().await;

    let node_a_storage = prepare_storage_dir(&format!("{test_name}_a"));
    let node_b_storage = prepare_storage_dir(&format!("{test_name}_b"));

    let node_a = Node::new().expect("node a storage");
    node_a
        .start(build_config(
            &format!("{test_name}-a"),
            node_a_storage.as_path(),
            relay.address().as_str(),
        ))
        .expect("start node a");

    let node_b = Node::new().expect("node b storage");
    node_b
        .start(build_config(
            &format!("{test_name}-b"),
            node_b_storage.as_path(),
            relay.address().as_str(),
        ))
        .expect("start node b");

    node_a.announce_now().expect("announce node a");
    node_b.announce_now().expect("announce node b");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let node_b_lxmf_destination_hex = node_b.get_status().lxmf_destination_hex;
    node_a
        .request_peer_identity(node_b_lxmf_destination_hex.clone())
        .expect("resolve node b");
    let node_a_lxmf_destination_hex = node_a.get_status().lxmf_destination_hex;
    node_b
        .request_peer_identity(node_a_lxmf_destination_hex.clone())
        .expect("resolve node a");
    tokio::time::sleep(Duration::from_millis(250)).await;

    (relay, node_a, node_b)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_built_checklist_replication_payloads_acknowledge_after_persistence() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("checklist_payload_ack").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    let target = MissionReplicationTarget {
        app_destination_hex: node_b_status.app_destination_hex.clone(),
        send_mode: SendMode::Auto {},
    };
    let create_args = checklist_create_online_args_json(&ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-operational-ack".to_string()),
        mission_uid: Some("mission-operational-ack".to_string()),
        template_uid: "template-operational-ack".to_string(),
        name: "Operational ACK checklist".to_string(),
        description: "Created by operational ACK test".to_string(),
        start_time: "2026-05-18T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some(node_a_status.identity_hex.clone()),
        created_by_team_member_display_name: Some(node_a_status.name.clone()),
    })
    .expect("checklist create args");
    let (create_body, create_fields) = build_checklist_replication_payload(
        &node_a_status,
        &target,
        "checklist.create.online",
        &create_args,
    )
    .expect("checklist create payload");
    let create_metadata =
        parse_mission_sync_metadata(create_fields.as_slice()).expect("create metadata");
    let create_command_type = create_metadata
        .command_type
        .clone()
        .expect("create command type");
    assert!(
        create_metadata.command_id.is_none(),
        "compact create updates intentionally omit command ids"
    );

    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            create_body,
            Some(create_fields),
            SendMode::Auto {},
        )
        .expect("send checklist create payload");

    let create_deadline = Instant::now() + TEST_TIMEOUT;
    let received_checklist = loop {
        let received = node_b
            .get_checklist("chk-operational-ack".to_string())
            .expect("get checklist");
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < create_deadline,
            "node b never persisted direct checklist create payload"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert_eq!(received_checklist.name, "Operational ACK checklist");
    assert_eq!(create_command_type, "checklist.create.online");

    let status_args = checklist_task_status_args_json(&ChecklistTaskStatusSetRequest {
        checklist_uid: "chk-operational-ack".to_string(),
        task_uid: "task-operational-ack".to_string(),
        user_status: crate::types::ChecklistUserTaskStatus::Complete {},
        changed_by_team_member_rns_identity: Some(node_a_status.identity_hex.clone()),
    });
    let (status_body, status_fields) = build_checklist_replication_payload(
        &node_a_status,
        &target,
        "checklist.task.status.set",
        &status_args,
    )
    .expect("checklist task status payload");
    let status_metadata =
        parse_mission_sync_metadata(status_fields.as_slice()).expect("status metadata");
    let status_command_type = status_metadata
        .command_type
        .clone()
        .expect("status command type");
    assert!(
        status_metadata.command_id.is_none(),
        "compact status updates intentionally omit command ids"
    );

    node_a
        .send_bytes(
            node_b_status.app_destination_hex.clone(),
            status_body,
            Some(status_fields),
            SendMode::Auto {},
        )
        .expect("send checklist task status payload");

    let status_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let received = node_b
            .get_checklist("chk-operational-ack".to_string())
            .expect("get checklist")
            .expect("checklist remains active");
        let task = received
            .tasks
            .into_iter()
            .find(|task| task.task_uid == "task-operational-ack");
        if let Some(task) = task {
            assert_eq!(
                task.user_status,
                crate::types::ChecklistUserTaskStatus::Complete {}
            );
            break;
        }
        assert!(
            Instant::now() < status_deadline,
            "node b never persisted direct checklist task status payload"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(status_command_type, "checklist.task.status.set");

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
