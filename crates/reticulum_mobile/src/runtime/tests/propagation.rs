#[test]
fn propagation_and_malformed_announces_keep_generic_sdk_normalization() {
    let sdk_record = lxmf_sdk_announce_record_from_raw(
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        DESTINATION_KIND_LXMF_PROPAGATION,
        &[0xff, 0xfe, 0x00],
        3,
        "dddddddddddddddddddddddddddddddd",
        200,
    );

    assert_eq!(sdk_record.app_data, "fffe00");
    assert!(sdk_record.display_name.is_none());

    let announce = from_lxmf_sdk_announce_record(sdk_record);
    assert!(matches!(
        announce.announce_class,
        AnnounceClass::PropagationNode {}
    ));
    assert_eq!(announce.app_data, "fffe00");
    assert!(announce.display_name.is_none());

    let malformed_tokens = parse_announce_metadata("fffe00").capability_tokens;
    assert!(malformed_tokens.is_empty());
}

#[test]
fn ack_timeout_auto_command_delivery_is_eligible_for_propagation_retry() {
    let metadata = MissionSyncMetadata {
        command_present: true,
        command_id: Some("cmd-timeout".to_string()),
        correlation_id: Some("corr-timeout".to_string()),
        command_type: Some("mission.registry.eam.upsert".to_string()),
        mission_uid: Some("mission-1".to_string()),
        ..MissionSyncMetadata::default()
    };
    let report = test_lxmf_report(metadata.clone(), true, false);
    let resend = build_pending_lxmf_resend(
        &report,
        "cccccccccccccccccccccccccccccccc",
        b"body",
        None,
        Some(vec![1, 2, 3]),
        Some(metadata),
        SendMode::Auto {},
        SendTaskClass::Mission,
        OutboundTrafficClass::Eam {},
    )
    .expect("auto command should retain resend payload");
    let pending = test_pending_delivery(Some(resend));

    assert!(should_retry_pending_ack_timeout_via_propagation(
        &pending, true
    ));
    assert!(!should_retry_pending_ack_timeout_via_propagation(
        &pending, false
    ));
    assert!(should_retry_pending_ack_timeout_via_direct(&pending));
}

#[test]
fn ack_timeout_retry_skips_results_direct_only_and_existing_propagation() {
    let command_metadata = MissionSyncMetadata {
        command_present: true,
        command_id: Some("cmd-timeout".to_string()),
        correlation_id: Some("corr-timeout".to_string()),
        command_type: Some("checklist.create.online".to_string()),
        ..MissionSyncMetadata::default()
    };
    let result_metadata = MissionSyncMetadata {
        result_present: true,
        command_id: Some("cmd-timeout".to_string()),
        correlation_id: Some("corr-timeout".to_string()),
        result_status: Some("accepted".to_string()),
        ..MissionSyncMetadata::default()
    };

    let command_report = test_lxmf_report(command_metadata.clone(), true, false);
    assert!(build_pending_lxmf_resend(
        &command_report,
        "cccccccccccccccccccccccccccccccc",
        b"body",
        None,
        Some(vec![1, 2, 3]),
        Some(command_metadata.clone()),
        SendMode::DirectOnly {},
        SendTaskClass::Mission,
        OutboundTrafficClass::Checklist {},
    )
    .is_none());

    let propagation_report = test_lxmf_report(command_metadata.clone(), true, true);
    assert!(build_pending_lxmf_resend(
        &propagation_report,
        "cccccccccccccccccccccccccccccccc",
        b"body",
        None,
        Some(vec![1, 2, 3]),
        Some(command_metadata),
        SendMode::Auto {},
        SendTaskClass::Mission,
        OutboundTrafficClass::Checklist {},
    )
    .is_none());

    let result_report = test_lxmf_report(result_metadata.clone(), false, false);
    assert!(build_pending_lxmf_resend(
        &result_report,
        "cccccccccccccccccccccccccccccccc",
        b"body",
        None,
        Some(vec![1, 2, 3]),
        Some(result_metadata),
        SendMode::Auto {},
        SendTaskClass::Mission,
        OutboundTrafficClass::Control {},
    )
    .is_none());

    let attempted = PendingLxmfResend {
        requested_destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        body: b"body".to_vec(),
        title: None,
        fields_bytes: Some(vec![1, 2, 3]),
        metadata: MissionSyncMetadata {
            command_present: true,
            command_id: Some("cmd-timeout".to_string()),
            correlation_id: Some("corr-timeout".to_string()),
            command_type: Some("checklist.create.online".to_string()),
            ..MissionSyncMetadata::default()
        },
        send_task_class: SendTaskClass::Mission,
        traffic_class: OutboundTrafficClass::Checklist {},
        original_send_mode: SendMode::Auto {},
        direct_ack_retry_attempted: true,
        propagation_fallback_attempted: true,
    };
    assert!(!should_retry_pending_ack_timeout_via_propagation(
        &test_pending_delivery(Some(attempted)),
        true,
    ));
}

