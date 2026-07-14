#[test]
fn lxmf_delivery_announce_mapping_uses_lxmf_sdk_normalization() {
    let raw_app_data =
        encode_delivery_display_name_app_data("Alice Router").expect("encoded app data");
    let sdk_record = lxmf_sdk_announce_record_from_raw(
        "cccccccccccccccccccccccccccccccc",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        DESTINATION_KIND_LXMF_DELIVERY,
        raw_app_data.as_slice(),
        2,
        "dddddddddddddddddddddddddddddddd",
        42,
    );

    assert_eq!(sdk_record.app_data, hex::encode(raw_app_data.as_slice()));
    assert_eq!(sdk_record.display_name.as_deref(), Some("Alice Router"));

    let announce = from_lxmf_sdk_announce_record(sdk_record.clone());
    assert!(matches!(
        announce.announce_class,
        AnnounceClass::LxmfDelivery {}
    ));
    assert_eq!(announce.display_name.as_deref(), Some("Alice Router"));

    let compat = to_compat_announce_record(&sdk_record);
    assert_eq!(compat.display_name.as_deref(), Some("Alice Router"));
    assert_eq!(compat.app_data, sdk_record.app_data);
}

#[test]
fn app_announce_mapping_keeps_rem_capability_policy() {
    let sdk_record = lxmf_sdk_announce_record_from_raw(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        DESTINATION_KIND_APP,
        b"R3AKT;EMergencyMessages;Telemetry;name=Bravo+Team",
        1,
        "dddddddddddddddddddddddddddddddd",
        100,
    );

    assert!(sdk_record.display_name.is_none());

    let announce = from_lxmf_sdk_announce_record(sdk_record);
    assert!(matches!(announce.announce_class, AnnounceClass::PeerApp {}));
    assert_eq!(announce.display_name.as_deref(), Some("Bravo Team"));
}

#[test]
fn announce_metadata_accepts_text_and_msgpack_layouts() {
    let text_metadata = parse_announce_metadata("R3AKT;EMergencyMessages;name=Legacy+Team");
    let text_name = text_metadata.display_name;
    let text_tokens = text_metadata.capability_tokens;
    assert_eq!(text_name.as_deref(), Some("Legacy Team"));
    assert!(text_tokens.iter().any(|token| token == "r3akt"));
    assert!(text_tokens.iter().any(|token| token == "emergencymessages"));

    let payload = MsgPackValue::Array(vec![
        MsgPackValue::from("Msgpack Team"),
        MsgPackValue::Map(vec![(
            MsgPackValue::from("caps"),
            MsgPackValue::Array(vec![
                MsgPackValue::from("R3AKT"),
                MsgPackValue::from("EMergencyMessages"),
            ]),
        )]),
    ]);
    let encoded = rmp_serde::to_vec(&payload).expect("msgpack");
    let msgpack_hex = hex::encode(encoded);
    let msgpack_metadata = parse_announce_metadata(msgpack_hex.as_str());
    let msgpack_name = msgpack_metadata.display_name;
    let msgpack_tokens = msgpack_metadata.capability_tokens;

    assert_eq!(msgpack_name.as_deref(), Some("Msgpack Team"));
    assert!(msgpack_tokens.iter().any(|token| token == "r3akt"));
    assert!(msgpack_tokens
        .iter()
        .any(|token| token == "emergencymessages"));
    assert!(matches!(
        classify_announce(DESTINATION_KIND_APP, msgpack_hex.as_str()),
        AnnounceClass::PeerApp {}
    ));
}

