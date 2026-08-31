#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setAppSettingsJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: AppSettingsInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid settings payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    let settings = match to_app_settings_record(payload) {
        Ok(settings) => settings,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };
    match node.set_app_settings(settings) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSavedPeersJson(
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
    match node.get_saved_peers() {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(saved_peer_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSavedPeersJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: SavedPeersPayload = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", format!("invalid saved peers payload: {e}")),
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    let peers = match payload
        .saved_peers
        .into_iter()
        .map(to_explicit_saved_peer_record)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(peers) => peers,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };
    match node.set_saved_peers(peers) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getOperationalSummaryJson(
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
    match node.get_operational_summary() {
        Ok(summary) => ok_json_result(&mut env, &operational_summary_json(&summary)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistsJson(
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
    let payload: ChecklistListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist list payload: {e}"),
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
    match node.list_active_checklists(Some(ChecklistListActiveRequest {
        search: payload.search,
        sort_by: payload.sort_by,
    })) {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(checklist_record_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistJson(
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
    let payload: ChecklistDeleteInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist get payload: {e}"),
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
    match node.get_checklist(payload.checklist_uid) {
        Ok(Some(record)) => ok_json_result(&mut env, &checklist_record_json(&record)),
        Ok(None) => ok_json_result(&mut env, &json!({ "checklist": null })),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getChecklistTemplatesJson(
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
    let payload: ChecklistListInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(
                "InvalidConfig",
                format!("invalid checklist template list payload: {e}"),
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
    match node.list_checklist_templates(Some(ChecklistTemplateListRequest {
        search: payload.search,
        sort_by: payload.sort_by,
    })) {
        Ok(items) => ok_json_result(
            &mut env,
            &json!({ "items": items.iter().map(checklist_template_json).collect::<Vec<_>>() }),
        ),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importChecklistTemplateCsvJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jstring {
    let err_result = |code: &str, message: String| {
        set_last_error(code, message);
        ptr::null_mut()
    };
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistTemplateImportInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist template import payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned".to_string()),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.import_checklist_template_csv(to_checklist_template_import_request(payload)) {
        Ok(template) => ok_json_result(&mut env, &checklist_template_json(&template)),
        Err(err) => {
            set_last_node_error(err);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createChecklistFromTemplateJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let err_result = |code: &str, message: String| {
        set_last_error(code, message);
        RESULT_ERR
    };
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistCreateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist create-from-template payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned".to_string()),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.create_checklist_from_template(ChecklistCreateFromTemplateRequest {
        checklist_uid: payload.checklist_uid,
        mission_uid: payload.mission_uid,
        template_uid: payload.template_uid,
        name: payload.name,
        description: payload.description,
        start_time: payload.start_time,
        created_by_team_member_rns_identity: payload.created_by_team_member_rns_identity,
        created_by_team_member_display_name: payload.created_by_team_member_display_name,
    }) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createOnlineChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistCreateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist create payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.create_online_checklist(to_checklist_create_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_updateChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistUpdateInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist update payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.update_checklist(to_checklist_update_request(payload)) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteChecklistJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(v) => v,
        Err(e) => return err_result("InvalidConfig", e),
    };
    let payload: ChecklistDeleteInput = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return err_result(
                "InvalidConfig",
                format!("invalid checklist delete payload: {e}"),
            )
        }
    };
    let mut guard = match bridge_state().lock() {
        Ok(v) => v,
        Err(_) => return err_result("InternalError", "bridge lock poisoned"),
    };
    let node = ensure_node_or_return!(&mut guard);
    match node.delete_checklist(ChecklistDeleteRequest {
        checklist_uid: payload.checklist_uid,
        delete_remote: payload.delete_remote,
    }) {
        Ok(_) => ok_result(),
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}
