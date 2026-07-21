#[test]
fn direct_delivery_health_blocks_and_restores_destinations_after_cooldown() {
    let health = DirectDeliveryHealth::default();
    let destinations = [
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
    ];

    assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 100));

    health.mark_unhealthy(destinations.iter().map(String::as_str), 200);

    assert!(!health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 150));
    assert!(!health.is_available("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 150));
    assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 201));

    health.mark_unhealthy(destinations.iter().map(String::as_str), 300);
    health.clear(destinations.iter().map(String::as_str));

    assert!(health.is_available("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 250));
    assert!(health.is_available("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 250));
}

#[test]
fn managed_peer_link_targets_include_saved_and_announced_lxmf_destinations() {
    let mut saved_online = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        true,
        Some(now_ms()),
    );
    saved_online.saved = true;
    let mut saved_stale = send_peer(
        "dddddddddddddddddddddddddddddddd",
        Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        Some("ffffffffffffffffffffffffffffffff"),
        true,
        false,
        None,
    );
    saved_stale.saved = true;
    let unsaved_online = send_peer(
        "11111111111111111111111111111111",
        Some("22222222222222222222222222222222"),
        Some("33333333333333333333333333333333"),
        false,
        true,
        Some(now_ms()),
    );

    assert_eq!(
        saved_peer_link_targets(&[saved_online, saved_stale, unsaved_online]),
        vec![
            ManagedPeerLinkTarget {
                destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            },
            ManagedPeerLinkTarget {
                destination_hex: "ffffffffffffffffffffffffffffffff".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            },
            ManagedPeerLinkTarget {
                destination_hex: "33333333333333333333333333333333".to_string(),
                kind: ManagedPeerLinkKind::LxmfDelivery,
            },
        ]
    );
}

#[test]
fn saved_raw_lxmf_peer_without_separate_lxmf_destination_uses_lxmf_link_kind() {
    let mut saved_raw_lxmf_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        None,
        false,
        false,
        Some(now_ms()),
    );
    saved_raw_lxmf_peer.saved = true;
    saved_raw_lxmf_peer.lxmf_last_seen_at_ms = Some(now_ms());

    assert_eq!(
        managed_peer_link_target(&saved_raw_lxmf_peer),
        Some(ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        })
    );
}

#[test]
fn send_destination_resolution_requires_current_peer() {
    let peers = vec![send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        false,
        Some(1),
    )];

    assert_eq!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        )
        .expect("current app peer should resolve"),
        "cccccccccccccccccccccccccccccccc"
    );
    assert_eq!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        )
        .expect("current identity should resolve"),
        "cccccccccccccccccccccccccccccccc"
    );
    assert!(matches!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "dddddddddddddddddddddddddddddddd"
        ),
        Err(NodeError::NetworkError {})
    ));
    assert!(matches!(
        resolve_current_lxmf_destination_from_peers(peers.as_slice(), "not-a-destination"),
        Err(NodeError::InvalidConfig {})
    ));
}

#[test]
fn send_destination_resolution_rejects_stale_or_unannounced_peers() {
    let peers = vec![
        send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            Some(1),
        ),
        send_peer(
            "dddddddddddddddddddddddddddddddd",
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            Some("ffffffffffffffffffffffffffffffff"),
            false,
            false,
            None,
        ),
    ];

    assert!(matches!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        Err(NodeError::NetworkError {})
    ));
    assert!(matches!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "dddddddddddddddddddddddddddddddd"
        ),
        Err(NodeError::NetworkError {})
    ));
}

#[test]
fn send_destination_resolution_uses_current_lxmf_route_for_stale_app_peer() {
    let peers = vec![
        send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            Some(1),
        ),
        send_peer(
            "cccccccccccccccccccccccccccccccc",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            false,
            false,
            Some(2),
        ),
    ];

    assert_eq!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("current lxmf route should satisfy stale equivalent app peer"),
        "cccccccccccccccccccccccccccccccc"
    );
}

