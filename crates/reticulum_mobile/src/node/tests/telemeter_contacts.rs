#[test]
fn telemetry_contacts_require_saved_lxmf_identity_and_explicit_permission() {
    let store = AppStateStore::new(prepare_storage_dir("telemeter-permissions").to_str()).unwrap();
    let status = build_status_for_tests();
    let peer = build_peer_record(&"a".repeat(32), &"b".repeat(32), true, true, false);
    let saved = saved_telemetry_fixtures(&[peer]);
    let mut settings = sample_app_settings();
    settings.telemetry.enabled = true;
    store.set_app_settings(&settings).unwrap();
    assert!(explicit_telemetry_targets(&store, &status, &saved)
        .unwrap()
        .is_empty());
    settings.telemetry.lxmf_contacts = vec!["b".repeat(32)];
    store.set_app_settings(&settings).unwrap();
    assert_eq!(
        explicit_telemetry_targets(&store, &status, &saved)
            .unwrap()
            .len(),
        1
    );
    assert!(explicit_telemetry_targets(&store, &status, &[])
        .unwrap()
        .is_empty());
    settings.telemetry.enabled = false;
    store.set_app_settings(&settings).unwrap();
    assert!(explicit_telemetry_targets(&store, &status, &saved)
        .unwrap()
        .is_empty());
    settings.telemetry.enabled = true;
    settings.telemetry.columba_contacts = settings.telemetry.lxmf_contacts.clone();
    assert!(store.set_app_settings(&settings).is_err());
    settings.telemetry.columba_contacts.clear();
    settings.telemetry.lxmf_contacts = vec!["invalid".into()];
    assert!(store.set_app_settings(&settings).is_err());
}

#[test]
fn telemetry_contact_revocation_and_profile_changes_cancel_queued_attempts() {
    let store =
        AppStateStore::new(prepare_storage_dir("telemeter-retry-permission").to_str()).unwrap();
    let status = build_status_for_tests();
    let peer = build_peer_record(&"a".repeat(32), &"b".repeat(32), true, true, false);
    store
        .set_saved_peers(&saved_telemetry_fixtures(std::slice::from_ref(&peer)))
        .unwrap();
    let mut settings = sample_app_settings();
    settings.telemetry.enabled = true;
    settings.telemetry.lxmf_contacts.push("b".repeat(32));
    store.set_app_settings(&settings).unwrap();
    let mut fix = build_telemetry();
    fix.updated_at_ms = now_ms();
    let target = MissionReplicationTarget {
        app_destination_hex: "b".repeat(32),
        send_mode: SendMode::Auto {},
    };
    let (_, fields) = build_profiled_telemetry_payload(&fix, &target, false).unwrap();
    let command = parse_mission_sync_metadata(&fields)
        .unwrap()
        .command_id
        .unwrap();
    store
        .update_telemetry_publication(|publication| {
            publication.fix = Some(fix.clone());
            publication.recipients = vec![crate::telemetry_publication::TelemetryRecipient {
                destination_hex: target.app_destination_hex.clone(),
                callsign: fix.callsign.clone(),
                fix_updated_at_ms: fix.updated_at_ms,
                command_id: command.clone(),
                state: PublicationState::QueuedForDelivery,
                message_id_hex: None,
                custody_message_id_hex: None,
                representation: None,
                detail: None,
                updated_at_ms: now_ms(),
                attempt_route: None,
            }];
        })
        .unwrap();
    let config = build_config_fingerprint_for_tests(HubMode::Autonomous {}, None).to_config();
    let allowed = || {
        telemetry_delivery_is_current(
            &store,
            &status,
            &config,
            std::slice::from_ref(&peer),
            None,
            (&target.app_destination_hex, &command),
        )
        .unwrap()
    };
    assert!(allowed());
    settings.telemetry.lxmf_contacts.clear();
    store.set_app_settings(&settings).unwrap();
    assert!(
        !allowed(),
        "removing the grant must cancel queued disclosure"
    );
    settings
        .telemetry
        .columba_contacts
        .push(target.app_destination_hex.clone());
    store.set_app_settings(&settings).unwrap();
    assert!(
        !allowed(),
        "old payload must not survive a unit-profile change"
    );
    settings.telemetry.columba_contacts.clear();
    settings
        .telemetry
        .lxmf_contacts
        .push(target.app_destination_hex.clone());
    store.set_app_settings(&settings).unwrap();
    assert!(allowed());
    store.set_saved_peers(&[]).unwrap();
    assert!(!allowed(), "a live route cannot replace a removed contact");
}

