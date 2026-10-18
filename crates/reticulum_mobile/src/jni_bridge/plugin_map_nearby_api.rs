// Nearby map markers for plugins (capability `mapNearbyRead`). The operator
// grants it once to let a plugin follow every marker around this device, for
// example to mirror the map on a display device, without sharing each marker.
// The centre is always this device's own fix; nothing else is exposed.

const PLUGIN_MAP_NEARBY_LIMIT_MAX: usize = 50;
const PLUGIN_MAP_NEARBY_RADIUS_MAX_M: f64 = 100_000.0;
/// IUGG mean Earth radius; ranking and range filtering only, not surveying.
const PLUGIN_MAP_EARTH_MEAN_RADIUS_M: f64 = 6_371_008.8;

fn plugin_map_distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (phi1, phi2) = (lat1.to_radians(), lat2.to_radians());
    let d_phi = (lat2 - lat1).to_radians();
    let d_lambda = (lon2 - lon1).to_radians();
    let a = (d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lambda / 2.0).sin().powi(2);
    2.0 * PLUGIN_MAP_EARTH_MEAN_RADIUS_M * a.sqrt().min(1.0).asin()
}

/// `map.markers.nearby {radiusM, limit?}` → `{available, radiusM, items}`,
/// items nearest first with `distanceM`. `notes` are omitted to keep the
/// response within the envelope limit.
fn plugin_map_nearby(node: &Node, payload: &Value) -> PluginMapResult {
    let radius = payload
        .get("radiusM")
        .and_then(Value::as_f64)
        .filter(|radius| radius.is_finite() && *radius > 0.0 && *radius <= PLUGIN_MAP_NEARBY_RADIUS_MAX_M)
        .ok_or_else(|| plugin_map_invalid("radiusM must be between 0 and 100000"))?;
    let limit = payload.get("limit").and_then(Value::as_u64).map_or(PLUGIN_MAP_NEARBY_LIMIT_MAX, |limit| {
        usize::try_from(limit)
            .unwrap_or(PLUGIN_MAP_NEARBY_LIMIT_MAX)
            .clamp(1, PLUGIN_MAP_NEARBY_LIMIT_MAX)
    });
    let Some(fix) = node.local_observer_fix().map_err(plugin_map_node_error)? else {
        return Ok(json!({ "available": false, "radiusM": radius, "items": [] }));
    };
    let mut nearby: Vec<(f64, String)> = node
        .list_map_markers()
        .map_err(plugin_map_node_error)?
        .into_iter()
        .filter_map(|marker| {
            let distance = plugin_map_distance_m(fix.lat, fix.lon, marker.lat, marker.lon);
            (distance <= radius).then_some((distance, marker.object_destination_hash))
        })
        .collect();
    nearby.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    nearby.truncate(limit);
    let mut items = Vec::with_capacity(nearby.len());
    for (distance, id) in nearby {
        if let Some(marker) = node.plugin_map_marker_record(&id).map_err(plugin_map_node_error)? {
            let mut item = plugin_marker_json(&marker);
            item["notes"] = Value::Null;
            item["distanceM"] = json!(distance);
            items.push(item);
        }
    }
    Ok(json!({ "available": true, "radiusM": radius, "items": items }))
}

#[cfg(test)]
mod plugin_map_nearby_tests {
    use super::*;

    const PLUGIN: &str = "org.example.nearby";

