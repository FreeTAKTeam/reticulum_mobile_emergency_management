fn handle_inbound_checklist_row_style(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let inserted_placeholder = ensure_task_for_incoming_update(
        &mut checklist,
        task_uid.as_str(),
        timestamp.as_str(),
        None,
    );
    let Ok(task) = find_checklist_task_mut(&mut checklist, task_uid.as_str()) else {
        return false;
    };
    if !inserted_placeholder
        && !incoming_timestamp_is_newer(task.updated_at.as_deref(), timestamp.as_str())
    {
        return false;
    }
    if let Some(value) =
        msgpack_get_checklist_arg(args, "row_background_color").and_then(msgpack_string)
    {
        task.row_background_color = normalize_optional_string(Some(value.as_str()));
    }
    if let Some(value) =
        msgpack_get_checklist_arg(args, "line_break_enabled").and_then(msgpack_bool)
    {
        task.line_break_enabled = value;
    }
    task.updated_at = Some(timestamp.clone());
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-task-row-style",
    );

    persisted_any
}
