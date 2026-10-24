#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listLocalPropagationJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.list_local_propagation() {
        Ok(items) => ok_json_result(&mut env, &json!({"items":items})),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getLocalPropagationConfigJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    ok_json_result(&mut env, &node.get_local_propagation_config())
}
