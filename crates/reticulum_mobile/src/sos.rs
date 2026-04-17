use sha2::{Digest, Sha256};

use crate::runtime::now_ms;
use crate::sos_fields::{sos_kind_to_str, trigger_source_to_str};
use crate::types::{
    NodeError, SosAlertRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosMessageKind,
    SosSettingsRecord, SosState, SosStatusRecord, SosTriggerSource,
};

const DEFAULT_TEMPLATE: &str = "SOS! I need help. This is an emergency distress signal.";
const CANCEL_BODY: &str = "SOS Cancelled - I am safe.";

pub(crate) fn default_sos_settings() -> SosSettingsRecord {
    SosSettingsRecord {
        enabled: false,
        message_template: DEFAULT_TEMPLATE.to_string(),
        cancel_message_template: CANCEL_BODY.to_string(),
        countdown_seconds: 5,
        include_location: true,
        trigger_shake: false,
        trigger_tap_pattern: false,
        trigger_power_button: false,
        shake_sensitivity: 2.8,
        audio_recording: false,
        audio_duration_seconds: 30,
        periodic_updates: false,
        update_interval_seconds: 120,
        floating_button: false,
        silent_auto_answer: false,
        deactivation_pin_hash: None,
        deactivation_pin_salt: None,
        floating_button_x: 24.0,
        floating_button_y: 420.0,
        active_pill_x: 16.0,
        active_pill_y: 72.0,
    }
}

pub(crate) fn idle_status() -> SosStatusRecord {
    SosStatusRecord {
        state: SosState::Idle {},
        incident_id: None,
        trigger_source: None,
        countdown_deadline_ms: None,
        activated_at_ms: None,
        last_sent_at_ms: None,
        last_update_at_ms: None,
        updated_at_ms: now_ms(),
    }
}

pub(crate) fn normalize_sos_settings(mut settings: SosSettingsRecord) -> SosSettingsRecord {
    let defaults = default_sos_settings();
    if settings.message_template.trim().is_empty() {
        settings.message_template = defaults.message_template;
    }
    if settings.cancel_message_template.trim().is_empty() {
        settings.cancel_message_template = defaults.cancel_message_template;
    }
    settings.countdown_seconds = settings.countdown_seconds.min(60);
    settings.shake_sensitivity = settings.shake_sensitivity.clamp(1.0, 8.0);
    settings.audio_duration_seconds = settings.audio_duration_seconds.clamp(15, 60);
    settings.update_interval_seconds = settings.update_interval_seconds.clamp(30, 3_600);
    if settings
        .deactivation_pin_hash
        .as_deref()
        .is_none_or(str::is_empty)
    {
        settings.deactivation_pin_hash = None;
        settings.deactivation_pin_salt = None;
    }
    settings
}

pub(crate) fn set_pin(settings: &mut SosSettingsRecord, pin: &str) -> Result<(), NodeError> {
    let normalized = pin.trim();
    if normalized.is_empty() {
        settings.deactivation_pin_hash = None;
        settings.deactivation_pin_salt = None;
        return Ok(());
    }
    if normalized.len() < 4 || !normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(NodeError::InvalidConfig {});
    }
    let salt = format!("{:x}{:x}", now_ms(), normalized.len());
    settings.deactivation_pin_hash = Some(hash_pin(normalized, salt.as_str()));
    settings.deactivation_pin_salt = Some(salt);
    Ok(())
}

pub(crate) fn verify_pin(settings: &SosSettingsRecord, pin: Option<&str>) -> bool {
    let Some(stored_hash) = settings.deactivation_pin_hash.as_deref() else {
        return true;
    };
    let Some(salt) = settings.deactivation_pin_salt.as_deref() else {
        return false;
    };
    let Some(pin) = pin.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    hash_pin(pin, salt) == stored_hash
}

pub(crate) fn hash_pin(pin: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) fn new_incident_id(local_identity_hex: &str) -> String {
    let prefix = local_identity_hex
        .trim()
        .chars()
        .take(8)
        .collect::<String>();
    format!(
        "sos-{}-{}",
        if prefix.is_empty() {
            "local"
        } else {
            prefix.as_str()
        },
        now_ms()
    )
}

pub(crate) fn active_status(
    incident_id: String,
    trigger_source: SosTriggerSource,
    sent_at_ms: u64,
) -> SosStatusRecord {
    SosStatusRecord {
        state: SosState::Active {},
        incident_id: Some(incident_id),
        trigger_source: Some(trigger_source),
        countdown_deadline_ms: None,
        activated_at_ms: Some(sent_at_ms),
        last_sent_at_ms: Some(sent_at_ms),
        last_update_at_ms: Some(sent_at_ms),
        updated_at_ms: sent_at_ms,
    }
}

pub(crate) fn countdown_status(
    incident_id: String,
    trigger_source: SosTriggerSource,
    deadline_ms: u64,
) -> SosStatusRecord {
    SosStatusRecord {
        state: SosState::Countdown {},
        incident_id: Some(incident_id),
        trigger_source: Some(trigger_source),
        countdown_deadline_ms: Some(deadline_ms),
        activated_at_ms: None,
        last_sent_at_ms: None,
        last_update_at_ms: None,
        updated_at_ms: now_ms(),
    }
}