#[test]
fn propagated_pending_deliveries_keep_waiting_for_late_acknowledgements() {
    let now = now_ms();
    let mut direct = test_pending_delivery(None);
    direct.sent_at_ms = now.saturating_sub(crate::numeric::u128_to_u64_saturating(
        DEFAULT_LXMF_ACK_TIMEOUT.as_millis(),
    ));
    assert!(pending_ack_timeout_elapsed(&direct, now));

    let mut propagated = direct.clone();
    propagated.method = LxmfDeliveryMethod::Propagated {};
    propagated.relay_destination_hex = Some("cccccccccccccccccccccccccccccccc".to_string());
    assert!(!pending_ack_timeout_elapsed(&propagated, now));

    propagated.sent_at_ms = now.saturating_sub(crate::numeric::u128_to_u64_saturating(
        PROPAGATED_LXMF_ACK_TIMEOUT.as_millis(),
    ));
    assert!(pending_ack_timeout_elapsed(&propagated, now));
}

#[test]
fn propagation_fallback_pending_deliveries_keep_waiting_for_late_acknowledgements() {
    let now = now_ms();
    let mut propagated = test_pending_delivery(Some(PendingLxmfResend {
        requested_destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        body: b"body".to_vec(),
        title: None,
        fields_bytes: Some(vec![1, 2, 3]),
        metadata: MissionSyncMetadata {
            command_present: true,
            command_id: Some("cmd-timeout".to_string()),
            correlation_id: Some("corr-timeout".to_string()),
            command_type: Some("sos.status".to_string()),
            ..MissionSyncMetadata::default()
        },
        send_task_class: SendTaskClass::Mission,
        traffic_class: OutboundTrafficClass::Sos {},
        original_send_mode: SendMode::Auto {},
        direct_ack_retry_attempted: true,
        propagation_fallback_attempted: true,
    }));
    propagated.method = LxmfDeliveryMethod::Propagated {};
    propagated.relay_destination_hex = Some("dddddddddddddddddddddddddddddddd".to_string());
    propagated.sent_at_ms = now.saturating_sub(crate::numeric::u128_to_u64_saturating(
        DEFAULT_LXMF_ACK_TIMEOUT.as_millis(),
    ));

    assert!(!pending_ack_timeout_elapsed(&propagated, now));
    propagated.sent_at_ms = now.saturating_sub(crate::numeric::u128_to_u64_saturating(
        PROPAGATED_LXMF_ACK_TIMEOUT.as_millis(),
    ));
    assert!(pending_ack_timeout_elapsed(&propagated, now));
}

#[test]
fn propagation_auto_selection_keeps_current_equal_hop_relay_stable() {
    let current = propagation_announce("11111111111111111111111111111111", 1, 100);
    let newer_equal_hop = propagation_announce("22222222222222222222222222222222", 1, 200);
    let lower_hop = propagation_announce("33333333333333333333333333333333", 0, 300);

    let stable_choice = [&current, &newer_equal_hop]
        .into_iter()
        .min_by_key(|record| {
            propagation_candidate_sort_key(
                record,
                None,
                Some("11111111111111111111111111111111"),
            )
        })
        .expect("stable relay");
    assert_eq!(stable_choice.destination_hex, current.destination_hex);

    let lower_hop_choice = [&current, &lower_hop]
        .into_iter()
        .min_by_key(|record| {
            propagation_candidate_sort_key(
                record,
                None,
                Some("11111111111111111111111111111111"),
            )
        })
        .expect("lower hop relay");
    assert_eq!(lower_hop_choice.destination_hex, lower_hop.destination_hex);
}

#[test]
fn propagation_auto_selection_prefers_fresh_equal_hop_relay_without_current() {
    let stale_equal_hop = propagation_announce("11111111111111111111111111111111", 1, 100);
    let fresh_equal_hop = propagation_announce("22222222222222222222222222222222", 1, 200);

    let choice = [&stale_equal_hop, &fresh_equal_hop]
        .into_iter()
        .min_by_key(|record| propagation_candidate_sort_key(record, None, None))
        .expect("fresh relay");

    assert_eq!(choice.destination_hex, fresh_equal_hop.destination_hex);
}

#[test]
fn propagation_sync_candidates_include_active_then_alternate_relays() {
    let active = "11111111111111111111111111111111";
    let announces = vec![
        propagation_announce(active, 1, 200),
        propagation_announce("22222222222222222222222222222222", 1, 100),
        propagation_announce("33333333333333333333333333333333", 2, 50),
        sdkmsg::AnnounceRecord {
            destination_hex: "44444444444444444444444444444444".to_string(),
            identity_hex: "non-propagation".to_string(),
            destination_kind: "lxmf_delivery".to_string(),
            app_data: String::new(),
            display_name: None,
            hops: 0,
            interface_hex: String::new(),
            received_at_ms: 1,
        },
    ];

    let candidates = propagation_sync_candidate_relays(announces.as_slice(), active, None);

    assert_eq!(
        candidates,
        vec![
            active.to_string(),
            "22222222222222222222222222222222".to_string(),
            "33333333333333333333333333333333".to_string(),
        ]
    );
}

