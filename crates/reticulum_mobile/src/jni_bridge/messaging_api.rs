#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_broadcastBase64(
    mut env: JNIEnv,
    _class: JClass,
    bytes_base64: JString,
) -> jint {
    let encoded = match jstring_to_rust(&mut env, bytes_base64) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let bytes = match BASE64_STANDARD.decode(encoded.as_bytes()) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid base64 payload: {e}")),
    };

    let node = initialized_node_or_return!();
    match node.broadcast_bytes(bytes) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setAnnounceCapabilities(
    mut env: JNIEnv,
    _class: JClass,
    capability_string: JString,
) -> jint {
    let value = match jstring_to_rust(&mut env, capability_string) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let node = initialized_node_or_return!();
    match node.set_announce_capabilities(value) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setLogLevel(
    mut env: JNIEnv,
    _class: JClass,
    level_string: JString,
) -> jint {
    let value = match jstring_to_rust(&mut env, level_string) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let node = initialized_node_or_return!();
    node.set_log_level(parse_log_level(Some(value.as_str())));
    ok_result()
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_refreshHubDirectory(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    let node = initialized_node_or_return!();
    match node.refresh_hub_directory() {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getHubDirectorySnapshotJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.get_hub_directory_snapshot() {
        Ok(snapshot) => ok_json_result(&mut env, &hub_directory_snapshot_json(&snapshot)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setActiveTeamJson(
    mut env: JNIEnv,
    _class: JClass,
    payload_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, payload_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let payload: SetActiveTeamInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid active-team payload: {error}"),
            )
        }
    };
    let node = initialized_node_or_return!();
    match node.set_active_team(payload.team_uid) {
        Ok(()) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listAnnouncesJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.list_announces() {
        Ok(items) => ok_json_result(&mut env, &json!({ "items": items })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listPeersJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.list_peers() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(peer_record_json).collect::<Vec<_>>()
            }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listConversationsJson(
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
    match node.list_conversations() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(conversation_record_json).collect::<Vec<_>>()
            }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listMessagesJson(
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
    let payload: MessageListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid message list payload: {e}"),
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
    match node.list_messages(payload.conversation_id) {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items.iter().map(message_record_json).collect::<Vec<_>>()
            }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteConversationJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error("InvalidConfig", e);
            return 1;
        }
    };
    let payload: ConversationDeleteInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid conversation delete payload: {e}"),
            );
            return 1;
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return 1;
        }
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.delete_conversation(payload.conversation_id) {
        Ok(()) => 0,
        Err(err) => {
            set_last_node_error(err);
            1
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getLxmfSyncStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.get_lxmf_sync_status() {
        Ok(status) => ok_json_result(&mut env, &status),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listTelemetryDestinationsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.list_telemetry_destinations() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({
                "items": items
            }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_legacyImportCompletedJson(
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
    match node.legacy_import_completed() {
        Ok(completed) => ok_json_result(&mut env, &json!({ "completed": completed })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importLegacyStateJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: LegacyImportInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid legacy import payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    let messages = match payload
        .messages
        .unwrap_or_default()
        .into_iter()
        .map(to_message_record)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(err) => {
            set_last_node_error(err);
            return RESULT_ERR;
        }
    };
    let settings = match payload.settings.map(to_app_settings_record).transpose() {
        Ok(settings) => settings,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };
    let legacy = LegacyImportPayload {
        settings,
        saved_peers: payload
            .saved_peers
            .unwrap_or_default()
            .into_iter()
            .map(to_saved_peer_record)
            .collect(),
        eams: payload
            .eams
            .unwrap_or_default()
            .into_iter()
            .map(to_eam_projection_record)
            .collect(),
        events: payload
            .events
            .unwrap_or_default()
            .into_iter()
            .map(to_event_projection_record)
            .collect(),
        messages,
        telemetry_positions: payload
            .telemetry_positions
            .unwrap_or_default()
            .into_iter()
            .map(to_telemetry_position_record)
            .collect(),
    };
    match node.import_legacy_state(legacy) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getAppSettingsJson(
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
    match node.get_app_settings() {
        Ok(Some(settings)) => ok_json_result(&mut env, &app_settings_json(&settings)),
        Ok(None) => ok_json_result(&mut env, &json!({ "settings": null })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}
