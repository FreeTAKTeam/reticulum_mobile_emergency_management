fn handle_inbound_checklist_cell(ctx: &InboundChecklistCommand<'_>) -> bool {
    let app_state = ctx.app_state;
    let bus = ctx.bus;
    let command_map = ctx.command_map;
    let args = ctx.args;
    let timestamp = &ctx.timestamp;
    let source_identity = &ctx.source_identity;
    let content_bytes = ctx.content_bytes;
    let _ = (command_map, content_bytes);
    let mut persisted_any = false;

    let Some(checklist_uid) = msgpack_get_checklist_arg(args, "checklist_uid")
        .and_then(msgpack_checklist_uid)
    else {
        return false;
    };
    let Some(task_uid) =
        msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string)
    else {
        return false;
    };
    let Some(column_uid) =
        msgpack_get_checklist_arg(args, "column_uid").and_then(msgpack_string)
    else {
        return false;
    };
    let Some(value) = msgpack_get_checklist_arg(args, "value").and_then(msgpack_string)
    else {
        return false;
    };
    let mut checklist = app_state
        .get_checklist_any(checklist_uid.as_str())
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            hidden_placeholder_checklist_record(
                checklist_uid.as_str(),
                timestamp.as_str(),
            )
        });
    if checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
        !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
    }) || (checklist.deleted_at.is_some()
        && !is_hidden_placeholder_checklist(&checklist))
    {
        return false;
    }
    if !checklist
        .columns
        .iter()
        .any(|column| column.column_uid == column_uid)
    {
        let display_order = crate::numeric::usize_to_u32_saturating(checklist.columns.len());
        checklist.columns.push(ChecklistColumnRecord {
            column_uid: column_uid.clone(),
            column_name: column_uid.clone(),
            display_order,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: true,
            system_key: None,
        });
    }
    if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
        checklist.tasks.push(placeholder_task_record(
            task_uid.as_str(),
            timestamp.as_str(),
        ));
    }
    let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
        return false;
    };
    if let Some(cell) = task.cells.iter().find(|cell| cell.column_uid == column_uid) {
        if !incoming_timestamp_is_newer(cell.updated_at.as_deref(), timestamp.as_str())
        {
            return false;
        }
    }
    if let Some(cell) = task
        .cells
        .iter_mut()
        .find(|cell| cell.column_uid == column_uid)
    {
        cell.value = Some(value);
        cell.updated_at = Some(timestamp.clone());
        cell.updated_by_team_member_rns_identity =
            msgpack_get_checklist_arg(args, "updated_by_team_member_rns_identity")
                .and_then(msgpack_string)
                .or_else(|| source_identity.clone());
    } else {
        task.cells.push(ChecklistCellRecord {
            cell_uid: format!("{}:{column_uid}", task.task_uid),
            task_uid: task.task_uid.clone(),
            column_uid: column_uid.clone(),
            value: Some(value),
            updated_at: Some(timestamp.clone()),
            updated_by_team_member_rns_identity: msgpack_get_checklist_arg(
                args,
                "updated_by_team_member_rns_identity",
            )
            .and_then(msgpack_string)
            .or_else(|| source_identity.clone()),
        });
    }
    task.updated_at = Some(timestamp.clone());
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-task-cell",
    );

    persisted_any
}
