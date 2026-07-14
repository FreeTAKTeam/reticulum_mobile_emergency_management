fn parse_checklist_template_csv(
    request: &ChecklistTemplateImportCsvRequest,
    default_task_due_step_minutes: u32,
) -> Result<ChecklistTemplateRecord, NodeError> {
    let name = request.name.trim();
    if name.is_empty() || request.csv_text.trim().is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(request.csv_text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| NodeError::InvalidConfig {})?
        .clone();
    let mut rows = Vec::<Vec<String>>::new();
    for row in reader.records() {
        let row = row.map_err(|_| NodeError::InvalidConfig {})?;
        let cells = row
            .iter()
            .map(|cell| cell.replace('\u{feff}', "").trim().to_string())
            .collect::<Vec<_>>();
        if cells.iter().any(|cell| !cell.is_empty()) {
            rows.push(cells);
        }
    }
    if rows.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let max_columns = rows
        .iter()
        .fold(headers.len(), |max, row| max.max(row.len()));
    if max_columns == 0 {
        return Err(NodeError::InvalidConfig {});
    }
    let header_names = (0..max_columns)
        .map(|index| {
            headers
                .get(index)
                .map(|value| value.replace('\u{feff}', "").trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("Column {}", index + 1))
        })
        .collect::<Vec<_>>();
    let due_header_index = header_names
        .iter()
        .position(|header| is_checklist_due_header(header));

    let mut columns = Vec::new();
    columns.push(ChecklistColumnRecord {
        column_uid: "col-due-relative-dtg".to_string(),
        column_name: due_header_index
            .and_then(|index| header_names.get(index))
            .cloned()
            .unwrap_or_else(|| "CompletedDTG".to_string()),
        display_order: 0,
        column_type: ChecklistColumnType::RelativeTime {},
        column_editable: false,
        background_color: None,
        text_color: None,
        is_removable: false,
        system_key: Some(crate::types::ChecklistSystemColumnKey::DueRelativeDtg {}),
    });

    let mut used_column_uids = HashMap::<String, u32>::new();
    used_column_uids.insert("col-due-relative-dtg".to_string(), 1);
    let mut header_column_uids = HashMap::<usize, String>::new();
    for (header_index, header) in header_names.iter().enumerate() {
        if Some(header_index) == due_header_index {
            continue;
        }
        let column_uid = checklist_csv_column_uid(header, header_index, &mut used_column_uids);
        header_column_uids.insert(header_index, column_uid.clone());
        columns.push(ChecklistColumnRecord {
            column_uid,
            column_name: header.clone(),
            display_order: columns.len() as u32,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: true,
            system_key: None,
        });
    }
    if header_column_uids.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let title_header_index = header_names
        .iter()
        .enumerate()
        .find(|(index, header)| {
            Some(*index) != due_header_index && is_checklist_title_header(header)
        })
        .map(|(index, _)| index);
    let description_header_index = header_names
        .iter()
        .enumerate()
        .find(|(index, header)| {
            Some(*index) != due_header_index && is_checklist_description_header(header)
        })
        .map(|(index, _)| index);
    let template_uid_seed = request
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("tmpl-import");
    let due_step = default_task_due_step_minutes.max(1);
    let mut tasks = Vec::new();
    for row in rows {
        let number = (tasks.len() + 1) as u32;
        let task_uid = format!("{template_uid_seed}-task-{number}");
        let due_relative_minutes = match due_header_index {
            Some(index) => {
                let value = csv_cell(&row, index);
                if value.is_empty() {
                    Some(number * due_step)
                } else {
                    Some(parse_checklist_due_relative_minutes(value)?)
                }
            }
            None => Some(number * due_step),
        };
        let title = title_header_index
            .map(|index| csv_cell(&row, index))
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                header_names
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| Some(*index) != due_header_index)
                    .map(|(index, _)| csv_cell(&row, index))
                    .find(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("Task {number}"));
        let notes = description_header_index
            .map(|index| csv_cell(&row, index))
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let cells = header_column_uids
            .iter()
            .map(|(header_index, column_uid)| {
                let value = csv_cell(&row, *header_index).to_string();
                ChecklistCellRecord {
                    cell_uid: format!("{task_uid}:{column_uid}"),
                    task_uid: task_uid.clone(),
                    column_uid: column_uid.clone(),
                    value: Some(value),
                    updated_at: None,
                    updated_by_team_member_rns_identity: None,
                }
            })
            .collect::<Vec<_>>();
        tasks.push(ChecklistTaskRecord {
            task_uid,
            number,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: None,
            deleted_at: None,
            custom_status: None,
            due_relative_minutes,
            due_dtg: None,
            notes,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some(title),
            cells,
        });
    }

    let timestamp = current_timestamp_rfc3339();
    let template_uid = request
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("tmpl-{}", now_ms()));
    let mut template = ChecklistTemplateRecord {
        uid: template_uid,
        name: name.to_string(),
        description: request
            .description
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        version: 1,
        origin_type: ChecklistOriginType::CsvImport {},
        created_at: Some(timestamp.clone()),
        updated_at: Some(timestamp),
        source_filename: normalize_optional_string(request.source_filename.as_deref()),
        columns,
        tasks,
    };
    normalize_checklist_template(&mut template);
    Ok(template)
}

fn csv_cell(row: &[String], index: usize) -> &str {
    row.get(index).map(String::as_str).unwrap_or_default()
}

fn normalize_checklist_csv_header(value: &str) -> String {
    value
        .replace('\u{feff}', "")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_checklist_due_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "completeddtg" | "due" | "duerelativedtg" | "duerelativeminutes" | "dueminutes"
    )
}

fn is_checklist_title_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "item" | "task" | "name" | "title"
    )
}

fn is_checklist_description_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "description" | "detail" | "details" | "notes"
    )
}

fn checklist_csv_column_uid(header: &str, index: usize, used: &mut HashMap<String, u32>) -> String {
    let mut slug = header
        .replace('\u{feff}', "")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = format!("column-{}", index + 1);
    }
    let base = format!("col-{slug}");
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{count}")
    }
}

fn parse_checklist_due_relative_minutes(value: &str) -> Result<u32, NodeError> {
    let mut text = value.trim().to_ascii_lowercase();
    if text.is_empty() || text.starts_with('-') {
        return Err(NodeError::InvalidConfig {});
    }
    if let Some(stripped) = text.strip_prefix('+') {
        text = stripped.trim().to_string();
    }
    if let Some((hours, minutes)) = text.split_once(':') {
        let hours = hours
            .trim()
            .parse::<u32>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        let minutes = minutes
            .trim()
            .parse::<u32>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        if minutes >= 60 {
            return Err(NodeError::InvalidConfig {});
        }
        return Ok(hours * 60 + minutes);
    }
    if let Some(hours) = text.strip_suffix('h') {
        return hours
            .trim()
            .parse::<u32>()
            .map(|value| value * 60)
            .map_err(|_| NodeError::InvalidConfig {});
    }
    for suffix in ["hours", "hour"] {
        if let Some(hours) = text.strip_suffix(suffix) {
            return hours
                .trim()
                .parse::<u32>()
                .map(|value| value * 60)
                .map_err(|_| NodeError::InvalidConfig {});
        }
    }
    text.parse::<u32>().map_err(|_| NodeError::InvalidConfig {})
}
