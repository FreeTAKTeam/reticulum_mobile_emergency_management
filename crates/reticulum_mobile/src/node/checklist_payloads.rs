fn checklist_snapshot_msgpack_entry(
    snapshot_json: &str,
) -> Result<(&'static str, MsgPackValue), NodeError> {
    let snapshot_value = serde_json::from_str::<JsonValue>(snapshot_json)
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    Ok(("snapshot", json_value_to_msgpack(&snapshot_value)?))
}

fn checklist_snapshot_content_bytes(
    checklist_uid: &str,
    snapshot_json: &str,
) -> Result<Vec<u8>, NodeError> {
    let snapshot_entry = checklist_snapshot_msgpack_entry(snapshot_json)?;
    let snapshot_msgpack =
        rmp_serde::to_vec(&snapshot_entry.1).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(snapshot_msgpack.as_slice())
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    let compressed_snapshot = encoder.finish().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    let content = msgpack_map(vec![
        ("type", MsgPackValue::from("rem.checklist.snapshot.v2")),
        ("checklist_uid", MsgPackValue::from(checklist_uid)),
        ("encoding", MsgPackValue::from("zlib+msgpack")),
        ("snapshot", MsgPackValue::Binary(compressed_snapshot)),
    ]);
    rmp_serde::to_vec(&content).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
}

fn build_checklist_replication_payload(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    build_checklist_replication_payload_with_command_id(status, target, command_type, args, None)
}

#[expect(
    clippy::too_many_arguments,
    reason = "delete replication planning keeps explicit runtime snapshots at the call boundary"
)]
fn build_checklist_delete_replication_sends(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
    checklist: Option<&ChecklistRecord>,
    checklist_uid: &str,
    delete_remote: bool,
) -> Result<Vec<ScheduledMissionSend>, NodeError> {
    if !delete_remote {
        return Ok(Vec::new());
    }

    let replication_targets = build_runtime_checklist_replication_targets(
        status,
        peers,
        saved_peers,
        active_propagation_node_hex,
        active_config,
        hub_directory_snapshot,
        checklist,
    )?;
    let args = checklist_uid_args_json(checklist_uid);
    let mut scheduled_sends = Vec::new();
    for target in replication_targets {
        let (body, fields) =
            build_checklist_replication_payload(status, &target, "checklist.delete", &args)?;
        scheduled_sends.push((target.app_destination_hex, body, fields, target.send_mode));
    }
    Ok(scheduled_sends)
}

fn build_checklist_replication_payload_with_command_id(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let fields =
        build_checklist_command_fields(status, target, command_type, args, command_id_override)?;
    let body = if matches!(
        command_type,
        "checklist.create.online" | "checklist.task.status.set"
    ) {
        command_wire_value(command_type).as_bytes().to_vec()
    } else if matches!(command_type, "checklist.task.row.add") {
        format!("C {}", command_wire_value(command_type)).into_bytes()
    } else {
        format!(
            "C {} {}",
            command_wire_value(command_type),
            checklist_subject_token(command_type, args)
        )
        .into_bytes()
    };
    Ok((body, fields))
}

fn build_checklist_replication_payload_with_snapshot(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
    snapshot_json: &str,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let fields =
        build_checklist_command_fields(status, target, command_type, args, command_id_override)?;
    let checklist_uid =
        checklist_key_arg(args, "checklist_uid").ok_or(NodeError::InvalidConfig {})?;
    let body = checklist_snapshot_content_bytes(checklist_uid.as_str(), snapshot_json)?;
    Ok((body, fields))
}

fn checklist_create_online_args_json(
    request: &ChecklistCreateOnlineRequest,
) -> Result<JsonMap<String, JsonValue>, NodeError> {
    let checklist_uid = request
        .checklist_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let name = request.name.trim();
    if name.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let template_uid = request.template_uid.trim();
    if template_uid.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let mission_uid = request
        .mission_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(NodeError::InvalidConfig {})?;
    let start_time = request.start_time.trim();
    if start_time.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let value = json!({
        "name": name,
        "template_uid": template_uid,
        "mission_uid": mission_uid,
        "description": request.description.trim(),
        "start_time": start_time,
    });
    match value {
        JsonValue::Object(mut map) => {
            if let Some(checklist_uid) = checklist_uid {
                map.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
            }
            Ok(map)
        }
        _ => Err(NodeError::InternalError {}),
    }
}

