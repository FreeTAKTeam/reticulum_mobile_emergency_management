#[test]
fn sos_beacon_events_use_mecp_b01_and_b03() {
    let identity = "e6fcf8f02e290ed46f88a729460dfffc";
    let incident = "sos-e6fcf8f0-1792422590074";
    let active = sos_beacon_event_for(identity, Some("Pixelcorvo".to_string()), incident, SosBeaconEvent::Activated, 1_792_422_590_074)
        .expect("activation event");
    assert_eq!(active.content, "MECP/0/B01 #SOS_e6fcf8f0_1792422590074");
    assert_eq!(active.uid, format!("rem-sos-b01-v1:{identity}:{incident}"));
    assert_eq!(active.callsign, "Pixelcorvo");
    assert_eq!(active.correlation_id.as_deref(), Some(incident));
    assert!(active.keywords.contains(&"B01".to_string()));
    assert_eq!(active.command_type, "mission.registry.log_entry.upsert");

    let cancelled = sos_beacon_event_for(identity, None, incident, SosBeaconEvent::Cancelled, 1_792_422_600_000)
        .expect("cancel event");
    assert_eq!(cancelled.content, "MECP/2/B03 #SOS_e6fcf8f0_1792422590074");
    assert_ne!(cancelled.uid, active.uid);
}

#[test]
fn sos_beacon_events_reject_bad_identity_or_incident() {
    assert!(sos_beacon_event_for("not-hex", None, "sos-a-1", SosBeaconEvent::Activated, 1).is_err());
    assert!(sos_beacon_event_for("e6fcf8f02e290ed46f88a729460dfffc", None, "  ", SosBeaconEvent::Cancelled, 1).is_err());
}
