#[test]
fn capability_relevant_unsaved_peer_appears_in_possible_peers() {
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
        received_at_ms: now.saturating_sub(20),
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
    assert!(!peers[0].saved);
    assert_eq!(peers[0].state, PeerState::Disconnected);
    assert!(!peers[0].stale);
}

#[test]
fn stale_capability_relevant_unsaved_peer_is_excluded_from_possible_peers() {
    let mut store = MessagingStore::new(30);
    let now = current_time_ms();
    let stale_seen_at = now.saturating_sub((31 * 60 * 1000) as u64);
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("S8".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale_seen_at,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=S8".into(),
        display_name: Some("S8".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale_seen_at,
    });

    assert!(store.list_peers().is_empty());
}

#[test]
fn capability_irrelevant_peer_is_excluded_from_possible_peers() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "chat".into(),
        display_name: Some("Ignored".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "chat".into(),
        display_name: Some("Chat Only".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });

    assert!(store.list_peers().is_empty());
}

#[test]
fn capability_relevant_saved_peer_becomes_stale_after_timeout() {
    let mut store = MessagingStore::new(1);
    let stale = current_time_ms().saturating_sub(70_000);
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale,
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Poco".into(),
        display_name: Some("Poco".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: stale,
    });
    store.mark_peer_saved("appdest", true);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert!(peers[0].saved);
    assert!(peers[0].stale);
}
