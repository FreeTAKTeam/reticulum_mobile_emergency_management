use crate::types::{
    LxmfDeliveryMethod, LxmfDeliveryRepresentation, LxmfDeliveryStatus, LxmfDeliveryUpdate,
};
#[test]
fn sensor_metadata_requires_receipt_and_does_not_repeat_for_a_moving_fix() {
    let (node, _) = setup();
    let id = origin(&node);
    let now = now_ms();
    node.sensor_geometry_operation(
        A,
        "map.origins.telemetry.upsert",
        &json!({"originId":id,"position":position(now)}),
    )
    .unwrap();
    node.sensor_geometry_operation(
        A,
        "map.sensors.upsert",
        &json!({"originId":id,"sensor":state("eo",now)}),
    )
    .unwrap();
    let store = node.inner.lock().unwrap().app_state.clone();
    let snapshot = sensor_snapshot(&store, store.sensor_origin(&id).unwrap().unwrap())
        .unwrap()
        .unwrap();
    let metadata = sensor_metadata_hash(&snapshot).unwrap();
    let command = sensor_command(&snapshot, "peer", "scope", &"11".repeat(32)).unwrap();
    store
        .record_sensor_attempt(&id, "peer", &command, &"11".repeat(32), &metadata)
        .unwrap();
    let mut receipt = LxmfDeliveryUpdate {
        message_id_hex: "cd".repeat(32),
        destination_hex: "peer".into(),
        source_hex: None,
        correlation_id: None,
        command_id: Some(command),
        command_type: Some("r3s.telemetry".into()),
        event_uid: None,
        mission_uid: None,
        status: LxmfDeliveryStatus::Sent {},
        transport_state: TransportDeliveryState::SentDirect {},
        application_ack_state: ApplicationAckState::Waiting {},
        method: LxmfDeliveryMethod::Direct {},
        representation: LxmfDeliveryRepresentation::Packet {},
        relay_destination_hex: None,
        fallback_stage: None,
        detail: None,
        sent_at_ms: now,
        updated_at_ms: now,
    };
    store.record_telemetry_delivery(&receipt).unwrap();
    assert!(store
        .sensor_publication_attempt(&id, "peer")
        .unwrap()
        .metadata_hash
        .is_empty());
    receipt.status = LxmfDeliveryStatus::Delivered {};
    store.record_telemetry_delivery(&receipt).unwrap();
    assert_eq!(
        store
            .sensor_publication_attempt(&id, "peer")
            .unwrap()
            .metadata_hash,
        metadata
    );
    let mut moved = position(now + 1000);
    moved.lon += 0.001;
    node.sensor_geometry_operation(
        A,
        "map.origins.telemetry.upsert",
        &json!({"originId":id,"position":moved}),
    )
    .unwrap();
    let snapshot = sensor_snapshot(&store, store.sensor_origin(&id).unwrap().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(sensor_metadata_hash(&snapshot).unwrap(), metadata);
    let next = sensor_command(&snapshot, "peer", "scope", &"22".repeat(32)).unwrap();
    store
        .record_sensor_attempt(&id, "peer", &next, &"22".repeat(32), &metadata)
        .unwrap();
    assert!(store
        .sensor_publication_attempt(&id, "peer")
        .unwrap()
        .metadata_hash
        .is_empty());
    store.record_telemetry_delivery(&receipt).unwrap();
    assert!(
        store
            .sensor_publication_attempt(&id, "peer")
            .unwrap()
            .metadata_hash
            .is_empty(),
        "old-route proof must not confirm reconnect metadata"
    );
    receipt.command_id = Some(next);
    receipt.status = LxmfDeliveryStatus::Failed {};
    store.record_telemetry_delivery(&receipt).unwrap();
    assert!(store
        .sensor_publication_attempt(&id, "peer")
        .unwrap()
        .command_id
        .is_empty());
}
