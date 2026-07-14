#[test]
fn saved_peer_profile_projects_stale_routable_lxmf_peer_without_announces() {
    let mut store = MessagingStore::default();
    let seen_at = current_time_ms().saturating_sub(DEFAULT_PEER_STALE_AFTER_MS + 1_000);

    store.mark_peer_saved("appdest", true);
    store.record_saved_peer_profile(
        "appdest",
        Some("identity"),
        Some("lxmfdest"),
        Some("R3AKT,EmergencyMessages,Telemetry"),
        Some("Alice"),
        Some(seen_at),
        Some(3),
    );

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].identity_hex.as_deref(), Some("identity"));
    assert_eq!(peers[0].lxmf_destination_hex.as_deref(), Some("lxmfdest"));
    assert_eq!(peers[0].display_name.as_deref(), Some("Alice"));
    assert_eq!(
        peers[0].app_data.as_deref(),
        Some("R3AKT,EmergencyMessages,Telemetry")
    );
    assert!(peers[0].saved);
    assert!(peers[0].stale);
    assert_eq!(peers[0].state, PeerState::Disconnected);
    assert_eq!(peers[0].lxmf_last_seen_at_ms, Some(seen_at));
}

#[test]
fn saved_peer_staleness_uses_recent_rem_lxmf_announce() {
    let mut store = MessagingStore::new(30);
    let now = current_time_ms();
    let stale_app_seen_at = now.saturating_sub((31 * 60 * 1000) as u64);
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale_app_seen_at,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });
    store.mark_peer_saved("appdest", true);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert!(!peers[0].stale);
    assert_eq!(peers[0].state, PeerState::Disconnected);
    assert!(!peers[0].active_link);
    assert_eq!(peers[0].last_seen_at_ms, now);
}

#[test]
fn failed_saved_peer_resolution_preserves_last_seen_from_rem_lxmf_announces() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 15,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10_000),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 15,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(9_000),
    });
    store.mark_peer_saved("appdest", true);
    store.record_resolution_attempt("appdest", now);
    store.record_resolution_error("appdest", Some("timeout".into()));

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert!(!peers[0].stale);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(9_000));
    assert_eq!(
        peers[0].announce_last_seen_at_ms,
        Some(now.saturating_sub(9_000))
    );
    assert_eq!(peers[0].last_resolution_error.as_deref(), Some("timeout"));

    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_add(1),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_add(1),
    });

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert!(!peers[0].stale);
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_add(1));
    assert_eq!(
        peers[0].announce_last_seen_at_ms,
        Some(now.saturating_add(1))
    );
    assert_eq!(peers[0].last_resolution_error.as_deref(), Some("timeout"));

    store.record_resolution_result("appdest", "identity", "lxmfdest", now.saturating_add(2));

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert!(!peers[0].stale);
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_add(1));
    assert_eq!(peers[0].last_resolution_error, None);
}

#[test]
fn older_announce_does_not_replace_newer_runtime_record() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages;name=New".into(),
        display_name: Some("New".into()),
        hops: 1,
        interface_hex: "iface-new".into(),
        received_at_ms: now,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages;name=Old".into(),
        display_name: Some("Old".into()),
        hops: 4,
        interface_hex: "iface-old".into(),
        received_at_ms: now.saturating_sub(10_000),
    });

    let record = store
        .list_announces()
        .into_iter()
        .find(|record| record.destination_hex == "appdest")
        .expect("announce should exist");
    assert_eq!(record.display_name.as_deref(), Some("New"));
    assert_eq!(record.interface_hex, "iface-new");
    assert_eq!(record.received_at_ms, now);
}

#[test]
fn current_lxmf_announce_destination_requires_fresh_lxmf_announce() {
    let mut store = MessagingStore::new(1);
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        identity_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        destination_kind: "lxmf_delivery".to_string(),
        app_data: "Peer".to_string(),
        display_name: Some("Peer".to_string()),
        hops: 1,
        interface_hex: String::new(),
        received_at_ms: now,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        identity_hex: "dddddddddddddddddddddddddddddddd".to_string(),
        destination_kind: "app".to_string(),
        app_data: "R3AKT,EMergencyMessages".to_string(),
        display_name: Some("Peer".to_string()),
        hops: 1,
        interface_hex: String::new(),
        received_at_ms: now,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
        identity_hex: "ffffffffffffffffffffffffffffffff".to_string(),
        destination_kind: "lxmf_delivery".to_string(),
        app_data: "Peer".to_string(),
        display_name: Some("Peer".to_string()),
        hops: 1,
        interface_hex: String::new(),
        received_at_ms: now.saturating_sub(120_000),
    });

    assert_eq!(
        store.current_lxmf_announce_destination("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
    );
    assert_eq!(
        store.current_lxmf_announce_destination("cccccccccccccccccccccccccccccccc"),
        None
    );
    assert_eq!(
        store.current_lxmf_announce_destination("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        None
    );
}

#[test]
fn saved_peer_with_recent_rem_lxmf_announce_is_reachable_without_active_transport_link() {
    let mut store = MessagingStore::new(30);
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });
    store.mark_peer_saved("lxmfdest", true);

    let peer = store
        .peer_by_destination("lxmfdest")
        .expect("saved recent peer should be projected");
    assert_eq!(peer.state, PeerState::Disconnected);
    assert!(!peer.active_link);
    assert!(!peer.stale);
}

