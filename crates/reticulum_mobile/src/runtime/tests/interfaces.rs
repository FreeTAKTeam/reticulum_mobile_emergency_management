#[test]
fn tcp_endpoint_connect_addr_accepts_plain_and_tcp_urls() {
    assert_eq!(
        tcp_endpoint_connect_addr("rns.beleth.net:4242"),
        "rns.beleth.net:4242"
    );
    assert_eq!(
        tcp_endpoint_connect_addr(" tcp://127.0.0.1:4242 "),
        "127.0.0.1:4242"
    );
    assert_eq!(tcp_endpoint_connect_addr(""), "");
}

#[test]
fn configured_tcp_client_endpoints_trim_strip_and_deduplicate() {
    let endpoints = configured_tcp_client_endpoints(&[
        " tcp://rns.beleth.net:4242 ".to_string(),
        "rns.beleth.net:4242".to_string(),
        " ".to_string(),
        "dfw.us.g00n.cloud:6969".to_string(),
    ]);

    assert_eq!(
        endpoints,
        vec![
            "rns.beleth.net:4242".to_string(),
            "dfw.us.g00n.cloud:6969".to_string(),
        ]
    );
}

#[test]
fn tcp_readiness_monitor_skips_loopback_test_relays() {
    let endpoints = tcp_readiness_monitor_endpoints(&[
        "127.0.0.1:4242".to_string(),
        "localhost:4242".to_string(),
        "[::1]:4242".to_string(),
        "rns.beleth.net:4242".to_string(),
    ]);

    assert_eq!(endpoints, vec!["rns.beleth.net:4242".to_string()]);
}

#[tokio::test]
async fn active_interface_registry_removes_stopped_tcp_endpoint_entries() {
    let registry: ActiveInterfaceRegistry = Arc::new(TokioMutex::new(HashMap::from([
        (
            AddressHash::new_from_slice(&[1u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[1u8; 16]),
                "rns.beleth.net:4242".to_string(),
                "connected",
            ),
        ),
        (
            AddressHash::new_from_slice(&[2u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[2u8; 16]),
                "rns.beleth.net:4242".to_string(),
                "connected",
            ),
        ),
        (
            AddressHash::new_from_slice(&[3u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[3u8; 16]),
                "dfw.us.g00n.cloud:6969".to_string(),
                "connected",
            ),
        ),
    ])));
    let status = Arc::new(Mutex::new(NodeStatus {
        readiness: crate::types::RuntimeReadinessSnapshot::default(),
        running: true,
        name: "test".to_string(),
        identity_hex: String::new(),
        app_destination_hex: String::new(),
        lxmf_destination_hex: String::new(),
        interfaces: Vec::new(),
    }));
    let bus = EventBus::new();
    let rx = bus.subscribe();

    unregister_tcp_client_endpoint(&registry, &status, &bus, "rns.beleth.net:4242").await;

    let guard = registry.lock().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(
        guard
            .get(&AddressHash::new_from_slice(&[3u8; 16]))
            .map(|status| status.label.as_str()),
        Some("dfw.us.g00n.cloud:6969"),
    );
    assert!(rx
        .try_iter()
        .any(|event| matches!(event, NodeEvent::InterfaceStatusChanged { status } if status.state == "disconnected")));
}

#[test]
fn active_relay_transport_requires_non_rnode_ble_interface() {
    let rnode_only = HashMap::from([(
        AddressHash::new_from_slice(&[1u8; 16]),
        new_interface_status(
            AddressHash::new_from_slice(&[1u8; 16]),
            "rnode-ble:RNode 4339".to_string(),
            "connected",
        ),
    )]);
    assert!(!active_interfaces_include_relay_transport(&rnode_only));
    assert!(active_interfaces_are_rnode_ble_only(&rnode_only));
    assert!(active_interface_is_rnode_ble(
        &rnode_only,
        &AddressHash::new_from_slice(&[1u8; 16]),
    ));
    assert_eq!(link_connect_timeout(true), RNODE_BLE_LINK_CONNECT_TIMEOUT);

    let with_tcp = HashMap::from([
        (
            AddressHash::new_from_slice(&[1u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[1u8; 16]),
                "rnode-ble:RNode 4339".to_string(),
                "connected",
            ),
        ),
        (
            AddressHash::new_from_slice(&[2u8; 16]),
            new_interface_status(
                AddressHash::new_from_slice(&[2u8; 16]),
                "rns.beleth.net:4242".to_string(),
                "connected",
            ),
        ),
    ]);
    assert!(active_interfaces_include_relay_transport(&with_tcp));
    assert!(!active_interfaces_are_rnode_ble_only(&with_tcp));
    assert!(active_interface_is_rnode_ble(
        &with_tcp,
        &AddressHash::new_from_slice(&[1u8; 16]),
    ));
    assert!(!active_interface_is_rnode_ble(
        &with_tcp,
        &AddressHash::new_from_slice(&[2u8; 16]),
    ));
    assert_eq!(link_connect_timeout(false), DEFAULT_LINK_CONNECT_TIMEOUT);
    assert_eq!(link_connect_timeout(true), RNODE_BLE_LINK_CONNECT_TIMEOUT);

    let no_interfaces = HashMap::new();
    assert!(!active_interfaces_are_rnode_ble_only(&no_interfaces));
    assert!(!active_interface_is_rnode_ble(
        &no_interfaces,
        &AddressHash::new_from_slice(&[1u8; 16]),
    ));
}