    fn node(granted: bool) -> Node {
        let mut dir = std::env::temp_dir();
        dir.push(format!("rem-plugin-nearby-{granted}-{}-{}", std::process::id(), now_ms()));
        let node = Node::with_storage_dir(Some(dir.to_string_lossy().as_ref())).expect("node");
        let capabilities = PluginCapabilityRecord {
            map_markers_read: true,
            map_nearby_read: true,
            ..PluginCapabilityRecord::default()
        };
        node.sync_discovered_plugins(vec![DiscoveredPluginRecord {
            plugin_id: PLUGIN.to_string(),
            display_name: PLUGIN.to_string(),
            version: "1.0.0".to_string(),
            api_major: 1,
            api_minor: 3,
            package_name: PLUGIN.to_string(),
            service_class_name: ".NearbyService".to_string(),
            publisher_fingerprint: "ab".repeat(32),
            publisher_history: Vec::new(),
            android_permissions: Vec::new(),
            declared_capabilities: capabilities.clone(),
            messages: Vec::new(),
            configuration_entrypoint: None,
        }])
        .expect("discover");
        node.approve_plugin_publisher(PLUGIN, Some("Test")).expect("approve");
        let granted_capabilities = if granted {
            capabilities
        } else {
            PluginCapabilityRecord { map_markers_read: true, ..PluginCapabilityRecord::default() }
        };
        node.grant_plugin_capabilities(PLUGIN, granted_capabilities).expect("grant");
        node.set_plugin_enabled(PLUGIN, true).expect("enable");
        node
    }

    fn marker(node: &Node, name: &str, lat: f64, lon: f64) {
        node.create_map_marker(crate::types::MapMarkerCreateRequest {
            idempotency_key: None,
            marker_type: "marker".to_string(),
            symbol: "marker".to_string(),
            name: Some(name.to_string()),
            category: None,
            lat,
            lon,
            notes: Some("operator notes".to_string()),
        })
        .expect("marker");
    }

    fn nearby(node: &Node, payload: &Value) -> PluginMapResult {
        handle_plugin_map_operation(node, PLUGIN, "map.markers.nearby", payload).expect("operation")
    }

    #[test]
    fn nearby_markers_are_ranked_bounded_and_need_a_fix() {
        let node = node(true);
        marker(&node, "far", 45.0, 7.62);
        marker(&node, "near", 45.0, 7.601);
        marker(&node, "out of range", 46.0, 7.6);
        let none = nearby(&node, &json!({ "radiusM": 10_000.0 })).expect("no fix");
        assert_eq!(none["available"], false);
        node.record_local_telemetry_fix(TelemetryPositionRecord {
            callsign: "me".to_string(),
            lat: 45.0,
            lon: 7.6,
            alt: None,
            course: None,
            speed: None,
            accuracy: Some(5.0),
            updated_at_ms: now_ms(),
        })
        .expect("fix");
        let result = nearby(&node, &json!({ "radiusM": 10_000.0 })).expect("nearby");
        let names: Vec<&str> = result["items"]
            .as_array()
            .expect("items")
            .iter()
            .map(|item| item["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, vec!["near", "far"]);
        let first = &result["items"][0];
        assert!(first["distanceM"].as_f64().expect("distance") < 100.0);
        assert_eq!(first["notes"], Value::Null);
        let limited = nearby(&node, &json!({ "radiusM": 10_000.0, "limit": 1 })).expect("limited");
        assert_eq!(limited["items"].as_array().expect("items").len(), 1);
        assert_eq!(nearby(&node, &json!({ "radiusM": 0.0 })).expect_err("radius").0, "InvalidRequest");
        assert_eq!(nearby(&node, &json!({})).expect_err("radius").0, "InvalidRequest");
    }

    #[test]
    fn nearby_markers_require_the_operator_grant() {
        let node = node(false);
        assert_eq!(
            nearby(&node, &json!({ "radiusM": 1000.0 })).expect_err("denied").0,
            "PermissionDenied"
        );
    }

    #[test]
    fn distance_is_symmetric_and_metric() {
        let one_degree = plugin_map_distance_m(0.0, 0.0, 0.0, 1.0);
        assert!((one_degree - 111_195.0).abs() < 10.0);
        assert!((plugin_map_distance_m(45.0, 7.6, 45.1, 7.7) - plugin_map_distance_m(45.1, 7.7, 45.0, 7.6)).abs() < 1e-6);
    }
}
