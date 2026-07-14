#[test]
fn sos_field_telemetry_promotes_to_regular_telemetry_position() {
    let telemetry = SosDeviceTelemetryRecord {
        lat: Some(43.967_349),
        lon: Some(-66.126_159),
        alt: Some(12.0),
        speed: Some(1.4),
        course: Some(270.0),
        accuracy: Some(5.5),
        battery_percent: Some(100.0),
        battery_charging: Some(false),
        updated_at_ms: 1_700_000_000_000,
    };

    let position = telemetry_position_from_sos(
        "66C38067874B18B4AF15909FD86D6394",
        Some(&telemetry),
        1_700_000_050_000,
    )
    .expect("sos telemetry should become a map telemetry fix");

    assert_eq!(position.callsign, "66c38067874b18b4af15909fd86d6394");
    assert_eq!(position.lat, 43.967_349);
    assert_eq!(position.lon, -66.126_159);
    assert_eq!(position.alt, Some(12.0));
    assert_eq!(position.speed, Some(1.4));
    assert_eq!(position.course, Some(270.0));
    assert_eq!(position.accuracy, Some(5.5));
    assert_eq!(position.updated_at_ms, 1_700_000_000_000);
}

#[test]
fn sos_telemetry_without_coordinates_does_not_create_map_position() {
    let telemetry = SosDeviceTelemetryRecord {
        lat: None,
        lon: None,
        alt: None,
        speed: None,
        course: None,
        accuracy: None,
        battery_percent: Some(87.0),
        battery_charging: Some(false),
        updated_at_ms: 1_700_000_000_000,
    };

    assert!(telemetry_position_from_sos("peer", Some(&telemetry), 42).is_none());
}

#[test]
fn sos_status_sends_use_dedicated_recovery_lane() {
    let metadata = MissionSyncMetadata {
        command_present: true,
        command_id: Some("sos:incident-1:active:123".to_string()),
        correlation_id: Some("incident-1".to_string()),
        command_type: Some("sos.status".to_string()),
        ..MissionSyncMetadata::default()
    };

    assert_eq!(
        SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::Auto {}),
        SendTaskClass::MissionRecovery
    );
    assert_eq!(
        SendTaskClass::from_lxmf_request(true, Some(&metadata), &SendMode::PropagationOnly {}),
        SendTaskClass::MissionRecovery
    );
}