pub(crate) fn compose_sos_body(
    settings: &SosSettingsRecord,
    kind: SosMessageKind,
    telemetry: Option<&SosDeviceTelemetryRecord>,
) -> String {
    if matches!(kind, SosMessageKind::Cancelled {}) {
        let body = settings.cancel_message_template.trim();
        return if body.is_empty() {
            CANCEL_BODY.to_string()
        } else {
            body.to_string()
        };
    }
    let mut body = settings.message_template.trim().to_string();
    if body.is_empty() {
        body = DEFAULT_TEMPLATE.to_string();
    }
    if settings.include_location {
        if let Some(telemetry) = telemetry {
            if let (Some(lat), Some(lon)) = (telemetry.lat, telemetry.lon) {
                body.push_str(format!("\nGPS: {lat:.6}, {lon:.6}").as_str());
            }
            if let Some(battery) = telemetry.battery_percent {
                body.push_str(format!("\nBattery: {battery:.0}%").as_str());
            }
        }
    }
    body
}

pub(crate) fn received_alert_from_sos(
    incident_id: String,
    source_hex: String,
    conversation_id: String,
    state: SosMessageKind,
    body_utf8: String,
    telemetry: Option<&SosDeviceTelemetryRecord>,
    audio_id: Option<String>,
    message_id_hex: Option<String>,
    received_at_ms: u64,
) -> SosAlertRecord {
    SosAlertRecord {
        incident_id,
        source_hex,
        conversation_id,
        active: !matches!(state, SosMessageKind::Cancelled {}),
        state,
        body_utf8,
        lat: telemetry.and_then(|value| value.lat),
        lon: telemetry.and_then(|value| value.lon),
        battery_percent: telemetry.and_then(|value| value.battery_percent),
        audio_id,
        message_id_hex,
        received_at_ms,
        updated_at_ms: received_at_ms,
    }
}

pub(crate) fn location_from_alert(alert: &SosAlertRecord) -> Option<SosLocationRecord> {
    Some(SosLocationRecord {
        incident_id: alert.incident_id.clone(),
        source_hex: alert.source_hex.clone(),
        lat: alert.lat?,
        lon: alert.lon?,
        alt: None,
        accuracy: None,
        battery_percent: alert.battery_percent,
        recorded_at_ms: alert.received_at_ms,
    })
}

pub(crate) fn sos_status_label(state: SosState) -> &'static str {
    match state {
        SosState::Idle {} => "Idle",
        SosState::Countdown {} => "Countdown",
        SosState::Sending {} => "Sending",
        SosState::Active {} => "Active",
    }
}

pub(crate) fn sos_trigger_label(source: SosTriggerSource) -> &'static str {
    trigger_source_to_str(source)
}

pub(crate) fn sos_kind_label(kind: SosMessageKind) -> &'static str {
    sos_kind_to_str(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_hash_is_salted_and_verifiable() {
        let mut settings = default_sos_settings();
        set_pin(&mut settings, "1234").expect("set pin");
        assert_ne!(settings.deactivation_pin_hash.as_deref(), Some("1234"));
        assert!(verify_pin(&settings, Some("1234")));
        assert!(!verify_pin(&settings, Some("9999")));
    }

    #[test]
    fn cancel_body_is_legacy_detectable() {
        let body = compose_sos_body(&default_sos_settings(), SosMessageKind::Cancelled {}, None);
        assert!(body.starts_with("SOS Cancelled"));
    }

    #[test]
    fn cancel_body_uses_configured_emergency_end_template() {
        let mut settings = default_sos_settings();
        settings.cancel_message_template = "SOS ended. I am safe at base.".to_string();
        let body = compose_sos_body(&settings, SosMessageKind::Cancelled {}, None);
        assert_eq!(body, "SOS ended. I am safe at base.");
    }

    #[test]
    fn normalize_backfills_blank_cancel_template() {
        let mut settings = default_sos_settings();
        settings.cancel_message_template.clear();
        let normalized = normalize_sos_settings(settings);
        assert_eq!(normalized.cancel_message_template, CANCEL_BODY);
    }

    #[test]
    fn old_persisted_sos_settings_without_cancel_template_are_supported() {
        let raw = r#"{
            "enabled": false,
            "message_template": "SOS",
            "countdown_seconds": 5,
            "include_location": true,
            "trigger_shake": false,
            "trigger_tap_pattern": false,
            "trigger_power_button": false,
            "shake_sensitivity": 2.8,
            "audio_recording": false,
            "audio_duration_seconds": 30,
            "periodic_updates": false,
            "update_interval_seconds": 120,
            "floating_button": false,
            "silent_auto_answer": false,
            "deactivation_pin_hash": null,
            "deactivation_pin_salt": null,
            "floating_button_x": 24.0,
            "floating_button_y": 420.0,
            "active_pill_x": 16.0,
            "active_pill_y": 72.0
        }"#;
        let settings: SosSettingsRecord = serde_json::from_str(raw).expect("legacy settings");
        assert!(settings.cancel_message_template.is_empty());
        let normalized = normalize_sos_settings(settings);
        assert_eq!(normalized.cancel_message_template, CANCEL_BODY);
    }
}
