#[test]
fn compact_eam_fields_derive_sender_identity_and_callsign_from_lxmf_source() {
    let source_hex = "fb4c70e20cfac047b899ca2f3671b50a";
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (MsgPackValue::from("i"), MsgPackValue::from("m:eam:1")),
            (MsgPackValue::from("t"), MsgPackValue::from("M1")),
            (
                MsgPackValue::from("a"),
                MsgPackValue::Map(vec![
                    (MsgPackValue::from("tu"), MsgPackValue::from("blue-team")),
                    (MsgPackValue::from("ss"), MsgPackValue::from("G")),
                    (MsgPackValue::from("ca"), MsgPackValue::from("Y")),
                    (MsgPackValue::from("pr"), MsgPackValue::from("G")),
                    (MsgPackValue::from("me"), MsgPackValue::from("G")),
                    (MsgPackValue::from("mo"), MsgPackValue::from("G")),
                    (MsgPackValue::from("co"), MsgPackValue::from("Y")),
                ]),
            ),
        ])]),
    )]);
    let bytes = rmp_serde::to_vec(&fields).expect("fields");

    let action = eam_command_action_from_fields(
        bytes.as_slice(),
        1_700_000_000_000,
        Some(source_hex),
        Some("Pixelcorvo"),
    )
    .expect("compact eam should parse");

    let EamCommandAction::Upsert(record) = action else {
        panic!("expected EAM upsert");
    };
    assert_eq!(record.callsign, "Pixelcorvo");
    assert_eq!(record.team_member_uid.as_deref(), Some(source_hex));
    assert_eq!(record.team_uid.as_deref(), Some("blue-team"));
    assert_eq!(
        record
            .source
            .as_ref()
            .map(|source| source.rns_identity.as_str()),
        Some(source_hex)
    );
    assert_eq!(
        record
            .source
            .as_ref()
            .and_then(|source| source.display_name.as_deref()),
        Some("Pixelcorvo")
    );
    assert!(record.notes.is_none());
}

#[test]
fn operational_ack_is_only_built_for_inbound_commands() {
    let metadata = MissionSyncMetadata {
        command_present: true,
        command_id: Some("cmd-accepted".to_string()),
        correlation_id: Some("corr-accepted".to_string()),
        command_type: Some("mission.registry.eam.upsert".to_string()),
        ..MissionSyncMetadata::default()
    };

    let ack = operational_ack_from_metadata(
        Some("ABCDEF0123456789ABCDEF0123456789"),
        Some(&metadata),
    )
    .expect("command metadata should produce ack request");

    assert_eq!(ack.destination_hex, "abcdef0123456789abcdef0123456789");
    assert_eq!(ack.command_id, "cmd-accepted");
    assert_eq!(ack.correlation_id.as_deref(), Some("corr-accepted"));
    assert_eq!(
        ack.command_type.as_deref(),
        Some("mission.registry.eam.upsert")
    );

    let result_metadata = MissionSyncMetadata {
        result_present: true,
        command_id: Some("cmd-accepted".to_string()),
        result_status: Some("accepted".to_string()),
        ..MissionSyncMetadata::default()
    };
    assert!(operational_ack_from_metadata(
        Some("abcdef0123456789abcdef0123456789"),
        Some(&result_metadata),
    )
    .is_none());

    let missing_id = MissionSyncMetadata {
        command_present: true,
        command_type: Some("checklist.create.online".to_string()),
        ..MissionSyncMetadata::default()
    };
    assert!(operational_ack_from_metadata(
        Some("abcdef0123456789abcdef0123456789"),
        Some(&missing_id),
    )
    .is_none());
}

#[test]
fn operational_ack_fields_use_existing_accepted_result_shape() {
    let ack = OperationalAck {
        destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
        command_id: "cmd-result-shape".to_string(),
        correlation_id: Some("corr-result-shape".to_string()),
        command_type: Some("checklist.task.status.set".to_string()),
    };

    let fields = build_operational_ack_fields(&ack, "0123456789abcdef0123456789abcdef")
        .expect("ack fields");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

    assert!(metadata.result_present);
    assert!(!metadata.command_present);
    assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
    assert_eq!(metadata.command_id.as_deref(), Some("cmd-result-shape"));
    assert_eq!(
        metadata.correlation_id.as_deref(),
        Some("corr-result-shape")
    );
}

#[test]
fn compact_operational_ack_fields_keep_result_tracking_metadata() {
    let ack = OperationalAck {
        destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
        command_id: "cmd-checklist-task-status-set-chk-operational-ack-task-operational-ack-abcdef01-1779627082723".to_string(),
        correlation_id: Some(
            "checklist-task-status-set-chk-operational-ack-task-operational-ack-abcdef01-1779627082723"
                .to_string(),
        ),
        command_type: Some("checklist.task.status.set".to_string()),
    };

    let fields = build_compact_operational_ack_fields(&ack).expect("ack fields");
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

    assert!(metadata.result_present);
    assert!(!metadata.command_present);
    assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
    assert_eq!(
        metadata.command_id.as_deref(),
        Some(ack.command_id.as_str())
    );
    assert!(metadata.correlation_id.is_none());
}

