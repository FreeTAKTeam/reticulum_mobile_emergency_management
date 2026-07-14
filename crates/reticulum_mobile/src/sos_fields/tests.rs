    use super::*;
    use crate::mission_sync::parse_mission_sync_metadata;

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
        let field_text = String::from_utf8_lossy(encoded.as_slice());
        for verbose in [
            "command_id",
            "correlation_id",
            "command_type",
            "sos.status",
            "sos_state",
            "incident_id",
            "trigger_source",
            "sent_at_ms",
            "audio_id",
        ] {
            assert!(
                !field_text.contains(verbose),
                "compact SOS fields should not contain verbose token {verbose}"
            );
        }
        let parsed = parse_sos_fields(&encoded).expect("parsed fields");
        let fields = rmp_serde::from_slice::<MsgPackValue>(&encoded).expect("field map");
        let entries = msgpack_map_entries(&fields).expect("map entries");

        assert!(msgpack_get_indexed(entries, FIELD_COMMANDS).is_some());
        let metadata = parse_mission_sync_metadata(&encoded).expect("mission metadata");
        assert_eq!(metadata.command_type.as_deref(), Some("sos.status"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("incident-1"));
        assert!(metadata
            .command_id
            .as_deref()
            .is_some_and(|value| { value.starts_with("sos:incident-1:active:") }));
        assert_eq!(parsed.command.expect("command"), command);
        let parsed_telemetry = parsed.telemetry.expect("telemetry");
        assert_eq!(parsed_telemetry.lat.expect("lat").round(), 46.0);
        assert_eq!(parsed_telemetry.battery_charging, Some(true));
    }

    #[test]
    fn sos_fields_encode_regular_lxmf_telemetry_part() {
        let command = SosCommand {
            state: SosMessageKind::Active {},
            incident_id: "incident-map".to_string(),
            trigger_source: SosTriggerSource::FloatingButton {},
            sent_at_ms: 42,
            audio_id: None,
        };
        let telemetry = SosDeviceTelemetryRecord {
            lat: Some(43.967_349),
            lon: Some(-66.126_159),
            alt: Some(25.0),
            speed: Some(0.5),
            course: Some(90.0),
            accuracy: Some(4.5),
            battery_percent: Some(76.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_000_000,
        };

        let encoded = build_sos_fields(&command, Some(&telemetry)).expect("encoded fields");
        let fields = rmp_serde::from_slice::<MsgPackValue>(&encoded).expect("field map");
        let entries = msgpack_map_entries(&fields).expect("map entries");
        let field = msgpack_get_indexed(entries, LXMF_FIELD_TELEMETRY)
            .expect("regular FIELD_TELEMETRY entry");
        let payload = match field {
            MsgPackValue::Binary(bytes) => {
                rmp_serde::from_slice::<MsgPackValue>(bytes).expect("telemeter payload")
            }
            other => panic!("FIELD_TELEMETRY must be raw binary MessagePack, got {other:?}"),
        };
        let telemetry_entries = msgpack_map_entries(&payload).expect("telemeter map");

        assert!(msgpack_get_indexed(telemetry_entries, SID_TIME).is_some());
        assert!(msgpack_get_indexed(telemetry_entries, SID_LOCATION).is_some());
        assert!(msgpack_get_indexed(telemetry_entries, SID_BATTERY).is_some());
    }

    #[test]
    fn text_detection_accepts_legacy_prefixes() {
        assert!(matches!(
            sos_kind_from_text("SOS! I need help"),
            Some(SosMessageKind::Active {})
        ));
        assert!(matches!(
            sos_kind_from_text("urgence besoin aide"),
            Some(SosMessageKind::Active {})
        ));
        assert!(matches!(
            sos_kind_from_text("Emergency at 45.1,-63.2"),
            Some(SosMessageKind::Active {})
        ));
        assert!(sos_kind_from_text("normal chat").is_none());
    }

    #[test]
    fn text_detection_classifies_legacy_cancel_messages() {
        assert!(matches!(
            sos_kind_from_text("SOS Cancelled - I am safe."),
            Some(SosMessageKind::Cancelled {})
        ));
        assert!(matches!(
            sos_kind_from_text("SOS ended. I am safe at base."),
            Some(SosMessageKind::Cancelled {})
        ));
        assert!(matches!(
            sos_kind_from_text("SOS! I need help"),
            Some(SosMessageKind::Active {})
        ));
    }

    #[test]
    fn sos_fields_keep_battery_when_location_is_missing() {
        let command = SosCommand {
            state: SosMessageKind::Active {},
            incident_id: "incident-battery".to_string(),
            trigger_source: SosTriggerSource::Manual {},
            sent_at_ms: 100,
            audio_id: None,
        };
        let telemetry = SosDeviceTelemetryRecord {
            lat: None,
            lon: None,
            alt: None,
            speed: None,
            course: None,
            accuracy: None,
            battery_percent: Some(52.0),
            battery_charging: Some(false),
            updated_at_ms: 1_700_000_000_000,
        };

        let encoded = build_sos_fields(&command, Some(&telemetry)).expect("encoded fields");
        let parsed = parse_sos_fields(&encoded).expect("parsed fields");
        let parsed_telemetry = parsed.telemetry.expect("battery-only telemetry");

        assert_eq!(parsed_telemetry.lat, None);
        assert_eq!(parsed_telemetry.lon, None);
        assert_eq!(parsed_telemetry.battery_percent, Some(52.0));
        assert_eq!(parsed_telemetry.battery_charging, Some(false));
    }

    #[test]
    fn parse_sos_fields_ignores_rch_command_envelope() {
        let fields = MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from("cmd-123"),
                ),
                (
                    MsgPackValue::from("correlation_id"),
                    MsgPackValue::from("corr-123"),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.log_entry.upsert"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![(
                        MsgPackValue::from("mission_uid"),
                        MsgPackValue::from("mission-1"),
                    )]),
                ),
            ])]),
        )]);
        let bytes = rmp_serde::to_vec(&fields).expect("msgpack");

        let parsed = parse_sos_fields(&bytes);

        assert!(parsed.is_none());
    }
