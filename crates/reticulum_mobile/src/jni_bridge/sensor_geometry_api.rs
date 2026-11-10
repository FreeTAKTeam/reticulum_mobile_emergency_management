#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getSensorOriginsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    let result = node
        .get_sensor_origins_json()
        .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|_| NodeError::InternalError {}));
    map_marker_result(&mut env, result)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SensorVideoApprovalInput {
    origin_id: String,
    sensor_id: String,
    approved: bool,
}
#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSensorVideoSharingJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<SensorVideoApprovalInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    map_marker_result(
        &mut env,
        node.set_sensor_video_sharing(input.origin_id, input.sensor_id, input.approved)
            .map(|()| json!({"accepted":true})),
    )
}
