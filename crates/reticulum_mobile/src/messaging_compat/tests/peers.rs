#[test]
fn legacy_app_alias_projects_canonical_lxmf_peer() {
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

    store.mark_peer_saved("appdest", true);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "lxmfdest");
    assert_eq!(peers[0].lxmf_destination_hex.as_deref(), Some("lxmfdest"));
    assert_eq!(peers[0].display_name.as_deref(), Some("Alice"));
    assert_eq!(peers[0].state, PeerState::Disconnected);
    assert!(!peers[0].active_link);
    assert!(peers[0].saved);
    assert_eq!(peers[0].last_seen_at_ms, now.saturating_sub(10));
    assert!(!peers[0].stale);
}

#[test]
fn latest_app_alias_resolves_to_canonical_lxmf_destination_for_identity() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest-old".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("Old".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(30),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "appdest-new".into(),
        identity_hex: "identity".into(),
        destination_kind: "app".into(),
        app_data: "R3AKT,EMergencyMessages".into(),
        display_name: Some("New".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=New".into(),
        display_name: Some("New".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(10),
    });

    assert_eq!(
        store.app_destination_for_identity("identity").as_deref(),
        Some("appdest-new")
    );

    let peer_destinations = store
        .list_peers()
        .into_iter()
        .map(|peer| peer.destination_hex)
        .collect::<Vec<_>>();
    assert_eq!(peer_destinations, vec!["lxmfdest".to_string()]);
}
