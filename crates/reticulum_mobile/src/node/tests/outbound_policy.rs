#[test]
fn outbound_policy_inventory_is_exhaustive_in_saver() {
    let cases = [
        (OutboundTrafficClass::Sos {}, true),
        (OutboundTrafficClass::Telemetry {}, true),
        (OutboundTrafficClass::CommunityStatus {}, true),
        (OutboundTrafficClass::Chat {}, false),
        (OutboundTrafficClass::Eam {}, false),
        (OutboundTrafficClass::Event {}, false),
        (OutboundTrafficClass::Checklist {}, false),
        (OutboundTrafficClass::Plugin {}, false),
        (OutboundTrafficClass::Raw {}, false),
        (OutboundTrafficClass::Control {}, false),
    ];
    for (class, allowed) in cases {
        assert_eq!(
            outbound_admission(true, class) == OutboundAdmission::Allow,
            allowed,
            "class {}",
            class.as_str()
        );
        assert_eq!(outbound_admission(false, class), OutboundAdmission::Allow);
    }

    let api_inventory = [
        ("send_lxmf", OutboundTrafficClass::Chat {}, false),
        ("retry_lxmf_legacy_chat", OutboundTrafficClass::Chat {}, false),
        ("retry_lxmf_sos", OutboundTrafficClass::Sos {}, true),
        ("announce_now", OutboundTrafficClass::Control {}, false),
        ("request_peer_identity", OutboundTrafficClass::Control {}, false),
        ("request_lxmf_sync", OutboundTrafficClass::Control {}, false),
        ("set_active_propagation_node", OutboundTrafficClass::Control {}, false),
        ("set_announce_capabilities_transmit", OutboundTrafficClass::Control {}, false),
        ("start_restart_burst", OutboundTrafficClass::Control {}, false),
        ("connect_peer", OutboundTrafficClass::Control {}, false),
        ("refresh_hub_directory", OutboundTrafficClass::Control {}, false),
        ("send_bytes", OutboundTrafficClass::Raw {}, false),
        ("broadcast_bytes", OutboundTrafficClass::Raw {}, false),
        ("trigger_or_deactivate_sos", OutboundTrafficClass::Sos {}, true),
        ("record_local_telemetry_fix", OutboundTrafficClass::Telemetry {}, true),
        ("publish_community_status", OutboundTrafficClass::CommunityStatus {}, true),
        ("upsert_or_delete_event", OutboundTrafficClass::Event {}, false),
        ("upsert_or_delete_eam", OutboundTrafficClass::Eam {}, false),
        ("checklist_replication", OutboundTrafficClass::Checklist {}, false),
        ("send_plugin_lxmf", OutboundTrafficClass::Plugin {}, false),
        ("autonomous_propagation_sync", OutboundTrafficClass::Control {}, false),
    ];
    let mut names = HashSet::new();
    for (api, class, allowed) in api_inventory {
        assert!(names.insert(api), "duplicate outbound API inventory row {api}");
        assert_eq!(
            outbound_admission(true, class) == OutboundAdmission::Allow,
            allowed,
            "outbound API {api} class {}",
            class.as_str()
        );
    }
}

#[test]
fn power_threshold_hysteresis_and_cadence_are_exact() {
    let policy = PowerPolicyRecord { enabled: true, threshold_percent: 20 };
    assert!(next_saver_state(false, &policy, 20, false));
    assert!(next_saver_state(true, &policy, 22, false));
    assert!(!next_saver_state(true, &policy, 23, false));
    assert!(!next_saver_state(true, &policy, 5, true));
    assert_eq!(effective_power_cadence_seconds(60, true), 300);
    assert_eq!(effective_power_cadence_seconds(600, true), 600);
    assert_eq!(effective_power_cadence_seconds(60, false), 60);
}

#[test]
fn exact_telemetry_and_chat_recognize_only_inner_saved_aliases() {
    let peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: None,
        saved_at_ms: 1,
        identity_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        lxmf_destination_hex: Some("cccccccccccccccccccccccccccccccc".to_string()),
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    };
    assert!(peer_is_inner_saved(&[peer.clone()], &peer.destination_hex));
    assert!(peer_is_inner_saved(
        &[peer.clone()],
        peer.lxmf_destination_hex.as_deref().expect("lxmf")
    ));
    let outer = SavedPeerRecord { circle_tier: CircleTier::Outer {}, ..peer };
    assert!(!peer_is_inner_saved(&[outer], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
}

#[test]
fn exact_telemetry_target_fixture_is_inner_direct_only_and_hub_fail_closed() {
    let saved_peers = vec![
        SavedPeerRecord {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            label: None,
            saved_at_ms: 1,
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
            circle_tier: CircleTier::Inner {},
        },
        SavedPeerRecord {
            destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            label: None,
            saved_at_ms: 1,
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
            circle_tier: CircleTier::Outer {},
        },
    ];
    let inner_direct = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::Auto {},
    };
    let outer_direct = MissionReplicationTarget {
        app_destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        send_mode: SendMode::Auto {},
    };
    let inner_via_propagation = MissionReplicationTarget {
        app_destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        send_mode: SendMode::PropagationOnly {},
    };

    assert!(exact_telemetry_target_is_allowed(
        &inner_direct,
        &saved_peers,
        false
    ));
    assert!(!exact_telemetry_target_is_allowed(
        &outer_direct,
        &saved_peers,
        false
    ));
    assert!(!exact_telemetry_target_is_allowed(
        &inner_via_propagation,
        &saved_peers,
        false
    ));
    assert!(!exact_telemetry_target_is_allowed(
        &inner_direct,
        &saved_peers,
        true
    ));
}