fn compact_checklist_create_online_args_json(
    request: &ChecklistCreateOnlineRequest,
    total_tasks: Option<u32>,
) -> Result<JsonMap<String, JsonValue>, NodeError> {
    let mut args = checklist_create_online_args_json(request)?;
    args.remove("description");
    args.remove("start_time");
    if let Some(total_tasks) = total_tasks {
        args.insert("total_tasks".to_string(), JsonValue::from(total_tasks));
    }
    Ok(args)
}

fn create_template_replicates_tasks_from_template(args: &JsonMap<String, JsonValue>) -> bool {
    let has_template = args
        .get("template_uid")
        .and_then(JsonValue::as_str)
        .and_then(default_checklist_template_wire_code)
        .is_some();
    let has_tasks = args
        .get("total_tasks")
        .and_then(JsonValue::as_u64)
        .is_some_and(|total| total > 0);
    has_template && has_tasks
}

#[cfg(test)]
fn append_checklist_create_snapshot_args(
    args: &mut JsonMap<String, JsonValue>,
    checklist: &ChecklistRecord,
) -> Result<(), NodeError> {
    let JsonValue::Object(snapshot) =
        serde_json::to_value(checklist).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
    else {
        return Err(NodeError::InternalError {});
    };
    for key in [
        "participant_rns_identities",
        "created_at",
        "created_by_team_member_rns_identity",
        "created_by_team_member_display_name",
    ] {
        if let Some(value) = snapshot.get(key) {
            args.insert(key.to_string(), value.clone());
        }
    }
    args.insert(
        "total_tasks".to_string(),
        JsonValue::from(checklist.expected_task_count.unwrap_or_else(|| {
            crate::numeric::usize_to_u32_saturating(
                checklist
                    .tasks
                    .iter()
                    .filter(|task| task.deleted_at.is_none())
                    .count(),
            )
        })),
    );
    Ok(())
}

#[cfg(test)]
fn checklist_task_row_add_args_from_task(
    checklist_uid: &str,
    task: &crate::types::ChecklistTaskRecord,
    changed_by_identity: Option<&str>,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(task.task_uid.as_str()),
    );
    args.insert("number".to_string(), JsonValue::from(task.number));
    if let Some(due_relative_minutes) = task.due_relative_minutes {
        args.insert(
            "due_relative_minutes".to_string(),
            JsonValue::from(due_relative_minutes),
        );
    }
    if let Some(due_dtg) = task
        .due_dtg
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("due_dtg".to_string(), JsonValue::from(due_dtg));
    }
    if let Some(notes) = task
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("notes".to_string(), JsonValue::from(notes));
    }
    if let Some(legacy_value) = task.legacy_value.as_deref() {
        args.insert("legacy_value".to_string(), JsonValue::from(legacy_value));
    }
    if let Some(identity) = changed_by_identity
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "changed_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}

fn compact_initial_checklist_task_row_add_args_from_task(
    checklist_uid: &str,
    task: &crate::types::ChecklistTaskRecord,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert("checklist_uid".to_string(), JsonValue::from(checklist_uid));
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(task.task_uid.as_str()),
    );
    args.insert("number".to_string(), JsonValue::from(task.number));
    if let Some(legacy_value) = task.legacy_value.as_deref() {
        args.insert("legacy_value".to_string(), JsonValue::from(legacy_value));
    }
    args
}

