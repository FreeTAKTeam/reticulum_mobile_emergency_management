#[tokio::test]
async fn request_lxmf_sync_without_selected_relay_waits_then_fails_status() {
    let _guard = test_lock().lock().await;
    let relay = TcpRelayHandle::start().await;
    let storage = prepare_storage_dir("sync_no_active_relay");
    let node = Node::new().expect("node storage");
    node.start(build_config(
        "sync-no-active-relay",
        storage.as_path(),
        relay.address().as_str(),
    ))
    .expect("start node");

    let result = node.request_lxmf_sync(Some(1));

    assert!(matches!(result, Err(NodeError::InvalidConfig {})));
    let status = node.get_lxmf_sync_status().expect("sync status");
    assert!(matches!(status.phase, SyncPhase::Failed {}));
    assert_eq!(status.messages_received, 0);
    assert_eq!(
        status.detail.as_deref(),
        Some("no active propagation relay selected after 0s")
    );

    stop_node(node).await;
    relay.shutdown().await;
}

async fn stop_node(node: Node) {
    let _ = tokio::task::spawn_blocking(move || node.stop()).await;
}

fn assert_packet_received(
    event: NodeEvent,
    expected_source_hex: &str,
    expected_body: &str,
    expected_fields: Option<&[u8]>,
) {
    match event {
        NodeEvent::MessageReceived { message } => {
            assert_eq!(message.source_hex.as_deref(), Some(expected_source_hex));
            assert_eq!(message.body_utf8, expected_body);
        }
        NodeEvent::PacketReceived {
            source_hex,
            bytes,
            fields_bytes,
            ..
        } => {
            assert_eq!(source_hex.as_deref(), Some(expected_source_hex));
            assert_eq!(bytes.as_slice(), expected_body.as_bytes());
            match (expected_fields, fields_bytes.as_deref()) {
                (None, None) => {}
                (Some(expected), Some(actual)) => {
                    let actual = rmp_serde::from_slice::<MsgPackValue>(actual)
                        .expect("actual mission fields");
                    let mut expected = rmp_serde::from_slice::<MsgPackValue>(expected)
                        .expect("expected mission fields");
                    let MsgPackValue::Map(entries) = &mut expected else {
                        panic!("expected mission field map");
                    };
                    entries.push((
                        MsgPackValue::from(FIELD_GROUP),
                        MsgPackValue::from(YELLOW_TEAM_UID),
                    ));
                    assert_eq!(actual, expected);
                }
                (None, Some(_)) => panic!("unexpected mission fields"),
                (Some(_), None) => panic!("expected mission fields"),
            }
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

fn wait_for_operational_ack(
    subscription: &Arc<EventSubscription>,
    command_id: &str,
    command_type: &str,
) -> crate::types::LxmfDeliveryUpdate {
    match wait_for_event(subscription, TEST_TIMEOUT, |event| {
        matches!(
            event,
            NodeEvent::LxmfDelivery { update }
                if matches!(update.status, crate::types::LxmfDeliveryStatus::Acknowledged {})
                    && update.command_id.as_deref() == Some(command_id)
                    && update.command_type.as_deref() == Some(command_type)
        )
    })
    .expect("sender received operational acknowledgement")
    {
        NodeEvent::LxmfDelivery { update } => update,
        other => panic!("unexpected event: {other:?}"),
    }
}

fn build_app_settings() -> AppSettingsRecord {
    AppSettingsRecord {
        display_name: "Atlas-1".to_string(),
        auto_connect_saved: true,
        announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
        tcp_clients: vec!["rns.beleth.net:4242".to_string()],
        broadcast: true,
        transport_node_enabled: true,
        announce_interval_seconds: 1800,
        telemetry: TelemetrySettingsRecord {
            enabled: true,
            publish_interval_seconds: 15,
            accuracy_threshold_meters: Some(10.0),
            stale_after_minutes: 30,
            expire_after_minutes: 180,
        },
        hub: HubSettingsRecord {
            mode: HubMode::Autonomous {},
            identity_hash: String::new(),
            api_base_url: String::new(),
            api_key: String::new(),
            refresh_interval_seconds: 3600,
        },
        teams: crate::types::TeamSettingsRecord::default(),
        checklists: crate::types::ChecklistSettingsRecord::default(),
        rnode: crate::types::RnodeSettingsRecord::default(),
        community: crate::types::CommunitySettingsRecord::default(),
        power: crate::types::PowerPolicyRecord::default(),
    }
}

fn build_saved_peer() -> SavedPeerRecord {
    build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
}

fn build_saved_peer_for(destination_hex: &str) -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: destination_hex.to_string(),
        label: Some("POCO".to_string()),
        saved_at_ms: 1_700_000_000_000,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    }
}

fn build_saved_peer_with_lxmf_route(
    destination_hex: &str,
    identity_hex: &str,
) -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: destination_hex.to_string(),
        label: Some("Routable saved peer".to_string()),
        saved_at_ms: 1_700_000_000_000,
        identity_hex: Some(identity_hex.to_string()),
        lxmf_destination_hex: Some(destination_hex.to_string()),
        app_data: Some("R3AKT,EmergencyMessages,Telemetry".to_string()),
        display_name: Some("Routable saved peer".to_string()),
        last_route_seen_at_ms: Some(1_700_000_000_000),
        last_hops: Some(2),
        circle_tier: CircleTier::Inner {},
    }
}

