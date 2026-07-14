use rmpv::Value as MsgPackValue;

use crate::lxmf_fields::FIELD_COMMANDS;
use crate::mission_commands::command_wire_value;
use crate::msgpack_values::{
    msgpack_bool, msgpack_f64, msgpack_get_indexed, msgpack_get_named, msgpack_map_entries,
    msgpack_string,
};
use crate::types::{NodeError, SosDeviceTelemetryRecord, SosMessageKind, SosTriggerSource};

pub(crate) const LXMF_FIELD_TELEMETRY: i64 = 0x02;
pub(crate) const SID_TIME: i64 = 0x01;
pub(crate) const SID_LOCATION: i64 = 0x02;
pub(crate) const SID_BATTERY: i64 = 0x04;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SosCommand {
    pub(crate) state: SosMessageKind,
    pub(crate) incident_id: String,
    pub(crate) trigger_source: SosTriggerSource,
    pub(crate) sent_at_ms: u64,
    pub(crate) audio_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SosFields {
    pub(crate) command: Option<SosCommand>,
    pub(crate) telemetry: Option<SosDeviceTelemetryRecord>,
}

pub(crate) fn build_sos_fields(
    command: &SosCommand,
    telemetry: Option<&SosDeviceTelemetryRecord>,
) -> Result<Vec<u8>, NodeError> {
    let mut entries = vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![command_to_msgpack(command)]),
    )];

    if let Some(telemetry) = telemetry {
        entries.push((
            MsgPackValue::from(LXMF_FIELD_TELEMETRY),
            MsgPackValue::Binary(build_telemeter_payload(telemetry)?),
        ));
    }

    rmp_serde::to_vec(&MsgPackValue::Map(entries)).map_err(|_| NodeError::InternalError {})
}

pub(crate) fn parse_sos_fields(fields_bytes: &[u8]) -> Option<SosFields> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let entries = msgpack_map_entries(&fields)?;
    let parsed = SosFields {
        command: parse_command_field(msgpack_get_indexed(entries, FIELD_COMMANDS)),
        telemetry: parse_telemetry_field(msgpack_get_indexed(entries, LXMF_FIELD_TELEMETRY)),
    };
    (parsed.command.is_some() || parsed.telemetry.is_some()).then_some(parsed)
}

pub(crate) fn sos_kind_from_text(body: &str) -> Option<SosMessageKind> {
    let normalized = body.trim_start().to_ascii_uppercase();
    if !normalized.starts_with("SOS")
        && !normalized.starts_with("URGENCE")
        && !normalized.starts_with("EMERGENCY")
    {
        return None;
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.contains("cancelled")
        || lower.contains("canceled")
        || lower.contains("cancel")
        || lower.contains("ended")
        || lower.contains("i am safe")
        || lower.contains("i'm safe")
    {
        return Some(SosMessageKind::Cancelled {});
    }
    Some(SosMessageKind::Active {})
}

pub(crate) fn extract_text_coordinates(body: &str) -> Option<(f64, f64)> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in body.chars() {
        if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.') {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(value) = current.parse::<f64>() {
            numbers.push(value);
        }
    }
    numbers.windows(2).find_map(|pair| {
        let lat = pair[0];
        let lon = pair[1];
        ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
    })
}

fn command_to_msgpack(command: &SosCommand) -> MsgPackValue {
    let state = sos_kind_to_str(command.state);
    let command_id = format!("sos:{}:{state}:{}", command.incident_id, command.sent_at_ms);
    let mut entries = vec![
        (
            MsgPackValue::from("i"),
            MsgPackValue::from(command_id.as_str()),
        ),
        (
            MsgPackValue::from("c"),
            MsgPackValue::from(command.incident_id.as_str()),
        ),
        (
            MsgPackValue::from("t"),
            MsgPackValue::from(command_wire_value("sos.status")),
        ),
        (MsgPackValue::from("ss"), MsgPackValue::from(state)),
        (
            MsgPackValue::from("ii"),
            MsgPackValue::from(command.incident_id.as_str()),
        ),
        (
            MsgPackValue::from("tr"),
            MsgPackValue::from(trigger_source_to_str(command.trigger_source)),
        ),
        (
            MsgPackValue::from("sm"),
            MsgPackValue::from(command.sent_at_ms),
        ),
        (
            MsgPackValue::from("a"),
            MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("ii"),
                    MsgPackValue::from(command.incident_id.as_str()),
                ),
                (MsgPackValue::from("ss"), MsgPackValue::from(state)),
                (
                    MsgPackValue::from("tr"),
                    MsgPackValue::from(trigger_source_to_str(command.trigger_source)),
                ),
            ]),
        ),
    ];
    if let Some(audio_id) = command.audio_id.as_deref() {
        entries.push((MsgPackValue::from("au"), MsgPackValue::from(audio_id)));
    }
    MsgPackValue::Map(entries)
}

