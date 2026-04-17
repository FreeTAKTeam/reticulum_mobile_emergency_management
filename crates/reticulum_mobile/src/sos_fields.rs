use rmpv::Value as MsgPackValue;

use crate::types::{NodeError, SosDeviceTelemetryRecord, SosMessageKind, SosTriggerSource};

pub(crate) const LXMF_FIELD_TELEMETRY: i64 = 0x02;
pub(crate) const LXMF_FIELD_COMMANDS: i64 = 0x09;
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
        MsgPackValue::from(LXMF_FIELD_COMMANDS),
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
    Some(SosFields {
        command: parse_command_field(msgpack_get_indexed(entries, LXMF_FIELD_COMMANDS)),
        telemetry: parse_telemetry_field(msgpack_get_indexed(entries, LXMF_FIELD_TELEMETRY)),
    })
}

pub(crate) fn looks_like_sos_text(body: &str) -> bool {
    let normalized = body.trim_start().to_ascii_uppercase();
    normalized.starts_with("SOS")
        || normalized.starts_with("URGENCE")
        || normalized.starts_with("EMERGENCY")
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
    let mut entries = vec![
        (
            MsgPackValue::from("sos_state"),
            MsgPackValue::from(sos_kind_to_str(command.state)),
        ),
        (
            MsgPackValue::from("incident_id"),
            MsgPackValue::from(command.incident_id.as_str()),
        ),
        (
            MsgPackValue::from("trigger_source"),
            MsgPackValue::from(trigger_source_to_str(command.trigger_source)),
        ),
        (
            MsgPackValue::from("sent_at_ms"),
            MsgPackValue::from(command.sent_at_ms),
        ),
    ];
    if let Some(audio_id) = command.audio_id.as_deref() {
        entries.push((MsgPackValue::from("audio_id"), MsgPackValue::from(audio_id)));
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
    let state = parse_sos_kind(msgpack_get_named(entries, &["sos_state", "state"])?)?;
    let incident_id = msgpack_get_named(entries, &["incident_id", "incidentId"])
        .and_then(msgpack_string)
        .unwrap_or_else(|| {
            format!(
                "sos-{}",
                msgpack_u64(
                    msgpack_get_named(entries, &["sent_at_ms"]).unwrap_or(&MsgPackValue::Nil)
                )
                .unwrap_or(0)
            )
        });
    let trigger_source = msgpack_get_named(entries, &["trigger_source", "triggerSource"])
        .and_then(parse_trigger_source)
        .unwrap_or(SosTriggerSource::Remote {});
    Some(SosCommand {
        state,
        incident_id,
        trigger_source,
        sent_at_ms: msgpack_get_named(entries, &["sent_at_ms", "sentAtMs"])
            .and_then(msgpack_u64)
            .unwrap_or(0),
        audio_id: msgpack_get_named(entries, &["audio_id", "audioId"]).and_then(msgpack_string),
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

fn msgpack_map_entries(value: &MsgPackValue) -> Option<&[(MsgPackValue, MsgPackValue)]> {
    match value {
        MsgPackValue::Map(entries) => Some(entries.as_slice()),
        _ => None,
    }
}

fn msgpack_get_indexed<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    key: i64,
) -> Option<&'a MsgPackValue> {
    let key_string = key.to_string();
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| match entry_key {
            MsgPackValue::Integer(value) if value.as_i64() == Some(key) => Some(entry_value),
            MsgPackValue::String(value) if value.as_str() == Some(key_string.as_str()) => {
                Some(entry_value)
            }
            _ => None,
        })
}

fn msgpack_get_named<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    keys: &[&str],
) -> Option<&'a MsgPackValue> {
    keys.iter().find_map(|wanted| {
        entries.iter().find_map(|(entry_key, entry_value)| {
            matches!(entry_key, MsgPackValue::String(actual) if actual.as_str() == Some(*wanted))
                .then_some(entry_value)
        })
    })
}

fn msgpack_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().map(str::to_string),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone()).ok(),
        _ => None,
    }
}

fn msgpack_bool(value: &MsgPackValue) -> Option<bool> {
    match value {
        MsgPackValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

fn msgpack_f64(value: &MsgPackValue) -> Option<f64> {
    match value {
        MsgPackValue::F32(value) => Some(f64::from(*value)),
        MsgPackValue::F64(value) => Some(*value),
        MsgPackValue::Integer(value) => value.as_i64().map(|entry| entry as f64),
        _ => None,
    }
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
    use super::*;

    #[test]
    fn sos_fields_round_trip_command_and_telemetry() {
        let command = SosCommand {
            state: SosMessageKind::Active {},
            incident_id: "incident-1".to_string(),
            trigger_source: SosTriggerSource::Shake {},
            sent_at_ms: 42,
            audio_id: Some("audio-1".to_string()),
        };
        let telemetry = SosDeviceTelemetryRecord {
            lat: Some(45.5),
            lon: Some(-63.25),
            alt: Some(20.0),
            speed: Some(1.5),
            course: Some(180.0),
            accuracy: Some(4.0),
            battery_percent: Some(88.0),
            battery_charging: Some(true),
            updated_at_ms: 1_700_000_000_000,
        };

        let encoded = build_sos_fields(&command, Some(&telemetry)).expect("encoded fields");
        let parsed = parse_sos_fields(&encoded).expect("parsed fields");

        assert_eq!(parsed.command.expect("command"), command);
        let parsed_telemetry = parsed.telemetry.expect("telemetry");
        assert_eq!(parsed_telemetry.lat.expect("lat").round(), 46.0);
        assert_eq!(parsed_telemetry.battery_charging, Some(true));
    }

    #[test]
    fn text_detection_accepts_legacy_prefixes() {
        assert!(looks_like_sos_text("SOS! I need help"));
        assert!(looks_like_sos_text("urgence besoin aide"));
        assert!(looks_like_sos_text("Emergency at 45.1,-63.2"));
        assert!(!looks_like_sos_text("normal chat"));
    }
}
