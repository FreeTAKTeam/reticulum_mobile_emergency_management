fn parse_rfc3339_sort_key(timestamp: &str) -> Option<(i64, u32)> {
    let trimmed = timestamp.trim();
    let suffix = trimmed.strip_suffix('Z')?;
    let (date, time) = suffix.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let (time_main, fraction) = match time.split_once('.') {
        Some((main, fraction)) => (main, Some(fraction)),
        None => (time, None),
    };
    let mut time_parts = time_main.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second = time_parts.next()?.parse::<i64>().ok()?;
    if time_parts.next().is_some() {
        return None;
    }

    let nanos = match fraction {
        Some(value) => {
            if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            let truncated = &value[..value.len().min(9)];
            let mut padded = truncated.to_string();
            while padded.len() < 9 {
                padded.push('0');
            }
            padded.parse::<u32>().ok()?
        }
        None => 0,
    };

    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch = era * 146_097 + doe - 719_468;
    let seconds_since_epoch = days_since_epoch * 86_400 + hour * 3_600 + minute * 60 + second;
    Some((seconds_since_epoch, nanos))
}

fn incoming_timestamp_is_newer(local_timestamp: Option<&str>, incoming_timestamp: &str) -> bool {
    match (
        local_timestamp.and_then(parse_rfc3339_sort_key),
        parse_rfc3339_sort_key(incoming_timestamp),
    ) {
        (None, Some(_)) => true,
        (Some(local), Some(incoming)) => local < incoming,
        _ => local_timestamp.is_none_or(|local| local < incoming_timestamp),
    }
}

fn checklist_command_source_identity(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries)?;
    msgpack_get_named(source, &["rns_identity", "r"]).and_then(msgpack_hex_or_string)
}

fn checklist_command_source_display_name(
    command_map: &[(MsgPackValue, MsgPackValue)],
) -> Option<String> {
    let source = msgpack_get_named(command_map, &["source", "s"]).and_then(msgpack_map_entries)?;
    let display_name =
        msgpack_get_named(source, &["display_name", "n"]).and_then(msgpack_string)?;
    normalize_optional_string(Some(display_name.as_str()))
}

fn apply_checklist_creator_from_command(
    checklist: &mut ChecklistRecord,
    args: &[(MsgPackValue, MsgPackValue)],
    command_map: &[(MsgPackValue, MsgPackValue)],
    source_identity: Option<&str>,
) {
    if let Some(created_by) = msgpack_get_checklist_arg(args, "created_by_team_member_rns_identity")
        .and_then(msgpack_string)
    {
        checklist.created_by_team_member_rns_identity = created_by;
    }
    if checklist
        .created_by_team_member_rns_identity
        .trim()
        .is_empty()
    {
        checklist.created_by_team_member_rns_identity =
            source_identity.unwrap_or_default().to_string();
    }
    checklist.created_by_team_member_display_name =
        msgpack_get_checklist_arg(args, "created_by_team_member_display_name")
            .and_then(msgpack_string)
            .and_then(|value| normalize_optional_string(Some(value.as_str())))
            .or_else(|| checklist_command_source_display_name(command_map))
            .or(checklist.created_by_team_member_display_name.take());
}

fn emit_checklist_invalidations(
    bus: &EventBus,
    invalidations: Vec<crate::types::ProjectionInvalidation>,
) {
    for invalidation in invalidations {
        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    }
}

fn upsert_inbound_checklist(
    app_state: &AppStateStore,
    bus: &EventBus,
    checklist: &ChecklistRecord,
    reason: &str,
) -> bool {
    match app_state.upsert_checklist(checklist, reason) {
        Ok(invalidations) => {
            emit_checklist_invalidations(bus, invalidations);
            true
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist inbound checklist uid={} reason={reason} error={err}",
                    checklist.uid
                ),
            });
            false
        }
    }
}

fn blank_checklist_record(
    checklist_uid: &str,
    timestamp: &str,
    source_identity: Option<&str>,
) -> ChecklistRecord {
    ChecklistRecord {
        uid: checklist_uid.to_string(),
        mission_uid: None,
        template_uid: None,
        template_version: None,
        template_name: None,
        name: String::new(),
        description: String::new(),
        start_time: None,
        mode: crate::types::ChecklistMode::Online {},
        sync_state: ChecklistSyncState::Synced {},
        origin_type: crate::types::ChecklistOriginType::RchTemplate {},
        checklist_status: ChecklistTaskStatus::Pending {},
        created_at: Some(timestamp.to_string()),
        created_by_team_member_rns_identity: source_identity.unwrap_or_default().to_string(),
        created_by_team_member_display_name: None,
        updated_at: Some(timestamp.to_string()),
        last_changed_by_team_member_rns_identity: normalize_optional_string(source_identity),
        deleted_at: None,
        uploaded_at: None,
        participant_rns_identities: source_identity
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        expected_task_count: None,
        progress_percent: 0.0,
        counts: crate::types::ChecklistStatusCounts {
            pending_count: 0,
            late_count: 0,
            complete_count: 0,
        },
        columns: Vec::new(),
        tasks: Vec::new(),
        feed_publications: Vec::new(),
    }
}

