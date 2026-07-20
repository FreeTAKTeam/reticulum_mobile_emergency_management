fn merge_uploaded_checklist_snapshot(
    existing: Option<ChecklistRecord>,
    incoming: ChecklistRecord,
    timestamp: &str,
    source_identity: Option<&str>,
) -> Option<ChecklistRecord> {
    let incoming = prepare_uploaded_snapshot(incoming, timestamp, source_identity);
    let incoming_snapshot_at = incoming
        .uploaded_at
        .as_deref()
        .or(incoming.updated_at.as_deref())
        .unwrap_or(timestamp)
        .to_string();
    let incoming_content_at = incoming
        .updated_at
        .as_deref()
        .unwrap_or(incoming_snapshot_at.as_str())
        .to_string();
    let Some(existing) = existing else {
        return Some(incoming);
    };
    if is_hidden_placeholder_checklist(&existing) {
        return Some(incoming);
    }
    if existing.deleted_at.as_deref().is_some_and(|deleted_at| {
        !incoming_timestamp_is_newer(Some(deleted_at), incoming_content_at.as_str())
    }) {
        return None;
    }

    let incoming_metadata_is_newer = incoming_timestamp_is_newer(
        existing.updated_at.as_deref(),
        incoming
            .updated_at
            .as_deref()
            .unwrap_or(incoming_snapshot_at.as_str()),
    );
    let mut merged = if incoming_metadata_is_newer {
        let mut record = incoming.clone();
        record.created_at = existing.created_at.clone().or(record.created_at);
        if record.created_by_team_member_rns_identity.trim().is_empty() {
            record.created_by_team_member_rns_identity =
                existing.created_by_team_member_rns_identity.clone();
        }
        record
    } else {
        existing.clone()
    };

    merged.deleted_at = None;
    merged.sync_state = ChecklistSyncState::Synced {};
    merged.uploaded_at = newest_timestamp(
        merged.uploaded_at.as_deref(),
        incoming.uploaded_at.as_deref(),
    )
    .map(ToString::to_string);
    merged.updated_at =
        newest_timestamp(merged.updated_at.as_deref(), incoming.updated_at.as_deref())
            .map(ToString::to_string);
    merged.columns = merge_uploaded_columns(existing.columns, incoming.columns);
    merged.tasks = merge_uploaded_tasks(existing.tasks, incoming.tasks);
    merged.participant_rns_identities = merge_uploaded_participants(
        existing.participant_rns_identities,
        incoming.participant_rns_identities,
        source_identity,
    );
    merged.expected_task_count = incoming
        .expected_task_count
        .or(existing.expected_task_count)
        .or_else(|| {
            Some(crate::numeric::usize_to_u32_saturating(
                merged
                    .tasks
                    .iter()
                    .filter(|task| task.deleted_at.is_none())
                    .count(),
            ))
        });
    merged.feed_publications =
        merge_uploaded_feed_publications(existing.feed_publications, incoming.feed_publications);
    set_checklist_last_changed_by(&mut merged, source_identity);
    normalize_checklist_record(&mut merged);
    Some(merged)
}

fn hydrate_checklist_from_local_template(
    app_state: &AppStateStore,
    checklist: &mut ChecklistRecord,
) {
    if !checklist.tasks.is_empty() {
        return;
    }
    let Some(template_uid) = checklist
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(Some(template)) = app_state.get_checklist_template(template_uid) else {
        return;
    };

    if checklist.columns.is_empty() {
        checklist.columns = template.columns;
    }
    checklist.tasks = template.tasks;
    checklist.template_version = Some(template.version);
    checklist.template_name = Some(template.name);
    checklist.origin_type = template.origin_type;
    checklist.expected_task_count = Some(crate::numeric::usize_to_u32_saturating(
        checklist
            .tasks
            .iter()
            .filter(|task| task.deleted_at.is_none())
            .count(),
    ));
}

fn blank_task_cells(columns: &[ChecklistColumnRecord], task_uid: &str) -> Vec<ChecklistCellRecord> {
    columns
        .iter()
        .map(|column| ChecklistCellRecord {
            cell_uid: format!("{task_uid}:{}", column.column_uid),
            task_uid: task_uid.to_string(),
            column_uid: column.column_uid.clone(),
            value: None,
            updated_at: None,
            updated_by_team_member_rns_identity: None,
        })
        .collect()
}

