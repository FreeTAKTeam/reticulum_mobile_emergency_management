async fn persist_received_eam_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    body_utf8: &str,
    source_hex: Option<&str>,
) -> bool {
    let received_at_ms = now_ms();
    let source_display_name = if let Some(source_hex) = source_hex {
        state
            .messaging
            .lock()
            .await
            .peer_by_destination(source_hex)
            .and_then(|peer| peer.display_name)
    } else {
        None
    };
    let parsed_from_fields = fields_bytes
        .and_then(|value| {
            eam_command_action_from_fields(
                value,
                received_at_ms,
                source_hex,
                source_display_name.as_deref(),
            )
        })
        .or_else(|| {
            metadata
                .and_then(|value| value.command_type.as_deref())
                .filter(|value| *value == "mission.registry.eam.upsert")
                .and_then(|_| {
                    compact_eam_action_from_body(
                        body_utf8,
                        received_at_ms,
                        source_hex,
                        source_display_name.as_deref(),
                    )
                })
        });
    if metadata.is_none() && parsed_from_fields.is_none() {
        return false;
    }
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| {
            value != "mission.registry.eam.upsert" && value != "mission.registry.eam.delete"
        })
        && parsed_from_fields.is_none()
    {
        return false;
    }

    let parsed = serde_json::from_str::<EamWireBody>(body_utf8)
        .ok()
        .and_then(|body| {
            eam_command_action_from_command(body.command, body.projection, received_at_ms)
        })
        .or_else(|| {
            serde_json::from_str::<MissionCommandEnvelope<EamUpsertCommandArgs>>(body_utf8)
                .ok()
                .and_then(|command| eam_command_action_from_command(command, None, received_at_ms))
        })
        .or(parsed_from_fields);

    let Some(action) = parsed else {
        return false;
    };

    match action {
        EamCommandAction::Upsert(record) => match state.app_state.upsert_eam(record.as_ref()) {
            Ok(invalidation) => {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                if let Ok(summary) = state.app_state.bump_projection_revision(
                    ProjectionScope::OperationalSummary {},
                    None,
                    Some("eam-received".to_string()),
                ) {
                    bus.emit(NodeEvent::ProjectionInvalidated {
                        invalidation: summary,
                    });
                }
                true
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to persist inbound eam callsign={} reason={}",
                        record.callsign, err
                    ),
                });
                false
            }
        },
        EamCommandAction::Delete {
            callsign,
            deleted_at_ms,
        } => match state.app_state.delete_eam(&callsign, deleted_at_ms) {
            Ok(invalidation) => {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                if let Ok(summary) = state.app_state.bump_projection_revision(
                    ProjectionScope::OperationalSummary {},
                    None,
                    Some("eam-deleted".to_string()),
                ) {
                    bus.emit(NodeEvent::ProjectionInvalidated {
                        invalidation: summary,
                    });
                }
                true
            }
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to delete inbound eam callsign={} reason={}",
                        callsign, err
                    ),
                });
                false
            }
        },
    }
}

