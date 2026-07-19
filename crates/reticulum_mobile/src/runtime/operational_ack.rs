#[derive(Debug, Clone)]
struct OperationalAck {
    destination_hex: String,
    command_id: String,
    correlation_id: Option<String>,
    command_type: Option<String>,
}

fn operational_ack_from_metadata(
    source_hex: Option<&str>,
    metadata: Option<&MissionSyncMetadata>,
) -> Option<OperationalAck> {
    let metadata = metadata?;
    if metadata.result_present || !metadata.command_present {
        return None;
    }
    let destination_hex = normalize_hex_32(source_hex?)?;
    let command_id = metadata
        .command_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    Some(OperationalAck {
        destination_hex,
        command_id,
        correlation_id: metadata
            .correlation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        command_type: metadata
            .command_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
fn build_operational_ack_fields(
    ack: &OperationalAck,
    by_identity: &str,
) -> Result<Vec<u8>, NodeError> {
    let mut result_entries = vec![
        (
            MsgPackValue::from("command_id"),
            MsgPackValue::from(ack.command_id.as_str()),
        ),
        (MsgPackValue::from("status"), MsgPackValue::from("accepted")),
        (
            MsgPackValue::from("accepted_at"),
            MsgPackValue::from(current_timestamp_rfc3339().as_str()),
        ),
        (
            MsgPackValue::from("by_identity"),
            MsgPackValue::from(by_identity),
        ),
    ];
    if let Some(correlation_id) = ack.correlation_id.as_deref() {
        result_entries.push((
            MsgPackValue::from("correlation_id"),
            MsgPackValue::from(correlation_id),
        ));
    }
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(result_entries),
    )]);
    rmp_serde::to_vec(&fields).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
}

fn compact_event_uid_ack_value(command_id: &str) -> Option<MsgPackValue> {
    let value = command_id.strip_prefix("log-entry-")?;
    let event_uid = if value.starts_with("evt-") && value.len() >= 40 {
        &value[..40]
    } else {
        value
    };
    let normalized = event_uid
        .trim_start_matches("evt-")
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if normalized.len() != 32 {
        return None;
    }
    hex::decode(normalized).ok().map(MsgPackValue::Binary)
}

fn build_compact_operational_ack_fields(ack: &OperationalAck) -> Result<Vec<u8>, NodeError> {
    let result_entries = if ack.command_type.as_deref() == Some("mission.registry.log_entry.upsert")
    {
        if let Some(event_uid) = compact_event_uid_ack_value(ack.command_id.as_str()) {
            vec![(MsgPackValue::from("u"), event_uid)]
        } else {
            vec![
                (
                    MsgPackValue::from("i"),
                    MsgPackValue::from(ack.command_id.as_str()),
                ),
                (MsgPackValue::from("s"), MsgPackValue::from("a")),
            ]
        }
    } else {
        vec![
            (
                MsgPackValue::from("i"),
                MsgPackValue::from(ack.command_id.as_str()),
            ),
            (MsgPackValue::from("s"), MsgPackValue::from("a")),
        ]
    };
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_RESULTS),
        MsgPackValue::Map(result_entries),
    )]);
    rmp_serde::to_vec(&fields).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
}

fn telemetry_position_from_sos(
    callsign: &str,
    telemetry: Option<&SosDeviceTelemetryRecord>,
    fallback_updated_at_ms: u64,
) -> Option<TelemetryPositionRecord> {
    let telemetry = telemetry?;
    let lat = telemetry.lat?;
    let lon = telemetry.lon?;
    let callsign = callsign.trim();
    if callsign.is_empty() {
        return None;
    }

    Some(TelemetryPositionRecord {
        callsign: callsign.to_ascii_lowercase(),
        lat,
        lon,
        alt: telemetry.alt,
        course: telemetry.course,
        speed: telemetry.speed,
        accuracy: telemetry.accuracy,
        updated_at_ms: if telemetry.updated_at_ms > 0 {
            telemetry.updated_at_ms
        } else {
            fallback_updated_at_ms
        },
    })
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

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
