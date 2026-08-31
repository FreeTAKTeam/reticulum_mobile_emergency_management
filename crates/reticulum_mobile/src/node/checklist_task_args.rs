fn checklist_task_cell_args_json(
    request: &ChecklistTaskCellSetRequest,
) -> JsonMap<String, JsonValue> {
    let mut args = JsonMap::new();
    args.insert(
        "checklist_uid".to_string(),
        JsonValue::from(request.checklist_uid.trim()),
    );
    args.insert(
        "task_uid".to_string(),
        JsonValue::from(request.task_uid.trim()),
    );
    args.insert(
        "column_uid".to_string(),
        JsonValue::from(request.column_uid.trim()),
    );
    args.insert("value".to_string(), JsonValue::from(request.value.clone()));
    if let Some(identity) = request
        .updated_by_team_member_rns_identity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.insert(
            "updated_by_team_member_rns_identity".to_string(),
            JsonValue::from(identity),
        );
    }
    args
}