#[test]
fn telemetry_standard_and_columba_profiles_keep_t1_and_separate_delivery_ids() {
    use crate::msgpack_values::{msgpack_get_indexed, msgpack_map_entries};
    let mut position = build_telemetry();
    position.speed = Some(1.25);
    position.alt = Some(-12.5);
    let target = MissionReplicationTarget {
        app_destination_hex: "b".repeat(32),
        send_mode: SendMode::Auto {},
    };
    let mut ids = Vec::new();
    for columba in [false, true] {
        let (_, fields) = build_profiled_telemetry_payload(&position, &target, columba).unwrap();
        let metadata = parse_mission_sync_metadata(&fields).unwrap();
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.telemetry.upsert")
        );
        ids.push(metadata.command_id);
        let value: MsgPackValue = rmp_serde::from_slice(&fields).unwrap();
        let entries = msgpack_map_entries(&value).unwrap();
        let MsgPackValue::Binary(packed) = msgpack_get_indexed(entries, 2).unwrap() else {
            panic!("binary telemetry");
        };
        let checked = r3akt_map_core::telemetry::decode_field(&json!(packed)).unwrap();
        assert_eq!(
            checked.telemetry["location"]["speed"],
            if columba { 1.25 } else { 4.5 }
        );
        assert_eq!(checked.telemetry["location"]["altitude"], -12.5);
        assert!(String::from_utf8_lossy(&fields).contains("T1"));
    }
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn telemetry_client_field_two_only_persists_over_synthetic_lxmf_pair() {
    let _guard = test_lock().lock().await;
    let (relay, sender, receiver) = start_node_pair("standard-client-telemetry").await;
    let source = sender.get_status().lxmf_destination_hex;
    let destination = receiver.get_status().lxmf_destination_hex;
    let cases = [
        (false, "8201cc800297c404faa2b580c4040aba9500c404ffffcfc7c40400000000c404ffffee29c402ffff7f", 127_000, -123.45, 0.0),
        (true, "8201ce6553f1000297c40400000000c40400000000c404fffffb1ec4040000007dc404ffffff6ac40201f4ce6553f100", 1_700_000_000_000, -12.5, 1.25),
    ];
    for (columba, bytes, timestamp, altitude, speed) in cases {
        let mut settings = sample_app_settings();
        settings.telemetry.enabled = false;
        if columba {
            settings.telemetry.columba_contacts.push(source.clone());
        }
        receiver.set_app_settings(settings).unwrap();
        let fields = rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(2),
            MsgPackValue::Binary(hex::decode(bytes).unwrap()),
        )]))
        .unwrap();
        sender
            .send_bytes(
                destination.clone(),
                Vec::new(),
                Some(fields),
                SendMode::Auto {},
            )
            .unwrap();
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            let records = receiver.get_telemetry_positions().unwrap();
            if let Some(record) = records
                .iter()
                .find(|record| record.updated_at_ms == timestamp)
            {
                assert_eq!(record.callsign, source);
                assert_eq!(record.alt, Some(altitude));
                assert_eq!(record.speed, Some(speed));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "standard field 2 did not reach persisted projection"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    stop_node(sender).await;
    stop_node(receiver).await;
    relay.shutdown().await;
}
