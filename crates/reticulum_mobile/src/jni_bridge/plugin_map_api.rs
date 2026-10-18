// Plugin map API (plugin API 1.2). Plugins create and edit ordinary shared
// markers through the same native service as the REM UI, but only read or
// change markers they created themselves. Marker rules stay in r3akt-map-core.

const PLUGIN_MAP_API_MINOR: u16 = 2;
const PLUGIN_MAP_LIST_DEFAULT: usize = 25;
const PLUGIN_MAP_LIST_MAX: usize = 50;

/// Typed plugin-facing failure: `(code, message)`.
type PluginMapResult = Result<Value, (&'static str, String)>;

fn plugin_map_denied(message: &str) -> (&'static str, String) {
    ("PermissionDenied", message.to_string())
}

fn plugin_map_invalid(message: impl Into<String>) -> (&'static str, String) {
    ("InvalidRequest", message.into())
}

fn plugin_map_not_found() -> (&'static str, String) {
    ("NotFound", "Marker not found or not created by this plugin".to_string())
}

/// Maps a node failure, surfacing the shared crate's validation message.
fn plugin_map_node_error(error: NodeError) -> (&'static str, String) {
    match error {
        NodeError::InvalidConfig {} => {
            let reason = crate::error_context::take_internal_failure("InvalidConfig")
                .map_or_else(|| "invalid request".to_string(), |failure| failure.cause);
            if reason == "Marker not found" {
                plugin_map_not_found()
            } else {
                plugin_map_invalid(reason)
            }
        }
        other => ("HostError", other.to_string()),
    }
}

fn plugin_map_allowed(
    plugin: &InstalledPluginRecord,
    plugin_id: &str,
    capability: fn(&PluginCapabilityRecord) -> bool,
) -> bool {
    plugin.discovered.plugin_id == plugin_id
        && crate::app_state::plugin_map_capability_allowed(plugin, PLUGIN_MAP_API_MINOR, capability)
}

fn require_plugin_map_capability(
    node: &Node,
    plugin_id: &str,
    capability: fn(&PluginCapabilityRecord) -> bool,
    name: &str,
) -> Result<(), (&'static str, String)> {
    let plugins = node.list_plugins().map_err(plugin_map_node_error)?;
    if plugins
        .iter()
        .any(|plugin| plugin_map_allowed(plugin, plugin_id, capability))
    {
        Ok(())
    } else {
        Err(plugin_map_denied(&format!(
            "{name} requires a trusted, enabled API 1.2 plugin with the capability declared and granted"
        )))
    }
}

fn require_owned_marker(node: &Node, plugin_id: &str, payload: &Value) -> Result<String, (&'static str, String)> {
    let id = payload
        .get("objectDestinationHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| plugin_map_invalid("objectDestinationHash is required"))?
        .to_ascii_lowercase();
    if node.plugin_owns_map_marker(plugin_id, &id).map_err(plugin_map_node_error)? {
        Ok(id)
    } else {
        Err(plugin_map_not_found())
    }
}

/// Plugin-scoped idempotency key: one plugin can never replay or collide
/// with another plugin's observation keys.
fn plugin_marker_idempotency_key(plugin_id: &str, key: &str) -> Result<String, (&'static str, String)> {
    r3akt_map_core::idempotency::validate_key(key).map_err(|error| plugin_map_invalid(error.to_string()))?;
    let digest = <sha2::Sha256 as sha2::Digest>::digest(format!("{plugin_id}\n{key}").as_bytes());
    Ok(format!("plugin:{}", hex::encode(digest)))
}

fn plugin_marker_json(marker: &r3akt_map_core::MarkerRecord) -> Value {
    crate::app_state::plugin_map_object_json(marker)
}

fn plugin_payload_f64(payload: &Value, key: &str) -> Result<f64, (&'static str, String)> {
    payload
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| plugin_map_invalid(format!("{key} is required")))
}

fn plugin_payload_text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

fn plugin_map_create(node: &Node, plugin_id: &str, payload: &Value) -> PluginMapResult {
    let key = plugin_payload_text(payload, "idempotencyKey")
        .ok_or_else(|| plugin_map_invalid("idempotencyKey is required"))?;
    let symbol = plugin_payload_text(payload, "symbol").unwrap_or_default();
    let request = crate::types::MapMarkerCreateRequest {
        idempotency_key: Some(plugin_marker_idempotency_key(plugin_id, &key)?),
        marker_type: plugin_payload_text(payload, "markerType").unwrap_or_else(|| symbol.clone()),
        symbol,
        name: plugin_payload_text(payload, "name"),
        category: plugin_payload_text(payload, "category"),
        lat: plugin_payload_f64(payload, "lat")?,
        lon: plugin_payload_f64(payload, "lon")?,
        notes: plugin_payload_text(payload, "notes"),
    };
    let result = node.create_map_marker(request).map_err(plugin_map_node_error)?;
    let id = result.marker.object_destination_hash;
    node.record_plugin_map_marker(plugin_id, &id).map_err(plugin_map_node_error)?;
    let marker = node
        .plugin_map_marker_record(&id)
        .map_err(plugin_map_node_error)?
        .ok_or_else(plugin_map_not_found)?;
    Ok(json!({ "marker": plugin_marker_json(&marker), "created": result.created }))
}

fn plugin_map_marker_value(node: &Node, id: &str) -> PluginMapResult {
    let marker = node
        .plugin_map_marker_record(id)
        .map_err(plugin_map_node_error)?
        .ok_or_else(plugin_map_not_found)?;
    Ok(json!({ "marker": plugin_marker_json(&marker) }))
}

/// The observer is always this REM device: its own identity and its own
/// latest fix. Peer telemetry and plugin input never become the observer.
fn plugin_map_observer_snapshot(node: &Node) -> PluginMapResult {
    let status = node.get_status();
    let observer = json!({
        "kind": "localDevice",
        "identityHex": Some(status.identity_hex).filter(|hex| !hex.is_empty()),
        "lxmfDestinationHex": Some(status.lxmf_destination_hex).filter(|hex| !hex.is_empty()),
    });
    let fix = node.local_observer_fix().map_err(plugin_map_node_error)?;
    Ok(match fix {
        Some(fix) => json!({
            "available": true,
            "observer": observer,
            "source": "device",
            "datum": "WGS84",
            "lat": fix.lat,
            "lon": fix.lon,
            "alt": fix.alt,
            "horizontalAccuracyM": fix.accuracy,
            "course": fix.course,
            "speed": fix.speed,
            "updatedAtMs": fix.updated_at_ms,
            "ageMs": now_ms().saturating_sub(fix.updated_at_ms),
        }),
        None => json!({ "available": false, "observer": observer }),
    })
}

/// Handles `map.*` operations; `None` for anything else.
fn handle_plugin_map_operation(
    node: &Node,
    plugin_id: &str,
    operation: &str,
    payload: &Value,
) -> Option<PluginMapResult> {
    let read = |caps: &PluginCapabilityRecord| caps.map_markers_read;
    let write = |caps: &PluginCapabilityRecord| caps.map_markers_write;
    let readable = |caps: &PluginCapabilityRecord| caps.map_markers_read || caps.map_markers_write;
    let result = match operation {
        "map.symbols.list" => require_plugin_map_capability(node, plugin_id, readable, operation)
            .map(|()| json!({ "items": crate::app_state::map_marker_symbols() })),
        "map.markers.create" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| plugin_map_create(node, plugin_id, payload)),
        "map.markers.get" => require_plugin_map_capability(node, plugin_id, read, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| plugin_map_marker_value(node, &id)),
        "map.markers.list" => require_plugin_map_capability(node, plugin_id, read, operation).and_then(|()| {
            let limit = payload
                .get("limit")
                .and_then(Value::as_u64)
                .map_or(PLUGIN_MAP_LIST_DEFAULT, |limit| {
                    usize::try_from(limit).unwrap_or(PLUGIN_MAP_LIST_MAX).clamp(1, PLUGIN_MAP_LIST_MAX)
                });
            let markers = node.plugin_map_markers(plugin_id, limit).map_err(plugin_map_node_error)?;
            Ok(json!({ "items": markers.iter().map(plugin_marker_json).collect::<Vec<_>>() }))
        }),
        "map.markers.update" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| {
                let name = plugin_payload_text(payload, "name")
                    .ok_or_else(|| plugin_map_invalid("name is required"))?;
                node.rename_map_marker(crate::types::MapMarkerRenameRequest {
                    object_destination_hash: id.clone(),
                    name,
                })
                .map_err(plugin_map_node_error)?;
                plugin_map_marker_value(node, &id)
            }),
        "map.markers.position.update" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| {
                node.move_map_marker(crate::types::MapMarkerMoveRequest {
                    object_destination_hash: id.clone(),
                    lat: plugin_payload_f64(payload, "lat")?,
                    lon: plugin_payload_f64(payload, "lon")?,
                })
                .map_err(plugin_map_node_error)?;
                plugin_map_marker_value(node, &id)
            }),
        "map.markers.delete" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| {
                let deleted = node.delete_map_marker(id).map_err(plugin_map_node_error)?;
                Ok(json!({ "deleted": deleted }))
            }),
        "map.observer.snapshot" => require_plugin_map_capability(
            node,
            plugin_id,
            |caps| caps.map_observer_read,
            operation,
        )
        .and_then(|()| plugin_map_observer_snapshot(node)),
        "map.markers.nearby" => require_plugin_map_capability(
            node,
            plugin_id,
            |caps| caps.map_nearby_read,
            operation,
        )
        .and_then(|()| plugin_map_nearby(node, payload)),
        _ => return handle_plugin_map_session_operation(node, plugin_id, operation, payload),
    };
    Some(result)
}

