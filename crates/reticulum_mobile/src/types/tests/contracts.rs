use super::{
    ChecklistColumnType, ChecklistMode, ChecklistOriginType, ChecklistSystemColumnKey,
    ChecklistTaskStatus, ChecklistUserTaskStatus, HubMode, InterfaceStatusRecord, NodeError,
    BlockNetworkSettings, CircleTier, CommunitySettingsRecord, OutboundTrafficClass,
    PowerPolicyRecord, RnodeConnectionMode, RuntimeInterfaceReadinessRecord,
    RuntimeReadinessSnapshot, RuntimeReadinessState, SavedPeerRecord,
};

#[test]
fn community_contracts_preserve_legacy_defaults_and_explicit_tiers() {
    let community = serde_json::from_str::<CommunitySettingsRecord>("{}")
        .expect("legacy community defaults");
    assert_eq!(community, CommunitySettingsRecord::default());
    let power = serde_json::from_str::<PowerPolicyRecord>("{}")
        .expect("legacy power defaults");
    assert_eq!(power, PowerPolicyRecord::default());

    let legacy_peer = serde_json::from_value::<SavedPeerRecord>(serde_json::json!({
        "destination_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "label": null,
        "saved_at_ms": 1
    }))
    .expect("legacy peer");
    assert_eq!(legacy_peer.circle_tier, CircleTier::Inner {});

    let outer_peer = serde_json::from_value::<SavedPeerRecord>(serde_json::json!({
        "destination_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "label": null,
        "saved_at_ms": 2,
        "circle_tier": "outer"
    }))
    .expect("current peer");
    assert_eq!(outer_peer.circle_tier, CircleTier::Outer {});
    assert!(serde_json::from_value::<SavedPeerRecord>(serde_json::json!({
        "destination_hex": "cccccccccccccccccccccccccccccccc",
        "label": null,
        "saved_at_ms": 3,
        "circle_tier": "trusted"
    }))
    .is_err());
}

#[test]
fn traffic_class_and_block_network_contracts_are_closed() {
    assert_eq!(OutboundTrafficClass::default(), OutboundTrafficClass::Chat {});
    assert!(serde_json::from_str::<OutboundTrafficClass>("\"unknown\"").is_err());

    let network = BlockNetworkSettings {
        tcp_clients: vec!["mesh.example:4242".to_string()],
        broadcast: true,
        hub_mode: HubMode::Autonomous {},
        hub_identity_hash: None,
        hub_api_base_url: None,
        hub_refresh_interval_seconds: 3600,
        radio: None,
    };
    let serialized = serde_json::to_value(network).expect("network settings");
    for excluded in [
        "api_key",
        "private_key",
        "storage_dir",
        "transport_node_enabled",
        "announce_capabilities",
        "display_name",
        "name",
        "rnode",
        "enabled",
        "connection_mode",
        "peripheral_id",
        "bluetooth_device_id",
        "usb_device_id",
        "pairing_data",
        "filesystem_path",
    ] {
        assert!(serialized.get(excluded).is_none(), "unexpected field {excluded}");
    }
}

#[test]
fn hub_mode_deserialize_migrates_legacy_values() {
    assert!(matches!(
        serde_json::from_str::<HubMode>("\"Disabled\"").expect("disabled mode"),
        HubMode::Autonomous {}
    ));
    assert!(matches!(
        serde_json::from_str::<HubMode>("\"RchLxmf\"").expect("rch lxmf mode"),
        HubMode::SemiAutonomous {}
    ));
    assert!(matches!(
        serde_json::from_str::<HubMode>("\"RchHttp\"").expect("rch http mode"),
        HubMode::SemiAutonomous {}
    ));
    assert!(matches!(
        serde_json::from_str::<HubMode>("\"Connected\"").expect("connected mode"),
        HubMode::Connected {}
    ));
}

#[test]
fn checklist_enums_serialize_as_contract_strings() {
    assert_eq!(
        serde_json::to_string(&ChecklistMode::Online {}).expect("serialize checklist mode"),
        "\"ONLINE\""
    );
    assert_eq!(
        serde_json::to_string(&ChecklistOriginType::RchTemplate {})
            .expect("serialize origin type"),
        "\"RCH_TEMPLATE\""
    );
    assert_eq!(
        serde_json::to_string(&ChecklistTaskStatus::CompleteLate {})
            .expect("serialize task status"),
        "\"COMPLETE_LATE\""
    );
    assert_eq!(
        serde_json::to_string(&ChecklistColumnType::RelativeTime {})
            .expect("serialize column type"),
        "\"RELATIVE_TIME\""
    );
    assert_eq!(
        serde_json::to_string(&ChecklistSystemColumnKey::DueRelativeDtg {})
            .expect("serialize system key"),
        "\"DUE_RELATIVE_DTG\""
    );
}

