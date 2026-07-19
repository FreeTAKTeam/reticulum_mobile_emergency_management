#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_start(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, config_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error_with_context(
                "InvalidConfig",
                e.clone(),
                Some("start"),
                Some(e),
            );
            return RESULT_ERR;
        }
    };
    let input: NodeConfigInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            let cause = e.to_string();
            set_last_error_with_context(
                "InvalidConfig",
                format!("invalid node config JSON: {e}"),
                Some("start"),
                Some(cause),
            );
            return RESULT_ERR;
        }
    };
    let config = match parse_node_config(input) {
        Ok(config) => config,
        Err(error) => {
            set_last_node_error_for("start", error);
            return RESULT_ERR;
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    let node = match ensure_node(&mut guard) {
        Ok(node) => node,
        Err(error) => {
            set_last_node_error_for("start", error);
            return RESULT_ERR;
        }
    };
    drop(guard);
    if let Err(err) = node.start(config) {
        set_last_node_error_for("start", err);
        return RESULT_ERR;
    }
    let subscription = node.subscribe_events();

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    guard.subscription = Some(subscription);
    ok_result()
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_stop(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };

    let subscription = guard.subscription.take();
    let node = guard.node.clone();
    drop(guard);

    if let Some(subscription) = subscription {
        subscription.close();
    }

    if let Some(node) = node {
        if let Err(err) = node.stop() {
            set_last_node_error(err);
            return RESULT_ERR;
        }
    }

    ok_result()
}

#[jni_boundary]
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

    let node = match ensure_node(&mut guard) {
        Ok(node) => node,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };
    drop(guard);
    if let Err(err) = node.restart(config) {
        set_last_node_error(err);
        return RESULT_ERR;
    }
    let subscription = node.subscribe_events();

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    guard.subscription = Some(subscription);
    ok_result()
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = {
        let guard = match bridge_state().lock() {
            Ok(v) => v,
            Err(_) => {
                set_last_error("InternalError", "bridge lock poisoned");
                return ptr::null_mut();
            }
        };
        guard.node.clone()
    };
    let status = node.map_or_else(
        || NodeStatus {
            running: false,
            name: String::new(),
            identity_hex: String::new(),
            app_destination_hex: String::new(),
            lxmf_destination_hex: String::new(),
            readiness: RuntimeReadinessSnapshot::default(),
            interfaces: Vec::new(),
        },
        |node| node.get_status(),
    );

    make_jstring_or_null(&mut env, status_to_json(status))
}

#[jni_boundary]
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

    let node = initialized_node_or_return!();
    match node.connect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
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

    let node = initialized_node_or_return!();
    match node.disconnect_peer(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_announceNow(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let node = initialized_node_or_return!();
    match node.announce_now() {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
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

    let node = initialized_node_or_return!();
    match node.request_peer_identity(destination) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}
