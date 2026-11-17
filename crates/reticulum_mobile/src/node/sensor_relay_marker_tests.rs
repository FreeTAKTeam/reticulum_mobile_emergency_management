/// Issue #341: after RCH stops resending `mission.marker.created`, the
/// relayed origin fixes alone must move the object's marker on the map.
#[test]
fn relayed_origin_fix_moves_marker_and_refreshes_marker_projection() {
    use r3akt_map_core::sensor_frame::{OriginAssertion, SensorFrame};
    let (node, _) = setup();
    let (store, bus) = {
        let inner = node.inner.lock().unwrap();
        (inner.app_state.clone(), inner.bus.clone())
    };
    let id = node
        .create_map_marker(MapMarkerCreateRequest {
            idempotency_key: None,
            marker_type: "uav".into(),
            symbol: "uav".into(),
            name: Some("Aircraft".into()),
            category: Some("uav".into()),
            lat: 44.,
            lon: -63.,
            notes: None,
        })
        .unwrap()
        .marker
        .object_destination_hash;
    let hub = "ef".repeat(16);
    let now = now_ms();
    let frame = |fix_at_ms: u64| SensorFrame {
        assertions: vec![OriginAssertion {
            origin_id: id.clone(),
            writer_id: "cd".repeat(16),
            origin_kind: "uav".into(),
            label: "Aircraft".into(),
        }],
        events: vec![SensorEvent::OriginFix(OriginFixEvent {
            origin_id: id.clone(),
            position: position(fix_at_ms),
        })],
        ..Default::default()
    };
    let events = bus.subscribe();
    assert_eq!(store.receive_sensor_frame(&hub, true, &frame(now)).unwrap(), vec![id.clone()]);
    emit_sensor_origin_invalidation(&store, &bus, &id).unwrap();
    let marker = store.get_map_marker(&id).unwrap().unwrap();
    assert_eq!((marker.lat, marker.lon), (45., -64.));
    assert!(events.try_iter().any(|event| matches!(
        event,
        NodeEvent::ProjectionInvalidated { invalidation }
            if invalidation.scope == ProjectionScope::MapMarkers {}
                && invalidation.key.as_deref() == Some(id.as_str())
    )));
    let origin = store.sensor_origin(&id).unwrap().unwrap().position.unwrap();
    assert_eq!((origin.alt_m, origin.accuracy_m), (None, None));

    let older = frame(now - 1_000);
    assert!(store.receive_sensor_frame(&hub, true, &older).unwrap().is_empty());
    assert_eq!(store.sensor_origin(&id).unwrap().unwrap().position.unwrap().fix_at_ms, now);
}
