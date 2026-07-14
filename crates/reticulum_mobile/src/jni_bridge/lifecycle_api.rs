#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_start(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, config_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let input: NodeConfigInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid node config JSON: {e}")),
    };
    let config = match parse_node_config(input) {
        Ok(config) => config,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    let subscription = {
        let node = ensure_node_or_return!(&mut guard);
        if let Err(err) = node.start(config) {
            set_last_node_error(err);
            return RESULT_ERR;
        }
        node.subscribe_events()
    };

    guard.subscription = Some(subscription);
    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_stop(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    if let Some(subscription) = guard.subscription.take() {
        subscription.close();
    }

    if let Some(node) = guard.node.as_ref() {
        if let Err(err) = node.stop() {
            set_last_node_error(err);
            return RESULT_ERR;
        }
    }

    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_restart(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, config_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let input: NodeConfigInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid node config JSON: {e}")),
    };
    let config = match parse_node_config(input) {
        Ok(config) => config,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    let subscription = {
        let node = ensure_node_or_return!(&mut guard);
        if let Err(err) = node.restart(config) {
            set_last_node_error(err);
            return RESULT_ERR;
        }
        node.subscribe_events()
    };

    guard.subscription = Some(subscription);
    ok_result()
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let status = {
        let guard = match bridge_state().lock() {
            Ok(v) => v,
            Err(_) => {
                set_last_error("InternalError", "bridge lock poisoned");
                return ptr::null_mut();
            }
        };
        if let Some(node) = guard.node.as_ref() {
            node.get_status()
        } else {
            NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
                readiness: RuntimeReadinessSnapshot::default(),
                interfaces: Vec::new(),
            }
        }
    };

    make_jstring_or_null(&mut env, status_to_json(status))
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_connectPeer(
    mut env: JNIEnv,
    _class: JClass,
    destination_hex: JString,
) -> jint {
    let destination = match jstring_to_rust(&mut env, destination_hex) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.connect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_disconnectPeer(
    mut env: JNIEnv,
    _class: JClass,
    destination_hex: JString,
) -> jint {
    let destination = match jstring_to_rust(&mut env, destination_hex) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.disconnect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_announceNow(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.announce_now() {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_requestPeerIdentity(
    mut env: JNIEnv,
    _class: JClass,
    destination_hex: JString,
) -> jint {
    let destination = match jstring_to_rust(&mut env, destination_hex) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };

    let guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = match guard.node.as_ref() {
        Some(v) => v,
        None => return err_result("NotRunning", "node not initialized"),
    };
    match node.request_peer_identity(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

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
                node_error_code(&err),
                err
            );
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

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
