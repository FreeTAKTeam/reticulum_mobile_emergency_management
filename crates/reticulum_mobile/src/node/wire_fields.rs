fn eam_status_rank(value: &str) -> u8 {
    match value {
        "Green" => 1,
        "Yellow" => 2,
        "Red" => 3,
        _ => 0,
    }
}

fn derive_eam_overall_status(record: &EamProjectionRecord) -> Option<String> {
    let mut best_status: Option<&str> = None;
    for value in [
        record.security_status.as_str(),
        record.capability_status.as_str(),
        record.preparedness_status.as_str(),
        record.medical_status.as_str(),
        record.mobility_status.as_str(),
        record.comms_status.as_str(),
    ] {
        if eam_status_rank(value) >= eam_status_rank(best_status.unwrap_or_default()) {
            best_status = Some(value);
        }
    }
    best_status
        .filter(|value| !value.is_empty() && *value != "Unknown")
        .map(str::to_string)
}

fn msgpack_map(entries: Vec<(&str, MsgPackValue)>) -> MsgPackValue {
    MsgPackValue::Map(
        entries
            .into_iter()
            .map(|(key, value)| (MsgPackValue::from(key), value))
            .collect(),
    )
}

fn msgpack_hex_identity(value: &str) -> MsgPackValue {
    match hex::decode(value.trim()) {
        Ok(bytes) if bytes.len() == 16 => MsgPackValue::Binary(bytes),
        _ => MsgPackValue::from(value),
    }
}

fn is_default_event_topics(values: &[String], mission_uid: &str) -> bool {
    values.len() == 2
        && values.first().is_some_and(|value| value == mission_uid)
        && values.get(1).is_some_and(|value| value == "Default")
}

fn json_value_to_msgpack(value: &JsonValue) -> Result<MsgPackValue, NodeError> {
    match value {
        JsonValue::Null => Ok(MsgPackValue::Nil),
        JsonValue::Bool(value) => Ok(MsgPackValue::Boolean(*value)),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_u64() {
                Ok(MsgPackValue::from(value))
            } else if let Some(value) = value.as_i64() {
                Ok(MsgPackValue::from(value))
            } else if let Some(value) = value.as_f64() {
                Ok(MsgPackValue::from(value))
            } else {
                Err(NodeError::InvalidConfig {})
            }
        }
        JsonValue::String(value) => Ok(MsgPackValue::from(value.as_str())),
        JsonValue::Array(values) => Ok(MsgPackValue::Array(
            values
                .iter()
                .map(json_value_to_msgpack)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        JsonValue::Object(entries) => Ok(MsgPackValue::Map(
            entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        MsgPackValue::from(key.as_str()),
                        json_value_to_msgpack(value)?,
                    ))
                })
                .collect::<Result<Vec<_>, NodeError>>()?,
        )),
    }
}

fn generated_checklist_uid_wire_value(value: &str) -> MsgPackValue {
    value
        .trim()
        .strip_prefix("chk-")
        .filter(|suffix| {
            suffix.len() >= 10
                && !suffix.starts_with('0')
                && suffix.chars().all(|ch| ch.is_ascii_digit())
        })
        .and_then(|suffix| suffix.parse::<u64>().ok())
        .map(MsgPackValue::from)
        .unwrap_or_else(|| MsgPackValue::from(value))
}

fn default_checklist_template_wire_code(value: &str) -> Option<u64> {
    match value.trim() {
        "tmpl-24-hour-survival-pack" => Some(1),
        "tmpl-72-hour-home-preparedness" => Some(2),
        "tmpl-vehicle-emergency-preparedness" => Some(3),
        _ => None,
    }
}

fn default_checklist_template_wire_value(value: &str) -> MsgPackValue {
    default_checklist_template_wire_code(value)
        .map(MsgPackValue::from)
        .unwrap_or_else(|| MsgPackValue::from(value))
}

fn checklist_arg_msgpack_value(key: &str, value: &JsonValue) -> Result<MsgPackValue, NodeError> {
    match key {
        "checklist_uid" => value
            .as_str()
            .map(generated_checklist_uid_wire_value)
            .ok_or(NodeError::InvalidConfig {}),
        "template_uid" => value
            .as_str()
            .map(default_checklist_template_wire_value)
            .ok_or(NodeError::InvalidConfig {}),
        _ => json_value_to_msgpack(value),
    }
}