#[test]
fn tcp_data_path_unavailable_message_is_readiness_classified() {
    let message = tcp_data_path_unavailable_message(&["rns.beleth.net:4242".to_string()]);

    assert!(message.contains("transport startup failed"));
    assert!(message.contains("no reachable Reticulum TCP interface"));
}

#[test]
fn rnode_runtime_readiness_requires_detected_online_radio() {
    let connecting = serde_json::json!({
        "probe_status": { "detected": false },
        "online": false,
        "last_command_error": null,
    });
    let detected_but_offline = serde_json::json!({
        "probe_status": { "detected": true },
        "online": false,
        "last_command_error": null,
    });
    let ready = serde_json::json!({
        "probe_status": { "detected": true },
        "online": true,
        "last_command_error": null,
    });

    assert_eq!(
        rnode_runtime_interface_state(&connecting, false),
        ("connecting", None)
    );
    assert_eq!(
        rnode_runtime_interface_state(&detected_but_offline, false),
        ("connecting", None)
    );
    assert_eq!(
        rnode_runtime_interface_state(&ready, false),
        ("connected", None)
    );
    assert_eq!(
        rnode_runtime_interface_state(&connecting, true),
        (
            "failed",
            Some(
                "RNode BLE/KISS startup did not report a detected online radio within 30 seconds"
                    .to_string()
            )
        )
    );
}

#[test]
fn rnode_runtime_readiness_preserves_command_failure() {
    let failed = serde_json::json!({
        "probe_status": { "detected": true },
        "online": true,
        "last_command_error": "radio configuration rejected",
    });

    assert_eq!(
        rnode_runtime_interface_state(&failed, true),
        ("failed", Some("radio configuration rejected".to_string()))
    );
}

#[test]
fn direct_delivery_readiness_requires_active_link() {
    let announced_peer = send_peer(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        Some("cccccccccccccccccccccccccccccccc"),
        false,
        false,
        Some(1),
    );
    let active_peer = send_peer(
        "dddddddddddddddddddddddddddddddd",
        Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
        Some("ffffffffffffffffffffffffffffffff"),
        false,
        true,
        Some(1),
    );

    assert!(!sdk_peer_is_direct_delivery_ready(&announced_peer, false));
    assert!(!sdk_peer_is_direct_delivery_ready(&announced_peer, true));
    assert!(sdk_peer_is_direct_delivery_ready(&active_peer, true));
}

#[cfg(target_os = "android")]
#[test]
fn rnode_ble_wiring_derives_kiss_and_native_settings_from_rem_settings() {
    let settings = RnodeSettingsRecord {
        enabled: true,
        connection_mode: RnodeConnectionMode::Ble.as_str().to_string(),
        peripheral_id: "AA:BB:CC:DD:EE:FF".to_string(),
        display_name: "Field RNode".to_string(),
        region: "EU868".to_string(),
        profile: "REM-MF-URBAN-v1".to_string(),
    };

    let wiring = rnode_ble_wiring_from_settings(&settings).expect("valid RNode wiring");

    assert_eq!(wiring.label, "rnode-ble:Field RNode");
    assert_eq!(wiring.native.peripheral_id, "AA:BB:CC:DD:EE:FF");
    assert!(wiring
        .native
        .peripheral_aliases
        .iter()
        .any(|alias| alias == "Field RNode"));
    assert_eq!(wiring.kiss.mtu, usize::from(wiring.lora.max_payload_bytes));
    assert_eq!(wiring.kiss.max_write_len, 20);
    assert_eq!(wiring.kiss.read_frame_timeout, RNODE_BLE_READ_FRAME_TIMEOUT);
    assert!(!wiring.kiss.initial_frames.is_empty());
    assert!(!wiring.kiss.deferred_frames.is_empty());
    assert!(!wiring.kiss.shutdown_frames.is_empty());
}

#[cfg(target_os = "android")]
#[test]
fn rnode_ble_wiring_falls_back_to_peripheral_label_without_display_name() {
    let settings = RnodeSettingsRecord {
        enabled: true,
        connection_mode: RnodeConnectionMode::Ble.as_str().to_string(),
        peripheral_id: "AA:BB:CC:DD:EE:FF".to_string(),
        display_name: " ".to_string(),
        region: "US915".to_string(),
        profile: "REM-LF-RURAL-v1".to_string(),
    };

    let wiring = rnode_ble_wiring_from_settings(&settings).expect("valid RNode wiring");

    assert_eq!(wiring.label, "rnode-ble:AA:BB:CC:DD:EE:FF");
    assert!(wiring.native.peripheral_aliases.is_empty());
    assert_eq!(wiring.kiss.mtu, usize::from(wiring.lora.max_payload_bytes));
    assert!(!wiring.kiss.initial_frames.is_empty());
    assert!(!wiring.kiss.deferred_frames.is_empty());
}
