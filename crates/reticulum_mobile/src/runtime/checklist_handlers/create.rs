fn handle_inbound_checklist_create(ctx: &InboundChecklistCommand<'_>) -> bool {
    let app_state = ctx.app_state;
    let bus = ctx.bus;
    let command_map = ctx.command_map;
    let args = ctx.args;
    let timestamp = &ctx.timestamp;
    let source_identity = &ctx.source_identity;
    let content_bytes = ctx.content_bytes;
    let _ = (command_map, content_bytes);
    let mut persisted_any = false;

    let checklist_uid = msgpack_get_checklist_arg(args, "checklist_uid")
        .and_then(msgpack_checklist_uid)
        .or_else(|| {
            msgpack_get_named(command_map, &["command_id", "i"])
                .and_then(msgpack_string)
                .map(|value| value.trim_start_matches("cmd-").to_string())
        });
    let Some(checklist_uid) = checklist_uid else {
        return false;
    };
    let Some(mission_uid) =
        msgpack_get_checklist_arg(args, "mission_uid").and_then(msgpack_string)
    else {
        return false;
    };
    let Some(template_uid) = msgpack_get_checklist_arg(args, "template_uid")
        .and_then(msgpack_checklist_template_uid)
    else {
        return false;
    };
    let Some(name) = msgpack_get_checklist_arg(args, "name").and_then(msgpack_string)
    else {
        return false;
    };
    let description = msgpack_get_checklist_arg(args, "description")
        .and_then(msgpack_string)
        .unwrap_or_default();
    let start_time =
        msgpack_get_checklist_arg(args, "start_time").and_then(msgpack_string);
    let existing = app_state
        .get_checklist_any(checklist_uid.as_str())
        .unwrap_or_default();
    if !should_apply_inbound_checklist_create(existing.as_ref(), timestamp.as_str()) {
        return false;
    }
    let mut checklist = match existing {
        Some(record)
            if record.deleted_at.is_some()
                && !is_hidden_placeholder_checklist(&record) =>
        {
            blank_checklist_record(
                checklist_uid.as_str(),
                timestamp.as_str(),
                source_identity.as_deref(),
            )
        }
        Some(record) => record,
        None => blank_checklist_record(
            checklist_uid.as_str(),
            timestamp.as_str(),
            source_identity.as_deref(),
        ),
    };
    checklist.mission_uid = Some(mission_uid);
    checklist.template_uid = Some(template_uid);
    checklist.name = name;
    checklist.description = description;
    checklist.start_time = start_time;
    if let Some(columns) =
        msgpack_json_arg::<Vec<ChecklistColumnRecord>>(args, "columns")
    {
        checklist.columns = columns;
    }
    if let Some(tasks) = msgpack_json_arg::<Vec<ChecklistTaskRecord>>(args, "tasks") {
        checklist.tasks = tasks;
    }
    if let Some(participants) =
        msgpack_json_arg::<Vec<String>>(args, "participant_rns_identities")
    {
        checklist.participant_rns_identities = merge_uploaded_participants(
            checklist.participant_rns_identities,
            participants,
            source_identity.as_deref(),
        );
    }
    if let Some(total_tasks) =
        msgpack_get_checklist_arg(args, "total_tasks").and_then(msgpack_u64)
    {
        checklist.expected_task_count = Some(crate::numeric::u64_to_u32_saturating(total_tasks));
    }
    if let Some(created_at) =
        msgpack_get_checklist_arg(args, "created_at").and_then(msgpack_string)
    {
        checklist.created_at = Some(created_at);
    }
    if let Some(uploaded_at) =
        msgpack_get_checklist_arg(args, "uploaded_at").and_then(msgpack_string)
    {
        checklist.uploaded_at = Some(uploaded_at);
    }
    checklist.updated_at = Some(timestamp.clone());
    checklist.deleted_at = None;
    if checklist.created_at.is_none() {
        checklist.created_at = Some(timestamp.clone());
    }
    apply_checklist_creator_from_command(
        &mut checklist,
        args,
        command_map,
        source_identity.as_deref(),
    );
    if let Some(source_identity) = checklist_command_source_identity(command_map) {
        if !checklist
            .participant_rns_identities
            .iter()
            .any(|value| value == &source_identity)
        {
            checklist.participant_rns_identities.push(source_identity);
        }
    }
    if let Some(snapshot_json) =
        checklist_snapshot_json_from_content(content_bytes, checklist_uid.as_str())
    {
        if let Ok(mut snapshot) =
            serde_json::from_str::<ChecklistRecord>(snapshot_json.as_str())
        {
            snapshot.uid = checklist_uid.clone();
            if snapshot.mission_uid.is_none() {
                snapshot.mission_uid = checklist.mission_uid.clone();
            }
            if snapshot.template_uid.is_none() {
                snapshot.template_uid = checklist.template_uid.clone();
            }
            if snapshot.name.trim().is_empty() {
                snapshot.name = checklist.name.clone();
            }
            if snapshot.description.trim().is_empty() {
                snapshot.description = checklist.description.clone();
            }
            if snapshot.start_time.is_none() {
                snapshot.start_time = checklist.start_time.clone();
            }
            if snapshot.created_at.is_none() {
                snapshot.created_at = checklist.created_at.clone();
            }
            if snapshot
                .created_by_team_member_rns_identity
                .trim()
                .is_empty()
            {
                snapshot.created_by_team_member_rns_identity =
                    checklist.created_by_team_member_rns_identity.clone();
            }
            if snapshot.created_by_team_member_display_name.is_none() {
                snapshot.created_by_team_member_display_name =
                    checklist.created_by_team_member_display_name.clone();
            }
            if snapshot.uploaded_at.is_none() {
                snapshot.uploaded_at = checklist.uploaded_at.clone();
            }
            snapshot.updated_at = Some(timestamp.clone());
            snapshot.deleted_at = None;
            snapshot.sync_state = ChecklistSyncState::Synced {};
            snapshot.participant_rns_identities = merge_uploaded_participants(
                checklist.participant_rns_identities,
                snapshot.participant_rns_identities,
                source_identity.as_deref(),
            );
            set_checklist_last_changed_by(&mut snapshot, source_identity.as_deref());
            normalize_checklist_record(&mut snapshot);
            checklist = snapshot;
        }
    }
    hydrate_checklist_from_local_template(app_state, &mut checklist);
    set_checklist_last_changed_by(&mut checklist, source_identity.as_deref());
    normalize_checklist_record(&mut checklist);
    persisted_any |= upsert_inbound_checklist(
        app_state,
        bus,
        &checklist,
        "checklist-received-create",
    );

    persisted_any
}