#[test]
fn checklist_enums_deserialize_from_contract_strings() {
    assert!(matches!(
        serde_json::from_str::<ChecklistMode>("\"online\"").expect("deserialize mode"),
        ChecklistMode::Online {}
    ));
    assert!(matches!(
        serde_json::from_str::<ChecklistUserTaskStatus>("\"COMPLETE\"")
            .expect("deserialize user status"),
        ChecklistUserTaskStatus::Complete {}
    ));
    assert!(matches!(
        serde_json::from_str::<ChecklistTaskStatus>("\"late\"")
            .expect("deserialize task status"),
        ChecklistTaskStatus::Late {}
    ));
}

#[test]
fn rnode_connection_mode_defaults_only_when_legacy_value_is_missing() {
    assert_eq!(
        RnodeConnectionMode::parse(None).expect("missing legacy mode"),
        RnodeConnectionMode::Ble
    );
    assert_eq!(
        RnodeConnectionMode::parse(Some("classic")).expect("classic alias"),
        RnodeConnectionMode::BluetoothClassic
    );
    assert!(matches!(
        RnodeConnectionMode::parse(Some("carrier-pigeon")),
        Err(NodeError::InvalidConfig {})
    ));
}

#[test]
fn runtime_readiness_transitions_from_pending_to_ready_from_typed_interface_state() {
    let mut snapshot = RuntimeReadinessSnapshot {
        state: RuntimeReadinessState::Pending,
        interfaces: vec![
            RuntimeInterfaceReadinessRecord {
                id: "rnode".to_string(),
                label: "LoRa".to_string(),
                state: RuntimeReadinessState::Pending,
                detail: "Starting".to_string(),
                last_error: None,
            },
            RuntimeInterfaceReadinessRecord {
                id: "local".to_string(),
                label: "Reticulum Net".to_string(),
                state: RuntimeReadinessState::Pending,
                detail: "Starting".to_string(),
                last_error: None,
            },
        ],
    };
    snapshot.refresh(
        true,
        &[InterfaceStatusRecord {
            interface_hex: "01".to_string(),
            label: "rnode-ble:Field RNode".to_string(),
            kind: "rnode_ble".to_string(),
            state: "connected".to_string(),
            last_error: None,
            rx_packets: 0,
            rx_bytes: 0,
            last_activity_ms: 0,
        }],
    );

    assert_eq!(snapshot.state, RuntimeReadinessState::Ready);
    assert!(snapshot
        .interfaces
        .iter()
        .all(|record| record.state == RuntimeReadinessState::Ready));
}

#[test]
fn runtime_readiness_is_ready_while_configured_interface_is_unavailable() {
    let mut snapshot = RuntimeReadinessSnapshot {
        state: RuntimeReadinessState::Pending,
        interfaces: vec![
            RuntimeInterfaceReadinessRecord {
                id: "tcp".to_string(),
                label: "TCP community".to_string(),
                state: RuntimeReadinessState::Pending,
                detail: "Starting".to_string(),
                last_error: None,
            },
            RuntimeInterfaceReadinessRecord {
                id: "local".to_string(),
                label: "Reticulum Net".to_string(),
                state: RuntimeReadinessState::Ready,
                detail: "Ready".to_string(),
                last_error: None,
            },
        ],
    };
    snapshot.set_interface_state(
        "tcp",
        RuntimeReadinessState::Failed,
        "Unavailable".to_string(),
        Some("connection refused".to_string()),
        true,
    );

    assert_eq!(snapshot.state, RuntimeReadinessState::Ready);
    assert_eq!(snapshot.interfaces[0].state, RuntimeReadinessState::Failed);
    assert_eq!(
        snapshot.interfaces[0].last_error.as_deref(),
        Some("connection refused")
    );
}

#[test]
fn network_interface_failure_does_not_impersonate_a_runtime_failure() {
    let mut snapshot = RuntimeReadinessSnapshot {
        state: RuntimeReadinessState::Pending,
        interfaces: vec![
            RuntimeInterfaceReadinessRecord {
                id: "tcp".to_string(),
                label: "TCP community".to_string(),
                state: RuntimeReadinessState::Pending,
                detail: "Starting".to_string(),
                last_error: None,
            },
            RuntimeInterfaceReadinessRecord {
                id: "local".to_string(),
                label: "Reticulum Net".to_string(),
                state: RuntimeReadinessState::Pending,
                detail: "Starting".to_string(),
                last_error: None,
            },
        ],
    };

    snapshot.set_interface_state(
        "tcp",
        RuntimeReadinessState::Failed,
        "Unavailable".to_string(),
        Some("connection refused".to_string()),
        false,
    );

    assert_eq!(snapshot.state, RuntimeReadinessState::Pending);
}

#[test]
fn local_runtime_failure_sets_the_aggregate_failure_state() {
    let mut snapshot = RuntimeReadinessSnapshot {
        state: RuntimeReadinessState::Pending,
        interfaces: vec![RuntimeInterfaceReadinessRecord {
            id: "local".to_string(),
            label: "Reticulum Net".to_string(),
            state: RuntimeReadinessState::Pending,
            detail: "Starting".to_string(),
            last_error: None,
        }],
    };

    snapshot.set_interface_state(
        "local",
        RuntimeReadinessState::Failed,
        "Runtime failed".to_string(),
        Some("database corrupt".to_string()),
        false,
    );

    assert_eq!(snapshot.state, RuntimeReadinessState::Failed);
}
