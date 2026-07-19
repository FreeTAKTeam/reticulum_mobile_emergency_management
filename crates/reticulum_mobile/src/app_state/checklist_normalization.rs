fn projection_scope_name(scope: ProjectionScope) -> &'static str {
    match scope {
        ProjectionScope::AppSettings {} => "AppSettings",
        ProjectionScope::SavedPeers {} => "SavedPeers",
        ProjectionScope::OperationalSummary {} => "OperationalSummary",
        ProjectionScope::Peers {} => "Peers",
        ProjectionScope::SyncStatus {} => "SyncStatus",
        ProjectionScope::HubRegistration {} => "HubRegistration",
        ProjectionScope::Checklists {} => "Checklists",
        ProjectionScope::ChecklistDetail {} => "ChecklistDetail",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
        ProjectionScope::Sos {} => "Sos",
        ProjectionScope::Plugins {} => "Plugins",
        ProjectionScope::PluginSensors {} => "PluginSensors",
    }
}

pub(crate) fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn set_checklist_last_changed_by(
    checklist: &mut ChecklistRecord,
    identity: Option<&str>,
) {
    checklist.last_changed_by_team_member_rns_identity = normalize_optional_string(identity);
}

fn sanitize_active_checklist(mut checklist: ChecklistRecord) -> Option<ChecklistRecord> {
    if checklist.deleted_at.is_some() {
        return None;
    }
    checklist.tasks.retain(|task| task.deleted_at.is_none());
    Some(checklist)
}

pub(crate) fn normalize_checklist_record(checklist: &mut ChecklistRecord) {
    let start_epoch_seconds = checklist
        .start_time
        .as_deref()
        .and_then(parse_rfc3339_epoch_seconds);
    let now_epoch_seconds = unix_seconds_now();
    for task in &mut checklist.tasks {
        let due_epoch_seconds = start_epoch_seconds.and_then(|start| {
            task.due_relative_minutes
                .map(|minutes| start.saturating_add(i64::from(minutes) * 60))
        });
        task.due_dtg = due_epoch_seconds.map(format_rfc3339_from_epoch_seconds);
        task.is_late =
            checklist_task_is_late_for_due_dtg(task, due_epoch_seconds, now_epoch_seconds);
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        task.cells.sort_by(|left, right| {
            left.column_uid
                .cmp(&right.column_uid)
                .then_with(|| left.cell_uid.cmp(&right.cell_uid))
        });
    }
    checklist.columns.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.column_uid.cmp(&right.column_uid))
    });
    checklist.tasks.sort_by(|left, right| {
        left.number
            .cmp(&right.number)
            .then_with(|| left.task_uid.cmp(&right.task_uid))
    });

    let active_tasks = checklist
        .tasks
        .iter()
        .filter(|task| task.deleted_at.is_none())
        .collect::<Vec<_>>();
    let pending_count = crate::numeric::usize_to_u32_saturating(
        active_tasks
            .iter()
            .copied()
            .filter(|task| matches!(task.task_status, ChecklistTaskStatus::Pending {}))
            .count(),
    );
    let late_count = crate::numeric::usize_to_u32_saturating(
        active_tasks
            .iter()
            .copied()
            .filter(|task| matches!(task.task_status, ChecklistTaskStatus::Late {}))
            .count(),
    );
    let complete_count = crate::numeric::usize_to_u32_saturating(
        active_tasks
            .iter()
            .copied()
            .filter(|task| task.task_status.is_complete())
            .count(),
    );
    checklist.counts.pending_count = pending_count;
    checklist.counts.late_count = late_count;
    checklist.counts.complete_count = complete_count;
    let total = crate::numeric::usize_to_u32_saturating(active_tasks.len());
    if checklist.expected_task_count.is_none() {
        checklist.expected_task_count = Some(total);
    }
    checklist.progress_percent = if total == 0 {
        0.0
    } else {
        (f64::from(complete_count) * 100.0) / f64::from(total)
    };
    checklist.checklist_status = if late_count > 0 {
        ChecklistTaskStatus::Late {}
    } else if pending_count > 0 || total == 0 {
        ChecklistTaskStatus::Pending {}
    } else if active_tasks
        .iter()
        .copied()
        .any(|task| matches!(task.task_status, ChecklistTaskStatus::CompleteLate {}))
    {
        ChecklistTaskStatus::CompleteLate {}
    } else {
        ChecklistTaskStatus::Complete {}
    };
}

fn normalize_checklist_template(template: &mut ChecklistTemplateRecord) {
    template.name = template.name.trim().to_string();
    template.description = template.description.trim().to_string();
    template
        .source_filename
        .clone_from(&normalize_optional_string(
            template.source_filename.as_deref(),
        ));
    for task in &mut template.tasks {
        task.is_late = task.task_status.is_late();
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        task.cells.sort_by(|left, right| {
            left.column_uid
                .cmp(&right.column_uid)
                .then_with(|| left.cell_uid.cmp(&right.cell_uid))
        });
    }
    template.columns.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.column_uid.cmp(&right.column_uid))
    });
    template.tasks.sort_by(|left, right| {
        left.number
            .cmp(&right.number)
            .then_with(|| left.task_uid.cmp(&right.task_uid))
    });
}

fn normalize_checklist(checklist: &mut ChecklistRecord) {
    normalize_checklist_record(checklist);
}

fn checklist_task_is_late_for_due_dtg(
    task: &ChecklistTaskRecord,
    due_epoch_seconds: Option<i64>,
    now_epoch_seconds: i64,
) -> bool {
    let Some(due_epoch_seconds) = due_epoch_seconds else {
        return task.task_status.is_late();
    };
    match task.user_status {
        ChecklistUserTaskStatus::Pending {} => now_epoch_seconds > due_epoch_seconds,
        ChecklistUserTaskStatus::Complete {} => task
            .completed_at
            .as_deref()
            .and_then(parse_rfc3339_epoch_seconds)
            .map(|completed_epoch_seconds| completed_epoch_seconds > due_epoch_seconds)
            .unwrap_or(false),
    }
}

pub(crate) fn checklist_task_status_for(
    user_status: ChecklistUserTaskStatus,
    is_late: bool,
) -> ChecklistTaskStatus {
    match user_status {
        ChecklistUserTaskStatus::Pending {} => {
            if is_late {
                ChecklistTaskStatus::Late {}
            } else {
                ChecklistTaskStatus::Pending {}
            }
        }
        ChecklistUserTaskStatus::Complete {} => {
            if is_late {
                ChecklistTaskStatus::CompleteLate {}
            } else {
                ChecklistTaskStatus::Complete {}
            }
        }
    }
}

pub(crate) fn find_checklist_task_mut<'a>(
    checklist: &'a mut ChecklistRecord,
    task_uid: &str,
) -> Result<&'a mut ChecklistTaskRecord, NodeError> {
    checklist
        .tasks
        .iter_mut()
        .find(|task| task.task_uid == task_uid && task.deleted_at.is_none())
        .ok_or(NodeError::InvalidConfig {})
}

pub(crate) fn current_timestamp_rfc3339() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds_since_epoch = crate::numeric::u64_to_i64_saturating(duration.as_secs());
    let nanos = duration.subsec_nanos();
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}