fn checklist_column_type_from_wire(value: &str) -> ChecklistColumnType {
    match value.trim().to_ascii_uppercase().as_str() {
        "LONG_STRING" => ChecklistColumnType::LongString {},
        "INTEGER" => ChecklistColumnType::Integer {},
        "ACTUAL_TIME" => ChecklistColumnType::ActualTime {},
        "RELATIVE_TIME" => ChecklistColumnType::RelativeTime {},
        _ => ChecklistColumnType::ShortString {},
    }
}

fn checklist_system_key_from_wire(value: &str) -> Option<ChecklistSystemColumnKey> {
    match value.trim().to_ascii_uppercase().as_str() {
        "DUE_RELATIVE_DTG" => Some(ChecklistSystemColumnKey::DueRelativeDtg {}),
        _ => None,
    }
}

fn checklist_column_from_patch(
    patch: &[(MsgPackValue, MsgPackValue)],
    fallback_display_order: u32,
) -> Option<ChecklistColumnRecord> {
    let column_uid = msgpack_get_checklist_arg(patch, "column_uid").and_then(msgpack_string)?;
    let column_name = msgpack_get_checklist_arg(patch, "column_name")
        .and_then(msgpack_string)
        .unwrap_or_else(|| column_uid.clone());
    let display_order = msgpack_get_checklist_arg(patch, "display_order")
        .and_then(msgpack_u64)
        .map_or(fallback_display_order, crate::numeric::u64_to_u32_saturating);
    let column_type = msgpack_get_checklist_arg(patch, "column_type")
        .and_then(msgpack_string)
        .map_or(ChecklistColumnType::ShortString {}, |value| {
            checklist_column_type_from_wire(value.as_str())
        });
    let column_editable = msgpack_get_checklist_arg(patch, "column_editable")
        .and_then(msgpack_bool)
        .unwrap_or(true);
    let background_color =
        msgpack_get_checklist_arg(patch, "row_background_color").and_then(msgpack_string);
    let text_color = msgpack_get_checklist_arg(patch, "text_color").and_then(msgpack_string);
    let is_removable = msgpack_get_checklist_arg(patch, "is_removable")
        .and_then(msgpack_bool)
        .unwrap_or(true);
    let system_key = msgpack_get_checklist_arg(patch, "system_key")
        .and_then(msgpack_string)
        .and_then(|value| checklist_system_key_from_wire(value.as_str()));

    Some(ChecklistColumnRecord {
        column_uid,
        column_name,
        display_order,
        column_type,
        column_editable,
        background_color,
        text_color,
        is_removable,
        system_key,
    })
}

fn merge_checklist_column(checklist: &mut ChecklistRecord, incoming: ChecklistColumnRecord) {
    if let Some(existing) = checklist
        .columns
        .iter_mut()
        .find(|column| column.column_uid == incoming.column_uid)
    {
        *existing = incoming;
    } else {
        checklist.columns.push(incoming);
        checklist.columns.sort_by_key(|column| column.display_order);
    }
}

fn should_apply_inbound_task_status(
    task: &ChecklistTaskRecord,
    incoming_status: ChecklistUserTaskStatus,
    incoming_timestamp: &str,
    inserted_placeholder: bool,
) -> bool {
    if inserted_placeholder {
        return true;
    }
    match (task.user_status, incoming_status) {
        (ChecklistUserTaskStatus::Pending {}, ChecklistUserTaskStatus::Complete {}) => true,
        (ChecklistUserTaskStatus::Complete {}, ChecklistUserTaskStatus::Pending {}) => {
            task.completed_at.as_deref().map_or_else(
                || incoming_timestamp_is_newer(task.updated_at.as_deref(), incoming_timestamp),
                |completed_at| incoming_timestamp_is_newer(Some(completed_at), incoming_timestamp),
            )
        }
        _ => incoming_timestamp_is_newer(task.updated_at.as_deref(), incoming_timestamp),
    }
}

