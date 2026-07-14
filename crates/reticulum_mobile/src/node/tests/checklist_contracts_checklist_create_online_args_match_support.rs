#[test]
fn checklist_create_online_args_match_supported_contract() {
    let args = checklist_create_online_args_json(&ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-001".to_string()),
        mission_uid: Some("mission-alpha".to_string()),
        template_uid: "tmpl-evac-001".to_string(),
        name: "Mission Alpha Evac".to_string(),
        description: "Shared run for Alpha".to_string(),
        start_time: "2026-04-22T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: Some("abcd1234".to_string()),
        created_by_team_member_display_name: None,
    })
    .expect("build create args");

    assert_eq!(
        args.get("name").and_then(JsonValue::as_str),
        Some("Mission Alpha Evac")
    );
    assert_eq!(
        args.get("template_uid").and_then(JsonValue::as_str),
        Some("tmpl-evac-001")
    );
    assert_eq!(
        args.get("mission_uid").and_then(JsonValue::as_str),
        Some("mission-alpha")
    );
    assert_eq!(
        args.get("description").and_then(JsonValue::as_str),
        Some("Shared run for Alpha")
    );
    assert_eq!(
        args.get("start_time").and_then(JsonValue::as_str),
        Some("2026-04-22T12:00:00Z")
    );
    assert_eq!(
        args.get("checklist_uid").and_then(JsonValue::as_str),
        Some("chk-001")
    );
}

#[test]
fn checklist_update_args_include_explicit_clears() {
    let args = checklist_update_args_json(&ChecklistUpdateRequest {
        checklist_uid: "chk-001".to_string(),
        patch: crate::types::ChecklistUpdatePatch {
            mission_uid: Some(String::new()),
            template_uid: Some(String::new()),
            name: Some("".to_string()),
            description: Some("".to_string()),
            start_time: Some(String::new()),
        },
        changed_by_team_member_rns_identity: None,
    });
    let patch = args
        .get("patch")
        .and_then(JsonValue::as_object)
        .expect("patch object");

    assert_eq!(
        patch.get("mission_uid").and_then(JsonValue::as_str),
        Some("")
    );
    assert_eq!(
        patch.get("template_uid").and_then(JsonValue::as_str),
        Some("")
    );
    assert_eq!(patch.get("name").and_then(JsonValue::as_str), Some(""));
    assert_eq!(
        patch.get("description").and_then(JsonValue::as_str),
        Some("")
    );
    assert_eq!(
        patch.get("start_time").and_then(JsonValue::as_str),
        Some("")
    );
}

#[test]
fn checklist_delete_replication_payload_uses_supported_command() {
    let status = build_status_for_tests();
    let saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("saved-peer".to_string()),
        saved_at_ms: now_ms(),
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
    };
    let peers = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    )];

    let scheduled = build_checklist_delete_replication_sends(
        &status,
        peers.as_slice(),
        &[saved_peer],
        None,
        None,
        None,
        None,
        "chk-001",
        true,
    )
    .expect("remote delete should build payload");

    assert_eq!(scheduled.len(), 1);
    let (destination_hex, body, fields, send_mode) = &scheduled[0];
    assert_eq!(destination_hex, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert!(matches!(send_mode, SendMode::Auto {}));
    assert_eq!(String::from_utf8_lossy(body.as_slice()), "C C4 chk-001");
    let field_text = String::from_utf8_lossy(fields.as_slice());
    for verbose in [
        "command_id",
        "correlation_id",
        "command_type",
        "source",
        "timestamp",
        "topics",
        "checklist_uid",
        "checklist.delete",
    ] {
        assert!(
            !field_text.contains(verbose),
            "compact checklist fields should not contain verbose token {verbose}"
        );
    }

    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");
    assert_eq!(metadata.command_type.as_deref(), Some("checklist.delete"));
    assert_eq!(metadata.checklist_uid.as_deref(), Some("chk-001"));
}

