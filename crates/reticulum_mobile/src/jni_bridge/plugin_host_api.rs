#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listPluginSensorsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.list_plugin_sensors() {
        Ok(items) => ok_json_result(&mut env, &json!({"items": items})),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_handlePluginHostRequestJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return ptr::null_mut();
        }
    };
    let input: PluginHostRequestInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid plugin host request: {error}"),
            );
            return ptr::null_mut();
        }
    };
    if input.protocol_version != 1 || input.request_id.trim().is_empty() {
        set_last_error("InvalidConfig", "unsupported plugin host request");
        return ptr::null_mut();
    }
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    let result = match input.operation.as_str() {
        "sensor.publish" => serde_json::from_value::<PluginSensorSampleRequest>(input.payload)
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, error))
            .and_then(|sample| node.record_plugin_sensor(input.plugin_id.as_str(), sample))
            .and_then(|record| {
                serde_json::to_value(record).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
            }),
        "lxmf.send" => serde_json::from_value::<PluginLxmfSendRequest>(input.payload)
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, error))
            .and_then(|mut request| {
                request.plugin_id = input.plugin_id.clone();
                node.send_plugin_lxmf(request)
            })
            .map(|()| json!({"accepted": true})),
        "events.publish" => node
            .publish_plugin_event(input.plugin_id.as_str(), input.payload)
            .map(|()| json!({"accepted": true})),
        "notifications.raise" => node.list_plugins().and_then(|plugins| {
            let operation = input.operation.as_str();
            plugins
                .into_iter()
                .find(|plugin| plugin.discovered.plugin_id == input.plugin_id)
                .filter(|plugin| {
                    plugin.trusted
                        && plugin.enabled
                        && match operation {
                            "events.publish" => {
                                plugin.discovered.declared_capabilities.events_publish
                                    && plugin.granted_capabilities.events_publish
                            }
                            "notifications.raise" => {
                                plugin.discovered.declared_capabilities.notifications_raise
                                    && plugin.granted_capabilities.notifications_raise
                            }
                            _ => false,
                        }
                })
                .map(|_| json!({"accepted": true}))
                .ok_or(NodeError::InvalidConfig {})
        }),
        "operational.snapshot" => node.list_plugins().and_then(|plugins| {
            let allowed = plugins
                .iter()
                .any(|plugin| can_read_operational_snapshot(plugin, input.plugin_id.as_str()));
            if !allowed {
                return Err(NodeError::InvalidConfig {});
            }

            let status = serde_json::from_str::<serde_json::Value>(&status_to_json(node.get_status()))
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let summary = node.get_operational_summary()?;
            let readiness = node.get_eam_readiness_summary()?;
            let latest_event = node
                .get_events()?
                .into_iter()
                .filter(|event| event.deleted_at_ms.is_none())
                .max_by_key(|event| event.updated_at_ms)
                .map(|event| event_projection_json(&event));
            let latest_position = node
                .get_telemetry_positions()?
                .into_iter()
                .max_by_key(|position| position.updated_at_ms)
                .map(|position| telemetry_position_json(&position));
            Ok(operational_snapshot_json(
                now_ms(),
                status,
                operational_summary_json(&summary),
                eam_readiness_summary_json(&readiness),
                latest_event,
                latest_position,
            ))
        }),
        _ => Err(NodeError::InvalidConfig {}),
    };
    let response = plugin_host_response_json(input.request_id.as_str(), result);
    if plugin_host_response_within_limit(&response) {
        ok_json_result(&mut env, &response)
    } else {
        ok_json_result(
            &mut env,
            &plugin_host_response_json(
                input.request_id.as_str(),
                Err(NodeError::InvalidConfig {}),
            ),
        )
    }
}
