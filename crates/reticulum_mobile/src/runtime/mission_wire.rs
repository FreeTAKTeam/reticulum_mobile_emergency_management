#[derive(Debug, Deserialize)]
struct MissionWireSource {
    rns_identity: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EamUpsertCommandArgs {
    callsign: String,
    team_member_uid: String,
    team_uid: String,
    security_status: String,
    capability_status: String,
    preparedness_status: String,
    medical_status: String,
    mobility_status: String,
    comms_status: String,
    eam_uid: Option<String>,
    reported_by: Option<String>,
    reported_at: Option<String>,
    notes: Option<String>,
    confidence: Option<f64>,
    ttl_seconds: Option<u64>,
    source: Option<MissionWireSource>,
}

#[derive(Debug, Deserialize)]
struct MissionCommandEnvelope<T> {
    source: MissionWireSource,
    timestamp: String,
    command_type: String,
    args: T,
}

#[derive(Debug, Deserialize)]
struct EamWireBody {
    command: MissionCommandEnvelope<EamUpsertCommandArgs>,
    projection: Option<EamProjectionRecord>,
}

fn msgpack_event_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => {
            let hex = hex::encode(value);
            Some(format!(
                "evt-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_eam_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => {
            let hex = hex::encode(value);
            Some(format!(
                "eam-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_eam_status(value: &MsgPackValue) -> Option<String> {
    msgpack_string(value).map(|status| match status.as_str() {
        "G" => "Green".to_string(),
        "Y" => "Yellow".to_string(),
        "R" => "Red".to_string(),
        "U" => "Unknown".to_string(),
        _ => status,
    })
}

fn msgpack_eam_status_array<'a>(
    args: &'a [(MsgPackValue, MsgPackValue)],
) -> [Option<&'a MsgPackValue>; 6] {
    let mut statuses = [None, None, None, None, None, None];
    if let Some(values) =
        msgpack_get_named(args, &["statuses", "s"]).and_then(MsgPackValue::as_array)
    {
        for (index, value) in values.iter().take(statuses.len()).enumerate() {
            statuses[index] = Some(value);
        }
    }
    statuses
}

fn event_command_id_from_tail(uid: &str, value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(bytes) if bytes.len() == 16 => {
            let hex = hex::encode(bytes);
            Some(format!(
                "log-entry-{uid}-{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32],
            ))
        }
        _ => {
            let tail = msgpack_string(value)?;
            if tail.starts_with("log-entry-") {
                Some(tail)
            } else {
                Some(format!("log-entry-{uid}-{tail}"))
            }
        }
    }
}

fn msgpack_mission_uid(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) if value.as_u64() == Some(0) => {
            Some(DEFAULT_R3AKT_MISSION_UID.to_string())
        }
        _ => msgpack_string(value),
    }
}

fn msgpack_timestamp(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().map(|timestamp| {
            if timestamp < 10_000_000_000 {
                timestamp_ms_to_rfc3339(timestamp.saturating_mul(1_000))
            } else {
                timestamp_ms_to_rfc3339(timestamp)
            }
        }),
        _ => msgpack_string(value),
    }
}

fn msgpack_string_vec(value: &MsgPackValue) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(entries.iter().filter_map(msgpack_string).collect())
}

fn msgpack_event_keywords(value: &MsgPackValue) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(
        entries
            .iter()
            .filter_map(|entry| {
                let keyword = msgpack_string(entry)?;
                if keyword.len() <= 4 && keyword.chars().all(|ch| ch.is_ascii_alphanumeric()) {
                    Some(format!("r3akt:event-type:{keyword}"))
                } else {
                    Some(keyword)
                }
            })
            .collect(),
    )
}

fn msgpack_event_topics(value: &MsgPackValue, mission_uid: &str) -> Option<Vec<String>> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    Some(
        entries
            .iter()
            .filter_map(|entry| match entry {
                MsgPackValue::Integer(value) if value.as_u64() == Some(0) => {
                    Some(mission_uid.to_string())
                }
                MsgPackValue::Integer(value) if value.as_u64() == Some(1) => {
                    Some("Default".to_string())
                }
                _ => msgpack_string(entry),
            })
            .collect(),
    )
}

fn msgpack_u64(value: &MsgPackValue) -> Option<u64> {
    match value {
        MsgPackValue::Integer(value) => value.as_u64().or_else(|| {
            value
                .as_i64()
                .and_then(|entry| (entry >= 0).then_some(entry as u64))
        }),
        _ => None,
    }
}
