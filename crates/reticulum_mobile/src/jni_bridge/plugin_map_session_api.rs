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

    include!("plugin_map_session_tests.rs");
}
