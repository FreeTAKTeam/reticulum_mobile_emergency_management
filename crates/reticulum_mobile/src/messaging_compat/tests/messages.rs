#[test]
fn mission_capability_check_accepts_msgpack_hex_app_data() {
    let payload = rmpv::Value::Array(vec![
        rmpv::Value::from("Msgpack Peer"),
        rmpv::Value::Map(vec![(
            rmpv::Value::from("caps"),
            rmpv::Value::Array(vec![
                rmpv::Value::from("R3AKT"),
                rmpv::Value::from("EMergencyMessages"),
            ]),
        )]),
    ]);
    let app_data = hex::encode(rmp_serde::to_vec(&payload).expect("msgpack"));

    assert!(supports_mission_traffic(Some(app_data.as_str())));
}

#[test]
fn peer_identity_and_last_seen_come_from_rem_lxmf_delivery_announce() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(60_000),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });
    store.mark_peer_saved("lxmfdest", true);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].lxmf_destination_hex.as_deref(), Some("lxmfdest"));
    assert_eq!(peers[0].last_seen_at_ms, now);
    assert_eq!(peers[0].announce_last_seen_at_ms, Some(now));
    assert_eq!(peers[0].lxmf_last_seen_at_ms, Some(now));
    assert_eq!(
        peers[0].app_data.as_deref(),
        Some("R3AKT,EMergencyMessages,Telemetry;name=Alice")
    );
}

#[test]
fn conversation_projection_uses_lxmf_destination_for_peer_lookup() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: None,
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });
    store.upsert_message(MessageRecord {
        message_id_hex: "msg".into(),
        conversation_id: "lxmfdest".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "lxmfdest".into(),
        source_hex: None,
        requested_destination_hex: Some("lxmfdest".into()),
        delivery_destination_hex: Some("lxmfdest".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("msg".into()),
        title: None,
        body_utf8: "hello".into(),
        method: MessageMethod::Direct,
        state: MessageState::Delivered,
        transport_state: TransportDeliveryState::TransportDelivered,
        application_ack_state: ApplicationAckState::Accepted,
        detail: None,
        sent_at_ms: Some(30),
        received_at_ms: None,
        updated_at_ms: now,
    });

    let conversations = store.list_conversations();
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].peer_display_name.as_deref(), Some("Alice"));
}

#[test]
fn delete_conversation_messages_removes_matching_alias_thread() {
    let mut store = MessagingStore::default();
    store.upsert_message(MessageRecord {
        message_id_hex: "outbound".into(),
        conversation_id: "identity".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "appdest".into(),
        source_hex: None,
        requested_destination_hex: Some("appdest".into()),
        delivery_destination_hex: Some("appdest".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("outbound".into()),
        title: None,
        body_utf8: "hello".into(),
        method: MessageMethod::Direct,
        state: MessageState::Delivered,
        transport_state: TransportDeliveryState::TransportDelivered,
        application_ack_state: ApplicationAckState::Accepted,
        detail: None,
        sent_at_ms: Some(10),
        received_at_ms: None,
        updated_at_ms: 10,
    });
    store.upsert_message(MessageRecord {
        message_id_hex: "inbound".into(),
        conversation_id: "identity".into(),
        direction: MessageDirection::Inbound,
        destination_hex: "local".into(),
        source_hex: Some("lxmfdest".into()),
        requested_destination_hex: Some("lxmfdest".into()),
        delivery_destination_hex: Some("local".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("inbound".into()),
        title: None,
        body_utf8: "copy".into(),
        method: MessageMethod::Direct,
        state: MessageState::Received,
        transport_state: TransportDeliveryState::TransportDelivered,
        application_ack_state: ApplicationAckState::NotRequired,
        detail: None,
        sent_at_ms: None,
        received_at_ms: Some(20),
        updated_at_ms: 20,
    });
    store.upsert_message(MessageRecord {
        message_id_hex: "unrelated".into(),
        conversation_id: "other".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "other".into(),
        source_hex: None,
        requested_destination_hex: Some("other".into()),
        delivery_destination_hex: Some("other".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("unrelated".into()),
        title: None,
        body_utf8: "keep".into(),
        method: MessageMethod::Direct,
        state: MessageState::Delivered,
        transport_state: TransportDeliveryState::TransportDelivered,
        application_ack_state: ApplicationAckState::Accepted,
        detail: None,
        sent_at_ms: Some(30),
        received_at_ms: None,
        updated_at_ms: 30,
    });

    assert!(store.delete_conversation_messages(["appdest", "lxmfdest", "identity"]));

    let remaining = store.list_messages(None);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].message_id_hex, "unrelated");
    assert_eq!(store.list_conversations().len(), 1);
}

#[test]
fn last_seen_comes_from_rem_lxmf_delivery_announces() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(40),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Poco".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(10));
    assert_eq!(
        peers[0].announce_last_seen_at_ms,
        Some(now.saturating_sub(10))
    );
    assert_eq!(peers[0].lxmf_last_seen_at_ms, Some(now.saturating_sub(10)));
}

