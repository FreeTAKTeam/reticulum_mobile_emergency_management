fn handle_inbound_checklist_row_delete(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let existing = app_state
        .get_checklist_any(checklist_uid.as_str())
        .ok()
        .flatten();
    if existing.as_ref().is_some_and(|checklist| {
        checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
        }) || (checklist.deleted_at.is_some()
            && !is_hidden_placeholder_checklist(checklist))
    }) {
        return false;
    }
    let mut checklist = existing.unwrap_or_else(|| {
        hidden_placeholder_checklist_record(checklist_uid.as_str(), timestamp.as_str())
    });
    if let Some(existing_task) = checklist
        .tasks
        .iter()
        .find(|task| task.task_uid == task_uid)
    {
        if !incoming_timestamp_is_newer(
            existing_task.updated_at.as_deref(),
            timestamp.as_str(),
        ) || existing_task
            .deleted_at
            .as_deref()
            .is_some_and(|deleted_at| {
                !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
            })
        {
            return false;
        }
    }
    if !checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
        checklist.tasks.push(tombstoned_task_record(
            task_uid.as_str(),
            timestamp.as_str(),
        ));
    }
    if let Some(task) = checklist
        .tasks
        .iter_mut()
        .find(|task| task.task_uid == task_uid)
    {
        task.deleted_at = Some(timestamp.clone());
        task.updated_at = Some(timestamp.clone());
    } else {
        return false;
    }
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-task-row-delete",
    );

    persisted_any
}