fn placeholder_task_record(task_uid: &str, timestamp: &str) -> ChecklistTaskRecord {
    ChecklistTaskRecord {
        task_uid: task_uid.to_string(),
        number: 0,
        user_status: ChecklistUserTaskStatus::Pending {},
        task_status: ChecklistTaskStatus::Pending {},
        is_late: false,
        updated_at: Some(timestamp.to_string()),
        deleted_at: None,
        custom_status: None,
        due_relative_minutes: None,
        due_dtg: None,
        notes: None,
        row_background_color: None,
        line_break_enabled: false,
        completed_at: None,
        completed_by_team_member_rns_identity: None,
        legacy_value: None,
        cells: Vec::new(),
    }
}

fn tombstoned_task_record(task_uid: &str, timestamp: &str) -> ChecklistTaskRecord {
    ChecklistTaskRecord {
        task_uid: task_uid.to_string(),
        number: 0,
        user_status: ChecklistUserTaskStatus::Pending {},
        task_status: ChecklistTaskStatus::Pending {},
        is_late: false,
        updated_at: Some(timestamp.to_string()),
        deleted_at: Some(timestamp.to_string()),
        custom_status: None,
        due_relative_minutes: None,
        due_dtg: None,
        notes: None,
        row_background_color: None,
        line_break_enabled: false,
        completed_at: None,
        completed_by_team_member_rns_identity: None,
        legacy_value: None,
        cells: Vec::new(),
    }
}

fn checklist_snapshot_json_from_command(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    if let Some(snapshot) = msgpack_get_named(command_map, &["snapshot", "sn"]) {
        let json = msgpack_value_to_json(snapshot)?;
        return serde_json::to_string(&json).ok();
    }
    if let Some(snapshot_json) =
        msgpack_get_named(command_map, &["snapshot_json", "sj"]).and_then(msgpack_string)
    {
        return Some(snapshot_json);
    }
    None
}

fn checklist_snapshot_json_from_content(
    content_bytes: Option<&[u8]>,
    checklist_uid: &str,
) -> Option<String> {
    let content = content_bytes?;
    let snapshot_payload = rmp_serde::from_slice::<MsgPackValue>(content).ok()?;
    let entries = msgpack_map_entries(&snapshot_payload)?;
    let payload_type = msgpack_get_named(entries, &["type"]).and_then(msgpack_string)?;
    if let Some(payload_uid) =
        msgpack_get_named(entries, &["checklist_uid"]).and_then(msgpack_string)
    {
        if payload_uid != checklist_uid {
            return None;
        }
    }
    match payload_type.as_str() {
        "rem.checklist.snapshot.v1" => {
            let snapshot = msgpack_get_named(entries, &["snapshot"])?;
            let json = msgpack_value_to_json(snapshot)?;
            serde_json::to_string(&json).ok()
        }
        "rem.checklist.snapshot.v2" => {
            let encoding = msgpack_get_named(entries, &["encoding"])
                .and_then(msgpack_string)
                .unwrap_or_default();
            if encoding != "zlib+msgpack" {
                return None;
            }
            let MsgPackValue::Binary(compressed_snapshot) =
                msgpack_get_named(entries, &["snapshot"])?
            else {
                return None;
            };
            let mut decoder = ZlibDecoder::new(compressed_snapshot.as_slice());
            let mut snapshot_msgpack = Vec::new();
            decoder.read_to_end(&mut snapshot_msgpack).ok()?;
            let snapshot =
                rmp_serde::from_slice::<MsgPackValue>(snapshot_msgpack.as_slice()).ok()?;
            let json = msgpack_value_to_json(&snapshot)?;
            serde_json::to_string(&json).ok()
        }
        _ => None,
    }
}

fn msgpack_json_arg<T: DeserializeOwned>(
    args: &[(MsgPackValue, MsgPackValue)],
    key: &str,
) -> Option<T> {
    msgpack_get_checklist_arg(args, key)
        .and_then(msgpack_value_to_json)
        .and_then(|value| serde_json::from_value(value).ok())
}

fn msgpack_get_checklist_arg<'a>(
    args: &'a [(MsgPackValue, MsgPackValue)],
    key: &str,
) -> Option<&'a MsgPackValue> {
    if let Some(code) = checklist_arg_code(key) {
        msgpack_get_named(args, &[key, code])
    } else {
        msgpack_get_named(args, &[key])
    }
}

fn msgpack_checklist_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().map(|value| format!("chk-{value}")),
        _ => msgpack_string(value),
    }
}