#[test]
fn successful_link_marks_canonical_saved_peer_active() {
    let mut messaging = sdkmsg::MessagingStore::default();
    let now = now_ms();
    let app_destination_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let identity_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let lxmf_destination_hex = "cccccccccccccccccccccccccccccccc";

    messaging.record_announce(sdkmsg::AnnounceRecord {
        destination_hex: app_destination_hex.to_string(),
        identity_hex: identity_hex.to_string(),
        destination_kind: "app".to_string(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Peer".to_string(),
        display_name: Some("Peer".to_string()),
        hops: 1,
        interface_hex: "dddddddddddddddddddddddddddddddd".to_string(),
        received_at_ms: now,
    });
    messaging.record_resolution_result(
        app_destination_hex,
        identity_hex,
        lxmf_destination_hex,
        now,
    );
    messaging.mark_peer_saved(app_destination_hex, true);

    mark_peer_active_after_successful_link(
        &mut messaging,
        lxmf_destination_hex,
        app_destination_hex,
        now,
    );

    let peer = messaging
        .list_peers()
        .into_iter()
        .find(|peer| peer.destination_hex == app_destination_hex)
        .expect("saved app peer should be listed");
    assert!(peer.active_link);
    assert_eq!(peer.state, sdkmsg::PeerState::Connected);
}

#[test]
fn destination_send_serialization_applies_to_data_but_not_fast_lanes() {
    assert!(should_serialize_lxmf_destination_send(false, false));
    assert!(!should_serialize_lxmf_destination_send(true, false));
    assert!(!should_serialize_lxmf_destination_send(false, true));
    assert!(!should_serialize_lxmf_destination_send(true, true));
}

fn test_lxmf_report(
    metadata: MissionSyncMetadata,
    track_delivery_timeout: bool,
    used_propagation_node: bool,
) -> LxmfSendReport {
    LxmfSendReport {
        outcome: RnsSendOutcome::SentDirect,
        message_id_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        resolved_destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        metadata: Some(metadata),
        track_delivery_timeout,
        used_propagation_node,
        method: LxmfDeliveryMethod::Direct {},
        representation: LxmfDeliveryRepresentation::Packet {},
        relay_destination_hex: None,
        fallback_stage: None,
        receipt_hash_hex: None,
    }
}

fn test_pending_delivery(resend: Option<PendingLxmfResend>) -> PendingLxmfDelivery {
    PendingLxmfDelivery {
        message_id_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        correlation_id: Some("corr-timeout".to_string()),
        command_id: Some("cmd-timeout".to_string()),
        command_type: Some("mission.registry.eam.upsert".to_string()),
        event_uid: None,
        mission_uid: Some("mission-1".to_string()),
        method: LxmfDeliveryMethod::Direct {},
        representation: LxmfDeliveryRepresentation::Packet {},
        relay_destination_hex: None,
        fallback_stage: None,
        resend,
        sent_at_ms: now_ms(),
    }
}

#[tokio::test]
async fn mission_destination_locks_serialize_same_destination() {
    let locks = MissionDestinationLocks::new();
    let first = locks
        .acquire("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        .await
        .expect("first destination lock");

    let blocked = tokio::time::timeout(
        Duration::from_millis(50),
        locks.acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .await;
    assert!(
        blocked.is_err(),
        "same destination should wait for the first mission send to finish"
    );

    drop(first);
    let second = tokio::time::timeout(
        Duration::from_millis(50),
        locks.acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .await
    .expect("same destination should unblock after first send finishes")
    .expect("second destination lock");
    drop(second);
}

#[tokio::test]
async fn mission_destination_locks_allow_different_destinations() {
    let locks = MissionDestinationLocks::new();
    let _first = locks
        .acquire("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .await
        .expect("first destination lock");

    let second = tokio::time::timeout(
        Duration::from_millis(50),
        locks.acquire("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    )
    .await
    .expect("different destinations should not block each other")
    .expect("second destination lock");
    drop(second);
}

#[tokio::test]
async fn managed_peer_links_dedupe_reconnect_and_clear_on_disconnect() {
    let links = ManagedPeerLinks::default();
    let target = ManagedPeerLinkTarget {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        kind: ManagedPeerLinkKind::LxmfDelivery,
    };

    links.add_desired(target.clone()).await;

    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(target.clone())
    );
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::AlreadyReconnecting
    );

    links.finish_reconnect(&target, Ok(())).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(target.clone())
    );

    links
        .remove_desired(["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"])
        .await;
    assert_eq!(links.desired_targets().await, Vec::new());
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::NotDesired
    );
}

#[tokio::test]
async fn managed_peer_links_keep_backoff_when_target_is_readded_without_new_route_evidence() {
    let links = ManagedPeerLinks::default();
    let target = ManagedPeerLinkTarget {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        kind: ManagedPeerLinkKind::LxmfDelivery,
    };

    links.add_desired(target.clone()).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(target.clone())
    );
    links
        .finish_reconnect(&target, Err("link failed".to_string()))
        .await;

    links.add_desired(target).await;

    match links
        .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .await
    {
        ManagedPeerReconnectStart::Backoff {
            last_failure_reason,
            ..
        } => assert_eq!(last_failure_reason.as_deref(), Some("link failed")),
        other => panic!("expected backoff, got {other:?}"),
    }
}

#[tokio::test]
async fn fresh_rem_announce_clears_managed_link_backoff_for_new_connection_attempt() {
    let links = ManagedPeerLinks::default();
    let target = ManagedPeerLinkTarget {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        kind: ManagedPeerLinkKind::LxmfDelivery,
    };

    links.add_desired(target.clone()).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(target.clone())
    );
    links
        .finish_reconnect(&target, Err("link failed".to_string()))
        .await;
    match links
        .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .await
    {
        ManagedPeerReconnectStart::Backoff { .. } => {}
        other => panic!("expected backoff before fresh announce, got {other:?}"),
    }

    links
        .clear_failure("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .await;

    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(target)
    );
}

#[tokio::test]
async fn fresh_lxmf_target_replaces_app_reconnect_for_same_destination() {
    let links = ManagedPeerLinks::default();
    let app_target = ManagedPeerLinkTarget {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        kind: ManagedPeerLinkKind::App,
    };
    let lxmf_target = ManagedPeerLinkTarget {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        kind: ManagedPeerLinkKind::LxmfDelivery,
    };

    links.add_desired(app_target.clone()).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(app_target.clone())
    );

    links.add_desired(lxmf_target.clone()).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(lxmf_target.clone())
    );

    links
        .finish_reconnect(&app_target, Err("app route failed".to_string()))
        .await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::AlreadyReconnecting
    );

    links.finish_reconnect(&lxmf_target, Ok(())).await;
    assert_eq!(
        links
            .begin_reconnect("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .await,
        ManagedPeerReconnectStart::Started(lxmf_target)
    );
}

