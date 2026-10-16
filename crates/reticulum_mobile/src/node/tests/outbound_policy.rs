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
