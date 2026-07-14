fn handle_inbound_checklist_status(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let incoming_number = msgpack_get_checklist_arg(args, "number")
        .and_then(msgpack_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);
    let explicit_task_uid =
        msgpack_get_checklist_arg(args, "task_uid").and_then(msgpack_string);
    if explicit_task_uid.is_none() && incoming_number.is_none() {
        return false;
    }
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
    let hidden_placeholder = is_hidden_placeholder_checklist(&checklist);
    if !hidden_placeholder
        && (checklist.deleted_at.as_deref().is_some_and(|deleted_at| {
            !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
        }) || checklist.deleted_at.is_some())
    {
        return true;
    }
    let Some(task_uid) = explicit_task_uid.or_else(|| {
        incoming_number.and_then(|number| {
            checklist
                .tasks
                .iter()
                .find(|task| task.number == number && task.deleted_at.is_none())
                .map(|task| task.task_uid.clone())
        })
    }) else {
        return false;
    };
    let resolved_task_uid = if checklist
        .tasks
        .iter()
        .any(|task| task.task_uid == task_uid && task.deleted_at.is_none())
    {
        task_uid.clone()
    } else {
        incoming_number
            .and_then(|number| {
                checklist
                    .tasks
                    .iter()
                    .find(|task| task.number == number && task.deleted_at.is_none())
                    .map(|task| task.task_uid.clone())
            })
            .unwrap_or_else(|| task_uid.clone())
    };
    let inserted_placeholder = ensure_task_for_incoming_update(
        &mut checklist,
        resolved_task_uid.as_str(),
        timestamp.as_str(),
        incoming_number,
    );
    let Ok(task) = find_checklist_task_mut(&mut checklist, resolved_task_uid.as_str())
    else {
        return false;
    };
    let user_status = if msgpack_get_checklist_arg(args, "completed")
        .and_then(msgpack_bool)
        .unwrap_or(false)
    {
        ChecklistUserTaskStatus::Complete {}
    } else {
        match msgpack_get_checklist_arg(args, "user_status")
            .and_then(msgpack_string)
            .as_deref()
        {
            Some("COMPLETE") => ChecklistUserTaskStatus::Complete {},
            _ => ChecklistUserTaskStatus::Pending {},
        }
    };
    if !should_apply_inbound_task_status(
        task,
        user_status,
        timestamp.as_str(),
        inserted_placeholder,
    ) {
        return true;
    }
    task.user_status = user_status;
    task.task_status = checklist_task_status_for(task.user_status, task.is_late);
    task.updated_at = Some(timestamp.clone());
    if task.task_status.is_complete() {
        task.completed_at = Some(timestamp.clone());
        task.completed_by_team_member_rns_identity =
            msgpack_get_checklist_arg(args, "changed_by_team_member_rns_identity")
                .and_then(msgpack_string)
                .or_else(|| source_identity.clone());
    } else {
        task.completed_at = None;
        task.completed_by_team_member_rns_identity = None;
    }
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-task-status",
    );

    persisted_any
}
