fn handle_inbound_checklist_join(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let Some(source_identity) = source_identity.clone() else {
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
        .participant_rns_identities
        .iter()
        .any(|value| value == &source_identity)
    {
        let changed_by = source_identity.clone();
        checklist.participant_rns_identities.push(source_identity);
        checklist.updated_at = Some(timestamp.clone());
        set_checklist_last_changed_by(&mut checklist, Some(changed_by.as_str()));
        normalize_checklist_record(&mut checklist);
        persisted_any |= upsert_inbound_checklist(
            app_state,
            bus,
            &checklist,
            "checklist-received-join",
        );
    }

    persisted_any
}