fn hidden_placeholder_checklist_record(checklist_uid: &str, timestamp: &str) -> ChecklistRecord {
    let mut record = blank_checklist_record(checklist_uid, timestamp, None);
    record.deleted_at = Some(timestamp.to_string());
    record.updated_at = Some(timestamp.to_string());
    record
}

fn is_hidden_placeholder_checklist(record: &ChecklistRecord) -> bool {
    record.deleted_at.is_some()
        && record.mission_uid.is_none()
        && record.template_uid.is_none()
        && record.template_version.is_none()
        && record.template_name.is_none()
        && record.name.is_empty()
        && record.description.is_empty()
        && record.start_time.is_none()
        && record.created_by_team_member_rns_identity.trim().is_empty()
}

fn should_apply_inbound_checklist_create(
    existing: Option<&ChecklistRecord>,
    timestamp: &str,
) -> bool {
    let Some(record) = existing else {
        return true;
    };
    if is_hidden_placeholder_checklist(record) {
        return true;
    }
    incoming_timestamp_is_newer(record.updated_at.as_deref(), timestamp)
        && record
            .deleted_at
            .as_deref()
            .is_none_or(|deleted_at| incoming_timestamp_is_newer(Some(deleted_at), timestamp))
}

fn checklist_delete_record_from_command(
    existing: Option<ChecklistRecord>,
    checklist_uid: &str,
    timestamp: &str,
    source_identity: Option<&str>,
) -> Option<ChecklistRecord> {
    if existing.as_ref().is_some_and(|checklist| {
        !incoming_timestamp_is_newer(checklist.updated_at.as_deref(), timestamp)
            || checklist
                .deleted_at
                .as_deref()
                .is_some_and(|deleted_at| !incoming_timestamp_is_newer(Some(deleted_at), timestamp))
    }) {
        return None;
    }

    let mut checklist =
        existing.unwrap_or_else(|| blank_checklist_record(checklist_uid, timestamp, None));
    checklist.deleted_at = Some(timestamp.to_string());
    checklist.updated_at = Some(timestamp.to_string());
    set_checklist_last_changed_by(&mut checklist, source_identity);
    normalize_checklist_record(&mut checklist);
    Some(checklist)
}

fn timestamp_is_newer(left: Option<&str>, right: Option<&str>) -> bool {
    match (
        left.and_then(parse_rfc3339_sort_key),
        right.and_then(parse_rfc3339_sort_key),
    ) {
        (Some(left), Some(right)) => left > right,
        (Some(_), None) => true,
        (None, Some(_)) | (None, None) => false,
    }
}

fn timestamp_is_at_least(left: Option<&str>, right: Option<&str>) -> bool {
    match (
        left.and_then(parse_rfc3339_sort_key),
        right.and_then(parse_rfc3339_sort_key),
    ) {
        (Some(left), Some(right)) => left >= right,
        (Some(_), None) | (None, None) => true,
        (None, Some(_)) => false,
    }
}

fn newest_timestamp<'a>(left: Option<&'a str>, right: Option<&'a str>) -> Option<&'a str> {
    if timestamp_is_at_least(left, right) {
        left.or(right)
    } else {
        right.or(left)
    }
}

fn task_freshness_timestamp(task: &ChecklistTaskRecord) -> Option<&str> {
    newest_timestamp(task.deleted_at.as_deref(), task.updated_at.as_deref())
}

fn merge_uploaded_cells(
    mut local_cells: Vec<ChecklistCellRecord>,
    incoming_cells: Vec<ChecklistCellRecord>,
) -> Vec<ChecklistCellRecord> {
    for incoming_cell in incoming_cells {
        if let Some(index) = local_cells
            .iter()
            .position(|cell| cell.column_uid == incoming_cell.column_uid)
        {
            if timestamp_is_newer(
                incoming_cell.updated_at.as_deref(),
                local_cells[index].updated_at.as_deref(),
            ) {
                local_cells[index] = incoming_cell;
            }
        } else {
            local_cells.push(incoming_cell);
        }
    }
    local_cells
}

