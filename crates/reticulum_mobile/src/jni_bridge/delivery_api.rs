#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_sendJson(
    mut env: JNIEnv,
    _class: JClass,
    send_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, send_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SendInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid send payload: {e}")),
    };
    let bytes = match BASE64_STANDARD.decode(payload.bytes_base64.as_bytes()) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid base64 payload: {e}")),
    };
    let fields_bytes = match payload.fields_base64 {
        Some(encoded) => match BASE64_STANDARD.decode(encoded.as_bytes()) {
            Ok(value) => Some(value),
            Err(e) => {
                return err_result(
                    "InvalidConfig",
                    format!("invalid fields base64 payload: {e}"),
                )
            }
        },
        None => None,
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.send_bytes(
        payload.destination_hex,
        bytes,
        fields_bytes,
        send_mode_from_input(payload.send_mode.as_deref(), payload.use_propagation_node),
    ) {
        Ok(_) => {
            log::debug!("jni sendJson result=ok");
            ok_result()
        }
        Err(err) => {
            log::error!(
                "jni sendJson result=err code={} message={}",
                crate::error_context::node_error_code(&err),
                err
            );
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_sendLxmfJson(
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
    let payload: SendLxmfInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", format!("invalid lxmf payload: {e}"));
            return ptr::null_mut();
        }
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return ptr::null_mut();
        }
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => {
            set_last_error("NotRunning", "node not initialized");
            return ptr::null_mut();
        }
    };
    match node.send_lxmf(SendLxmfRequest {
        destination_hex: payload.destination_hex,
        body_utf8: payload.body_utf8,
        title: payload.title,
        send_mode: send_mode_from_input(payload.send_mode.as_deref(), payload.use_propagation_node),
    }) {
        Ok(message_id_hex) => ok_json_result(&mut env, &json!({ "messageIdHex": message_id_hex })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_retryLxmfJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: MessageIdInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid retry payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.retry_lxmf(payload.message_id_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_cancelLxmfJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: MessageIdInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid cancel payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.cancel_lxmf(payload.message_id_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setActivePropagationNodeJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: OptionalDestinationInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid propagation node payload: {e}"),
            )
        }
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.set_active_propagation_node(payload.destination_hex) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_requestLxmfSyncJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SyncRequestInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid sync payload: {e}")),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.request_lxmf_sync(payload.limit) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}
