#[test]
fn lxmf_only_resolution_projects_saved_peer() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 5,
        interface_hex: "iface".into(),
        received_at_ms: now,
    });
    store.record_resolution_result("appdest", "identity", "lxmfdest", now);
    store.mark_peer_saved("appdest", true);

    let peer = store
        .peer_by_destination("appdest")
        .expect("saved peer should be projected from lxmf announce");
    assert_eq!(peer.destination_hex, "lxmfdest");
    assert_eq!(peer.lxmf_destination_hex.as_deref(), Some("lxmfdest"));
    assert_eq!(peer.display_name.as_deref(), Some("Alice"));
    assert_eq!(peer.lxmf_last_seen_at_ms, Some(now));
}

#[test]
fn saved_peer_with_active_link_is_connected() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    store.record_announce(AnnounceRecord {
        destination_hex: "lxmfdest".into(),
        identity_hex: "identity".into(),
        destination_kind: "lxmf_delivery".into(),
        app_data: "R3AKT,EMergencyMessages;name=Alice".into(),
        display_name: Some("Alice".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: now.saturating_sub(20),
    });
    store.mark_peer_saved("lxmfdest", true);
    store.set_peer_active_link("lxmfdest", true, now);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].state, PeerState::Connected);
    assert!(peers[0].active_link);
}

#[test]
fn unsaved_recent_peer_does_not_project_connected_without_active_link() {
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

    let peer = store
        .peer_by_destination("lxmfdest")
        .expect("recent mission-capable peer should be projected");
    assert_eq!(peer.state, PeerState::Disconnected);
    assert!(!peer.active_link);
    assert!(!peer.stale);
}

#[test]
fn saved_peer_without_resolution_stays_connecting() {
    let mut store = MessagingStore::default();
    store.mark_peer_saved("appdest", true);

    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "appdest");
    assert!(peers[0].saved);
    assert_eq!(peers[0].state, PeerState::Connecting);
    assert_eq!(peers[0].last_seen_at_ms, 0);
    assert!(!peers[0].stale);
}

#[test]
fn replace_saved_destinations_clears_removed_peer_presence() {
    let mut store = MessagingStore::default();
    store.mark_peer_saved("oldpeer", true);
    store.mark_peer_saved("keptpeer", true);
    store.set_peer_active_link("oldpeer", true, current_time_ms());
    store.record_resolution_error("oldpeer", Some("stale route".to_string()));

    let (added, removed) =
        store.replace_saved_destinations(["keptpeer".to_string(), "newpeer".to_string()]);

    assert_eq!(added, vec!["newpeer".to_string()]);
    assert_eq!(removed, vec!["oldpeer".to_string()]);
    assert_eq!(
        store.saved_destination_hexes(),
        vec!["keptpeer".to_string(), "newpeer".to_string()]
    );
    assert!(!store.is_peer_saved("oldpeer"));
    assert!(!store
        .peer_by_destination("oldpeer")
        .is_some_and(|peer| peer.active_link));
}
