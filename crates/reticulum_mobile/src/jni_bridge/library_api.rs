// Mission Library (REM #356) JNI surface. Android copies a picked document
// into a private temporary file and passes its path; Rust hashes, stores and
// indexes it. Only metadata and managed file paths cross back.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryImportInput {
    path: String,
    name: String,
    media_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryItemInput {
    item_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryMissionsInput {
    item_id: String,
    #[serde(default)]
    mission_uids: Vec<String>,
}

fn library_entry_json(entry: Option<crate::app_state::LibraryEntry>) -> serde_json::Value {
    match entry {
        Some(entry) => json!({
            "item": entry.item,
            "path": entry.path.map(|path| path.to_string_lossy().into_owned()),
        }),
        None => json!({ "item": null, "path": null }),
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importLibraryFileJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryImportInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .library_import(
            std::path::Path::new(&input.path),
            &input.name,
            input.media_type.as_deref(),
        )
        .map(|item| json!({ "item": item }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listLibraryItemsJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(args) = map_marker_input::<serde_json::Value>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let query = match r3akt_library_core::LibraryQuery::from_args(&args) {
        Ok(query) => query,
        Err(reason) => {
            set_last_error("InvalidConfig", reason);
            return ptr::null_mut();
        }
    };
    let node = initialized_node_or_return!();
    let result = node.library_list(&query);
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getLibraryItemJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryItemInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node.library_entry(&input.item_id).map(library_entry_json);
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteLibraryItemJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryItemInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node.library_delete(&input.item_id).map(|removed| json!({ "removed": removed }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setLibraryItemMissionsJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryMissionsInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .library_set_missions(&input.item_id, input.mission_uids)
        .map(|item| json!({ "item": item }));
    map_marker_result(&mut env, result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryShareInput {
    item_id: String,
    destination_hex: String,
}

/// Publishes and offers a local item to one saved peer (REM #358).
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_shareLibraryItemJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryShareInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .library_share(input.item_id, input.destination_hex)
        .map(|()| json!({ "queued": true }));
    map_marker_result(&mut env, result)
}

/// Downloads an offered item over RPP; progress arrives as Library updates.
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_downloadLibraryItemJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<LibraryItemInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node.library_download(input.item_id).map(|()| json!({ "queued": true }));
    map_marker_result(&mut env, result)
}
