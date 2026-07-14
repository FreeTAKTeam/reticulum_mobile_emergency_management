#[tokio::test]
async fn mission_recovery_sends_do_not_wait_on_saturated_mission_lanes() {
    let permits = SendTaskPermits::with_limits(1, 1);
    let _direct = acquire_send_task_permit(&permits, SendTaskClass::Mission)
        .await
        .expect("saturate direct mission pool");
    let _propagation = acquire_send_task_permit(&permits, SendTaskClass::MissionPropagation)
        .await
        .expect("saturate propagation mission pool");

    let recovery = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::MissionRecovery),
    )
    .await
    .expect("recovery permit should not wait on direct or propagation saturation")
    .expect("recovery permit acquisition should succeed");

    let blocked_recovery = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::MissionRecovery),
    )
    .await;
    assert!(
        blocked_recovery.is_err(),
        "recovery pool should remain saturated while the original permit is held"
    );
    drop(recovery);
}

#[tokio::test]
async fn accepted_ack_sends_do_not_wait_on_saturated_direct_mission_capacity() {
    let permits = SendTaskPermits::with_limits(1, 1);
    let _mission = acquire_send_task_permit(&permits, SendTaskClass::Mission)
        .await
        .expect("saturate direct mission pool");

    let ack = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_send_task_permit(&permits, SendTaskClass::MissionAck),
    )
    .await
    .expect("accepted acknowledgement should not wait on mission pool saturation")
    .expect("accepted acknowledgement permit acquisition should succeed");
    drop(ack);
}

#[test]
fn mission_delivery_failures_do_not_emit_global_send_bytes_error() {
    assert!(!should_emit_global_send_bytes_error(SendTaskClass::Mission));
    assert!(!should_emit_global_send_bytes_error(
        SendTaskClass::MissionAck
    ));
    assert!(!should_emit_global_send_bytes_error(
        SendTaskClass::MissionPropagation
    ));
    assert!(!should_emit_global_send_bytes_error(
        SendTaskClass::MissionRecovery
    ));
    assert!(should_emit_global_send_bytes_error(SendTaskClass::General));
}

fn send_peer(
    destination_hex: &str,
    identity_hex: Option<&str>,
    lxmf_destination_hex: Option<&str>,
    stale: bool,
    active_link: bool,
    announce_last_seen_at_ms: Option<u64>,
) -> sdkmsg::PeerRecord {
    sdkmsg::PeerRecord {
        destination_hex: destination_hex.to_string(),
        identity_hex: identity_hex.map(ToOwned::to_owned),
        lxmf_destination_hex: lxmf_destination_hex.map(ToOwned::to_owned),
        display_name: Some("Peer".to_string()),
        app_data: Some("R3AKT,EMergencyMessages".to_string()),
        state: if active_link {
            sdkmsg::PeerState::Connected
        } else {
            sdkmsg::PeerState::Disconnected
        },
        saved: false,
        stale,
        active_link,
        last_resolution_error: None,
        last_resolution_attempt_at_ms: None,
        last_seen_at_ms: announce_last_seen_at_ms.unwrap_or_default(),
        announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: lxmf_destination_hex.map(|_| now_ms()),
    }
}
