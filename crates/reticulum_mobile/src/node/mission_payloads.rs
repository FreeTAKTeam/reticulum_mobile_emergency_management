fn build_eam_replication_payload(
    _status: &NodeStatus,
    record: &EamProjectionRecord,
    _target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    if record.callsign.trim().is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let command_id = "m";
    let body = format!(
        "E|{}|{}{}{}{}{}{}",
        record.callsign.trim(),
        status_wire_code(&record.security_status),
        status_wire_code(&record.capability_status),
        status_wire_code(&record.preparedness_status),
        status_wire_code(&record.medical_status),
        status_wire_code(&record.mobility_status),
        status_wire_code(&record.comms_status),
    )
    .into_bytes();

    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (MsgPackValue::from("i"), MsgPackValue::from(command_id)),
            (
                MsgPackValue::from("t"),
                MsgPackValue::from(command_wire_value("mission.registry.eam.upsert")),
            ),
        ])]),
    )]);
    let fields = rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})?;

    Ok((body, fields))
}

fn build_eam_delete_replication_payload(
    callsign: &str,
    deleted_at_ms: u64,
    _target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let normalized_callsign = callsign.trim();
    if normalized_callsign.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let subject = sanitize_correlation_token(normalized_callsign);
    let delete_token = compact_u64_token(deleted_at_ms);
    let command_id = format!("md:{subject}:{delete_token}");
    let body = b"ED".to_vec();
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("i"),
                MsgPackValue::from(command_id.as_str()),
            ),
            (
                MsgPackValue::from("t"),
                MsgPackValue::from(command_wire_value("mission.registry.eam.delete")),
            ),
            (
                MsgPackValue::from("a"),
                msgpack_map(vec![
                    ("cs", MsgPackValue::from(normalized_callsign)),
                    ("d", MsgPackValue::from(deleted_at_ms)),
                ]),
            ),
        ])]),
    )]);
    let fields = rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})?;

    Ok((body, fields))
}

fn build_event_replication_payload(
    _status: &NodeStatus,
    record: &EventProjectionRecord,
    _target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let uid = record.uid.trim();
    let command_id = record.command_id.trim();
    let mission_uid = record.mission_uid.trim();
    let content = record.content.trim();
    let timestamp = record.timestamp.trim();
    let command_type = record.command_type.trim();
    let source_identity = record.source_identity.trim();
    if uid.is_empty()
        || command_id.is_empty()
        || mission_uid.is_empty()
        || content.is_empty()
        || timestamp.is_empty()
        || command_type.is_empty()
        || source_identity.is_empty()
    {
        return Err(NodeError::InvalidConfig {});
    }

    let body = event_content_wire_body(content);
    let mut args_entries = vec![("u", event_uid_wire_value(uid))];
    if mission_uid != DEFAULT_R3AKT_MISSION_UID {
        args_entries.push(("m", mission_uid_wire_value(mission_uid)));
    }
    if let Some(deleted_at_ms) = record.deleted_at_ms {
        let delete_token = compact_u64_token(deleted_at_ms);
        args_entries.push(("ci", MsgPackValue::from(format!("d:{delete_token}"))));
        args_entries.push(("d", MsgPackValue::from(deleted_at_ms)));
    }

    let mut command_entries = vec![(MsgPackValue::from("a"), msgpack_map(args_entries))];
    if !is_default_event_topics(record.topics.as_slice(), mission_uid) {
        command_entries.push((
            MsgPackValue::from("to"),
            event_topics_wire_value(record.topics.as_slice(), mission_uid),
        ));
    }

    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(command_entries)]),
    )]);
    let fields_bytes = rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})?;

    Ok((body, fields_bytes))
}

fn build_event_delete_replication_payload(
    status: &NodeStatus,
    record: &EventProjectionRecord,
    deleted_at_ms: u64,
    target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let uid = record.uid.trim();
    if uid.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let mut tombstone = record.clone();
    let correlation_id = format!(
        "event-delete-{}-{}-{deleted_at_ms}",
        sanitize_correlation_token(uid),
        &target.app_destination_hex[..8],
    );
    tombstone.command_id = format!("cmd-{correlation_id}");
    tombstone.correlation_id = Some(correlation_id);
    tombstone.command_type = "mission.registry.log_entry.upsert".to_string();
    tombstone.deleted_at_ms = Some(deleted_at_ms);
    tombstone.updated_at_ms = deleted_at_ms;
    build_event_replication_payload(status, &tombstone, target)
}

fn build_telemetry_replication_payload(
    position: &TelemetryPositionRecord,
    target: &MissionReplicationTarget,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let callsign = position.callsign.trim();
    if callsign.is_empty() || !position.lat.is_finite() || !position.lon.is_finite() {
        return Err(NodeError::InvalidConfig {});
    }

    let send_ts_ms = now_ms();
    let send_token = compact_u64_token(send_ts_ms);
    let command_id = format!("t:{}:{send_token}", &target.app_destination_hex[..4]);
    let correlation_id = format!("t:{}:{send_token}", &target.app_destination_hex[..8]);
    let body = b"T".to_vec();
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("i"),
                MsgPackValue::from(command_id.as_str()),
            ),
            (
                MsgPackValue::from("c"),
                MsgPackValue::from(correlation_id.as_str()),
            ),
            (
                MsgPackValue::from("t"),
                MsgPackValue::from(command_wire_value("mission.registry.telemetry.upsert")),
            ),
            (
                MsgPackValue::from("a"),
                msgpack_map(
                    vec![
                        ("cs", MsgPackValue::from(callsign)),
                        ("la", MsgPackValue::from(position.lat)),
                        ("lo", MsgPackValue::from(position.lon)),
                        ("u", MsgPackValue::from(position.updated_at_ms)),
                    ]
                    .into_iter()
                    .chain(position.alt.map(|value| ("al", MsgPackValue::from(value))))
                    .chain(
                        position
                            .course
                            .map(|value| ("cr", MsgPackValue::from(value))),
                    )
                    .chain(
                        position
                            .speed
                            .map(|value| ("sp", MsgPackValue::from(value))),
                    )
                    .chain(
                        position
                            .accuracy
                            .map(|value| ("ac", MsgPackValue::from(value))),
                    )
                    .collect(),
                ),
            ),
        ])]),
    )]);
    let fields = rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})?;

    Ok((body, fields))
}