#[test]
fn transport_receipt_does_not_mark_application_ack_accepted() {
    let mut store = MessagingStore::default();
    store.upsert_message(MessageRecord {
        message_id_hex: "msg".into(),
        conversation_id: "peer".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "peer".into(),
        source_hex: None,
        requested_destination_hex: Some("peer".into()),
        delivery_destination_hex: Some("peer".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("wire-1".into()),
        title: None,
        body_utf8: "hello".into(),
        method: MessageMethod::Direct,
        state: MessageState::SentDirect,
        transport_state: TransportDeliveryState::SentDirect,
        application_ack_state: ApplicationAckState::Waiting,
        detail: None,
        sent_at_ms: Some(10),
        received_at_ms: None,
        updated_at_ms: 10,
    });

    let updated = store
        .update_message_delivery_state(
            "msg",
            None,
            Some(TransportDeliveryState::TransportDelivered),
            None,
            Some("transport receipt".to_string()),
            None,
            20,
        )
        .expect("message updated");

    assert_eq!(updated.state, MessageState::SentDirect);
    assert_eq!(
        updated.transport_state,
        TransportDeliveryState::TransportDelivered
    );
    assert_eq!(updated.application_ack_state, ApplicationAckState::Waiting);
}

#[test]
fn chat_ack_marks_application_ack_accepted() {
    let mut store = MessagingStore::default();
    store.upsert_message(MessageRecord {
        message_id_hex: "msg".into(),
        conversation_id: "peer".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "peer".into(),
        source_hex: None,
        requested_destination_hex: Some("peer".into()),
        delivery_destination_hex: Some("peer".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("wire-1".into()),
        title: None,
        body_utf8: "hello".into(),
        method: MessageMethod::Direct,
        state: MessageState::SentDirect,
        transport_state: TransportDeliveryState::SentDirect,
        application_ack_state: ApplicationAckState::Waiting,
        detail: None,
        sent_at_ms: Some(10),
        received_at_ms: None,
        updated_at_ms: 10,
    });

    let updated = store
        .update_message_delivery_state(
            "msg",
            Some(MessageState::Delivered),
            Some(TransportDeliveryState::TransportDelivered),
            Some(ApplicationAckState::Accepted),
            Some("chat delivery ack".to_string()),
            None,
            20,
        )
        .expect("message updated");

    assert_eq!(updated.state, MessageState::Delivered);
    assert_eq!(
        updated.transport_state,
        TransportDeliveryState::TransportDelivered
    );
    assert_eq!(updated.application_ack_state, ApplicationAckState::Accepted);
}

#[test]
fn retry_chat_ack_updates_original_record_by_wire_message_id() {
    let mut store = MessagingStore::default();
    store.upsert_message(MessageRecord {
        message_id_hex: "logical-msg".into(),
        conversation_id: "peer".into(),
        direction: MessageDirection::Outbound,
        destination_hex: "peer".into(),
        source_hex: None,
        requested_destination_hex: Some("peer".into()),
        delivery_destination_hex: Some("peer".into()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some("retry-wire-msg".into()),
        title: None,
        body_utf8: "hello".into(),
        method: MessageMethod::Direct,
        state: MessageState::SentDirect,
        transport_state: TransportDeliveryState::SentDirect,
        application_ack_state: ApplicationAckState::Waiting,
        detail: None,
        sent_at_ms: Some(10),
        received_at_ms: None,
        updated_at_ms: 10,
    });

    let updated = store
        .update_message_delivery_state(
            "retry-wire-msg",
            Some(MessageState::Delivered),
            Some(TransportDeliveryState::TransportDelivered),
            Some(ApplicationAckState::Accepted),
            Some("chat delivery ack".to_string()),
            None,
            20,
        )
        .expect("message updated by wire id");

    assert_eq!(updated.message_id_hex, "logical-msg");
    assert_eq!(
        updated.last_wire_message_id_hex.as_deref(),
        Some("retry-wire-msg")
    );
    assert_eq!(updated.state, MessageState::Delivered);
    assert_eq!(
        updated.transport_state,
        TransportDeliveryState::TransportDelivered
    );
    assert_eq!(updated.application_ack_state, ApplicationAckState::Accepted);
}
