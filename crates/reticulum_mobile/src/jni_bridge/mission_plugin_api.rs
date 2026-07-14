#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_upsertEventJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: EventProjectionInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid event payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.upsert_event(to_event_projection_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteEventJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: DeleteEventInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid event delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.delete_event(
        payload.uid,
        payload.deleted_at_ms.unwrap_or_else(crate::runtime::now_ms),
    ) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getTelemetryPositionsJson(
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
    match node.get_telemetry_positions() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(telemetry_position_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_recordLocalTelemetryFixJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: TelemetryPositionInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid telemetry payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.record_local_telemetry_fix(to_telemetry_position_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteLocalTelemetryJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: CallsignInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid telemetry delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.delete_local_telemetry(payload.callsign) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSosSettingsJson(
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
    match node.get_sos_settings() {
        Ok(settings) => ok_json_result(&mut env, &sos_settings_json(&settings)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSosSettingsJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosSettingsInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid SOS settings payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.set_sos_settings(to_sos_settings_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSosPinJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosPinInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid SOS PIN payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.set_sos_pin(payload.pin) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSosStatusJson(
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
    match node.get_sos_status() {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_triggerSosJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: SosTriggerInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid SOS trigger payload: {e}"));
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.trigger_sos(parse_sos_trigger_source(payload.source.as_deref())) {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deactivateSosJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: SosDeactivateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid SOS deactivate payload: {e}"),
            );
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.deactivate_sos(payload.pin) {
        Ok(status) => ok_json_result(&mut env, &sos_status_json(&status)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosTelemetryJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SosTelemetryInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid SOS telemetry payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.submit_sos_device_telemetry(to_sos_telemetry_record(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosAccelerometerJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: SosAccelerometerInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid SOS accelerometer payload: {e}"),
            );
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    let at_ms = payload.at_ms.unwrap_or_else(crate::runtime::now_ms);
    match node.submit_sos_accelerometer_sample(payload.x, payload.y, payload.z, at_ms) {
        Ok(Some(status)) => ok_json_result(
            &mut env,
            &json!({ "triggered": true, "status": sos_status_json(&status) }),
        ),
        Ok(None) => ok_json_result(&mut env, &json!({ "triggered": false })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_submitSosScreenEventJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return ptr::null_mut();
        }
    };
    let payload: SosScreenEventInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid SOS screen payload: {e}"));
            return ptr::null_mut();
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    let at_ms = payload.at_ms.unwrap_or_else(crate::runtime::now_ms);
    match node.submit_sos_screen_event(at_ms) {
        Ok(Some(status)) => ok_json_result(
            &mut env,
            &json!({ "triggered": true, "status": sos_status_json(&status) }),
        ),
        Ok(None) => ok_json_result(&mut env, &json!({ "triggered": false })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listSosAlertsJson(
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
    match node.list_sos_alerts() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(sos_alert_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}