fn build_telemeter_payload(telemetry: &SosDeviceTelemetryRecord) -> Result<Vec<u8>, NodeError> {
    let mut entries = vec![(
        MsgPackValue::from(SID_TIME),
        MsgPackValue::from((telemetry.updated_at_ms / 1000) as i64),
    )];

    if let (Some(lat), Some(lon)) = (telemetry.lat, telemetry.lon) {
        entries.push((
            MsgPackValue::from(SID_LOCATION),
            MsgPackValue::Array(vec![
                MsgPackValue::from((lat * 1_000_000.0).round() as i64),
                MsgPackValue::from((lon * 1_000_000.0).round() as i64),
                MsgPackValue::from(telemetry.alt.unwrap_or(0.0).round().max(0.0) as u64),
                MsgPackValue::from((telemetry.speed.unwrap_or(0.0) * 100.0).round().max(0.0) as u64),
                MsgPackValue::from((telemetry.course.unwrap_or(0.0) * 100.0).round().max(0.0) as u64),
                MsgPackValue::from((telemetry.accuracy.unwrap_or(0.0) * 10.0).round().max(0.0) as u64),
                MsgPackValue::from((telemetry.updated_at_ms / 1000) as i64),
            ]),
        ));
    }

    if let Some(percent) = telemetry.battery_percent {
        entries.push((
            MsgPackValue::from(SID_BATTERY),
            MsgPackValue::Array(vec![
                MsgPackValue::from((percent / 100.0).clamp(0.0, 1.0)),
                MsgPackValue::Boolean(telemetry.battery_charging.unwrap_or(false)),
            ]),
        ));
    }

    rmp_serde::to_vec(&MsgPackValue::Map(entries)).map_err(|_| NodeError::InternalError {})
}

fn parse_command_field(value: Option<&MsgPackValue>) -> Option<SosCommand> {
    let value = value?;
    let command = match value {
        MsgPackValue::Array(items) => items
            .iter()
            .find(|item| parse_command_map(item).is_some())?,
        other => other,
    };
    parse_command_map(command)
}

fn parse_command_map(value: &MsgPackValue) -> Option<SosCommand> {
    let entries = msgpack_map_entries(value)?;
    let state = parse_sos_kind(msgpack_get_named(entries, &["sos_state", "state", "ss"])?)?;
    let incident_id = msgpack_get_named(entries, &["incident_id", "incidentId", "ii"])
        .and_then(msgpack_string)
        .unwrap_or_else(|| {
            format!(
                "sos-{}",
                msgpack_u64(
                    msgpack_get_named(entries, &["sent_at_ms", "sentAtMs", "sm"])
                        .unwrap_or(&MsgPackValue::Nil)
                )
                .unwrap_or(0)
            )
        });
    let trigger_source = msgpack_get_named(entries, &["trigger_source", "triggerSource", "tr"])
        .and_then(parse_trigger_source)
        .unwrap_or(SosTriggerSource::Remote {});
    Some(SosCommand {
        state,
        incident_id,
        trigger_source,
        sent_at_ms: msgpack_get_named(entries, &["sent_at_ms", "sentAtMs", "sm"])
            .and_then(msgpack_u64)
            .unwrap_or(0),
        audio_id: msgpack_get_named(entries, &["audio_id", "audioId", "au"])
            .and_then(msgpack_string),
    })
}

