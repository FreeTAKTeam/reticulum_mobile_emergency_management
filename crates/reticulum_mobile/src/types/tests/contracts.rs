use super::{
    ChecklistColumnType, ChecklistMode, ChecklistOriginType, ChecklistSystemColumnKey,
    ChecklistTaskStatus, ChecklistUserTaskStatus, HubMode, InterfaceStatusRecord, NodeError,
    RnodeConnectionMode, RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot,
    RuntimeReadinessState,
};

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
fn runtime_readiness_stays_ready_when_one_of_multiple_interfaces_is_usable() {
    let mut snapshot = RuntimeReadinessSnapshot {
        state: RuntimeReadinessState::Pending,
        interfaces: vec![
            RuntimeInterfaceReadinessRecord {
                id: "rnode".to_string(),
                label: "LoRa".to_string(),
                state: RuntimeReadinessState::Ready,
                detail: "Connected".to_string(),
                last_error: None,
            },
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
    assert_eq!(snapshot.interfaces[1].state, RuntimeReadinessState::Failed);
}
