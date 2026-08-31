#[test]
fn connected_chat_routing_allows_only_confirmed_inbound_correspondents() {
    let requested_destination = "77777777777777777777777777777777";
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );
    let mut snapshot = HubDirectorySnapshot::yellow_only(123);
    snapshot.schema_version = HUB_DIRECTORY_SCHEMA_VERSION;
    snapshot.caller_memberships = vec![crate::types::HubCallerMembershipRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        team_member_uid: "caller-member".to_string(),
    }];
    snapshot.members = vec![crate::types::HubTeamMemberRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        team_member_uid: "peer-member".to_string(),
        identity: "78787878787878787878787878787878".to_string(),
        destination_hash: "abababababababababababababababab".to_string(),
        display_name: Some("REM peer".to_string()),
        announce_capabilities: vec!["r3akt".to_string()],
        client_type: Some("rem".to_string()),
        registered_mode: Some("connected".to_string()),
        last_seen: None,
        status: Some("active".to_string()),
    }];

    assert_eq!(
        routed_chat_destination_hex(
            "abababababababababababababababab".to_string(),
            Some(&config),
            Some(&snapshot),
            || panic!("normal connected routing must not load fallback context"),
        )
        .expect("active-team peer routes through the hub"),
        "56565656565656565656565656565656"
    );

    let inbound = MessageRecord {
        message_id_hex: "inbound-message".to_string(),
        conversation_id: "66666666666666666666666666666666".to_string(),
        direction: MessageDirection::Inbound {},
        destination_hex: "55555555555555555555555555555555".to_string(),
        source_hex: Some(requested_destination.to_string()),
        requested_destination_hex: None,
        delivery_destination_hex: None,
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("inbound-wire-message".to_string()),
        title: None,
        body_utf8: "hello from another LXMF client".to_string(),
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

    assert_eq!(
        routed_chat_destination_hex(
            requested_destination.to_string(),
            Some(&config),
            Some(&snapshot),
            || Ok((vec![inbound.clone()], Vec::new())),
        )
        .expect("persisted inbound correspondent keeps its direct LXMF destination"),
        requested_destination
    );

    let outbound = MessageRecord {
        direction: MessageDirection::Outbound {},
        ..inbound
    };
    assert!(routed_chat_destination_hex(
        requested_destination.to_string(),
        Some(&config),
        Some(&snapshot),
        || Ok((vec![outbound], Vec::new())),
    )
    .is_err());
}

#[test]
fn connected_chat_routing_authorizes_inbound_correspondent_aliases() {
    let app_destination = "88888888888888888888888888888888";
    let lxmf_destination = "77777777777777777777777777777777";
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );
    let peer = build_peer_record(app_destination, lxmf_destination, false, false, false);
    let inbound = MessageRecord {
        message_id_hex: "inbound-alias-message".to_string(),
        conversation_id: lxmf_destination.to_string(),
        direction: MessageDirection::Inbound {},
        destination_hex: lxmf_destination.to_string(),
        source_hex: Some(lxmf_destination.to_string()),
        requested_destination_hex: Some(lxmf_destination.to_string()),
        delivery_destination_hex: Some(lxmf_destination.to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("inbound-alias-wire-message".to_string()),
        title: None,
        body_utf8: "hello from an aliased LXMF correspondent".to_string(),
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

    assert_eq!(
        routed_chat_destination_hex(
            app_destination.to_string(),
            Some(&config),
            None,
            || Ok((vec![inbound], vec![peer.clone()])),
        )
        .expect("the app alias resolves to the confirmed inbound LXMF correspondent"),
        app_destination
    );
    assert!(routed_chat_destination_hex(
        app_destination.to_string(),
        Some(&config),
        None,
        || Ok((vec![outbound], vec![peer])),
    )
    .is_err());
}
