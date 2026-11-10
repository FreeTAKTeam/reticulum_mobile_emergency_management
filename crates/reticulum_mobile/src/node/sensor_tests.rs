#[cfg(test)]
mod sensor_tests {
    use super::*;
    const A: &str = "org.rem.sensor.a";
    const B: &str = "org.rem.sensor.b";
    fn capabilities() -> PluginCapabilityRecord {
        PluginCapabilityRecord {
            map_markers_write: true,
            map_markers_read: true,
            sensor_geometry_publish: true,
            origin_telemetry_publish: true,
            sensor_geometry_share: true,
            sensor_video_share: true,
            lxmf_send: true,
            ..Default::default()
        }
    }
    fn setup() -> (Node, String) {
        let path = std::env::temp_dir().join(format!(
            "rem-r3s-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let node = Node::with_storage_dir(path.to_str()).unwrap();
        node.sync_discovered_plugins(
            [A, B]
                .iter()
                .map(|id| DiscoveredPluginRecord {
                    plugin_id: (*id).into(),
                    display_name: (*id).into(),
                    version: "1".into(),
                    api_major: 1,
                    api_minor: 4,
                    package_name: (*id).into(),
                    service_class_name: ".Service".into(),
                    publisher_fingerprint: "cd".repeat(32),
                    publisher_history: vec![],
                    android_permissions: vec![],
                    declared_capabilities: capabilities(),
                    messages: vec![],
                    configuration_entrypoint: None,
                })
                .collect(),
        )
        .unwrap();
        for id in [A, B] {
            node.approve_plugin_publisher(id, None).unwrap();
        }
        for id in [A, B] {
            node.grant_plugin_capabilities(id, capabilities()).unwrap();
            node.set_plugin_enabled(id, true).unwrap();
        }
        let writer = "ab".repeat(16);
        let store = node.inner.lock().unwrap().app_state.clone();
        store.remember_local_sensor_writer(&writer).unwrap();
        (node, writer)
    }
    fn state(id: &str, time: u64) -> SensorState {
        SensorState {
            sensor_id: id.into(),
            sample_at_ms: time,
            azimuth_deg: Some(0.),
            elevation_deg: Some(-30.),
            roll_deg: Some(12.),
            horizontal_fov_deg: Some(20.),
            vertical_fov_deg: Some(10.),
            slant_range_m: Some(2000.),
            image_north_deg: Some(180.),
            range_kind: r3akt_map_core::sensor::RangeKind::MeasuredTarget,
        }
    }
    fn origin(node: &Node) -> String {
        let marker = node
            .create_map_marker(MapMarkerCreateRequest {
                idempotency_key: None,
                marker_type: "uav".into(),
                symbol: "uav".into(),
                name: Some("Drone B".into()),
                category: Some("uav".into()),
                lat: 44.,
                lon: -63.,
                notes: None,
            })
            .unwrap()
            .marker;
        node.record_plugin_map_marker(A, &marker.object_destination_hash)
            .unwrap();
        marker.object_destination_hash
    }
    fn position(time: u64) -> OriginPosition {
        OriginPosition {
            lat: 45.,
            lon: -64.,
            alt_m: None,
            altitude_datum: r3akt_map_core::sensor::AltitudeDatum::Unknown,
            fix_at_ms: time,
            speed_mps: Some(20.),
            course_deg: Some(90.),
            accuracy_m: None,
        }
    }
    #[test]
    fn sensor_plugins_are_isolated_monotonic_and_revocable() {
        let (node, writer) = setup();
        let id = origin(&node);
        let now = now_ms();
        node.sensor_geometry_operation(
            A,
            "map.origins.telemetry.upsert",
            &json!({"originId":id,"position":position(now)}),
        )
        .unwrap();
        for sensor in ["eo", "ir", "thermal", "range"] {
            node.sensor_geometry_operation(
                A,
                "map.sensors.upsert",
                &json!({"originId":id,"sensor":state(sensor,now)}),
            )
            .unwrap();
        }
        assert!(node
            .sensor_geometry_operation(
                A,
                "map.sensors.upsert",
                &json!({"originId":id,"sensor":state("fifth",now)})
            )
            .is_err());
        assert!(node
            .sensor_geometry_operation(
                B,
                "map.sensors.upsert",
                &json!({"originId":id,"sensor":state("eo",now+1)})
            )
            .is_err());
        assert!(node
            .sensor_geometry_operation(
                A,
                "map.origins.telemetry.upsert",
                &json!({"originId":"self","position":position(now)})
            )
            .is_err());
        let store = node.inner.lock().unwrap().app_state.clone();
        assert!(store.local_observer_fix().unwrap().is_none());
        let saved = store.sensor_origin(&id).unwrap().unwrap();
        assert_eq!(saved.writer_id, writer);
        assert_eq!(saved.sensors.len(), 4);
        assert!(sensor_snapshot(&store, saved.clone()).unwrap().is_some());
        node.grant_plugin_capabilities(
            A,
            PluginCapabilityRecord {
                origin_telemetry_publish: false,
                ..capabilities()
            },
        )
        .unwrap();
        assert!(
            sensor_snapshot(&store, saved).unwrap().is_none(),
            "marker metadata cannot disclose a revoked producer fix"
        );
        node.set_plugin_enabled(A, false).unwrap();
        assert!(node
            .sensor_geometry_operation(
                A,
                "map.sensors.remove",
                &json!({"originId":id,"sensorId":"eo"})
            )
            .is_err());
    }
    #[test]
    fn sensor_frame_preserves_drone_identity_and_tombstones() {
        let (node, writer) = setup();
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
        let command = sensor_command(&snapshot, "recipient", "scope", &"11".repeat(32)).unwrap();
        let bytes = sensor_fields(&snapshot, &command, true, "yellow").unwrap();
        assert_eq!(
            parse_mission_sync_metadata(&bytes)
                .unwrap()
                .command_id
                .as_deref(),
            Some(command.as_str())
        );
        let frame = r3akt_map_core::sensor_frame::decode(&bytes, &writer).unwrap();
        assert_eq!(frame.samples[0].origin_id, id);
        let receiver = AppStateStore::new(
            std::env::temp_dir()
                .join(format!(
                    "rem-r3s-rx-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ))
                .to_str(),
        )
        .unwrap();
        assert!(
            receiver
                .receive_sensor_frame(&writer, false, &frame)
                .unwrap()
                .is_empty(),
            "a tuple cannot bootstrap object ownership"
        );
        let marker = store.map_marker_core_record(&id).unwrap().unwrap();
        receiver
            .apply_map_marker_replica(&marker.api_value())
            .unwrap();
        receiver
            .remember_sensor_marker_writer(&id, &writer)
            .unwrap();
        assert_eq!(
            receiver
                .receive_sensor_frame(&writer, false, &frame)
                .unwrap(),
            vec![id.clone()]
        );
        assert!(receiver.local_observer_fix().unwrap().is_none());
        assert!(receiver.get_telemetry_positions().unwrap().is_empty());
        let received = receiver.sensor_origin(&id).unwrap().unwrap();
        assert_eq!(received.position.unwrap().alt_m, None);
        assert!(receiver
            .receive_sensor_frame(&"ef".repeat(16), false, &frame)
            .unwrap()
            .is_empty());
        node.delete_map_marker(id.clone()).unwrap();
        let deleted = sensor_snapshot(&store, store.sensor_origin(&id).unwrap().unwrap())
            .unwrap()
            .unwrap();
        assert!(deleted
            .events
            .iter()
            .any(|e| e["event_type"] == sensor_wire::ORIGIN_REMOVE_EVENT));
        let deletion = r3akt_map_core::sensor_frame::decode(
            &sensor_fields(&deleted, "r3s:test", true, "yellow").unwrap(),
            &writer,
        )
        .unwrap();
        receiver
            .receive_sensor_frame(&writer, false, &deletion)
            .unwrap();
        assert!(receiver
            .sensor_origin(&id)
            .unwrap()
            .unwrap()
            .deleted_at_ms
            .is_some());
        assert!(receiver
            .receive_sensor_frame(&writer, false, &frame)
            .unwrap()
            .is_empty());
        assert!(receiver
            .apply_map_marker_replica(&marker.api_value())
            .unwrap()
            .is_none());
    }
    #[test]
    fn relay_replays_deleted_origin_without_blocking_a_live_origin() {
        use r3akt_map_core::sensor_frame::{OriginAssertion, SensorFrame};
        let (node, _) = setup();
        let store = node.inner.lock().unwrap().app_state.clone();
        let old = "ac".repeat(16);
        let new = "bd".repeat(16);
        let hub = "ef".repeat(16);
        let now = now_ms();
        let assertion = |id: &String| OriginAssertion {
            origin_id: id.clone(),
            writer_id: id.clone(),
            origin_kind: "device".into(),
            label: id.clone(),
        };
        let first = SensorFrame {
            assertions: vec![assertion(&old)],
            events: vec![SensorEvent::OriginRemoved(
                sensor_wire::OriginRemovedEvent {
                    origin_id: old.clone(),
                    deleted_at_ms: now,
                },
            )],
            ..Default::default()
        };
        store.receive_sensor_frame(&hub, true, &first).unwrap();
        let batch = SensorFrame {
            assertions: vec![assertion(&old), assertion(&new)],
            events: vec![
                first.events[0].clone(),
                SensorEvent::OriginFix(OriginFixEvent {
                    origin_id: new.clone(),
                    position: position(now),
                }),
            ],
            ..Default::default()
        };
        assert_eq!(
            store.receive_sensor_frame(&hub, true, &batch).unwrap(),
            vec![new.clone()]
        );
        assert!(store
            .sensor_origin(&old)
            .unwrap()
            .unwrap()
            .deleted_at_ms
            .is_some());
        assert_eq!(
            store.sensor_origin(&new).unwrap().unwrap().position,
            Some(position(now))
        );
    }
    #[test]
    fn video_approval_changes_metadata_revision_and_deletion_dominates_it() {
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
        node.sensor_geometry_operation(A,"map.sensors.manifest.upsert",&json!({"originId":id,"manifest":{"sensorId":"eo","label":"Camera","video":{"sensorId":"eo","protocol":"rtsp","endpointRef":"rtsp://camera.local/live"}}})).unwrap();
        let store = node.inner.lock().unwrap().app_state.clone();
        let snapshot = || {
            sensor_snapshot(&store, store.sensor_origin(&id).unwrap().unwrap())
                .unwrap()
                .unwrap()
        };
        let manifest = |s: &SensorSnapshot| {
            s.events
                .iter()
                .find(|e| e["event_type"] == sensor_wire::MANIFEST_EVENT)
                .unwrap()["payload"]
                .clone()
        };
        let hidden = manifest(&snapshot());
        assert!(hidden["manifest"]["video"].is_null());
        node.set_sensor_video_sharing(id.clone(), "eo".into(), true)
            .unwrap();
        let approved = manifest(&snapshot());
        assert!(!approved["manifest"]["video"].is_null());
        assert!(approved["updatedAtMs"].as_u64() > hidden["updatedAtMs"].as_u64());
        node.set_sensor_video_sharing(id.clone(), "eo".into(), false)
            .unwrap();
        let revoked = manifest(&snapshot());
        assert!(revoked["manifest"]["video"].is_null());
        assert!(revoked["updatedAtMs"].as_u64() > approved["updatedAtMs"].as_u64());
        node.sensor_geometry_operation(
            A,
            "map.sensors.remove",
            &json!({"originId":id,"sensorId":"eo"}),
        )
        .unwrap();
        assert!(
            store.sensor_origin(&id).unwrap().unwrap().sensor_tombstones["eo"]
                > revoked["updatedAtMs"].as_u64().unwrap()
        );
    }
    include!("sensor_receipt_tests.rs");
    include!("sensor_routing_tests.rs");
}
