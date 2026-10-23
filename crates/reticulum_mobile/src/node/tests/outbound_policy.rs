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
fn battery_transitions_return_before_community_delivery_completes() {
    for (name, initial_percent, percent, saver_active) in [
        ("battery-enter", 80, 20, true),
        ("battery-exit", 20, 80, false),
    ] {
        let storage = prepare_storage_dir(name);
        let node = situation_release_node(&storage);
        let mut settings = sample_app_settings();
        settings.community = CommunitySettingsRecord::default();
        settings.power = PowerPolicyRecord { enabled: true, threshold_percent: 20 };
        node.set_app_settings(settings).expect("settings");
        node.inner.lock().expect("inner").app_state
            .set_saved_peers(&[build_saved_peer()]).expect("saved peer");
        node.update_battery_state(initial_percent, false).expect("initial battery");
        let (tx, mut rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        node.inner.lock().expect("inner").cmd_tx = Some(tx);
        let events = node.inner.lock().expect("inner").bus.subscribe();
        let (result_tx, result_rx) = cb::bounded(1);
        let worker = std::thread::spawn(move || {
            result_tx.send(node.update_battery_state(percent, false)).expect("result");
            node
        });
        let Command::SendBytes { resp, traffic_class, .. } = rx.blocking_recv().expect("community send") else {
            panic!("expected community send");
        };
        assert_eq!(traffic_class, OutboundTrafficClass::CommunityStatus {});
        let result = result_rx.recv_timeout(Duration::from_secs(1));
        // Complete a regressed synchronous send too, so failure never waits for its 120s timeout.
        let _ = resp.send(Ok(()));
        let node = worker.join().expect("battery worker");
        assert_eq!(result.expect("battery callback must not wait for radio delivery")
            .expect("battery update").saver_active, saver_active);
        assert_eq!(node.get_community_statuses().expect("persisted community status")[0].saver_active, saver_active);
        assert!(events.try_iter().any(|event| matches!(event,
            NodeEvent::PowerStateChanged { state } if state.saver_active == saver_active)));
    }
}

#[test]
fn battery_community_fanout_reports_a_full_runtime_queue_without_waiting() {
    let storage = prepare_storage_dir("battery-full-queue");
    let node = situation_release_node(&storage);
    let mut settings = sample_app_settings();
    settings.community = CommunitySettingsRecord::default();
    settings.power = PowerPolicyRecord { enabled: true, threshold_percent: 20 };
    node.set_app_settings(settings).expect("settings");
    node.update_battery_state(20, false).expect("enter saver");
    node.set_announce_capabilities("telemetry".to_string()).expect("deferred capabilities");
    let (tx, mut rx) = mpsc::channel(1);
    {
        let mut inner = node.inner.lock().expect("inner");
        inner.app_state.set_saved_peers(&[build_saved_peer()]).expect("saved peer");
        inner.cmd_tx = Some(tx.clone());
    }
    let (resp, _) = cb::bounded(1);
    tx.try_send(Command::SendBytes {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        bytes: vec![], fields_bytes: None, send_mode: SendMode::Auto {},
        traffic_class: OutboundTrafficClass::Raw {}, resp,
    }).expect("fill queue");
    let events = node.inner.lock().expect("inner").bus.subscribe();
    let (result_tx, result_rx) = cb::bounded(1);
    let worker = std::thread::spawn(move || {
        result_tx.send(node.update_battery_state(80, false)).expect("result");
        node
    });
    let result = result_rx.recv_timeout(Duration::from_secs(1));
    // Free capacity even on regression, so the worker can finish before asserting.
    rx.try_recv().expect("original queue entry");
    let node = worker.join().expect("battery worker");
    assert!(!result.expect("full runtime queue must not block battery callback")
        .expect("battery update").saver_active);
    assert!(events.try_iter().any(|event| matches!(event,
        NodeEvent::Error { code, message } if code == "Timeout" && message.contains("event replication delivery failed"))));
    assert_eq!(node.inner.lock().expect("inner").deferred_announce_capabilities.as_deref(), Some("telemetry"));
    node.update_battery_state(79, false).expect("retry deferred capabilities");
    assert!(matches!(rx.try_recv().expect("capability retry"),
        Command::SetAnnounceCapabilities { capability_string, .. } if capability_string == "telemetry"));
    assert!(node.inner.lock().expect("inner").deferred_announce_capabilities.is_none());
}

#[test]
fn battery_exit_does_not_wait_for_deferred_capability_response() {
    let node = situation_test_node("battery-deferred-capability");
    node.update_battery_state(10, false).expect("enter saver");
    node.set_announce_capabilities("telemetry".to_string()).expect("defer capabilities");
    let (tx, mut rx) = mpsc::channel(1);
    node.inner.lock().expect("inner").cmd_tx = Some(tx);
    let (result_tx, result_rx) = cb::bounded(1);
    let worker = std::thread::spawn(move || {
        result_tx.send(node.update_battery_state(80, false)).expect("result");
    });
    let Command::SetAnnounceCapabilities { resp, .. } = rx.blocking_recv().expect("capability send") else {
        panic!("expected deferred capability send");
    };
    let result = result_rx.recv_timeout(Duration::from_secs(1));
    let _ = resp.send(Ok(()));
    worker.join().expect("battery worker");
    assert!(!result.expect("battery callback must not wait for capability response")
        .expect("battery update").saver_active);
}
