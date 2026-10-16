#[test]
fn connected_chat_routes_only_roster_peers_without_their_own_route_through_hub() {
    let destination = "abababababababababababababababab";
    let hub = "56565656565656565656565656565656";
    let config = build_config_fingerprint_for_tests(HubMode::Connected {}, Some(hub));
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    snapshot.items = vec![crate::types::HubDirectoryPeerRecord {
        identity: "78787878787878787878787878787878".to_string(),
        destination_hash: destination.to_string(),
        display_name: None,
        announce_capabilities: vec!["telemetry".to_string()],
        client_type: None,
        registered_mode: None,
        last_seen: None,
        status: None,
    }];
    assert_eq!(
        routed_chat_destination_hex(
            destination.to_string(),
            Some(&config),
            Some(&snapshot),
            || Ok(vec![])
        )
        .expect("hub route"),
        hub
    );
    let peer = build_peer_record(
        destination,
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        false,
        true,
        true,
    );
    assert_eq!(
        routed_chat_destination_hex(
            destination.to_string(),
            Some(&config),
            Some(&snapshot),
            || Ok(vec![peer])
        )
        .expect("own route"),
        destination
    );
    for snapshot in [None, Some(&snapshot)] {
        let contact = "77777777777777777777777777777777";
        assert_eq!(
            routed_chat_destination_hex(contact.to_string(), Some(&config), snapshot, || panic!(
                "contact admission needs no history"
            ))
            .expect("contact"),
            contact
        );
    }
}
