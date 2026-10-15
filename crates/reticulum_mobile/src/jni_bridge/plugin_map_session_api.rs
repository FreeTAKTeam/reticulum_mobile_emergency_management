// Plugin map sessions (plugin API 1.3). Only the REM UI starts a session,
// with objects the operator selected; the plugin can list its sessions,
// read the selected objects, and stop a session. Session rules live in
// app_state/plugin_map_sessions.rs; this file is the Binder/JNI surface.

const PLUGIN_MAP_SESSION_DRAIN_LIMIT: usize = 32;

fn plugin_map_session_payload_id(payload: &Value) -> Result<String, (&'static str, String)> {
    payload
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| plugin_map_invalid("sessionId is required"))
}

fn plugin_map_session_not_found() -> (&'static str, String) {
    (
        "NotFound",
        "Map session not found or not started for this plugin".to_string(),
    )
}

/// Handles `map.session.*` plugin requests; `None` for anything else.
fn handle_plugin_map_session_operation(
    node: &Node,
    plugin_id: &str,
    operation: &str,
    payload: &Value,
) -> Option<PluginMapResult> {
    if !matches!(
        operation,
        "map.session.list" | "map.session.objects.get" | "map.session.stop"
    ) {
        return None;
    }
    Some(
        require_plugin_map_session_capability(node, plugin_id, operation).and_then(|()| {
            match operation {
                "map.session.list" => {
                    let sessions = node
                        .list_plugin_map_sessions(Some(plugin_id))
                        .map_err(plugin_map_node_error)?;
                    Ok(json!({ "items": sessions }))
                }
                "map.session.objects.get" => {
                    let session_id = plugin_map_session_payload_id(payload)?;
                    let (session, objects) = node
                        .plugin_map_session_objects(plugin_id, &session_id)
                        .map_err(plugin_map_node_error)?
                        .ok_or_else(plugin_map_session_not_found)?;
                    Ok(json!({ "session": session, "objects": objects }))
                }
                _ => {
                    let session_id = plugin_map_session_payload_id(payload)?;
                    // The plugin asked for this, so it gets no stop event back.
                    let stopped = node
                        .stop_plugin_map_sessions(
                            Some(plugin_id),
                            Some(&session_id),
                            "plugin",
                            false,
                        )
                        .map_err(plugin_map_node_error)?;
                    if stopped {
                        Ok(json!({ "stopped": true }))
                    } else {
                        Err(plugin_map_session_not_found())
                    }
                }
            }
        }),
    )
}