#[test]
fn direct_delivery_rejects_fresh_route_without_active_link() {
    let mut inconsistent_connected_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        false,
        Some(1),
    );
    inconsistent_connected_peer.state = sdkmsg::PeerState::Connected;

    assert!(!sdk_peer_is_directly_reachable(
        &inconsistent_connected_peer
    ));
    assert!(!sdk_peer_is_direct_delivery_ready(
        &inconsistent_connected_peer,
        true
    ));
}

#[test]
fn direct_delivery_rejects_observed_lxmf_route_for_stale_peer() {
    let stale_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        true,
        false,
        None,
    );

    assert!(sdk_peer_has_observed_lxmf_delivery_route(&stale_peer));
    assert!(!sdk_peer_is_direct_delivery_ready(&stale_peer, true));
}

#[test]
fn direct_delivery_rejects_current_app_peer_with_old_lxmf_timestamp_without_link() {
    let mut peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        false,
        Some(now_ms()),
    );
    peer.lxmf_last_seen_at_ms =
        Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

    assert!(!sdk_peer_has_observed_lxmf_delivery_route(&peer));
    assert!(!sdk_peer_is_direct_delivery_ready(&peer, true));
}

#[test]
fn direct_delivery_rejects_old_observed_lxmf_route_for_stale_peer() {
    let mut stale_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        true,
        false,
        None,
    );
    stale_peer.lxmf_last_seen_at_ms =
        Some(now_ms().saturating_sub(sdkmsg::DEFAULT_PEER_STALE_AFTER_MS + 1));

    assert!(!sdk_peer_has_observed_lxmf_delivery_route(&stale_peer));
    assert!(!sdk_peer_is_direct_delivery_ready(&stale_peer, true));
}

#[test]
fn announced_rem_lxmf_peers_are_managed_link_targets_without_save() {
    let announced_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        false,
        Some(now_ms()),
    );

    assert_eq!(
        managed_peer_link_target(&announced_peer),
        Some(ManagedPeerLinkTarget {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        })
    );
}
