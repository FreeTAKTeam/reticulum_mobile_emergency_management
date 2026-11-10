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
    (
        "NotFound",
        "Marker not found or not created by this plugin".to_string(),
    )
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

fn require_owned_marker(
    node: &Node,
    plugin_id: &str,
    payload: &Value,
) -> Result<String, (&'static str, String)> {
    let id = payload
        .get("objectDestinationHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| plugin_map_invalid("objectDestinationHash is required"))?
        .to_ascii_lowercase();
    if node
        .plugin_owns_map_marker(plugin_id, &id)
        .map_err(plugin_map_node_error)?
    {
        Ok(id)
    } else {
        Err(plugin_map_not_found())
    }
}

/// Plugin-scoped idempotency key: one plugin can never replay or collide
/// with another plugin's observation keys.
fn plugin_marker_idempotency_key(
    plugin_id: &str,
    key: &str,
) -> Result<String, (&'static str, String)> {
    r3akt_map_core::idempotency::validate_key(key)
        .map_err(|error| plugin_map_invalid(error.to_string()))?;
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
    let result = node
        .create_map_marker(request)
        .map_err(plugin_map_node_error)?;
    let id = result.marker.object_destination_hash;
    node.record_plugin_map_marker(plugin_id, &id)
        .map_err(plugin_map_node_error)?;
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
        "map.sensors.upsert"
        | "map.sensors.remove"
        | "map.sensors.manifest.upsert"
        | "map.origins.telemetry.upsert" => {
            node.sensor_geometry_operation(plugin_id, operation, payload)
        }
        "map.symbols.list" => require_plugin_map_capability(node, plugin_id, readable, operation)
            .map(|()| json!({ "items": crate::app_state::map_marker_symbols() })),
        "map.markers.create" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| plugin_map_create(node, plugin_id, payload)),
        "map.markers.get" => require_plugin_map_capability(node, plugin_id, read, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| plugin_map_marker_value(node, &id)),
        "map.markers.list" => require_plugin_map_capability(node, plugin_id, read, operation)
            .and_then(|()| {
                let limit = payload.get("limit").and_then(Value::as_u64).map_or(
                    PLUGIN_MAP_LIST_DEFAULT,
                    |limit| {
                        usize::try_from(limit)
                            .unwrap_or(PLUGIN_MAP_LIST_MAX)
                            .clamp(1, PLUGIN_MAP_LIST_MAX)
                    },
                );
                let markers = node
                    .plugin_map_markers(plugin_id, limit)
                    .map_err(plugin_map_node_error)?;
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
        "map.markers.position.update" => {
            require_plugin_map_capability(node, plugin_id, write, operation)
                .and_then(|()| require_owned_marker(node, plugin_id, payload))
                .and_then(|id| {
                    node.move_map_marker(crate::types::MapMarkerMoveRequest {
                        object_destination_hash: id.clone(),
                        lat: plugin_payload_f64(payload, "lat")?,
                        lon: plugin_payload_f64(payload, "lon")?,
                    })
                    .map_err(plugin_map_node_error)?;
                    plugin_map_marker_value(node, &id)
                })
        }
        "map.markers.delete" => require_plugin_map_capability(node, plugin_id, write, operation)
            .and_then(|()| require_owned_marker(node, plugin_id, payload))
            .and_then(|id| {
                let deleted = node.delete_map_marker(id).map_err(plugin_map_node_error)?;
                Ok(json!({ "deleted": deleted }))
            }),
        "map.observer.snapshot" => {
            require_plugin_map_capability(node, plugin_id, |caps| caps.map_observer_read, operation)
                .and_then(|()| plugin_map_observer_snapshot(node))
        }
        "map.markers.nearby" => {
            require_plugin_map_capability(node, plugin_id, |caps| caps.map_nearby_read, operation)
                .and_then(|()| plugin_map_nearby(node, payload))
        }
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
    include!("plugin_map_tests.rs");
}