fn checklist_args_to_msgpack(args: &JsonMap<String, JsonValue>) -> Result<MsgPackValue, NodeError> {
    Ok(MsgPackValue::Map(
        args.iter()
            .map(|(key, value)| {
                let value = if key == "patch" {
                    match value {
                        JsonValue::Object(patch) => checklist_args_to_msgpack(patch)?,
                        _ => json_value_to_msgpack(value)?,
                    }
                } else {
                    checklist_arg_msgpack_value(key, value)?
                };
                Ok((
                    MsgPackValue::from(checklist_arg_wire_key(key.as_str())),
                    value,
                ))
            })
            .collect::<Result<Vec<_>, NodeError>>()?,
    ))
}

fn checklist_string_arg<'a>(args: &'a JsonMap<String, JsonValue>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(JsonValue::as_str).map(str::trim)
}

fn checklist_key_arg(args: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    checklist_string_arg(args, key)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sanitize_correlation_token(value: &str) -> String {
    let mut token = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while token.contains("--") {
        token = token.replace("--", "-");
    }
    token.trim_matches('-').to_string()
}

fn compact_u64_token(value: u64) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let mut remaining = value;
    let mut chars = Vec::new();
    while remaining > 0 {
        let digit = (remaining % 36) as u8;
        chars.push(match digit {
            0..=9 => (b'0' + digit) as char,
            _ => (b'a' + (digit - 10)) as char,
        });
        remaining /= 36;
    }
    chars.iter().rev().collect()
}

fn compact_subject_token(token: &str) -> String {
    const MAX_SUBJECT_LEN: usize = 32;
    let token = token.trim();
    if token.len() <= MAX_SUBJECT_LEN {
        return token.to_string();
    }

    let mut hash = 0xcbf29ce484222325_u64;
    for byte in token.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let prefix = token.chars().take(12).collect::<String>();
    format!("{prefix}-{}", compact_u64_token(hash))
}

fn compact_hex_binary(value: &str) -> Option<MsgPackValue> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if normalized.len() != 32 {
        return None;
    }
    hex::decode(normalized).ok().map(MsgPackValue::Binary)
}

fn event_uid_wire_value(uid: &str) -> MsgPackValue {
    compact_hex_binary(uid.trim_start_matches("evt-")).unwrap_or_else(|| MsgPackValue::from(uid))
}

fn event_content_wire_body(content: &str) -> Vec<u8> {
    let trimmed = content.trim();
    trimmed
        .strip_prefix("MECP/2/")
        .filter(|event_code| !event_code.trim().is_empty())
        .unwrap_or(trimmed)
        .as_bytes()
        .to_vec()
}

fn status_wire_code(status: &str) -> &str {
    match status.trim() {
        "Green" => "G",
        "Yellow" => "Y",
        "Red" => "R",
        "Unknown" => "U",
        _ => "U",
    }
}

fn mission_uid_wire_value(mission_uid: &str) -> MsgPackValue {
    if mission_uid == DEFAULT_R3AKT_MISSION_UID {
        MsgPackValue::from(0_u64)
    } else {
        MsgPackValue::from(mission_uid)
    }
}

fn event_topics_wire_value(topics: &[String], mission_uid: &str) -> MsgPackValue {
    MsgPackValue::Array(
        topics
            .iter()
            .map(|topic| {
                if topic == mission_uid {
                    MsgPackValue::from(0_u64)
                } else if topic == "Default" {
                    MsgPackValue::from(1_u64)
                } else {
                    MsgPackValue::from(topic.as_str())
                }
            })
            .collect(),
    )
}

fn checklist_topics_from_args(args: &JsonMap<String, JsonValue>) -> Vec<String> {
    let mut topics = Vec::new();
    for key in ["mission_uid", "checklist_uid"] {
        if let Some(value) = checklist_key_arg(args, key) {
            if !topics.iter().any(|existing| existing == &value) {
                topics.push(value);
            }
        }
    }
    topics
}