#[test]
fn compact_event_operational_ack_fields_use_event_uid_tracking_metadata() {
    let ack = OperationalAck {
        destination_hex: "abcdef0123456789abcdef0123456789".to_string(),
        command_id: "log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe".to_string(),
        correlation_id: Some("log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe".to_string()),
        command_type: Some("mission.registry.log_entry.upsert".to_string()),
    };

    let fields = build_compact_operational_ack_fields(&ack).expect("ack fields");
    assert!(
        fields.len() < 32,
        "compact event ack fields were {} bytes",
        fields.len()
    );
    let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("metadata");

    assert!(metadata.result_present);
    assert!(!metadata.command_present);
    assert_eq!(metadata.result_status.as_deref(), Some("accepted"));
    assert_eq!(
        metadata.event_uid.as_deref(),
        Some("evt-984bfa16-cfe3-430a-a201-3294310a91fe")
    );
    assert_eq!(
        metadata.command_id.as_deref(),
        Some("log-entry-evt-984bfa16-cfe3-430a-a201-3294310a91fe")
    );
    assert!(metadata.correlation_id.is_none());
}

#[test]
fn accepted_result_metadata_is_identified_for_direct_ack_return() {
    let accepted = MissionSyncMetadata {
        result_present: true,
        result_status: Some("accepted".to_string()),
        ..MissionSyncMetadata::default()
    };
    assert!(is_accepted_result_metadata(Some(&accepted)));

    let command = MissionSyncMetadata {
        command_present: true,
        command_type: Some("checklist.task.status.set".to_string()),
        ..MissionSyncMetadata::default()
    };
    assert!(!is_accepted_result_metadata(Some(&command)));
}

#[test]
fn mission_result_metadata_maps_application_ack_states() {
    for (status, expected) in [
        ("accepted", ApplicationAckState::Accepted {}),
        ("completed", ApplicationAckState::Completed {}),
        ("rejected", ApplicationAckState::Rejected {}),
        ("failed", ApplicationAckState::Failed {}),
    ] {
        let metadata = MissionSyncMetadata {
            result_present: true,
            result_status: Some(status.to_string()),
            ..MissionSyncMetadata::default()
        };

        assert_eq!(
            application_ack_state_for_mission_metadata(&metadata),
            expected
        );
    }
}

#[test]
fn chat_delivery_ack_body_round_trips_message_id() {
    let message_id = "482ecb36f44826e45aea88562e6ebda4a66d30575eb42557732adced08e0db7d";
    let body = chat_delivery_ack_body(message_id);

    assert_eq!(
        parse_chat_delivery_ack_body(body.as_str()),
        Some(message_id.to_string())
    );
    assert_eq!(
        parse_chat_delivery_ack_body("REM_DELIVERY_ACK:not-hex"),
        None
    );
    assert_eq!(parse_chat_delivery_ack_body("regular chat"), None);
}

fn propagation_announce(
    destination_hex: &str,
    hops: u8,
    received_at_ms: u64,
) -> sdkmsg::AnnounceRecord {
    sdkmsg::AnnounceRecord {
        destination_hex: destination_hex.to_string(),
        identity_hex: format!("id-{destination_hex}"),
        destination_kind: "lxmf_propagation".to_string(),
        app_data: String::new(),
        display_name: None,
        hops,
        interface_hex: String::new(),
        received_at_ms,
    }
}

#[test]
fn event_projection_from_trimmed_fields_uses_lxmf_body_content() {
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_type"),
                MsgPackValue::from("mission.registry.log_entry.upsert"),
            ),
            (
                MsgPackValue::from("source"),
                MsgPackValue::Map(vec![(
                    MsgPackValue::from("rns_identity"),
                    MsgPackValue::from("identity-1"),
                )]),
            ),
            (
                MsgPackValue::from("args"),
                MsgPackValue::Map(vec![
                    (MsgPackValue::from("entry_uid"), MsgPackValue::from("evt-1")),
                    (
                        MsgPackValue::from("mission_uid"),
                        MsgPackValue::from("mission-1"),
                    ),
                    (MsgPackValue::from("callsign"), MsgPackValue::from("Pixel")),
                ]),
            ),
        ])]),
    )]);
    let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

    let record = event_projection_from_fields(
        &bytes,
        Some(b"P01"),
        None,
        Some("Pixel"),
        1_700_000_000_000,
    )
    .expect("event projection");

    assert_eq!(record.uid, "evt-1");
    assert_eq!(record.command_id, "log-entry-evt-1");
    assert_eq!(record.command_type, "mission.registry.log_entry.upsert");
    assert_eq!(record.mission_uid, "mission-1");
    assert_eq!(record.content, "MECP/2/P01");
    assert_eq!(record.callsign, "Pixel");
    assert_eq!(record.source_identity, "identity-1");
    assert_eq!(record.source_display_name.as_deref(), Some("Pixel"));
    assert_eq!(record.keywords, Vec::<String>::new());
    assert_eq!(record.content_hashes, Vec::<String>::new());
    assert_eq!(record.topics, vec!["mission-1".to_string()]);
}