fn merge_uploaded_task_record(
    local_task: ChecklistTaskRecord,
    incoming_task: ChecklistTaskRecord,
) -> ChecklistTaskRecord {
    let local_task_at = task_freshness_timestamp(&local_task);
    let incoming_task_at = task_freshness_timestamp(&incoming_task);
    if local_task.deleted_at.is_some()
        && timestamp_is_at_least(local_task.deleted_at.as_deref(), incoming_task_at)
    {
        return local_task;
    }
    if incoming_task.deleted_at.is_some()
        && timestamp_is_at_least(incoming_task.deleted_at.as_deref(), local_task_at)
    {
        return incoming_task;
    }

    let mut merged = if timestamp_is_newer(
        incoming_task.updated_at.as_deref(),
        local_task.updated_at.as_deref(),
    ) {
        incoming_task.clone()
    } else {
        local_task.clone()
    };
    merged.cells = merge_uploaded_cells(local_task.cells, incoming_task.cells);
    merged
}

fn merge_uploaded_columns(
    mut local_columns: Vec<ChecklistColumnRecord>,
    incoming_columns: Vec<ChecklistColumnRecord>,
) -> Vec<ChecklistColumnRecord> {
    for incoming_column in incoming_columns {
        if !local_columns
            .iter()
            .any(|column| column.column_uid == incoming_column.column_uid)
        {
            local_columns.push(incoming_column);
        }
    }
    local_columns
}

fn merge_uploaded_tasks(
    mut local_tasks: Vec<ChecklistTaskRecord>,
    incoming_tasks: Vec<ChecklistTaskRecord>,
) -> Vec<ChecklistTaskRecord> {
    for incoming_task in incoming_tasks {
        if let Some(index) = local_tasks
            .iter()
            .position(|task| task.task_uid == incoming_task.task_uid)
        {
            let local_task = local_tasks[index].clone();
            local_tasks[index] = merge_uploaded_task_record(local_task, incoming_task);
        } else {
            local_tasks.push(incoming_task);
        }
    }
    local_tasks
}

fn merge_uploaded_participants(
    mut local_participants: Vec<String>,
    incoming_participants: Vec<String>,
    source_identity: Option<&str>,
) -> Vec<String> {
    for participant in incoming_participants {
        if !local_participants.iter().any(|value| value == &participant) {
            local_participants.push(participant);
        }
    }
    if let Some(source_identity) = normalize_optional_string(source_identity) {
        if !local_participants
            .iter()
            .any(|value| value == &source_identity)
        {
            local_participants.push(source_identity);
        }
    }
    local_participants
}

fn merge_uploaded_feed_publications(
    mut local_publications: Vec<crate::types::ChecklistFeedPublicationRecord>,
    incoming_publications: Vec<crate::types::ChecklistFeedPublicationRecord>,
) -> Vec<crate::types::ChecklistFeedPublicationRecord> {
    for incoming_publication in incoming_publications {
        if !local_publications
            .iter()
            .any(|publication| publication.publication_uid == incoming_publication.publication_uid)
        {
            local_publications.push(incoming_publication);
        }
    }
    local_publications
}

fn prepare_uploaded_snapshot(
    mut incoming: ChecklistRecord,
    timestamp: &str,
    source_identity: Option<&str>,
) -> ChecklistRecord {
    incoming.deleted_at = None;
    incoming.uploaded_at = normalize_optional_string(
        incoming
            .uploaded_at
            .clone()
            .or_else(|| Some(timestamp.to_string()))
            .as_deref(),
    );
    if incoming.created_at.is_none() {
        incoming.created_at = Some(timestamp.to_string());
    }
    if incoming.updated_at.is_none() {
        incoming.updated_at = Some(timestamp.to_string());
    }
    if incoming
        .created_by_team_member_rns_identity
        .trim()
        .is_empty()
    {
        incoming.created_by_team_member_rns_identity =
            source_identity.unwrap_or_default().to_string();
    }
    set_checklist_last_changed_by(&mut incoming, source_identity);
    incoming.participant_rns_identities = merge_uploaded_participants(
        Vec::new(),
        incoming.participant_rns_identities,
        source_identity,
    );
    incoming.sync_state = ChecklistSyncState::Synced {};
    if incoming.expected_task_count.is_none() {
        incoming.expected_task_count = Some(
            incoming
                .tasks
                .iter()
                .filter(|task| task.deleted_at.is_none())
                .count() as u32,
        );
    }
    normalize_checklist_record(&mut incoming);
    incoming
}