fn plugin_map_response_json(request_id: &str, result: PluginMapResult) -> Value {
    match result {
        Ok(value) => plugin_host_response_json(request_id, Ok(value)),
        Err((code, message)) => json!({
            "protocolVersion": 1,
            "requestId": request_id,
            "ok": false,
            "error": { "code": code, "message": message },
        }),
    }
}

#[cfg(test)]
mod plugin_map_tests {
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
        dir.push(format!("rem-plugin-map-{name}-{}-{}", std::process::id(), now_ms()));
        let node = Node::with_storage_dir(Some(dir.to_string_lossy().as_ref())).expect("node");
        node.sync_discovered_plugins(
            plugins.iter().map(|(id, minor)| discovered(id, *minor)).collect(),
        )
        .expect("discover");
        // Approving a publisher resets every plugin it signed, so approve all first.
        for (id, _) in plugins {
            node.approve_plugin_publisher(id, Some("Test")).expect("approve");
        }
        for (id, _) in plugins {
            node.grant_plugin_capabilities(id, map_capabilities()).expect("grant");
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
        let created = call(&node, PLUGIN_A, "map.markers.create", &observation("obs-1")).expect("create");
        assert_eq!(created["created"], true);
        assert_eq!(created["marker"]["symbol"], "hostile");
        let id = created["marker"]["object_destination_hash"].as_str().expect("id").to_string();

        let replay = call(&node, PLUGIN_A, "map.markers.create", &observation("obs-1")).expect("replay");
        assert_eq!(replay["created"], false);
        assert_eq!(replay["marker"]["object_destination_hash"], id.as_str());
        let other = call(&node, PLUGIN_B, "map.markers.create", &observation("obs-1")).expect("b create");
        assert_ne!(other["marker"]["object_destination_hash"], id.as_str(), "keys are plugin-scoped");
        assert_eq!(node.list_map_markers().expect("all").len(), 2, "plugin markers are shared markers");

        let listed = call(&node, PLUGIN_A, "map.markers.list", &json!({})).expect("list");
        assert_eq!(listed["items"].as_array().expect("items").len(), 1);
        let key = json!({ "objectDestinationHash": id });
        assert_eq!(call(&node, PLUGIN_B, "map.markers.get", &key).expect_err("isolated").0, "NotFound");
        assert_eq!(
            call(&node, PLUGIN_B, "map.markers.delete", &key).expect_err("isolated").0,
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
        assert_eq!(call(&node, PLUGIN_A, "map.markers.delete", &key).expect("delete")["deleted"], true);
        assert_eq!(call(&node, PLUGIN_A, "map.markers.get", &key).expect_err("gone").0, "NotFound");
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
        assert_eq!(invalid, ("InvalidRequest", "lat must be between -90 and 90".to_string()));
        let missing_key = call(&node, PLUGIN_A, "map.markers.create", &json!({ "symbol": "fire", "lat": 1.0, "lon": 1.0 }))
            .expect_err("key");
        assert_eq!(missing_key.1, "idempotencyKey is required");

        let old_api = call(&node, PLUGIN_B, "map.symbols.list", &json!({})).expect_err("api 1.1");
        assert_eq!(old_api.0, "PermissionDenied");
        let unknown = call(&node, "org.example.unknown", "map.markers.list", &json!({})).expect_err("unknown");
        assert_eq!(unknown.0, "PermissionDenied");

        node.grant_plugin_capabilities(PLUGIN_A, PluginCapabilityRecord::default()).expect("revoke");
        let revoked = call(&node, PLUGIN_A, "map.markers.list", &json!({})).expect_err("revoked");
        assert_eq!(revoked.0, "PermissionDenied");
        assert!(handle_plugin_map_operation(&node, PLUGIN_A, "sensor.publish", &json!({})).is_none());
    }

    #[test]
    fn observer_snapshot_returns_only_the_local_device_fix() {
        let node = node_with_plugins("observer", &[(PLUGIN_A, 2)]);
        let empty = call(&node, PLUGIN_A, "map.observer.snapshot", &json!({})).expect("snapshot");
        assert_eq!(empty["available"], false);
        assert_eq!(empty["observer"]["kind"], "localDevice", "observer identity is explicit");

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
        let still_empty = call(&node, PLUGIN_A, "map.observer.snapshot", &json!({})).expect("snapshot");
        assert_eq!(still_empty["available"], false, "peer telemetry is never the observer");

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
            plugin_marker_idempotency_key(PLUGIN_A, "has space").expect_err("key").0,
            "InvalidRequest"
        );
    }
}
