#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosLocationsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.list_sos_locations() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_location_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosAudioJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.list_sos_audio() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_audio_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_recordSosAudioJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosAudioInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid SOS audio payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.record_sos_audio(to_sos_audio_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_syncDiscoveredPluginsJson(
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
    let input: DiscoveredPluginsInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid plugin discovery payload: {error}"),
            );
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.sync_discovered_plugins(input.items) {
        Ok(items) => ok_json_result(&mut env, &json!({"items": items})),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listPluginsJson(
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
    match node.list_plugins() {
        Ok(items) => ok_json_result(&mut env, &json!({"items": items})),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_approvePluginPublisherJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let input: PluginApprovalInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return err_result("InvalidConfig", format!("invalid plugin approval: {error}"))
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.approve_plugin_publisher(input.plugin_id.as_str(), input.display_name.as_deref()) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_revokePluginPublisherJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let input: PluginPublisherInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid publisher revocation: {error}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.revoke_plugin_publisher(input.fingerprint.as_str()) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setPluginEnabledJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let input: PluginEnabledInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid plugin enablement: {error}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.set_plugin_enabled(input.plugin_id.as_str(), input.enabled) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_grantPluginCapabilitiesJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let input: PluginCapabilitiesInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid plugin capabilities: {error}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.grant_plugin_capabilities(input.plugin_id.as_str(), input.capabilities) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setPluginRuntimeStateJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let input: PluginRuntimeStateInput = match serde_json::from_str(raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid plugin runtime state: {error}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(value) => value,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.set_plugin_runtime_state(
        input.plugin_id.as_str(),
        input.state.as_str(),
        input.diagnostic,
    ) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

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
            .map_err(|_| NodeError::InvalidConfig {})
            .and_then(|sample| node.record_plugin_sensor(input.plugin_id.as_str(), sample))
            .and_then(|record| {
                serde_json::to_value(record).map_err(|_| NodeError::InternalError {})
            }),
        "lxmf.send" => serde_json::from_value::<PluginLxmfSendRequest>(input.payload)
            .map_err(|_| NodeError::InvalidConfig {})
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
        _ => Err(NodeError::InvalidConfig {}),
    };
    match result {
        Ok(value) => ok_json_result(
            &mut env,
            &json!({
                "protocolVersion": 1,
                "requestId": input.request_id,
                "ok": true,
                "result": value
            }),
        ),
        Err(error) => ok_json_result(
            &mut env,
            &json!({
                "protocolVersion": 1,
                "requestId": input.request_id,
                "ok": false,
                "error": {"code": "PermissionDeniedOrInvalid", "message": error.to_string()}
            }),
        ),
    }
}