#[tokio::test]
async fn propagation_mission_sends_do_not_block_direct_mission_capacity() {
    let permits = SendTaskPermits::with_limits(1, 1);
    let _propagation = acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation)
        .await
        .expect("saturate propagation mission pool");

    let direct = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::Mission),
    )
    .await
    .expect("direct mission permit should not wait on propagation pool saturation")
    .expect("direct mission permit acquisition should succeed");
    drop(direct);

    let blocked_propagation = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation),
    )
    .await;
    assert!(
        blocked_propagation.is_err(),
        "propagation mission pool should remain saturated while the original permit is held"
    );
}

#[test]
fn connected_auto_send_keeps_direct_retry_budget_even_with_relay() {
    assert_eq!(
        direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, false, None),
        LXMF_DIRECT_ATTEMPTS
    );
    assert_eq!(
        direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, true, Some(11)),
        LXMF_DIRECT_ATTEMPTS
    );
    assert_eq!(
        direct_attempt_budget_for_send(SendMode::Auto {}, true, true, false, true, Some(1)),
        LXMF_DIRECT_ATTEMPTS
    );
    assert_eq!(
        direct_attempt_budget_for_send(SendMode::Auto {}, false, true, false, false, Some(11)),
        LXMF_DIRECT_ATTEMPTS
    );
    assert_eq!(
        direct_attempt_budget_for_send(
            SendMode::DirectOnly {},
            true,
            true,
            false,
            false,
            Some(11)
        ),
        LXMF_DIRECT_ATTEMPTS
    );
}

#[test]
fn inbound_correspondent_without_current_route_uses_relay_without_direct_probe() {
    assert!(should_skip_direct_for_inbound_correspondent(
        SendMode::Auto {},
        true,
        true,
        false,
    ));
    assert!(!should_skip_direct_for_inbound_correspondent(
        SendMode::Auto {},
        true,
        true,
        true,
    ));
    assert!(!should_skip_direct_for_inbound_correspondent(
        SendMode::DirectOnly {},
        true,
        true,
        false,
    ));
    assert!(!should_skip_direct_for_inbound_correspondent(
        SendMode::Auto {},
        true,
        false,
        false,
    ));
}

#[test]
fn only_inbound_history_authorizes_correspondent_reply_routing() {
    let destination = "77777777777777777777777777777777";
    let mut destinations = HashSet::new();
    destinations.insert(destination.to_string());
    let inbound = MessageRecord {
        message_id_hex: "inbound-message".to_string(),
        conversation_id: destination.to_string(),
        direction: MessageDirection::Inbound {},
        destination_hex: destination.to_string(),
        source_hex: Some(destination.to_string()),
        requested_destination_hex: Some(destination.to_string()),
        delivery_destination_hex: Some(destination.to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("inbound-wire-message".to_string()),
        title: None,
        body_utf8: "hello".to_string(),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Received {},
        transport_state: TransportDeliveryState::TransportDelivered {},
        application_ack_state: ApplicationAckState::NotRequired {},
        detail: None,
        sent_at_ms: None,
        received_at_ms: Some(1),
        updated_at_ms: 1,
    };
    let outbound = MessageRecord {
        direction: MessageDirection::Outbound {},
        ..inbound.clone()
    };

    assert!(delivery_policy::inbound_message_matches_destinations(
        &inbound,
        &destinations
    ));
    assert!(!delivery_policy::inbound_message_matches_destinations(
        &outbound,
        &destinations
    ));
    assert!(!delivery_policy::inbound_message_matches_destinations(
        &inbound,
        &HashSet::from(["88888888888888888888888888888888".to_string()]),
    ));
}

#[test]
fn high_hop_stale_saved_route_prefers_propagation_lane() {
    let mut stale_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        true,
        false,
        None,
    );
    stale_peer.saved = true;
    let mut current_peer = send_peer(
        "dddddddddddddddddddddddddddddddd",
        Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        Some("ffffffffffffffffffffffffffffffff"),
        false,
        false,
        Some(now_ms()),
    );
    current_peer.saved = true;
    let mut active_peer = send_peer(
        "11111111111111111111111111111111",
        Some("22222222222222222222222222222222"),
        Some("33333333333333333333333333333333"),
        false,
        true,
        Some(now_ms()),
    );
    active_peer.saved = true;

    assert!(saved_peer_stored_route_prefers_propagation(
        &stale_peer,
        true,
        Some(11),
    ));
    assert!(!saved_peer_stored_route_prefers_propagation(
        &stale_peer,
        true,
        Some(1),
    ));
    assert!(saved_peer_stored_route_prefers_propagation(
        &current_peer,
        true,
        Some(11),
    ));
    assert!(!saved_peer_stored_route_prefers_propagation(
        &active_peer,
        true,
        Some(11),
    ));
    assert!(!saved_peer_stored_route_prefers_propagation(
        &stale_peer,
        false,
        Some(11),
    ));
}
