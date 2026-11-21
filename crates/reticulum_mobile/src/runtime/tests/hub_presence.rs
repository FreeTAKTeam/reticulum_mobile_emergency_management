const HUB_HEX: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn hub_link_is_maintained_only_in_hub_modes() {
    let expected = Some(ManagedPeerLinkTarget {
        destination_hex: HUB_HEX.to_string(),
        kind: ManagedPeerLinkKind::LxmfDelivery,
    });
    let upper = HUB_HEX.to_ascii_uppercase();
    assert_eq!(hub_link_target(HubMode::SemiAutonomous {}, Some(HUB_HEX)), expected);
    assert_eq!(hub_link_target(HubMode::Connected {}, Some(upper.as_str())), expected);
    assert_eq!(hub_link_target(HubMode::Autonomous {}, Some(HUB_HEX)), None);
    assert_eq!(hub_link_target(HubMode::Connected {}, None), None);
    assert_eq!(hub_link_target(HubMode::Connected {}, Some("not-a-hash")), None);
}

#[test]
fn hub_interface_announce_fires_on_interface_up_with_a_minimum_gap() {
    let now = 1_000_000;
    assert!(hub_interface_announce_due(false, true, None, now));
    assert!(!hub_interface_announce_due(true, true, None, now), "already up");
    assert!(!hub_interface_announce_due(false, false, None, now), "still down");
    assert!(!hub_interface_announce_due(true, false, None, now), "went down");
    assert!(
        !hub_interface_announce_due(false, true, Some(now - 30_000), now),
        "flapping interface stays rate limited"
    );
    assert!(hub_interface_announce_due(false, true, Some(now - 60_000), now));
}

#[test]
fn only_direct_only_sends_without_a_route_wait_for_a_path() {
    let no_route = NodeError::NetworkError {};
    assert!(should_wait_for_direct_path(SendMode::DirectOnly {}, &no_route));
    assert!(!should_wait_for_direct_path(SendMode::Auto {}, &no_route));
    assert!(!should_wait_for_direct_path(SendMode::PropagationOnly {}, &no_route));
    assert!(!should_wait_for_direct_path(
        SendMode::DirectOnly {},
        &NodeError::InvalidConfig {}
    ));
}
