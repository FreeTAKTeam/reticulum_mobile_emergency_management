#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_decodePluginLxmfFieldsJson(
    mut env: JNIEnv,
    _class: JClass,
    fields_base64: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, fields_base64) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return ptr::null_mut();
        }
    };
    let fields = match BASE64_STANDARD.decode(raw.as_bytes()) {
        Ok(value) => value,
        Err(error) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid plugin fields base64: {error}"),
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
    match node.decode_plugin_lxmf_fields(fields.as_slice()) {
        Ok(Some(envelope)) => ok_json_result(&mut env, &envelope),
        Ok(None) => ok_json_result(&mut env, &Value::Null),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_nextEventJson(
    mut env: JNIEnv,
    _class: JClass,
    timeout_ms: jint,
) -> jstring {
    let subscription = {
        let guard = match bridge_state().lock() {
            Ok(v) => v,
            Err(_) => {
                set_last_error("InternalError", "bridge lock poisoned");
                return ptr::null_mut();
            }
        };
        guard.subscription.clone()
    };

    let Some(subscription) = subscription else {
        return ptr::null_mut();
    };

    let timeout = if timeout_ms < 0 { 0 } else { timeout_ms as u32 };
    let Some(event) = subscription.next(timeout) else {
        return ptr::null_mut();
    };

    make_jstring_or_null(&mut env, event_to_wire_json(event))
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_takeLastErrorJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let value = {
        let mut guard = match last_error().lock() {
            Ok(v) => v,
            Err(_) => return ptr::null_mut(),
        };
        guard.take()
    };

    let Some(value) = value else {
        return ptr::null_mut();
    };

    match serde_json::to_string(&value) {
        Ok(payload) => make_jstring_or_null(&mut env, payload),
        Err(_) => ptr::null_mut(),
    }
}