#[test]
fn prune_expired_buffered_acknowledgements_removes_only_stale_entries() {
    let now = now_ms();
    let mut pending = HashMap::from([
        (
            "fresh".to_string(),
            PendingLxmfAcknowledgement {
                source_hex: "src-fresh".to_string(),
                detail: None,
                application_ack_state: ApplicationAckState::Accepted {},
                buffered_at_ms: now,
            },
        ),
        (
            "stale".to_string(),
            PendingLxmfAcknowledgement {
                source_hex: "src-stale".to_string(),
                detail: None,
                application_ack_state: ApplicationAckState::Accepted {},
                buffered_at_ms: now
                    .saturating_sub(
                        crate::numeric::u128_to_u64_saturating(
                            DEFAULT_BUFFERED_ACK_TTL.as_millis(),
                        ) + 1,
                    ),
            },
        ),
    ]);

    let pruned = prune_expired_buffered_acknowledgements(&mut pending, now);

    assert_eq!(pruned, 1);
    assert!(pending.contains_key("fresh"));
    assert!(!pending.contains_key("stale"));
}

#[test]
fn prune_expired_receipt_tracking_removes_only_stale_entries() {
    let now = now_ms();
    let mut tracking = HashMap::from([
        (
            "fresh".to_string(),
            ReceiptMessageTracking {
                message_id_hex: "msg-fresh".to_string(),
                recorded_at_ms: now,
            },
        ),
        (
            "stale".to_string(),
            ReceiptMessageTracking {
                message_id_hex: "msg-stale".to_string(),
                recorded_at_ms: now
                    .saturating_sub(
                        crate::numeric::u128_to_u64_saturating(
                            DEFAULT_RECEIPT_TRACKING_TTL.as_millis(),
                        ) + 1,
                    ),
            },
        ),
    ]);

    let pruned = prune_expired_receipt_tracking(&mut tracking, now);

    assert_eq!(pruned, 1);
    assert!(tracking.contains_key("fresh"));
    assert!(!tracking.contains_key("stale"));
}

#[tokio::test]
async fn retry_backoff_releases_general_send_permit_before_sleep() {
    let permits = SendTaskPermits::with_limits(1, 1);
    let permits_for_retry = permits.clone();
    let (sleeping_tx, sleeping_rx) = oneshot::channel();

    tokio::spawn(async move {
        {
            let _permit = acquire_send_task_permit(&permits_for_retry, SendTaskClass::General)
                .await
                .expect("first attempt permit");
        }
        let _ = sleeping_tx.send(());
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    sleeping_rx.await.expect("retry task entered backoff");
    let permit = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::General),
    )
    .await
    .expect("general permit should be available during retry sleep")
    .expect("general permit acquisition should succeed");
    drop(permit);
}

#[tokio::test]
async fn mission_sends_keep_reserved_capacity_when_general_pool_is_full() {
    let permits = SendTaskPermits::with_limits(1, 1);
    let _general = acquire_send_task_permit(&permits, SendTaskClass::General)
        .await
        .expect("saturate general pool");

    let mission = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::Mission),
    )
    .await
    .expect("mission permit should not wait on general pool saturation")
    .expect("mission permit acquisition should succeed");
    drop(mission);

    let blocked_general = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::General),
    )
    .await;
    assert!(
        blocked_general.is_err(),
        "general pool should remain saturated while the original permit is held"
    );
}

#[test]
fn direct_recovery_fallback_uses_dedicated_recovery_lane() {
    assert_eq!(
        SendTaskClass::Mission.direct_recovery_equivalent(),
        SendTaskClass::MissionRecovery
    );
    assert_eq!(
        SendTaskClass::MissionAck.direct_recovery_equivalent(),
        SendTaskClass::MissionRecovery
    );
    assert_eq!(
        SendTaskClass::MissionPropagation.direct_recovery_equivalent(),
        SendTaskClass::MissionRecovery
    );
    assert_eq!(
        SendTaskClass::MissionRecovery.direct_recovery_equivalent(),
        SendTaskClass::MissionRecovery
    );
    assert_eq!(
        SendTaskClass::General.direct_recovery_equivalent(),
        SendTaskClass::General
    );
}

#[test]
fn accepted_result_sends_use_dedicated_ack_lane() {
    let metadata = MissionSyncMetadata {
        result_present: true,
        result_status: Some("accepted".to_string()),
        command_id: Some("cmd-accepted".to_string()),
        ..MissionSyncMetadata::default()
    };

    assert_eq!(
        SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::Auto {}),
        SendTaskClass::MissionAck
    );
}