#[test]
fn checklist_task_cell_payload_uses_compact_args_with_compatible_metadata() {
    let status = build_status_for_tests();
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
        checklist_uid: "chk-001".to_string(),
        task_uid: "task-001".to_string(),
        column_uid: "col-task".to_string(),
        value: "  Move to alternate pickup  ".to_string(),
        updated_by_team_member_rns_identity: Some(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ),
    });

    let (body, fields) =
        build_checklist_replication_payload(&status, &target, "checklist.task.cell.set", &args)
            .expect("checklist task cell payload");

    assert_eq!(
        String::from_utf8_lossy(body.as_slice()),
        "C CA chk-001-task-001-col-task"
    );
    assert!(
        fields.len() <= 260,
        "compact checklist task cell fields should stay small, fields bytes={}",
        fields.len()
    );
    let field_text = String::from_utf8_lossy(fields.as_slice());
    for verbose in [
        "command_type",
        "checklist.task.cell.set",
        "checklist_uid",
        "task_uid",
        "column_uid",
        "updated_by_team_member_rns_identity",
    ] {
        assert!(
            !field_text.contains(verbose),
            "compact checklist fields should not contain verbose token {verbose}"
        );
    }

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
    let command_args = command
        .iter()
        .find(|(key, _)| key.as_str() == Some("a"))
        .and_then(|(_, value)| value.as_map())
        .expect("command args");
    let has_arg = |name: &str| {
        command_args
            .iter()
            .any(|(key, _)| key.as_str() == Some(name))
    };
    for compact in ["cl", "tsk", "col", "v", "ub"] {
        assert!(
            has_arg(compact),
            "compact checklist arg {compact} should be present"
        );
    }

    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("checklist.task.cell.set")
    );
    assert_eq!(metadata.checklist_uid.as_deref(), Some("chk-001"));
    assert_eq!(metadata.task_uid.as_deref(), Some("task-001"));
    assert_eq!(metadata.column_uid.as_deref(), Some("col-task"));
}

#[test]
fn checklist_task_status_payload_stays_packet_sized_for_template_task_ids() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod,
    };
    use reticulum::transport::identity::PrivateIdentity;

    let status = build_status_for_tests();
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let mut args = checklist_task_status_args_json(&ChecklistTaskStatusSetRequest {
        checklist_uid: "chk-1779802362961".to_string(),
        task_uid: "tmpl-vehicle-emergency-preparedness-task-1".to_string(),
        user_status: crate::types::ChecklistUserTaskStatus::Complete {},
        changed_by_team_member_rns_identity: Some(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ),
    });
    args.insert("number".to_string(), JsonValue::from(1_u64));

    let (body, fields) = build_checklist_replication_payload(
        &status,
        &target,
        "checklist.task.status.set",
        &args,
    )
    .expect("checklist task status payload");

    let source = hex::decode(status.lxmf_destination_hex.as_str()).expect("source hex");
    let destination = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(destination.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(body.as_slice());
    message.fields = Some(rmp_serde::from_slice(fields.as_slice()).expect("fields"));
    let identity = PrivateIdentity::new_from_name("compact-checklist-task-status");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");
    const RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES: usize = 145;
    assert!(
        wire.len() <= RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES,
        "task status should fit RNode direct packet budget, body={} fields={} wire={} budget={}",
        body.len(),
        fields.len(),
        wire.len(),
        RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES
    );
    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(
        decision.representation,
        LxmfMessageMethod::Packet,
        "task status should avoid resource mode, body={} fields={} wire={}",
        body.len(),
        fields.len(),
        wire.len()
    );
    let field_text = String::from_utf8_lossy(fields.as_slice());
    assert!(
        !field_text.contains("changed_by_team_member_rns_identity")
            && !field_text.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        "status attribution should use command source identity, not duplicate identity args"
    );
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");
    assert_eq!(
        metadata.command_type.as_deref(),
        Some("checklist.task.status.set")
    );
    assert_eq!(metadata.checklist_uid.as_deref(), Some("chk-1779802362961"));
    assert_eq!(metadata.task_uid.as_deref(), None);
}

