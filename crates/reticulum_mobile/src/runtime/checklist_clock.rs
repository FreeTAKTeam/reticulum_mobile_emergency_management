const CHECKLIST_MAX_CLOCK_SKEW_SECONDS: i64 = 15 * 60;

fn checklist_timestamp_is_acceptable(timestamp: &str, now_seconds: i64) -> bool {
    parse_rfc3339_sort_key(timestamp).is_some_and(|(seconds, _)| {
        seconds <= now_seconds.saturating_add(CHECKLIST_MAX_CLOCK_SKEW_SECONDS)
    })
}

fn checklist_snapshot_clock_is_acceptable(checklist: &ChecklistRecord, now_seconds: i64) -> bool {
    let valid = |timestamp: &Option<String>| {
        timestamp
            .as_deref()
            .is_none_or(|value| checklist_timestamp_is_acceptable(value, now_seconds))
    };
    [
        &checklist.created_at,
        &checklist.updated_at,
        &checklist.uploaded_at,
        &checklist.deleted_at,
    ]
    .into_iter()
    .all(valid)
        && checklist.tasks.iter().all(|task| {
            [&task.updated_at, &task.deleted_at, &task.completed_at]
                .into_iter()
                .all(valid)
                && task.cells.iter().all(|cell| valid(&cell.updated_at))
        })
        && checklist
            .feed_publications
            .iter()
            .all(|publication| valid(&publication.published_at))
}

fn validate_inbound_checklist_clock(checklist: &ChecklistRecord, bus: &EventBus) -> bool {
    if checklist_snapshot_clock_is_acceptable(
        checklist,
        crate::numeric::u64_to_i64_saturating(now_ms() / 1_000),
    ) {
        return true;
    }
    bus.emit(NodeEvent::Error {
        code: "InvalidConfig".to_string(),
        message: format!("rejected inbound checklist uid={}: invalid timestamp or clock more than 15 minutes ahead", checklist.uid),
    });
    false
}
