use super::*;

    const PLUGIN_A: &str = "org.freetakteam.rem.plugin.map_a";
    const PLUGIN_B: &str = "org.freetakteam.rem.plugin.map_b";

    fn map_capabilities() -> PluginCapabilityRecord {
        PluginCapabilityRecord {
            map_markers_read: true,
            map_markers_write: true,
            map_observer_read: true,
            ..PluginCapabilityRecord::default()
        }
    }

    fn discovered(plugin_id: &str, api_minor: u16) -> DiscoveredPluginRecord {
        DiscoveredPluginRecord {
            plugin_id: plugin_id.to_string(),
            display_name: plugin_id.to_string(),
            version: "1.0.0".to_string(),
            api_major: 1,
            api_minor,
            package_name: plugin_id.to_string(),
            service_class_name: ".MapPluginService".to_string(),
            publisher_fingerprint: "cd".repeat(32),
            publisher_history: Vec::new(),
            android_permissions: Vec::new(),
            declared_capabilities: map_capabilities(),
            messages: Vec::new(),
            configuration_entrypoint: None,
        }
    }

    fn node_with_plugins(name: &str, plugins: &[(&str, u16)]) -> Node {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "rem-plugin-map-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let node = Node::with_storage_dir(Some(dir.to_string_lossy().as_ref())).expect("node");
        node.sync_discovered_plugins(
            plugins
                .iter()
                .map(|(id, minor)| discovered(id, *minor))
                .collect(),
        )
        .expect("discover");
        // Approving a publisher resets every plugin it signed, so approve all first.
        for (id, _) in plugins {
            node.approve_plugin_publisher(id, Some("Test"))
                .expect("approve");
        }
        for (id, _) in plugins {
            node.grant_plugin_capabilities(id, map_capabilities())
                .expect("grant");
            node.set_plugin_enabled(id, true).expect("enable");
        }
        node
    }

    fn call(node: &Node, plugin_id: &str, operation: &str, payload: &Value) -> PluginMapResult {
        handle_plugin_map_operation(node, plugin_id, operation, payload).expect("map operation")
    }

    fn observation(key: &str) -> Value {
        json!({ "idempotencyKey": key, "symbol": "Hostile", "name": "Observation", "lat": 45.0, "lon": 7.5 })
    }

    #[test]
    fn plugins_create_and_manage_only_their_own_markers() {
        let node = node_with_plugins("ownership", &[(PLUGIN_A, 2), (PLUGIN_B, 2)]);
        let created =
            call(&node, PLUGIN_A, "map.markers.create", &observation("obs-1")).expect("create");
        assert_eq!(created["created"], true);
        assert_eq!(created["marker"]["symbol"], "hostile");
        let id = created["marker"]["object_destination_hash"]
            .as_str()
            .expect("id")
            .to_string();

        let replay =
            call(&node, PLUGIN_A, "map.markers.create", &observation("obs-1")).expect("replay");
        assert_eq!(replay["created"], false);
        assert_eq!(replay["marker"]["object_destination_hash"], id.as_str());
        let other =
            call(&node, PLUGIN_B, "map.markers.create", &observation("obs-1")).expect("b create");
        assert_ne!(
            other["marker"]["object_destination_hash"],
            id.as_str(),
            "keys are plugin-scoped"
        );
        assert_eq!(
            node.list_map_markers().expect("all").len(),
            2,
            "plugin markers are shared markers"
        );

        let listed = call(&node, PLUGIN_A, "map.markers.list", &json!({})).expect("list");
        assert_eq!(listed["items"].as_array().expect("items").len(), 1);
        let key = json!({ "objectDestinationHash": id });
        assert_eq!(
            call(&node, PLUGIN_B, "map.markers.get", &key)
                .expect_err("isolated")
                .0,
            "NotFound"
        );
        assert_eq!(
            call(&node, PLUGIN_B, "map.markers.delete", &key)
                .expect_err("isolated")
                .0,
            "NotFound"
        );

        let moved = call(
            &node,
            PLUGIN_A,
            "map.markers.position.update",
            &json!({ "objectDestinationHash": id, "lat": 45.5, "lon": 7.25 }),
        )
        .expect("move");
        assert_eq!(moved["marker"]["position"]["lat"], 45.5);
        let renamed = call(
            &node,
            PLUGIN_A,
            "map.markers.update",
            &json!({ "objectDestinationHash": id, "name": "Ranged target" }),
        )
        .expect("rename");
        assert_eq!(renamed["marker"]["name"], "Ranged target");
        assert_eq!(
            call(&node, PLUGIN_A, "map.markers.delete", &key).expect("delete")["deleted"],
            true
        );
        assert_eq!(
            call(&node, PLUGIN_A, "map.markers.get", &key)
                .expect_err("gone")
                .0,
            "NotFound"
        );
    }

    #[test]
    fn plugin_map_requests_are_validated_and_gated() {
        let node = node_with_plugins("gates", &[(PLUGIN_A, 2), (PLUGIN_B, 1)]);
        let invalid = call(
            &node,
            PLUGIN_A,
            "map.markers.create",
            &json!({ "idempotencyKey": "k", "symbol": "fire", "lat": 91.0, "lon": 0.0 }),
        )
        .expect_err("latitude");
        assert_eq!(
            invalid,
            (
                "InvalidRequest",
                "lat must be between -90 and 90".to_string()
            )
        );
        let missing_key = call(
            &node,
            PLUGIN_A,
            "map.markers.create",
            &json!({ "symbol": "fire", "lat": 1.0, "lon": 1.0 }),
        )
        .expect_err("key");
        assert_eq!(missing_key.1, "idempotencyKey is required");

        let old_api = call(&node, PLUGIN_B, "map.symbols.list", &json!({})).expect_err("api 1.1");
        assert_eq!(old_api.0, "PermissionDenied");
        let unknown = call(&node, "org.example.unknown", "map.markers.list", &json!({}))
            .expect_err("unknown");
        assert_eq!(unknown.0, "PermissionDenied");

        node.grant_plugin_capabilities(PLUGIN_A, PluginCapabilityRecord::default())
            .expect("revoke");
        let revoked = call(&node, PLUGIN_A, "map.markers.list", &json!({})).expect_err("revoked");
        assert_eq!(revoked.0, "PermissionDenied");
        assert!(
            handle_plugin_map_operation(&node, PLUGIN_A, "sensor.publish", &json!({})).is_none()
        );
    }

    #[test]
    fn observer_snapshot_returns_only_the_local_device_fix() {
        let node = node_with_plugins("observer", &[(PLUGIN_A, 2)]);
        let empty = call(&node, PLUGIN_A, "map.observer.snapshot", &json!({})).expect("snapshot");
        assert_eq!(empty["available"], false);
        assert_eq!(
            empty["observer"]["kind"], "localDevice",
            "observer identity is explicit"
        );

        node.with_app_state(|app_state, _| {
            app_state.record_local_telemetry_fix(&TelemetryPositionRecord {
                callsign: "peer".to_string(),
                lat: 1.0,
                lon: 1.0,
                alt: None,
                course: None,
                speed: None,
                accuracy: Some(3.0),
                updated_at_ms: now_ms(),
            })
        })
        .expect("peer telemetry");
        let still_empty =
            call(&node, PLUGIN_A, "map.observer.snapshot", &json!({})).expect("snapshot");
        assert_eq!(
            still_empty["available"], false,
            "peer telemetry is never the observer"
        );

        node.record_local_telemetry_fix(TelemetryPositionRecord {
            callsign: "me".to_string(),
            lat: 45.1,
            lon: 7.6,
            alt: Some(300.0),
            course: None,
            speed: None,
            accuracy: Some(4.5),
            updated_at_ms: now_ms(),
        })
        .expect("local fix");
        let fix = call(&node, PLUGIN_A, "map.observer.snapshot", &json!({})).expect("snapshot");
        assert_eq!(fix["available"], true);
        assert_eq!(fix["lat"], 45.1);
        assert_eq!(fix["horizontalAccuracyM"], 4.5);
        assert_eq!(fix["datum"], "WGS84");
    }

    #[test]
    fn plugin_idempotency_keys_are_scoped_and_bounded() {
        let a = plugin_marker_idempotency_key(PLUGIN_A, "obs-1").expect("a");
        let b = plugin_marker_idempotency_key(PLUGIN_B, "obs-1").expect("b");
        assert_ne!(a, b);
        assert!(r3akt_map_core::idempotency::validate_key(&a).is_ok());
        assert_eq!(
            plugin_marker_idempotency_key(PLUGIN_A, "has space")
                .expect_err("key")
                .0,
            "InvalidRequest"
        );
    }
