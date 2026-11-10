/// Marker replicas arrive as `mission.marker.*` events in `FIELD_EVENT`,
/// whether relayed by RCH or sent by a REM peer. Only the event is applied;
/// the accompanying command is for RCH and is ignored here.
pub(crate) fn map_marker_events(fields_bytes: &[u8]) -> Vec<(String, serde_json::Value)> {
    let Ok(fields) = rmp_serde::from_slice::<MsgPackValue>(fields_bytes) else {
        return Vec::new();
    };
    let Some(event) = msgpack_map_entries(&fields)
        .and_then(|entries| msgpack_get_indexed(entries, crate::lxmf_fields::FIELD_EVENT))
    else {
        return Vec::new();
    };
    let events = match event {
        MsgPackValue::Array(items) => items.iter().collect::<Vec<_>>(),
        other => vec![other],
    };
    events
        .into_iter()
        .filter_map(msgpack_value_to_json)
        .filter_map(|event| {
            let event_type = event.get("event_type")?.as_str()?.trim().to_string();
            let payload = event.get("payload")?.clone();
            event_type
                .starts_with("mission.marker.")
                .then_some((event_type, payload))
        })
        .collect()
}

fn apply_map_marker_event(
    app_state: &AppStateStore,
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<Option<crate::types::ProjectionInvalidation>, NodeError> {
    if event_type == r3akt_map_core::replica::DELETE_EVENT {
        app_state.apply_map_marker_replica_deletion(payload)
    } else if r3akt_map_core::replica::UPSERT_EVENTS.contains(&event_type) {
        app_state.apply_map_marker_replica(payload)
    } else {
        Ok(None)
    }
}

fn persist_received_map_marker_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source: Option<&str>,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
) -> bool {
    if !metadata.is_some_and(|metadata| metadata.event_present) {
        return false;
    }
    let app_state = &state.app_state;
    let mut applied = false;
    for (event_type, payload) in fields_bytes.map(map_marker_events).unwrap_or_default() {
        let Ok(id) = r3akt_map_core::replica::replica_id(&payload) else {
            continue;
        };
        let policy = source.and_then(|source| {
            sensor_receive_policy(state, source)
                .ok()
                .map(|p| (source, p))
        });
        let binding = app_state.sensor_marker_writer(&id).ok().flatten();
        if binding.is_some()
            && !policy.is_some_and(|(source, (allowed, relay))| {
                allowed && (relay || binding.as_deref() == Some(source))
            })
        {
            continue;
        }
        let fresh = event_type == "mission.marker.created"
            && app_state.get_map_marker(&id).ok().flatten().is_none();
        match apply_map_marker_event(app_state, &event_type, &payload) {
            Ok(Some(invalidation)) => {
                crate::node::emit_map_marker_invalidation(app_state, bus, invalidation);
                if let (Some(source), Ok(Some(marker))) = (source, app_state.get_map_marker(&id)) {
                    let imported = marker.notes.as_deref().is_some_and(|notes| notes.starts_with("[goTenna:") || notes.starts_with("[REM:"));
                    if !imported && event_type != r3akt_map_core::replica::DELETE_EVENT {
                        let key = format!("marker:{}:{}", id, marker.updated_at_ms);
                        if let Err(error) = app_state.record_relay_ingress("position", source, &key,
                            &serde_json::json!({"callsign":marker.name,"lat":marker.lat,"lon":marker.lon,"updatedAtMs":marker.updated_at_ms})) {
                            warn!("[plugin-relay] marker not queued: {error}");
                        }
                    }
                }
                if fresh {
                    if let Some((source, (true, relay))) = policy {
                        let writer = if relay {
                            payload
                                .get("origin_rch")
                                .and_then(serde_json::Value::as_str)
                                .and_then(crate::delivery_policy::normalize_hex_32)
                        } else {
                            Some(source.to_string())
                        };
                        if let Some(writer) = writer {
                            let _ = app_state.remember_sensor_marker_writer(&id, &writer);
                        }
                    }
                }
                applied = true;
            }
            Ok(None) => applied = true,
            Err(error) => warn!("[lxmf][marker] replica rejected event={event_type} error={error}"),
        }
    }
    applied
}

/// RCH also sends markers to older REM builds as telemetry; markers are a
/// separate model here, so that copy is not stored as a telemetry position.
pub(crate) fn is_marker_telemetry(fields_bytes: &[u8]) -> bool {
    let Ok(fields) = rmp_serde::from_slice::<MsgPackValue>(fields_bytes) else {
        return false;
    };
    msgpack_map_entries(&fields)
        .and_then(|entries| msgpack_get_indexed(entries, FIELD_COMMANDS))
        .and_then(msgpack_value_to_json)
        .is_some_and(|commands| {
            let commands = match commands {
                serde_json::Value::Array(items) => items,
                other => vec![other],
            };
            commands.iter().any(|command| {
                let telemetry = command
                    .get("command_type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| {
                        canonical_command_type(value) == "mission.registry.telemetry.upsert"
                    });
                let args = command.get("args");
                let has = |key: &str| args.and_then(|args| args.get(key)).is_some();
                telemetry && has("object_destination_hash") && has("marker_type")
            })
        })
}