fn build_status_for_tests() -> NodeStatus {
    NodeStatus {
        readiness: RuntimeReadinessSnapshot::default(),
        running: true,
        name: "Atlas-1".to_string(),
        identity_hex: "99999999999999999999999999999999".to_string(),
        app_destination_hex: "12121212121212121212121212121212".to_string(),
        lxmf_destination_hex: "34343434343434343434343434343434".to_string(),
        interfaces: Vec::new(),
    }
}

fn build_config_fingerprint_for_tests(
    hub_mode: HubMode,
    hub_identity_hash: Option<&str>,
) -> NodeConfigFingerprint {
    NodeConfigFingerprint {
        name: "Atlas-1".to_string(),
        storage_dir: None,
        tcp_clients: Vec::new(),
        broadcast: true,
        transport_node_enabled: true,
        announce_interval_seconds: 1800,
        stale_after_minutes: 30,
        announce_capabilities: "R3AKT,EMergencyMessages,Telemetry".to_string(),
        hub_mode,
        hub_identity_hash: hub_identity_hash.map(str::to_string),
        hub_api_base_url: None,
        hub_api_key: None,
        hub_refresh_interval_seconds: 3600,
        rnode: crate::types::RnodeSettingsRecord::default(),
    }
}

fn build_peer_record(
    destination_hex: &str,
    lxmf_destination_hex: &str,
    saved: bool,
    connected: bool,
    active_link: bool,
) -> PeerRecord {
    PeerRecord {
        destination_hex: destination_hex.to_string(),
        identity_hex: Some(format!("identity-{destination_hex}")),
        lxmf_destination_hex: Some(lxmf_destination_hex.to_string()),
        display_name: Some(format!("peer-{destination_hex}")),
        app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
        state: if connected {
            crate::types::PeerState::Connected {}
        } else {
            crate::types::PeerState::Disconnected {}
        },
        saved,
        stale: false,
        active_link,
        hub_derived: false,
        last_resolution_error: None,
        last_resolution_attempt_at_ms: Some(now_ms()),
        last_seen_at_ms: now_ms(),
        announce_last_seen_at_ms: Some(now_ms()),
        lxmf_last_seen_at_ms: Some(now_ms()),
    }
}

fn build_announce_record(
    destination_hex: &str,
    destination_kind: &str,
    announce_class: AnnounceClass,
    hops: u8,
    received_at_ms: u64,
) -> AnnounceRecord {
    AnnounceRecord {
        destination_hex: destination_hex.to_string(),
        identity_hex: format!("identity-{destination_hex}"),
        destination_kind: destination_kind.to_string(),
        announce_class,
        app_data: "R3AKT,EMergencyMessages,Telemetry".to_string(),
        display_name: Some(format!("peer-{destination_hex}")),
        hops,
        interface_hex: "dddddddddddddddddddddddddddddddd".to_string(),
        received_at_ms,
    }
}

#[test]
fn semi_autonomous_replication_targets_use_propagation_for_unsaved_directory_peers() {
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![build_peer_record(
        "abababababababababababababababab",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        false,
        true,
        true,
    )];
    let snapshot = HubDirectorySnapshot {
        effective_connected_mode: false,
        items: vec![crate::types::HubDirectoryPeerRecord {
            identity: "78787878787878787878787878787878".to_string(),
            destination_hash: "abababababababababababababababab".to_string(),
            display_name: Some("S8".to_string()),
            announce_capabilities: vec!["r3akt".to_string(), "telemetry".to_string()],
            client_type: Some("rem".to_string()),
            registered_mode: Some("semi_autonomous".to_string()),
            last_seen: Some("2026-04-02T12:43:28Z".to_string()),
            status: Some("active".to_string()),
        }],
        received_at_ms: 456,
        ..HubDirectorySnapshot::yellow_only(456)
    };

    let targets = build_runtime_mission_replication_targets(
        &status,
        peers.as_slice(),
        &[],
        Some("99999999999999999999999999999999"),
        Some(&config),
        Some(&snapshot),
    )
    .expect("semi-autonomous targets");

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "abababababababababababababababab"
    );
    assert!(matches!(targets[0].send_mode, SendMode::PropagationOnly {}));
}

#[test]
fn mission_replication_targets_use_propagation_for_observed_lxmf_route_without_active_link() {
    let status = build_status_for_tests();
    let saved_peer = build_saved_peer();
    let mut peer = build_peer_record(
        saved_peer.destination_hex.as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;

    let targets = build_mission_replication_targets(
        &status,
        &[peer],
        &[saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::PropagationOnly {});
}

#[test]
fn mission_replication_targets_use_direct_link_then_relay_for_saved_stored_routes() {
    let status = build_status_for_tests();
    let direct_saved_peer = SavedPeerRecord {
        destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        label: Some("Pixel".to_string()),
        saved_at_ms: 1,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    };
    let relay_saved_peer = SavedPeerRecord {
        destination_hex: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        label: Some("RelayOnly".to_string()),
        saved_at_ms: 2,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    };
    let direct_peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "11111111111111111111111111111111",
        true,
        true,
        true,
    );
    let mut relay_peer = build_peer_record(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "22222222222222222222222222222222",
        true,
        false,
        false,
    );
    relay_peer.stale = true;
    relay_peer.announce_last_seen_at_ms = None;
    relay_peer.lxmf_last_seen_at_ms = None;

    let targets = build_mission_replication_targets(
        &status,
        &[direct_peer, relay_peer],
        &[direct_saved_peer, relay_saved_peer],
        Some("99999999999999999999999999999999"),
    );

    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets[0].app_destination_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(targets[0].send_mode, SendMode::Auto {});
    assert_eq!(
        targets[1].app_destination_hex,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(targets[1].send_mode, SendMode::PropagationOnly {});
}