fn checklist_subject_part(args: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    checklist_key_arg(args, key)
        .map(|value| sanitize_correlation_token(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn checklist_subject_token(command_type: &str, args: &JsonMap<String, JsonValue>) -> String {
    let checklist_uid = checklist_subject_part(args, "checklist_uid");
    let task_uid = checklist_subject_part(args, "task_uid");
    let column_uid = checklist_subject_part(args, "column_uid");
    if task_uid.is_some() || column_uid.is_some() {
        let parts = [
            checklist_uid.as_deref(),
            task_uid.as_deref(),
            column_uid.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if !parts.is_empty() {
            return parts.join("-");
        }
    }
    for key in ["checklist_uid", "mission_uid", "template_uid"] {
        if let Some(sanitized) = checklist_subject_part(args, key) {
            return sanitized;
        }
    }
    sanitize_correlation_token(command_type)
}

fn build_checklist_command_fields(
    status: &NodeStatus,
    _target: &MissionReplicationTarget,
    command_type: &str,
    args: &JsonMap<String, JsonValue>,
    command_id_override: Option<&str>,
) -> Result<Vec<u8>, NodeError> {
    let send_ts_ms = now_ms();
    let subject = compact_subject_token(checklist_subject_token(command_type, args).as_str());
    let command_code = command_wire_value(command_type).to_ascii_lowercase();
    let correlation_id = command_id_override.map_or_else(
        || {
            format!(
                "c:{command_code}:{subject}:{}",
                compact_u64_token(send_ts_ms)
            )
        },
        str::to_string,
    );
    let command_id = command_id_override
        .map(str::to_string)
        .unwrap_or_else(|| correlation_id.clone());
    if command_type == "checklist.task.status.set" {
        let mut command_entries = vec![("t", MsgPackValue::from(command_wire_value(command_type)))];
        if let Some(checklist_uid) = checklist_key_arg(args, "checklist_uid") {
            command_entries.push((
                "cl",
                generated_checklist_uid_wire_value(checklist_uid.as_str()),
            ));
        }
        if let Some(number) = args.get("number").and_then(JsonValue::as_u64) {
            command_entries.push(("no", MsgPackValue::from(number)));
        } else if let Some(task_uid) = checklist_key_arg(args, "task_uid") {
            command_entries.push(("tsk", MsgPackValue::from(task_uid.as_str())));
        }
        let completed = args
            .get("user_status")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value == "COMPLETE");
        command_entries.push(("x", MsgPackValue::from(completed)));
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![msgpack_map(command_entries)]),
        )]);
        return rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {});
    }
    if command_type == "checklist.create.online" {
        let command_entries = MsgPackValue::Array(vec![
            MsgPackValue::from(1_u64),
            args.get("checklist_uid")
                .map(|value| checklist_arg_msgpack_value("checklist_uid", value))
                .transpose()?
                .ok_or(NodeError::InvalidConfig {})?,
            args.get("mission_uid")
                .map(|value| checklist_arg_msgpack_value("mission_uid", value))
                .transpose()?
                .ok_or(NodeError::InvalidConfig {})?,
            args.get("template_uid")
                .map(|value| checklist_arg_msgpack_value("template_uid", value))
                .transpose()?
                .ok_or(NodeError::InvalidConfig {})?,
            args.get("name")
                .map(|value| checklist_arg_msgpack_value("name", value))
                .transpose()?
                .ok_or(NodeError::InvalidConfig {})?,
        ]);
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![command_entries]),
        )]);
        return rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {});
    }
    if command_type == "checklist.task.row.add" {
        let mut command_entries = vec![("t", MsgPackValue::from(command_wire_value(command_type)))];
        for key in [
            "checklist_uid",
            "task_uid",
            "number",
            "due_relative_minutes",
            "due_dtg",
            "legacy_value",
            "notes",
        ] {
            if let Some(value) = args.get(key) {
                command_entries.push((
                    checklist_arg_wire_key(key),
                    checklist_arg_msgpack_value(key, value)?,
                ));
            }
        }
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![msgpack_map(command_entries)]),
        )]);
        return rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {});
    }
    let topics = checklist_topics_from_args(args)
        .into_iter()
        .map(MsgPackValue::from)
        .collect::<Vec<_>>();
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![msgpack_map(vec![
            ("i", MsgPackValue::from(command_id.as_str())),
            ("c", MsgPackValue::from(correlation_id.as_str())),
            ("t", MsgPackValue::from(command_wire_value(command_type))),
            (
                "s",
                msgpack_map(vec![(
                    "r",
                    msgpack_hex_identity(status.identity_hex.as_str()),
                )]),
            ),
            ("ts", MsgPackValue::from(send_ts_ms)),
            ("to", MsgPackValue::Array(topics)),
            ("a", checklist_args_to_msgpack(args)?),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).map_err(|_| NodeError::InternalError {})
}