#[test]
fn send_destination_resolution_rejects_stale_app_peer_without_current_route() {
    let peers = vec![
        send_peer(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            Some(1),
        ),
        send_peer(
            "cccccccccccccccccccccccccccccccc",
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            Some("cccccccccccccccccccccccccccccccc"),
            true,
            false,
            Some(2),
        ),
    ];

    assert!(matches!(
        resolve_current_lxmf_destination_from_peers(
            peers.as_slice(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        Err(NodeError::NetworkError {})
    ));
}

#[test]
fn saved_peer_route_refresh_targets_saved_peers_without_known_delivery_route() {
    let mut messaging = sdkmsg::MessagingStore::new(30);
    let now = now_ms();
    messaging.mark_peer_saved("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true);
    messaging.mark_peer_saved("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", true);
    messaging.mark_peer_saved("cccccccccccccccccccccccccccccccc", false);
    messaging.record_resolution_result(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "dddddddddddddddddddddddddddddddd",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        now,
    );

    assert_eq!(
        saved_peer_destinations_needing_route_refresh(&messaging),
        vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
    );
}

#[test]
fn restore_saved_peer_management_marks_saved_peers_managed() {
    let mut messaging = sdkmsg::MessagingStore::new(30);
    let now = now_ms();
    messaging.record_announce(sdkmsg::AnnounceRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        identity_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        destination_kind: "lxmf_delivery".to_string(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Pixel".to_string(),
        display_name: Some("Pixel".to_string()),
        hops: 0,
        interface_hex: String::new(),
        received_at_ms: now,
    });
    messaging.record_announce(sdkmsg::AnnounceRecord {
        destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        identity_hex: "dddddddddddddddddddddddddddddddd".to_string(),
        destination_kind: "lxmf_delivery".to_string(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Other".to_string(),
        display_name: Some("Other".to_string()),
        hops: 0,
        interface_hex: String::new(),
        received_at_ms: now,
    });
    messaging.record_announce(sdkmsg::AnnounceRecord {
        destination_hex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
        identity_hex: "ffffffffffffffffffffffffffffffff".to_string(),
        destination_kind: "lxmf_delivery".to_string(),
        app_data: "Sideband;name=NonRem".to_string(),
        display_name: Some("NonRem".to_string()),
        hops: 0,
        interface_hex: String::new(),
        received_at_ms: now,
    });

    let restored = restore_saved_peer_management(
        &mut messaging,
        &[
            crate::types::SavedPeerRecord {
                destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                label: Some("Pixel".to_string()),
                saved_at_ms: now,
                identity_hex: None,
                lxmf_destination_hex: None,
                app_data: None,
                display_name: None,
                last_route_seen_at_ms: None,
                last_hops: None,
            },
            crate::types::SavedPeerRecord {
                destination_hex: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                label: Some("Pixel duplicate".to_string()),
                saved_at_ms: now,
                identity_hex: None,
                lxmf_destination_hex: None,
                app_data: None,
                display_name: None,
                last_route_seen_at_ms: None,
                last_hops: None,
            },
            crate::types::SavedPeerRecord {
                destination_hex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
                label: Some("Non REM".to_string()),
                saved_at_ms: now,
                identity_hex: None,
                lxmf_destination_hex: None,
                app_data: None,
                display_name: None,
                last_route_seen_at_ms: None,
                last_hops: None,
            },
        ],
    );

    assert_eq!(
        restored.route_request_destinations,
        vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
    );
    assert_eq!(
        restored.link_targets,
        vec![ManagedPeerLinkTarget {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kind: ManagedPeerLinkKind::LxmfDelivery,
        }]
    );
    assert_eq!(
        restored.pruned_destinations,
        vec!["eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string()]
    );
    let mut peers = messaging.list_peers();
    peers.sort_by(|left, right| left.destination_hex.cmp(&right.destination_hex));
    assert!(peers[0].saved);
    assert!(!peers[1].saved);
    assert!(!messaging.is_peer_saved("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
}

#[test]
fn operator_announce_message_accepts_rch_hub_announces() {
    let message = operator_announce_message(
        AnnounceClass::RchHubServer {},
        false,
        Some("North Hub"),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        2,
    )
    .expect("hub announce should be relevant");

    assert!(message.contains("RCH hub North Hub"));
    assert!(message.contains("dest=aaaaa..."));
    assert!(!message.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    assert!(!message.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    assert!(!message.contains("id="));
}

#[test]
fn operator_announce_message_accepts_rem_capable_lxmf_announces() {
    let message = operator_announce_message(
        AnnounceClass::LxmfDelivery {},
        true,
        Some("Pixel"),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        1,
    )
    .expect("peer announce should be relevant");

    assert!(message.contains("[announce] Pixel"));
    assert!(!message.contains("REM peer"));
    assert!(message.contains("dest=aaaaa..."));
    assert!(!message.contains("id="));
    assert!(message.contains("hops=1"));
}

#[test]
fn operator_announce_message_ignores_legacy_app_peer_announces() {
    let message = operator_announce_message(
        AnnounceClass::PeerApp {},
        false,
        Some("Pixel"),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        1,
    );

    assert!(message.is_none());
}

#[test]
fn effective_announce_interval_honors_configured_default() {
    assert_eq!(effective_announce_interval_seconds(0), 60);
    assert_eq!(effective_announce_interval_seconds(60), 60);
    assert_eq!(effective_announce_interval_seconds(1800), 1800);
    assert_eq!(effective_announce_interval_seconds(7200), 7200);
}

#[test]
fn peer_staleness_never_precedes_the_next_announce_window() {
    assert_eq!(effective_peer_stale_after_minutes(30, 1800), 31);
    assert_eq!(effective_peer_stale_after_minutes(45, 1800), 45);
    assert_eq!(effective_peer_stale_after_minutes(1, 60), 2);
}

#[test]
fn startup_announce_burst_leaves_reticulum_rate_limit_headroom() {
    assert_eq!(STARTUP_ANNOUNCE_DELAYS_SECS.len(), 3);
    assert_eq!(STARTUP_ANNOUNCE_DELAYS_SECS[0], 0);
}
