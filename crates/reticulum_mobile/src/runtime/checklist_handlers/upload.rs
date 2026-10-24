fn handle_inbound_checklist_upload(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let Some(snapshot_json) =
        checklist_snapshot_json_from_content(content_bytes, checklist_uid.as_str())
            .or_else(|| checklist_snapshot_json_from_command(command_map))
    else {
        return false;
    };
    let Ok(mut checklist) =
        serde_json::from_str::<ChecklistRecord>(snapshot_json.as_str())
    else {
        return false;
    };
    checklist.uid = checklist_uid.clone();
    let existing = app_state
        .get_checklist_any(checklist_uid.as_str())
        .unwrap_or_default();
    let Some(checklist) = merge_uploaded_checklist_snapshot(
        existing,
        checklist,
        timestamp.as_str(),
        source_identity.as_deref(),
    ) else {
        return false;
    };
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-upload",
    );

    persisted_any
}