#[test]
fn checklist_task_payloads_preserve_whitespace_and_style_clears() {
    let row_add_args = checklist_task_row_add_args_json(&ChecklistTaskRowAddRequest {
        checklist_uid: "chk-001".to_string(),
        task_uid: Some("task-001".to_string()),
        number: 1,
        due_relative_minutes: None,
        legacy_value: Some("  Confirm rally point  ".to_string()),
        changed_by_team_member_rns_identity: None,
    });
    assert_eq!(
        row_add_args.get("legacy_value").and_then(JsonValue::as_str),
        Some("  Confirm rally point  ")
    );

    let detail_row_args = checklist_task_row_add_args_from_task(
        "chk-001",
        &ChecklistTaskRecord {
            task_uid: "task-detail".to_string(),
            number: 2,
            user_status: crate::types::ChecklistUserTaskStatus::Pending {},
            task_status: crate::types::ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some("2026-04-24T12:00:00Z".to_string()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: Some(30),
            due_dtg: Some("2026-04-24T12:30:00Z".to_string()),
            notes: Some("Bring printed route card".to_string()),
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some("Confirm rally point".to_string()),
            cells: Vec::new(),
        },
        Some("peer-a"),
    );
    assert_eq!(
        detail_row_args.get("due_dtg").and_then(JsonValue::as_str),
        Some("2026-04-24T12:30:00Z")
    );
    assert_eq!(
        detail_row_args.get("notes").and_then(JsonValue::as_str),
        Some("Bring printed route card")
    );

    let compact_initial_args = compact_initial_checklist_task_row_add_args_from_task(
        "chk-001",
        &ChecklistTaskRecord {
            task_uid: "tmpl-import-task-2".to_string(),
            number: 2,
            user_status: crate::types::ChecklistUserTaskStatus::Pending {},
            task_status: crate::types::ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some("2026-04-24T12:00:00Z".to_string()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: Some(30),
            due_dtg: Some("2026-04-24T12:30:00Z".to_string()),
            notes: Some(
                "Document where everyone will shelter, which room is safest, and when to leave."
                    .to_string(),
            ),
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some("Shelter room assignment".to_string()),
            cells: Vec::new(),
        },
    );
    assert_eq!(
        compact_initial_args
            .get("legacy_value")
            .and_then(JsonValue::as_str),
        Some("Shelter room assignment")
    );
    assert!(compact_initial_args.get("notes").is_none());
    assert!(compact_initial_args.get("due_dtg").is_none());
    assert!(compact_initial_args.get("due_relative_minutes").is_none());

    let style_args = checklist_task_row_style_args_json(&ChecklistTaskRowStyleSetRequest {
        checklist_uid: "chk-001".to_string(),
        task_uid: "task-001".to_string(),
        row_background_color: Some(String::new()),
        line_break_enabled: None,
        changed_by_team_member_rns_identity: None,
    });
    assert_eq!(
        style_args
            .get("row_background_color")
            .and_then(JsonValue::as_str),
        Some("")
    );

    let cell_args = checklist_task_cell_args_json(&ChecklistTaskCellSetRequest {
        checklist_uid: "chk-001".to_string(),
        task_uid: "task-001".to_string(),
        column_uid: "col-task".to_string(),
        value: "  Move to alternate pickup  ".to_string(),
        updated_by_team_member_rns_identity: None,
    });
    assert_eq!(
        cell_args.get("value").and_then(JsonValue::as_str),
        Some("  Move to alternate pickup  ")
    );
}

#[test]
fn compact_initial_checklist_row_add_stays_direct_packet_sized() {
    use lxmf::message::{
        decide_delivery, Message as LxmfMessage, MessageMethod as LxmfMessageMethod,
        TransportMethod,
    };
    use reticulum::transport::identity::PrivateIdentity;

    let status = build_status_for_tests();
    let target = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let task = ChecklistTaskRecord {
        task_uid: "tmpl-import-task-2".to_string(),
        number: 2,
        user_status: crate::types::ChecklistUserTaskStatus::Pending {},
        task_status: crate::types::ChecklistTaskStatus::Pending {},
        is_late: false,
        updated_at: Some("2026-04-24T12:00:00Z".to_string()),
        deleted_at: None,
        custom_status: None,
        due_relative_minutes: Some(30),
        due_dtg: Some("2026-04-24T12:30:00Z".to_string()),
        notes: Some(
            "Document where everyone will shelter, which room is safest, and when to leave."
                .to_string(),
        ),
        row_background_color: None,
        line_break_enabled: false,
        completed_at: None,
        completed_by_team_member_rns_identity: None,
        legacy_value: Some("Shelter room assignment".to_string()),
        cells: Vec::new(),
    };
    let args = compact_initial_checklist_task_row_add_args_from_task("chk-shelter-csv", &task);
    let (body, fields) =
        build_checklist_replication_payload(&status, &target, "checklist.task.row.add", &args)
            .expect("row-add payload");
    assert_eq!(String::from_utf8_lossy(body.as_slice()), "C C7");
    assert!(
        body.len() + fields.len() <= 260,
        "compact row-add should stay safely under direct packet budget, body={} fields={} total={}",
        body.len(),
        fields.len(),
        body.len() + fields.len()
    );

    let source = hex::decode(status.lxmf_destination_hex.as_str()).expect("source hex");
    let destination = hex::decode(target.app_destination_hex.as_str()).expect("target hex");
    let mut message = LxmfMessage::new();
    message.source_hash = Some(source.as_slice().try_into().expect("source hash"));
    message.destination_hash = Some(destination.as_slice().try_into().expect("target hash"));
    message.set_content_from_bytes(body.as_slice());
    message.fields = Some(rmp_serde::from_slice(fields.as_slice()).expect("fields msgpack"));
    let identity = PrivateIdentity::new_from_name("compact-initial-checklist-row-add");
    let signer = crate::runtime::lxmf_private_identity(&identity).expect("signer");
    let wire = message.to_wire(Some(&signer)).expect("wire");
    let decision =
        decide_delivery(TransportMethod::Direct, false, wire.len()).expect("delivery decision");
    assert_eq!(decision.representation, LxmfMessageMethod::Packet);
}