fn require_plugin_map_session_capability(
    node: &Node,
    plugin_id: &str,
    name: &str,
) -> Result<(), (&'static str, String)> {
    let plugins = node.list_plugins().map_err(plugin_map_node_error)?;
    let allowed = plugins.iter().any(|plugin| {
        plugin.discovered.plugin_id == plugin_id
            && crate::app_state::plugin_map_capability_allowed(
                plugin,
                crate::app_state::PLUGIN_MAP_SESSION_API_MINOR,
                |caps| caps.map_selection_read,
            )
    });
    if allowed {
        Ok(())
    } else {
        Err(plugin_map_denied(&format!(
            "{name} requires a trusted, enabled API 1.3 plugin with mapSelectionRead declared and granted"
        )))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginMapSessionStartInput {
    plugin_id: String,
    object_destination_hashes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginMapSessionStopInput {
    plugin_id: Option<String>,
    session_id: Option<String>,
    reason: String,
    #[serde(default)]
    notify: bool,
}

/// Starts a session for objects the operator selected in the REM UI.
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_startPluginMapSessionJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<PluginMapSessionStartInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result =
        node.start_plugin_map_session(input.plugin_id.trim(), &input.object_destination_hashes);
    map_marker_result(&mut env, result)
}

/// Stops sessions by id and/or plugin. The host passes `notify: false` when
/// the plugin connection is already gone (Binder death, unbind, node stop).
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_stopPluginMapSessionsJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<PluginMapSessionStopInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    if input.plugin_id.is_none() && input.session_id.is_none() {
        set_last_error("InvalidConfig", "pluginId or sessionId is required");
        return ptr::null_mut();
    }
    let node = initialized_node_or_return!();
    let result = node
        .stop_plugin_map_sessions(
            input.plugin_id.as_deref(),
            input.session_id.as_deref(),
            input.reason.trim(),
            input.notify,
        )
        .map(|stopped| json!({ "stopped": stopped }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listPluginMapSessionsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    let result = node
        .list_plugin_map_sessions(None)
        .map(|items| json!({ "items": items }));
    map_marker_result(&mut env, result)
}

/// Hands queued session events to the host for delivery over Binder.
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_drainPluginMapSessionEventsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    let result = node
        .drain_plugin_map_session_events(PLUGIN_MAP_SESSION_DRAIN_LIMIT)
        .map(|events| {
            let items = events
                .into_iter()
                .map(|(plugin_id, event)| json!({ "pluginId": plugin_id, "event": event }))
                .collect::<Vec<_>>();
            json!({ "items": items })
        });
    map_marker_result(&mut env, result)
}

#[cfg(test)]
mod plugin_map_session_tests {
    use super::*;

    const RANGEFINDER: &str = "org.freetakteam.rem.plugin.rangefinder";
    const OTHER: &str = "org.freetakteam.rem.plugin.other";
    const LEGACY: &str = "org.freetakteam.rem.plugin.legacy";

    fn capabilities() -> PluginCapabilityRecord {
        PluginCapabilityRecord {
            map_markers_read: true,
            map_markers_write: true,
            map_observer_read: true,
            map_selection_read: true,
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
            declared_capabilities: capabilities(),
            messages: Vec::new(),
            configuration_entrypoint: None,
        }
    }

    fn node(name: &str) -> Node {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "rem-plugin-map-session-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let node = Node::with_storage_dir(Some(dir.to_string_lossy().as_ref())).expect("node");
        let plugins = [
            (RANGEFINDER, 3),
            (OTHER, 3),
            (LEGACY, 2),
            ("org.example.future", 4),
        ];
        node.sync_discovered_plugins(
            plugins
                .iter()
                .map(|(id, minor)| discovered(id, *minor))
                .collect(),
        )
        .expect("discover");
        // One publisher signs every test plugin, so one approval trusts them all.
        node.approve_plugin_publisher(RANGEFINDER, Some("Test"))
            .expect("approve");
        for (id, _) in &plugins[..3] {
            node.grant_plugin_capabilities(id, capabilities())
                .expect("grant");
            node.set_plugin_enabled(id, true).expect("enable");
        }
        node
    }

    fn call(node: &Node, plugin_id: &str, operation: &str, payload: &Value) -> PluginMapResult {
        handle_plugin_map_operation(node, plugin_id, operation, payload).expect("map operation")
    }

    fn operator_marker(node: &Node, name: &str) -> String {
        node.create_map_marker(crate::types::MapMarkerCreateRequest {
            idempotency_key: None,
            marker_type: "medic".to_string(),
            symbol: "medic".to_string(),
            name: Some(name.to_string()),
            category: None,
            lat: 45.0,
            lon: 7.0,
            notes: None,
        })
        .expect("operator marker")
        .marker
        .object_destination_hash
    }

    fn drained(node: &Node) -> Vec<(String, Value)> {
        node.drain_plugin_map_session_events(64).expect("drain")
    }

    #[test]
    fn fixture_flow_creates_a_marker_and_reads_only_selected_objects() {
        let node = node("fixture");
        let symbols = call(&node, RANGEFINDER, "map.symbols.list", &json!({})).expect("symbols");
        assert!(!symbols["items"].as_array().expect("catalog").is_empty());
        let created = call(
            &node,
            RANGEFINDER,
            "map.markers.create",
            &json!({ "idempotencyKey": "fix-1", "symbol": "hostile", "name": "Target", "lat": 45.2, "lon": 7.7 }),
        )
        .expect("create");
        let own = created["marker"]["object_destination_hash"]
            .as_str()
            .expect("id")
            .to_string();
        let selected = operator_marker(&node, "Selected by operator");
        let hidden = operator_marker(&node, "Not selected");

        let no_session = call(&node, RANGEFINDER, "map.session.list", &json!({})).expect("list");
        assert_eq!(no_session["items"], json!([]));
        let session = node
            .start_plugin_map_session(RANGEFINDER, &[selected.clone()])
            .expect("operator start");
        let events = drained(&node);
        assert_eq!(events[0].0, RANGEFINDER);
        assert_eq!(events[0].1["event"], "map.session.started");

        let read = call(
            &node,
            RANGEFINDER,
            "map.session.objects.get",
            &json!({ "sessionId": session.session_id }),
        )
        .expect("objects");
        let ids = read["objects"]
            .as_array()
            .expect("objects")
            .iter()
            .map(|object| {
                object["object_destination_hash"]
                    .as_str()
                    .expect("id")
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![selected.clone()]);
        let unselected = call(
            &node,
            RANGEFINDER,
            "map.markers.get",
            &json!({ "objectDestinationHash": hidden }),
        )
        .expect_err("unselected operator marker");
        assert_eq!(unselected.0, "NotFound");
        let selected_write = call(
            &node,
            RANGEFINDER,
            "map.markers.delete",
            &json!({ "objectDestinationHash": selected }),
        )
        .expect_err("selection grants read only");
        assert_eq!(selected_write.0, "NotFound");
        assert!(call(
            &node,
            RANGEFINDER,
            "map.markers.get",
            &json!({ "objectDestinationHash": own })
        )
        .is_ok());

        let foreign = call(
            &node,
            OTHER,
            "map.session.objects.get",
            &json!({ "sessionId": session.session_id }),
        )
        .expect_err("other plugin");
        assert_eq!(foreign.0, "NotFound");

        node.rename_map_marker(crate::types::MapMarkerRenameRequest {
            object_destination_hash: selected.clone(),
            name: "Moved on".to_string(),
        })
        .expect("operator rename");
        let events = drained(&node);
        assert_eq!(
            events.len(),
            1,
            "only the session that selected the object hears about it"
        );
        assert_eq!(events[0].1["event"], "map.session.object.updated");
        node.move_map_marker(crate::types::MapMarkerMoveRequest {
            object_destination_hash: hidden,
            lat: 1.0,
            lon: 1.0,
        })
        .expect("move hidden");
        assert!(drained(&node).is_empty());
    }

    #[test]
    fn sessions_stop_from_operator_plugin_revocation_and_binder_death() {
        let node = node("stop");
        let marker = operator_marker(&node, "Target");
        let first = node
            .start_plugin_map_session(RANGEFINDER, &[marker.clone()])
            .expect("start");
        drained(&node);

        // Operator stop from the REM UI notifies the plugin.
        assert!(node
            .stop_plugin_map_sessions(None, Some(&first.session_id), "operator", true)
            .expect("operator stop"));
        let events = drained(&node);
        assert_eq!(events[0].1["event"], "map.session.stopped");
        assert_eq!(events[0].1["payload"]["reason"], "operator");
        let gone = call(
            &node,
            RANGEFINDER,
            "map.session.objects.get",
            &json!({ "sessionId": first.session_id }),
        )
        .expect_err("stopped");
        assert_eq!(gone.0, "NotFound");

        // The plugin stops its own session.
        let second = node
            .start_plugin_map_session(RANGEFINDER, &[marker.clone()])
            .expect("start");
        drained(&node);
        let stopped = call(
            &node,
            RANGEFINDER,
            "map.session.stop",
            &json!({ "sessionId": second.session_id }),
        )
        .expect("plugin stop");
        assert_eq!(stopped["stopped"], true);
        assert!(
            drained(&node).is_empty(),
            "the plugin is not told about its own stop"
        );

        // Revoking mapSelectionRead ends the session and denies further reads.
        let third = node
            .start_plugin_map_session(RANGEFINDER, &[marker.clone()])
            .expect("start");
        drained(&node);
        node.grant_plugin_capabilities(
            RANGEFINDER,
            PluginCapabilityRecord {
                map_markers_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("revoke");
        let events = drained(&node);
        assert_eq!(events[0].1["payload"]["reason"], "capability-revoked");
        let denied = call(
            &node,
            RANGEFINDER,
            "map.session.objects.get",
            &json!({ "sessionId": third.session_id }),
        )
        .expect_err("revoked");
        assert_eq!(denied.0, "PermissionDenied");
        node.grant_plugin_capabilities(RANGEFINDER, capabilities())
            .expect("regrant");

        // Binder death: the host ends sessions without queueing events for a dead connection.
        node.start_plugin_map_session(RANGEFINDER, &[marker.clone()])
            .expect("start");
        node.start_plugin_map_session(OTHER, &[marker])
            .expect("other start");
        drained(&node);
        assert!(node
            .stop_plugin_map_sessions(Some(RANGEFINDER), None, "binder-died", false)
            .expect("death"));
        assert!(drained(&node).is_empty());
        let remaining = node.list_plugin_map_sessions(None).expect("list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(
            remaining[0].plugin_id, OTHER,
            "other plugins keep their sessions"
        );

        // Disabling or uninstalling ends the rest.
        node.set_plugin_enabled(OTHER, false).expect("disable");
        assert_eq!(drained(&node)[0].1["payload"]["reason"], "plugin-disabled");
        assert!(node
            .list_plugin_map_sessions(None)
            .expect("list")
            .is_empty());
    }

    #[test]
    fn session_operations_negotiate_api_1_3_and_capability() {
        let node = node("negotiation");
        let marker = operator_marker(&node, "Target");
        let legacy = call(&node, LEGACY, "map.session.list", &json!({})).expect_err("api 1.2");
        assert_eq!(legacy.0, "PermissionDenied");
        assert!(
            call(&node, LEGACY, "map.symbols.list", &json!({})).is_ok(),
            "1.2 operations still work"
        );
        assert!(node
            .start_plugin_map_session(LEGACY, &[marker.clone()])
            .is_err());

        let future = node
            .list_plugins()
            .expect("plugins")
            .into_iter()
            .find(|plugin| plugin.discovered.plugin_id == "org.example.future")
            .expect("future plugin");
        assert_eq!(
            future.state, "Incompatible",
            "API 1.4 is newer than this host"
        );
        assert!(node
            .start_plugin_map_session("org.example.future", &[marker])
            .is_err());

        let missing =
            call(&node, RANGEFINDER, "map.session.objects.get", &json!({})).expect_err("payload");
        assert_eq!(
            missing,
            ("InvalidRequest", "sessionId is required".to_string())
        );
    }

    #[test]
    fn oversize_responses_become_a_typed_error() {
        let response = plugin_host_oversize_response("req-1");
        assert_eq!(response["ok"], false);
        assert_eq!(response["requestId"], "req-1");
        assert_eq!(response["error"]["code"], "ResponseTooLarge");
        assert!(plugin_host_response_within_limit(&response));
        assert!(!plugin_host_response_within_limit(
            &json!({ "value": "x".repeat(65_536) })
        ));
    }
}
