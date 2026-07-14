fn handle_inbound_checklist_delete(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let Some(checklist) = checklist_delete_record_from_command(
        app_state
            .get_checklist_any(checklist_uid.as_str())
            .ok()
            .flatten(),
        checklist_uid.as_str(),
        timestamp.as_str(),
        source_identity.as_deref(),
    ) else {
        return false;
    };
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-delete",
    );

    persisted_any
}
