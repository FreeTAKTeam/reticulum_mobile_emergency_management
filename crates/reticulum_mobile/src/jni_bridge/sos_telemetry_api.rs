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

const PLUGIN_HOST_RESPONSE_MAX_BYTES: usize = 65_536;

fn can_read_operational_snapshot(plugin: &InstalledPluginRecord, plugin_id: &str) -> bool {
    plugin.discovered.plugin_id == plugin_id
        && plugin.trusted
        && plugin.enabled
        && plugin.discovered.api_major == 1
        && plugin.discovered.api_minor >= 1
        && plugin.discovered.declared_capabilities.operational_read
        && plugin.granted_capabilities.operational_read
}

fn operational_snapshot_json(
    captured_at_ms: u64,
    status: Value,
    operational_summary: Value,
    eam_readiness: Value,
    latest_event: Option<Value>,
    latest_position: Option<Value>,
) -> Value {
    json!({
        "capturedAtMs": captured_at_ms,
        "status": status,
        "operationalSummary": operational_summary,
        "eamReadiness": eam_readiness,
        "latestEvent": latest_event,
        "latestPosition": latest_position
    })
}

fn plugin_host_response_json(request_id: &str, result: Result<Value, NodeError>) -> Value {
    match result {
        Ok(value) => json!({
            "protocolVersion": 1,
            "requestId": request_id,
            "ok": true,
            "result": value
        }),
        Err(error) => json!({
            "protocolVersion": 1,
            "requestId": request_id,
            "ok": false,
            "error": {"code": "PermissionDeniedOrInvalid", "message": error.to_string()}
        }),
    }
}

fn plugin_host_response_within_limit(response: &Value) -> bool {
    serde_json::to_vec(response)
        .map(|encoded| encoded.len() <= PLUGIN_HOST_RESPONSE_MAX_BYTES)
        .unwrap_or(false)
}

#[cfg(test)]
mod operational_snapshot_tests {
    use super::*;

    fn permitted_plugin() -> InstalledPluginRecord {
        InstalledPluginRecord {
            discovered: DiscoveredPluginRecord {
                plugin_id: "org.freetakteam.rem.plugin.watch_status".to_string(),
                display_name: "Watch Status".to_string(),
                version: "1.0.0".to_string(),
                api_major: 1,
                api_minor: 1,
                package_name: "org.freetakteam.rem.plugin.watchstatus".to_string(),
                service_class_name: ".WatchStatusPluginService".to_string(),
                publisher_fingerprint: "ab".repeat(32),
                publisher_history: Vec::new(),
                android_permissions: Vec::new(),
                declared_capabilities: PluginCapabilityRecord {
                    operational_read: true,
                    ..PluginCapabilityRecord::default()
                },
                messages: Vec::new(),
                configuration_entrypoint: Some("rem-plugin-config/index.html".to_string()),
            },
            state: "Enabled".to_string(),
            trusted: true,
            enabled: true,
            granted_capabilities: PluginCapabilityRecord {
                operational_read: true,
                ..PluginCapabilityRecord::default()
            },
            diagnostic: None,
            updated_at_ms: 1,
        }
    }

    #[test]
    fn operational_snapshot_requires_every_permission_gate() {
        let plugin = permitted_plugin();
        assert!(can_read_operational_snapshot(
            &plugin,
            "org.freetakteam.rem.plugin.watch_status"
        ));

        let mut denied = plugin.clone();
        denied.trusted = false;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        denied = plugin.clone();
        denied.enabled = false;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        denied = plugin.clone();
        denied.discovered.api_major = 2;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        denied = plugin.clone();
        denied.discovered.api_minor = 0;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        denied = plugin.clone();
        denied.discovered.declared_capabilities.operational_read = false;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        denied = plugin.clone();
        denied.granted_capabilities.operational_read = false;
        assert!(!can_read_operational_snapshot(
            &denied,
            &plugin.discovered.plugin_id
        ));
        assert!(!can_read_operational_snapshot(&plugin, "org.example.another"));
    }

    #[test]
    fn operational_snapshot_has_the_stable_public_shape() {
        let snapshot = operational_snapshot_json(
            42,
            json!({"state": "Running"}),
            json!({"callsign": "REM"}),
            json!({"readinessColor": "Orange"}),
            Some(json!({"id": "event-1"})),
            None,
        );
        assert_eq!(snapshot["capturedAtMs"], 42);
        assert_eq!(snapshot["status"]["state"], "Running");
        assert_eq!(snapshot["operationalSummary"]["callsign"], "REM");
        assert_eq!(snapshot["eamReadiness"]["readinessColor"], "Orange");
        assert_eq!(snapshot["latestEvent"]["id"], "event-1");
        assert!(snapshot["latestPosition"].is_null());
        assert_eq!(snapshot.as_object().expect("snapshot object").len(), 6);
    }

    #[test]
    fn plugin_host_response_enforces_the_protocol_size_limit() {
        let small = plugin_host_response_json("request-1", Ok(json!({"value": "ok"})));
        assert!(plugin_host_response_within_limit(&small));

        let oversized = plugin_host_response_json(
            "request-2",
            Ok(json!({"value": "x".repeat(PLUGIN_HOST_RESPONSE_MAX_BYTES)})),
        );
        assert!(!plugin_host_response_within_limit(&oversized));
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
        "operational.snapshot" => node.list_plugins().and_then(|plugins| {
            let allowed = plugins
                .iter()
                .any(|plugin| can_read_operational_snapshot(plugin, input.plugin_id.as_str()));
            if !allowed {
                return Err(NodeError::InvalidConfig {});
            }

            let status = serde_json::from_str::<serde_json::Value>(&status_to_json(node.get_status()))
                .map_err(|_| NodeError::InternalError {})?;
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