fn msgpack_checklist_template_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => match value.as_u64()? {
            1 => Some("tmpl-24-hour-survival-pack".to_string()),
            2 => Some("tmpl-72-hour-home-preparedness".to_string()),
            3 => Some("tmpl-vehicle-emergency-preparedness".to_string()),
            _ => None,
        },
        _ => msgpack_string(value),
    }
}

fn positional_checklist_command_args(
    command: &MsgPackValue,
) -> Option<(String, Vec<(MsgPackValue, MsgPackValue)>)> {
    let MsgPackValue::Array(values) = command else {
        return None;
    };
    let command_type = match values.first()? {
        MsgPackValue::Integer(value) if value.as_u64() == Some(1) => {
            "checklist.create.online".to_string()
        }
        value => {
            msgpack_string(value).map(|value| canonical_command_type(value.as_str()).to_string())?
        }
    };
    if command_type != "checklist.create.online" || values.len() < 5 {
        return None;
    }
    let checklist_uid = values.get(1)?.clone();
    let mission_uid = values.get(2)?.clone();
    let template_uid = values.get(3)?.clone();
    let name = values.get(4)?.clone();
    Some((
        command_type,
        vec![
            (MsgPackValue::from("cl"), checklist_uid),
            (MsgPackValue::from("m"), mission_uid),
            (MsgPackValue::from("tp"), template_uid),
            (MsgPackValue::from("n"), name),
        ],
    ))
}

fn msgpack_value_to_json(value: &MsgPackValue) -> Option<serde_json::Value> {
    match value {
        MsgPackValue::Nil => Some(serde_json::Value::Null),
        MsgPackValue::Boolean(value) => Some(serde_json::Value::Bool(*value)),
        MsgPackValue::Integer(value) => {
            if let Some(value) = value.as_u64() {
                Some(serde_json::Value::Number(serde_json::Number::from(value)))
            } else {
                value
                    .as_i64()
                    .map(serde_json::Number::from)
                    .map(serde_json::Value::Number)
            }
        }
        MsgPackValue::F32(value) => {
            serde_json::Number::from_f64(f64::from(*value)).map(serde_json::Value::Number)
        }
        MsgPackValue::F64(value) => {
            serde_json::Number::from_f64(*value).map(serde_json::Value::Number)
        }
        MsgPackValue::String(value) => value
            .as_str()
            .map(|value| serde_json::Value::String(value.to_string())),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone())
            .ok()
            .map(serde_json::Value::String),
        MsgPackValue::Array(values) => values
            .iter()
            .map(msgpack_value_to_json)
            .collect::<Option<Vec<_>>>()
            .map(serde_json::Value::Array),
        MsgPackValue::Map(entries) => {
            let mut object = serde_json::Map::new();
            for (key, value) in entries {
                object.insert(msgpack_string(key)?, msgpack_value_to_json(value)?);
            }
            Some(serde_json::Value::Object(object))
        }
        MsgPackValue::Ext(_, _) => None,
    }
}

fn ensure_task_for_incoming_update(
    checklist: &mut ChecklistRecord,
    task_uid: &str,
    timestamp: &str,
    number: Option<u32>,
) -> bool {
    if checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
        return false;
    }
    let mut task = placeholder_task_record(task_uid, timestamp);
    if let Some(number) = number.filter(|value| *value > 0) {
        task.number = number;
    }
    checklist.tasks.push(task);
    true
}

fn task_needs_row_metadata_hydration(task: &ChecklistTaskRecord) -> bool {
    task.number == 0
        && task.legacy_value.is_none()
        && task.due_relative_minutes.is_none()
        && task.due_dtg.is_none()
        && task.notes.is_none()
}

fn checklist_task_from_row_add_args(
    args: &[(MsgPackValue, MsgPackValue)],
    task_uid: &str,
    number: u32,
    timestamp: &str,
) -> Option<ChecklistTaskRecord> {
    msgpack_json_arg::<ChecklistTaskRecord>(args, "task").map(|mut task| {
        task.task_uid = task_uid.to_string();
        task.number = number;
        task.deleted_at = None;
        task.updated_at =
            newest_timestamp(task.updated_at.as_deref(), Some(timestamp)).map(ToString::to_string);
        for cell in &mut task.cells {
            cell.task_uid = task_uid.to_string();
            if cell.cell_uid.trim().is_empty() {
                cell.cell_uid = format!("{}:{}", task_uid, cell.column_uid);
            }
        }
        task
    })
}