fn parse_telemetry_field(value: Option<&MsgPackValue>) -> Option<SosDeviceTelemetryRecord> {
    let value = value?;
    let payload = match value {
        MsgPackValue::Binary(bytes) => rmp_serde::from_slice::<MsgPackValue>(bytes).ok()?,
        other => other.clone(),
    };
    let entries = msgpack_map_entries(&payload)?;
    let mut telemetry = SosDeviceTelemetryRecord {
        lat: None,
        lon: None,
        alt: None,
        speed: None,
        course: None,
        accuracy: None,
        battery_percent: None,
        battery_charging: None,
        updated_at_ms: 0,
    };
    if let Some(time) = msgpack_get_indexed(entries, SID_TIME).and_then(msgpack_u64) {
        telemetry.updated_at_ms = time.saturating_mul(1000);
    }
    if let Some(MsgPackValue::Array(items)) = msgpack_get_indexed(entries, SID_LOCATION) {
        telemetry.lat = items
            .first()
            .and_then(msgpack_f64)
            .map(|value| value / 1_000_000.0);
        telemetry.lon = items
            .get(1)
            .and_then(msgpack_f64)
            .map(|value| value / 1_000_000.0);
        telemetry.alt = items.get(2).and_then(msgpack_f64);
        telemetry.speed = items
            .get(3)
            .and_then(msgpack_f64)
            .map(|value| value / 100.0);
        telemetry.course = items
            .get(4)
            .and_then(msgpack_f64)
            .map(|value| value / 100.0);
        telemetry.accuracy = items.get(5).and_then(msgpack_f64).map(|value| value / 10.0);
        if let Some(time) = items.get(6).and_then(msgpack_u64) {
            telemetry.updated_at_ms = time.saturating_mul(1000);
        }
    }
    if let Some(MsgPackValue::Array(items)) = msgpack_get_indexed(entries, SID_BATTERY) {
        telemetry.battery_percent = items
            .first()
            .and_then(msgpack_f64)
            .map(|value| value * 100.0);
        telemetry.battery_charging = items.get(1).and_then(msgpack_bool);
    }
    (telemetry.lat.is_some() || telemetry.lon.is_some() || telemetry.battery_percent.is_some())
        .then_some(telemetry)
}

fn msgpack_u64(value: &MsgPackValue) -> Option<u64> {
    match value {
        MsgPackValue::Integer(value) => value
            .as_u64()
            .or_else(|| value.as_i64().map(|v| v.max(0) as u64)),
        _ => None,
    }
}

fn parse_sos_kind(value: &MsgPackValue) -> Option<SosMessageKind> {
    match msgpack_string(value)?.trim().to_ascii_lowercase().as_str() {
        "active" => Some(SosMessageKind::Active {}),
        "update" => Some(SosMessageKind::Update {}),
        "cancelled" | "canceled" => Some(SosMessageKind::Cancelled {}),
        _ => None,
    }
}

pub(crate) fn sos_kind_to_str(value: SosMessageKind) -> &'static str {
    match value {
        SosMessageKind::Active {} => "active",
        SosMessageKind::Update {} => "update",
        SosMessageKind::Cancelled {} => "cancelled",
    }
}

fn parse_trigger_source(value: &MsgPackValue) -> Option<SosTriggerSource> {
    match msgpack_string(value)?.trim().to_ascii_lowercase().as_str() {
        "manual" => Some(SosTriggerSource::Manual {}),
        "floatingbutton" | "floating_button" | "floating-button" => {
            Some(SosTriggerSource::FloatingButton {})
        }
        "shake" => Some(SosTriggerSource::Shake {}),
        "tappattern" | "tap_pattern" | "tap-pattern" => Some(SosTriggerSource::TapPattern {}),
        "powerbutton" | "power_button" | "power-button" => Some(SosTriggerSource::PowerButton {}),
        "restore" => Some(SosTriggerSource::Restore {}),
        "remote" => Some(SosTriggerSource::Remote {}),
        _ => None,
    }
}

pub(crate) fn trigger_source_to_str(value: SosTriggerSource) -> &'static str {
    match value {
        SosTriggerSource::Manual {} => "manual",
        SosTriggerSource::FloatingButton {} => "floating_button",
        SosTriggerSource::Shake {} => "shake",
        SosTriggerSource::TapPattern {} => "tap_pattern",
        SosTriggerSource::PowerButton {} => "power_button",
        SosTriggerSource::Restore {} => "restore",
        SosTriggerSource::Remote {} => "remote",
    }
}

#[cfg(test)]
mod tests {
    include!("sos_fields/tests.rs");
}
