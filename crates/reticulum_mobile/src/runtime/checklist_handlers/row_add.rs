fn handle_inbound_checklist_row_add(ctx: &InboundChecklistCommand<'_>) -> bool {
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
    let Some(number) = msgpack_get_checklist_arg(args, "number").and_then(msgpack_u64)
    else {
        return false;
    };
    let incoming_task_payload = checklist_task_from_row_add_args(
        args,
        task_uid.as_str(),
        number as u32,
        timestamp.as_str(),
    );
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
    if let Some(task) = checklist
        .tasks
        .iter()
        .find(|task| task.task_uid == task_uid)
    {
        if incoming_task_payload.is_none()
            && (task.deleted_at.as_deref().is_some_and(|deleted_at| {
                !incoming_timestamp_is_newer(Some(deleted_at), timestamp.as_str())
            }) || (!task_needs_row_metadata_hydration(task)
                && !incoming_timestamp_is_newer(
                    task.updated_at.as_deref(),
                    timestamp.as_str(),
                )))
        {
            return false;
        }
    }
    let due_relative_minutes = msgpack_get_checklist_arg(args, "due_relative_minutes")
        .and_then(msgpack_u64)
        .map(|value| value as u32);
    let legacy_value =
        msgpack_get_checklist_arg(args, "legacy_value").and_then(msgpack_string);
    let due_dtg = msgpack_get_checklist_arg(args, "due_dtg").and_then(msgpack_string);
    let notes = msgpack_get_checklist_arg(args, "notes").and_then(msgpack_string);
    if let Some(incoming_task) = incoming_task_payload {
        if let Some(index) = checklist
            .tasks
            .iter()
            .position(|task| task.task_uid == task_uid)
        {
            let local_task = checklist.tasks[index].clone();
            checklist.tasks[index] =
                merge_uploaded_task_record(local_task, incoming_task);
        } else {
            checklist.tasks.push(incoming_task);
        }
    } else if let Some(task) = checklist
        .tasks
        .iter_mut()
        .find(|task| task.task_uid == task_uid)
    {
        task.number = number as u32;
        task.due_relative_minutes = due_relative_minutes;
        task.due_dtg = due_dtg.clone();
        task.notes = notes.clone();
        task.legacy_value = legacy_value;
        task.deleted_at = None;
        task.updated_at =
            newest_timestamp(task.updated_at.as_deref(), Some(timestamp.as_str()))
                .map(ToString::to_string);
    } else {
        let cells = blank_task_cells(checklist.columns.as_slice(), task_uid.as_str());
        checklist.tasks.push(ChecklistTaskRecord {
            task_uid,
            number: number as u32,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some(timestamp.clone()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes,
            due_dtg,
            notes,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value,
            cells,
        });
    }
    checklist.updated_at = Some(timestamp.clone());
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-task-row-add",
    );

    persisted_any
}
