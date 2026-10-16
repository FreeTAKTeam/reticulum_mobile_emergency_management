#[test]
fn sos_preserves_saved_contacts_and_omits_positions_for_saved_only_hub_alias() {
    let hub_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let hub_lxmf = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let contact = "cccccccccccccccccccccccccccccccc";
    let dir = prepare_storage_dir("sos-saved-hub-alias");
    let store = AppStateStore::new(dir.to_str()).unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let telemetry = SosDeviceTelemetryRecord {
        lat: Some(45.5),
        lon: Some(-63.25),
        alt: None,
        speed: None,
        course: None,
        accuracy: Some(4.0),
        battery_percent: None,
        battery_charging: None,
        updated_at_ms: now_ms(),
    };
    run_sos_fanout(
        store,
        EventBus::new(),
        tx,
        build_status_for_tests(),
        default_sos_settings(),
        vec![
            build_saved_peer_with_lxmf_route(hub_lxmf, hub_identity),
            build_saved_peer_with_lxmf_route(contact, contact),
        ],
        vec![],
        Some("dddddddddddddddddddddddddddddddd".to_string()),
        Some(build_config_fingerprint_for_tests(
            HubMode::Connected {},
            Some(hub_identity),
        )),
        None,
        Some(telemetry),
        "sos-hub-alias".to_string(),
        SosTriggerSource::Manual {},
        SosMessageKind::Active {},
    )
    .unwrap();
    let mut recipients = HashSet::new();
    for _ in 0..2 {
        let Command::SendBytes {
            destination_hex,
            fields_bytes,
            send_mode,
            resp,
            ..
        } = rx.try_recv().unwrap()
        else {
            panic!("expected SOS send");
        };
        assert!(matches!(send_mode, SendMode::PropagationOnly {}));
        let fields: MsgPackValue = rmp_serde::from_slice(&fields_bytes.unwrap()).unwrap();
        let MsgPackValue::Map(fields) = fields else {
            panic!("fields map");
        };
        let has_position = fields
            .iter()
            .any(|(key, _)| key.as_i64() == Some(crate::sos_fields::LXMF_FIELD_TELEMETRY));
        assert_eq!(has_position, destination_hex == contact);
        recipients.insert(destination_hex);
        resp.send(Ok(())).unwrap();
    }
    assert_eq!(
        recipients,
        HashSet::from([hub_lxmf.to_string(), contact.to_string()])
    );
    assert!(rx.try_recv().is_err());
}