#[test]
fn saved_peer_with_recent_lxmf_announce_is_not_stale_after_link_clear() {
    let mut store = MessagingStore::new(30);
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(31 * 60 * 1000),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });
    store.record_resolution_result("appdest", "identity", "lxmfdest", now);
    store.mark_peer_saved("appdest", true);
    store.set_peer_active_link("lxmfdest", true, now);
    store.set_peer_active_link("lxmfdest", false, now.saturating_add(1));

    let peer = store
        .peer_by_destination("appdest")
        .expect("saved peer with fresh lxmf route should be projected");
    assert_eq!(peer.destination_hex, "lxmfdest");
    assert_eq!(peer.state, PeerState::Disconnected);
    assert!(!peer.active_link);
    assert!(!peer.stale);
}

#[test]
fn saved_peer_with_stale_rem_lxmf_announce_is_disconnected() {
    let mut store = MessagingStore::new(30);
    let now = current_time_ms();
    let stale_seen_at = now.saturating_sub(31 * 60 * 1000);
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale_seen_at,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale_seen_at,
    });
    store.record_resolution_result("appdest", "identity", "lxmfdest", now);
    store.mark_peer_saved("appdest", true);

    let peer = store
        .peer_by_destination("appdest")
        .expect("saved stale peer should be projected");
    assert_eq!(peer.destination_hex, "lxmfdest");
    assert_eq!(peer.state, PeerState::Disconnected);
    assert!(!peer.active_link);
    assert!(peer.stale);
}

#[test]
fn empty_legacy_app_announce_does_not_erase_lxmf_mission_capabilities() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages,Telemetry".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(30),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Poco".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: String::new(),
        display_name: None,
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(
        peers[0].app_data.as_deref(),
        Some("R3AKT,EMergencyMessages,Telemetry;name=Poco")
    );
    assert_eq!(peers[0].display_name.as_deref(), Some("Poco"));
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(20));
}

#[test]
fn prune_saved_destinations_with_non_rem_lxmf_evidence_removes_contaminated_peers() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.mark_peer_saved("sidebandlxmf", true);
    store.mark_peer_saved("rempeer", true);
    store.mark_peer_saved("emptyroute", true);
    store.record_announce(AnnounceRecord {
        destination_hex: "sidebandlxmf".into(),
        identity_hex: "sidebandidentity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "92c40553696c6b65c0".into(),
        display_name: Some("Silke".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "rempeer".into(),
        identity_hex: "remidentity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages,Telemetry;name=Pixel".into(),
        display_name: Some("Pixel".into()),
        hops: 0,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "emptyroute".into(),
        identity_hex: "emptyidentity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "".into(),
        display_name: None,
        hops: 0,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(5),
    });

    let removed = store.prune_saved_destinations_with_non_rem_announce_evidence();

    assert_eq!(removed, vec!["sidebandlxmf".to_string()]);
    assert!(!store.is_peer_saved("sidebandlxmf"));
    assert!(store.is_peer_saved("emptyroute"));
    assert!(store.is_peer_saved("rempeer"));
}

#[test]
fn rem_lxmf_only_announce_creates_canonical_peer_record() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Poco".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].lxmf_destination_hex.as_deref(), Some("lxmfdest"));
    assert_eq!(peers[0].last_seen_at_ms, now);
    assert_eq!(peers[0].announce_last_seen_at_ms, Some(now));
    assert_eq!(peers[0].lxmf_last_seen_at_ms, Some(now));
    assert_eq!(peers[0].display_name.as_deref(), Some("Poco"));
}

#[test]
fn non_rem_lxmf_only_announce_does_not_create_peer_record() {
    let mut store = MessagingStore::default();
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "chat".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: current_time_ms(),
    });

    assert!(store.list_peers().is_empty());
}
