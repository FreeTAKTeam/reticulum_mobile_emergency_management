#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_upsertEventToDestinationJson(
    mut env: JNIEnv,
    _class: JClass,
    request_json: JString,
    destination_hex: JString,
) -> jint {
    let raw = match jstring_to_rust(&mut env, request_json) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let destination_hex = match jstring_to_rust(&mut env, destination_hex) {
        Ok(value) => value,
        Err(error) => return err_result("InvalidConfig", error),
    };
    let payload: EventProjectionInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            return err_result(
                "InvalidConfig",
                format!("invalid event payload: {error}"),
            )
        }
    };
    let node = initialized_node_or_return!();
    match node.upsert_event_to_destination(to_event_projection_record(payload), destination_hex) {
        Ok(()) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}
