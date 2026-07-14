fn handle_inbound_checklist_update(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    if !incoming_timestamp_is_newer(checklist.updated_at.as_deref(), timestamp.as_str())
        || (checklist.deleted_at.is_some()
            && !is_hidden_placeholder_checklist(&checklist))
    {
        return false;
    }
    let Some(patch) =
        msgpack_get_checklist_arg(args, "patch").and_then(msgpack_map_entries)
    else {
        return false;
    };
    if let Some(value) =
        msgpack_get_checklist_arg(patch, "mission_uid").and_then(msgpack_string)
    {
        checklist.mission_uid = normalize_optional_string(Some(value.as_str()));
    }
    if let Some(value) = msgpack_get_checklist_arg(patch, "template_uid")
        .and_then(msgpack_checklist_template_uid)
    {
        checklist.template_uid = normalize_optional_string(Some(value.as_str()));
    }
    if let Some(value) =
        msgpack_get_checklist_arg(patch, "name").and_then(msgpack_string)
    {
        checklist.name = value.trim().to_string();
    }
    if let Some(value) =
        msgpack_get_checklist_arg(patch, "description").and_then(msgpack_string)
    {
        checklist.description = value.trim().to_string();
    }
    if let Some(value) =
        msgpack_get_checklist_arg(patch, "start_time").and_then(msgpack_string)
    {
        checklist.start_time = normalize_optional_string(Some(value.as_str()));
    }
    if let Some(column) =
        checklist_column_from_patch(patch, checklist.columns.len() as u32)
    {
        merge_checklist_column(&mut checklist, column);
    }
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-update",
    );

    persisted_any
}