fn build_initial_checklist_task_payloads(
    status: &NodeStatus,
    target: &MissionReplicationTarget,
    checklist_uid: &str,
    tasks: &[crate::types::ChecklistTaskRecord],
    _changed_by_identity: Option<&str>,
) -> Vec<ScheduledMissionSend> {
    tasks
        .iter()
        .filter(|task| task.deleted_at.is_none())
        .filter_map(|task| {
            let args = compact_initial_checklist_task_row_add_args_from_task(checklist_uid, task);
            build_checklist_replication_payload(status, target, "checklist.task.row.add", &args)
                .ok()
                .map(|(body, fields)| {
                    (
                        target.app_destination_hex.clone(),
                        body,
                        fields,
                        target.send_mode,
                    )
                })
        })
        .collect()
}

fn dispatch_scheduled_mission_send(
    tx: &mpsc::Sender<Command>,
    send: ScheduledMissionSend,
) -> Result<(), NodeError> {
    let (destination_hex, body, fields_bytes, send_mode) = send;
    let (resp_tx, _resp_rx) = cb::bounded(1);
    dispatch_command(
        tx,
        Command::SendBytes {
            destination_hex,
            bytes: body,
            fields_bytes: Some(fields_bytes),
            send_mode,
            resp: resp_tx,
        },
    )
}

fn checklist_update_args_json(request: &ChecklistUpdateRequest) -> JsonMap<String, JsonValue> {
    let mut patch = JsonMap::new();
    if let Some(mission_uid) = request.patch.mission_uid.as_deref() {
        patch.insert(
            "mission_uid".to_string(),
            JsonValue::from(mission_uid.trim()),
        );
    }
    if let Some(template_uid) = request.patch.template_uid.as_deref() {
        patch.insert(
            "template_uid".to_string(),
            JsonValue::from(template_uid.trim()),
        );
    }
    if let Some(name) = request.patch.name.as_deref() {
        patch.insert("name".to_string(), JsonValue::from(name.trim()));
    }
    if let Some(description) = request.patch.description.as_deref() {
        patch.insert(
            "description".to_string(),
            JsonValue::from(description.trim()),
        );
    }
    if let Some(start_time) = request.patch.start_time.as_deref() {
        patch.insert("start_time".to_string(), JsonValue::from(start_time.trim()));
    }

    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert("patch".to_string(), JsonValue::Object(patch));
    args
}

fn checklist_uid_args_json(checklist_uid: &str) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(checklist_uid.trim()),
    );
    args
}

fn checklist_task_status_args_json(
    request: &ChecklistTaskStatusSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args.insert(
        "user_status".to_string(),
        JsonValue::from(request.user_status.as_str()),
    );
    args
}

fn checklist_task_row_add_args_json(
    request: &ChecklistTaskRowAddRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    if let Some(task_uid) = request
        .task_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert("task_uid".to_string(), JsonValue::from(task_uid));
    }
    args.insert("number".to_string(), JsonValue::from(request.number));
    if let Some(due_relative_minutes) = request.due_relative_minutes {
        args.insert(
            "due_relative_minutes".to_string(),
            JsonValue::from(due_relative_minutes),
        );
    }
    if let Some(legacy_value) = request.legacy_value.as_deref() {
        args.insert("legacy_value".to_string(), JsonValue::from(legacy_value));
    }
    args
}

fn checklist_task_row_delete_args_json(
    request: &ChecklistTaskRowDeleteRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args
}

fn checklist_task_row_style_args_json(
    request: &ChecklistTaskRowStyleSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    if let Some(color) = request.row_background_color.as_deref() {
        args.insert(
            "row_background_color".to_string(),
            JsonValue::from(color.trim()),
        );
    }
    if let Some(line_break_enabled) = request.line_break_enabled {
        args.insert(
            "line_break_enabled".to_string(),
            JsonValue::from(line_break_enabled),
        );
    }
    args
}

fn checklist_task_cell_args_json(
    request: &ChecklistTaskCellSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args.insert(
        "column_uid".to_string(),
        JsonValue::from(request.column_uid.trim()),
    );
    args.insert("value".to_string(), JsonValue::from(request.value.clone()));
    if let Some(identity) = request
        .updated_by_team_member_rns_identity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "updated_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}
