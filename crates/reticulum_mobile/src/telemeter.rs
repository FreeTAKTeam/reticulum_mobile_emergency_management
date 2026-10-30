//! REM adapters for the shared RCH Telemeter contract. REM stores speed in m/s.
use crate::types::{NodeError, SosDeviceTelemetryRecord, TelemetryPositionRecord};
use r3akt_map_core::telemetry_encode::{self, Location};

pub(crate) fn encode_position(
    position: &TelemetryPositionRecord,
    columba_mps: bool,
) -> Result<Vec<u8>, NodeError> {
    telemetry_encode::encode(
        position.updated_at_ms / 1000,
        Some(Location {
            latitude: position.lat,
            longitude: position.lon,
            altitude: position.alt.unwrap_or(0.0),
            speed: position.speed.unwrap_or(0.0) * if columba_mps { 1.0 } else { 3.6 },
            bearing: position.course.unwrap_or(0.0),
            accuracy: position.accuracy.unwrap_or(0.0),
            updated_at_s: position.updated_at_ms / 1000,
        }),
        None,
    )
    .map_err(|_| NodeError::InvalidConfig {})
}

pub(crate) fn encode_sos(record: &SosDeviceTelemetryRecord) -> Result<Vec<u8>, NodeError> {
    let location = match (record.lat, record.lon) {
        (Some(latitude), Some(longitude)) => Some(Location {
            latitude,
            longitude,
            altitude: record.alt.unwrap_or(0.0),
            speed: record.speed.unwrap_or(0.0) * 3.6,
            bearing: record.course.unwrap_or(0.0),
            accuracy: record.accuracy.unwrap_or(0.0),
            updated_at_s: record.updated_at_ms / 1000,
        }),
        (None, None) => None,
        _ => return Err(NodeError::InvalidConfig {}),
    };
    telemetry_encode::encode(
        record.updated_at_ms / 1000,
        location,
        record
            .battery_percent
            .map(|percent| (percent, record.battery_charging.unwrap_or(false))),
    )
    .map_err(|_| NodeError::InvalidConfig {})
}

pub(crate) fn decode(
    bytes: &[u8],
    columba_mps: bool,
) -> Result<SosDeviceTelemetryRecord, &'static str> {
    let checked = r3akt_map_core::telemetry::decode_field(&serde_json::json!(bytes))?;
    let location = &checked.telemetry["location"];
    let battery = &checked.telemetry["battery"];
    let timestamp = location["last_update_timestamp"]
        .as_f64()
        .or_else(|| checked.telemetry["time"]["timestamp"].as_f64())
        .ok_or("missing telemetry source time")?;
    Ok(SosDeviceTelemetryRecord {
        lat: location["latitude"].as_f64(),
        lon: location["longitude"].as_f64(),
        alt: location["altitude"].as_f64(),
        course: location["bearing"].as_f64(),
        speed: location["speed"]
            .as_f64()
            .map(|speed| speed / if columba_mps { 1.0 } else { 3.6 }),
        accuracy: location["accuracy"].as_f64(),
        battery_percent: battery["charge_percent"].as_f64(),
        battery_charging: battery["charging"].as_bool(),
        updated_at_ms: crate::numeric::f64_to_u64_saturating(timestamp * 1000.0),
    })
}
