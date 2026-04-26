use super::events::{WearableSensorEvent, WearableSensorType, WearableSensorValue};
use super::registry::{WearableDeviceConfigRecord, WearableStatusKind, WearableStatusRecord};

pub const DEFAULT_WEARABLE_STALE_TIMEOUT_SECONDS: u32 = 30;

pub fn validate_event(event: &WearableSensorEvent) -> Result<(), String> {
    if event.event_type.trim() != "wearable.heart_rate" {
        return Err("unsupported wearable event type".to_string());
    }
    if event.source.trim().is_empty() {
        return Err("wearable event source is required".to_string());
    }
    if event.device_id.trim().is_empty() {
        return Err("wearable device_id is required".to_string());
    }
    if !event.confidence.is_finite() || event.confidence < 0.0 || event.confidence > 1.0 {
        return Err("wearable confidence must be between 0 and 1".to_string());
    }
    match (&event.sensor_type, &event.value) {
        (WearableSensorType::HeartRateBpm {}, WearableSensorValue::Integer(value)) => {
            if *value <= 0 || *value > 240 {
                return Err("heart-rate BPM is outside the accepted range".to_string());
            }
        }
        (WearableSensorType::HeartRateBpm {}, _) => {
            return Err("heart-rate BPM value must be an integer".to_string());
        }
        _ => {}
    }
    Ok(())
}

pub fn status_from_event(
    event: WearableSensorEvent,
    device_config: Option<&WearableDeviceConfigRecord>,
    stale_timeout_seconds: u32,
    now_ms: i64,
) -> Result<WearableStatusRecord, String> {
    validate_event(&event)?;
    let stale_after_ms = i64::from(stale_timeout_seconds.max(1)) * 1_000;
    let status = if connection_is_offline(event.connection_state.as_deref()) {
        WearableStatusKind::Offline {}
    } else if now_ms.saturating_sub(event.timestamp_ms) > stale_after_ms {
        WearableStatusKind::Stale {}
    } else {
        WearableStatusKind::Active {}
    };

    Ok(WearableStatusRecord {
        device_id: event.device_id,
        device_name: event
            .device_name
            .or_else(|| device_config.and_then(|config| config.alias.clone())),
        device_model: event.device_model,
        operator_rns_identity: device_config.and_then(|config| config.operator_rns_identity.clone()),
        sensor_type: event.sensor_type,
        value: event.value,
        unit: event.unit,
        confidence: event.confidence,
        connection_state: event.connection_state,
        last_seen_timestamp_ms: event.timestamp_ms,
        stale_after_ms,
        status,
    })
}

fn connection_is_offline(value: Option<&str>) -> bool {
    matches!(
        value.unwrap_or_default().trim(),
        "DISCONNECTED" | "ERROR" | "UNSUPPORTED"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heart_rate_event(value: i64) -> WearableSensorEvent {
        WearableSensorEvent {
            event_type: "wearable.heart_rate".to_string(),
            source: "ble_gatt_standard_hr".to_string(),
            device_id: "ble-hr-test".to_string(),
            device_name: Some("Generic BLE Heart Rate Device".to_string()),
            device_model: Some("generic_ble_heart_rate_device".to_string()),
            timestamp_ms: 1_700_000_000_000,
            sensor_type: WearableSensorType::HeartRateBpm {},
            value: WearableSensorValue::Integer(value),
            unit: Some("bpm".to_string()),
            confidence: 0.95,
            connection_state: Some("SUBSCRIBED".to_string()),
        }
    }

    #[test]
    fn validates_heart_rate_event() {
        assert!(validate_event(&heart_rate_event(82)).is_ok());
    }

    #[test]
    fn rejects_invalid_heart_rate_values() {
        assert!(validate_event(&heart_rate_event(0)).is_err());
        assert!(validate_event(&heart_rate_event(241)).is_err());
    }

    #[test]
    fn stores_rns_operator_mapping_in_status() {
        let config = WearableDeviceConfigRecord {
            device_id: "ble-hr-test".to_string(),
            alias: Some("Field operator watch".to_string()),
            operator_rns_identity: Some("abcd1234".to_string()),
            sensor_type: WearableSensorType::HeartRateBpm {},
        };

        let status = status_from_event(
            heart_rate_event(82),
            Some(&config),
            30,
            1_700_000_000_000,
        )
        .expect("status");

        assert_eq!(status.operator_rns_identity.as_deref(), Some("abcd1234"));
        assert_eq!(status.status, WearableStatusKind::Active {});
    }

    #[test]
    fn marks_stale_after_configured_timeout() {
        let status = status_from_event(
            heart_rate_event(82),
            None,
            30,
            1_700_000_031_000,
        )
        .expect("status");

        assert_eq!(status.status, WearableStatusKind::Stale {});
    }
}
