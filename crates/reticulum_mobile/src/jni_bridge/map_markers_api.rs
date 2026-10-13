#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapMarkerJson<'a> {
    object_destination_hash: &'a str,
    origin_rch: &'a str,
    marker_type: &'a str,
    symbol: &'a str,
    name: &'a str,
    category: &'a str,
    lat: f64,
    lon: f64,
    notes: Option<&'a str>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

fn map_marker_json(marker: &crate::types::MapMarkerRecord) -> MapMarkerJson<'_> {
    MapMarkerJson {
        object_destination_hash: &marker.object_destination_hash,
        origin_rch: &marker.origin_rch,
        marker_type: &marker.marker_type,
        symbol: &marker.symbol,
        name: &marker.name,
        category: &marker.category,
        lat: marker.lat,
        lon: marker.lon,
        notes: marker.notes.as_deref(),
        created_at_ms: marker.created_at_ms,
        updated_at_ms: marker.updated_at_ms,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapMarkerCreateInput {
    idempotency_key: Option<String>,
    marker_type: String,
    symbol: String,
    name: Option<String>,
    category: Option<String>,
    lat: f64,
    lon: f64,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapMarkerKeyInput {
    object_destination_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapMarkerRenameInput {
    object_destination_hash: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapMarkerMoveInput {
    object_destination_hash: String,
    lat: f64,
    lon: f64,
}

/// Decodes a JSON payload, recording an `InvalidConfig` error on failure.
fn map_marker_input<T: serde::de::DeserializeOwned>(
    env: &mut JNIEnv,
    payload: JString,
) -> Option<T> {
    let raw = match jstring_to_rust(env, payload) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return None;
        }
    };
    match serde_json::from_str(&raw) {
        Ok(value) => Some(value),
        Err(error) => {
            set_last_error("InvalidConfig", error.to_string());
            None
        }
    }
}

fn map_marker_result<T: Serialize>(
    env: &mut JNIEnv,
    result: Result<T, NodeError>,
) -> jstring {
    match result {
        Ok(value) => ok_json_result(env, &value),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listMapMarkerSymbolsJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    let items = node.list_map_marker_symbols();
    ok_json_result(&mut env, &json!({ "items": items }))
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_listMapMarkersJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    let result = node.list_map_markers().map(|markers| {
        json!({ "items": markers.iter().map(map_marker_json).collect::<Vec<_>>() })
    });
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createMapMarkerJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<MapMarkerCreateInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .create_map_marker(crate::types::MapMarkerCreateRequest {
            idempotency_key: input.idempotency_key,
            marker_type: input.marker_type,
            symbol: input.symbol,
            name: input.name,
            category: input.category,
            lat: input.lat,
            lon: input.lon,
            notes: input.notes,
        })
        .map(|result| json!({ "marker": map_marker_json(&result.marker), "created": result.created }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_renameMapMarkerJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<MapMarkerRenameInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .rename_map_marker(crate::types::MapMarkerRenameRequest {
            object_destination_hash: input.object_destination_hash,
            name: input.name,
        })
        .map(|marker| json!({ "marker": map_marker_json(&marker) }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_moveMapMarkerJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<MapMarkerMoveInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .move_map_marker(crate::types::MapMarkerMoveRequest {
            object_destination_hash: input.object_destination_hash,
            lat: input.lat,
            lon: input.lon,
        })
        .map(|marker| json!({ "marker": map_marker_json(&marker) }));
    map_marker_result(&mut env, result)
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_deleteMapMarkerJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = map_marker_input::<MapMarkerKeyInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    let result = node
        .delete_map_marker(input.object_destination_hash)
        .map(|deleted| json!({ "deleted": deleted }));
    map_marker_result(&mut env, result)
}

#[cfg(test)]
mod map_marker_json_tests {
    use super::*;

    #[test]
    fn map_marker_json_uses_camel_case_wire_keys() {
        let marker = crate::types::MapMarkerRecord {
            object_destination_hash: "abc".to_string(),
            origin_rch: String::new(),
            marker_type: "fire".to_string(),
            symbol: "fire".to_string(),
            name: "Ridge".to_string(),
            category: "incident".to_string(),
            lat: 44.5,
            lon: -1.25,
            notes: None,
            created_at_ms: 1,
            updated_at_ms: 2,
        };
        let value = serde_json::to_value(map_marker_json(&marker)).expect("marker json");
        assert_eq!(value["objectDestinationHash"], "abc");
        assert_eq!(value["markerType"], "fire");
        assert_eq!(value["updatedAtMs"], 2);
        assert!(value["notes"].is_null());
    }

    #[test]
    fn map_marker_create_input_requires_coordinates() {
        let missing = serde_json::from_str::<MapMarkerCreateInput>(
            r#"{"markerType":"fire","symbol":"fire"}"#,
        );
        assert!(missing.is_err());
        let parsed = serde_json::from_str::<MapMarkerCreateInput>(
            r#"{"markerType":"fire","symbol":"fire","lat":1.0,"lon":2.0,"idempotencyKey":"k"}"#,
        )
        .expect("create input");
        assert_eq!(parsed.idempotency_key.as_deref(), Some("k"));
    }
}