fn expand_event_wire_content(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.contains('/') || trimmed.is_empty() {
        trimmed.to_string()
    } else if trimmed.len() <= 8 && trimmed.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        format!("MECP/2/{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn event_projection_from_fields(
    fields_bytes: &[u8],
    content_bytes: Option<&[u8]>,
    source_identity_fallback: Option<&str>,
    source_display_name_fallback: Option<&str>,
    received_at_ms: u64,
) -> Option<EventProjectionRecord> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string());
        if command_type
            .as_deref()
            .is_some_and(|value| value != "mission.registry.log_entry.upsert")
        {
            continue;
        }
        let command_type =
            command_type.unwrap_or_else(|| "mission.registry.log_entry.upsert".to_string());
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries);
        let uid = msgpack_get_named(args, &["entry_uid", "u"]).and_then(msgpack_event_uid)?;
        let mission_uid = msgpack_get_named(args, &["mission_uid", "m"])
            .and_then(msgpack_mission_uid)
            .unwrap_or_else(|| DEFAULT_R3AKT_MISSION_UID.to_string());
        let content = msgpack_get_named(args, &["content", "ct"])
            .and_then(msgpack_string)
            .or_else(|| {
                content_bytes.and_then(|bytes| {
                    let text = String::from_utf8_lossy(bytes).trim().to_string();
                    (!text.is_empty()).then_some(expand_event_wire_content(text.as_str()))
                })
            })?;
        let callsign = msgpack_get_named(args, &["callsign", "cs"])
            .and_then(msgpack_string)
            .or_else(|| {
                source.and_then(|source_map| {
                    msgpack_get_named(source_map, &["display_name", "n"]).and_then(msgpack_string)
                })
            })
            .or_else(|| {
                source_display_name_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                source_identity_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.chars().take(8).collect())
            })?;
        let timestamp = msgpack_get_named(command_map, &["timestamp", "ts"])
            .and_then(msgpack_timestamp)
            .or_else(|| msgpack_get_named(args, &["server_time", "st"]).and_then(msgpack_timestamp))
            .or_else(|| msgpack_get_named(args, &["client_time", "ct"]).and_then(msgpack_timestamp))
            .unwrap_or_else(current_timestamp_rfc3339);
        let command_id = msgpack_get_named(args, &["ci"])
            .and_then(|value| event_command_id_from_tail(uid.as_str(), value))
            .or_else(|| {
                msgpack_get_named(command_map, &["command_id", "i"]).and_then(msgpack_string)
            })
            .unwrap_or_else(|| format!("log-entry-{uid}"));
        let source_identity = msgpack_get_named(args, &["source_identity", "si"])
            .and_then(msgpack_string)
            .or_else(|| {
                source.and_then(|source_map| {
                    msgpack_get_named(source_map, &["rns_identity", "r"])
                        .and_then(msgpack_hex_or_string)
                })
            })
            .or_else(|| {
                source_identity_fallback
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })?;
        if uid.trim().is_empty()
            || mission_uid.trim().is_empty()
            || content.trim().is_empty()
            || callsign.trim().is_empty()
            || timestamp.trim().is_empty()
            || command_id.trim().is_empty()
            || source_identity.trim().is_empty()
        {
            return None;
        }
        let topics = msgpack_get_named(command_map, &["topics", "to"])
            .and_then(|value| msgpack_event_topics(value, mission_uid.as_str()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| vec![mission_uid.clone()]);
        let server_time = msgpack_get_named(args, &["server_time", "st"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(|| timestamp.clone());
        let client_time = msgpack_get_named(args, &["client_time", "ct"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(|| timestamp.clone());
        let correlation_id = msgpack_get_named(command_map, &["correlation_id", "c"])
            .and_then(msgpack_string)
            .or_else(|| Some(command_id.clone()));
        return Some(EventProjectionRecord {
            uid,
            command_id,
            source_identity,
            source_display_name: msgpack_get_named(args, &["source_display_name", "sn"])
                .and_then(msgpack_string)
                .or_else(|| {
                    source.and_then(|source_map| {
                        msgpack_get_named(source_map, &["display_name", "n"])
                            .and_then(msgpack_string)
                    })
                })
                .or_else(|| {
                    source_display_name_fallback
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                }),
            timestamp,
            command_type,
            mission_uid,
            content,
            callsign,
            server_time: Some(server_time),
            client_time: Some(client_time),
            keywords: msgpack_get_named(args, &["keywords", "kw"])
                .and_then(msgpack_event_keywords)
                .unwrap_or_default(),
            content_hashes: msgpack_get_named(args, &["content_hashes", "ch"])
                .and_then(msgpack_string_vec)
                .unwrap_or_default(),
            updated_at_ms: received_at_ms,
            deleted_at_ms: msgpack_get_named(args, &["deleted_at_ms", "d"]).and_then(msgpack_u64),
            correlation_id,
            topics,
        });
    }

    None
}

fn telemetry_position_from_fields(
    fields_bytes: &[u8],
    received_at_ms: u64,
) -> Option<TelemetryPositionRecord> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())?;
        if command_type != "mission.registry.telemetry.upsert" {
            continue;
        }
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let callsign = msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string)?;
        let lat = msgpack_get_named(args, &["lat", "la"]).and_then(msgpack_f64)?;
        let lon = msgpack_get_named(args, &["lon", "lo"]).and_then(msgpack_f64)?;
        if callsign.trim().is_empty() || !lat.is_finite() || !lon.is_finite() {
            return None;
        }
        return Some(TelemetryPositionRecord {
            callsign: callsign.trim().to_string(),
            lat,
            lon,
            alt: msgpack_get_named(args, &["alt", "al"]).and_then(msgpack_f64),
            course: msgpack_get_named(args, &["course", "cr"]).and_then(msgpack_f64),
            speed: msgpack_get_named(args, &["speed", "sp"]).and_then(msgpack_f64),
            accuracy: msgpack_get_named(args, &["accuracy", "ac"]).and_then(msgpack_f64),
            updated_at_ms: msgpack_get_named(args, &["updated_at_ms", "updatedAt", "u"])
                .and_then(msgpack_u64)
                .unwrap_or(received_at_ms),
        });
    }

    None
}

async fn persist_received_telemetry_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
) -> bool {
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| value != "mission.registry.telemetry.upsert")
    {
        return false;
    }

    let Some(record) =
        fields_bytes.and_then(|value| telemetry_position_from_fields(value, now_ms()))
    else {
        return false;
    };

    match state.app_state.record_local_telemetry_fix(&record) {
        Ok(invalidation) => {
            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            if let Ok(summary) = state.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("telemetry-received".to_string()),
            ) {
                bus.emit(NodeEvent::ProjectionInvalidated {
                    invalidation: summary,
                });
            }
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound telemetry callsign={} reason={}",
                    record.callsign, err
                ),
            });
            false
        }
    }
}

async fn persist_received_event_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    content_bytes: Option<&[u8]>,
    source_identity_fallback: Option<&str>,
) -> bool {
    let source_display_name = if let Some(source_hex) = source_identity_fallback {
        state
            .messaging
            .lock()
            .await
            .peer_by_destination(source_hex)
            .and_then(|peer| peer.display_name)
    } else {
        None
    };
    let parsed_from_fields = fields_bytes.and_then(|value| {
        event_projection_from_fields(
            value,
            content_bytes,
            source_identity_fallback,
            source_display_name.as_deref(),
            now_ms(),
        )
    });
    if metadata.is_none() && parsed_from_fields.is_none() {
        return false;
    }
    if metadata
        .and_then(|value| value.command_type.as_deref())
        .is_none_or(|value| value != "mission.registry.log_entry.upsert")
        && parsed_from_fields.is_none()
    {
        return false;
    }

    let Some(record) = parsed_from_fields else {
        return false;
    };

    match state.app_state.upsert_event(&record) {
        Ok(invalidation) => {
            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            if let Ok(summary) = state.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("event-received".to_string()),
            ) {
                bus.emit(NodeEvent::ProjectionInvalidated {
                    invalidation: summary,
                });
            }
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound event uid={} reason={}",
                    record.uid, err
                ),
            });
            false
        }
    }
}
