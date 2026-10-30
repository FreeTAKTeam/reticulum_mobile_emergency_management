#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TelemetryDecodeError {
    CorruptMessagePack,
    UnsupportedEncoding,
    UnsupportedVersion,
    MissingCommandType,
    MissingArgs,
    MissingCallsign,
    MissingLatitude,
    MissingLongitude,
    InvalidCallsign,
    InvalidNumericType,
    NonFiniteValue,
    InvalidCoordinates,
    PersistenceFailure,
    InvalidTelemeter,
    MissingSource,
}

impl TelemetryDecodeError {
    fn code(self) -> String {
        format!("{self:?}")
    }
}

fn decode_telemetry_position(
    fields_bytes: Option<&[u8]>,
    telemetry_body: bool,
    received_at_ms: u64,
) -> Result<Option<TelemetryPositionRecord>, TelemetryDecodeError> {
    use TelemetryDecodeError as Error;
    let Some(bytes) = fields_bytes else {
        return Ok(None);
    };
    if is_marker_telemetry(bytes) {
        return Ok(None);
    }
    let fields = match rmp_serde::from_slice::<MsgPackValue>(bytes) {
        Ok(fields) => fields,
        Err(_) => {
            return if telemetry_body {
                Err(Error::CorruptMessagePack)
            } else {
                Ok(None)
            };
        }
    };
    let Some(entries) = msgpack_map_entries(&fields) else {
        return if telemetry_body {
            Err(Error::UnsupportedEncoding)
        } else {
            Ok(None)
        };
    };
    let Some(commands) = msgpack_get_indexed(entries, FIELD_COMMANDS) else {
        return Ok(None);
    };
    let MsgPackValue::Array(commands) = commands else {
        return if telemetry_body {
            Err(Error::UnsupportedEncoding)
        } else {
            Ok(None)
        };
    };
    for command in commands {
        let Some(command) = msgpack_map_entries(command) else {
            if telemetry_body {
                return Err(Error::UnsupportedEncoding);
            }
            continue;
        };
        let Some(command_type) =
            msgpack_get_named(command, &["command_type", "t"]).and_then(msgpack_string)
        else {
            if telemetry_body {
                return Err(Error::MissingCommandType);
            }
            continue;
        };
        if canonical_command_type(&command_type) != "mission.registry.telemetry.upsert" {
            continue;
        }
        if msgpack_get_named(command, &["version", "v"])
            .is_some_and(|value| value.as_u64() != Some(1))
        {
            return Err(Error::UnsupportedVersion);
        }
        let args = msgpack_get_named(command, &["args", "a"])
            .and_then(msgpack_map_entries)
            .ok_or(Error::MissingArgs)?;
        let callsign = msgpack_get_named(args, &["callsign", "cs"])
            .ok_or(Error::MissingCallsign)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(Error::InvalidCallsign)?;
        let number = |keys: &[&str], missing| -> Result<f64, Error> {
            let value = msgpack_get_named(args, keys).ok_or(missing)?;
            let value = msgpack_f64(value).ok_or(Error::InvalidNumericType)?;
            if !value.is_finite() {
                return Err(Error::NonFiniteValue);
            }
            Ok(value)
        };
        let lat = number(&["lat", "la"], Error::MissingLatitude)?;
        let lon = number(&["lon", "lo"], Error::MissingLongitude)?;
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(Error::InvalidCoordinates);
        }
        let optional = |keys: &[&str]| -> Result<Option<f64>, Error> {
            if msgpack_get_named(args, keys).is_none() {
                return Ok(None);
            }
            number(keys, Error::InvalidNumericType).map(Some)
        };
        return Ok(Some(TelemetryPositionRecord {
            callsign: callsign.to_string(),
            lat,
            lon,
            alt: optional(&["alt", "al"])?,
            course: optional(&["course", "cr"])?,
            speed: optional(&["speed", "sp"])?,
            accuracy: optional(&["accuracy", "ac"])?,
            updated_at_ms: match msgpack_get_named(args, &["updated_at_ms", "updatedAt", "u"]) {
                Some(value) => msgpack_u64(value).ok_or(Error::InvalidNumericType)?,
                None => received_at_ms,
            },
        }));
    }
    if telemetry_body && commands.is_empty() {
        return Err(Error::MissingCommandType);
    }
    Ok(None)
}

struct TelemetryReceiveContext<'a> {
    metadata: Option<&'a MissionSyncMetadata>,
    fields_bytes: Option<&'a [u8]>,
    body: &'a [u8],
    source_hex: Option<&'a str>,
    destination_hex: &'a str,
    message_id_hex: Option<&'a str>,
}

async fn persist_received_telemetry_if_present(
    state: &NodeRuntimeState,
    bus: &EventBus,
    context: TelemetryReceiveContext<'_>,
) -> bool {
    let TelemetryReceiveContext {
        metadata,
        fields_bytes,
        body,
        source_hex,
        destination_hex,
        message_id_hex,
    } = context;
    let decoded = state
        .app_state
        .get_app_settings()
        .map_err(|_| TelemetryDecodeError::PersistenceFailure)
        .and_then(|settings| {
            decode_received_position(
                fields_bytes,
                body == b"T",
                now_ms(),
                source_hex,
                settings.as_ref().is_some_and(|settings| {
                    source_hex.is_some_and(|source| {
                        settings
                            .telemetry
                            .columba_contacts
                            .iter()
                            .any(|contact| contact == source)
                    })
                }),
            )
        });
    let failure = match decoded {
        Ok(None) => return false,
        Ok(Some(record)) => match state.app_state.record_local_telemetry_fix(&record) {
            Ok(invalidation) => {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                if let Ok(invalidation) = state.app_state.bump_projection_revision(
                    ProjectionScope::OperationalSummary {},
                    None,
                    Some("telemetry-received".to_string()),
                ) {
                    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                }
                return true;
            }
            Err(_) => TelemetryDecodeError::PersistenceFailure,
        },
        Err(error) => error,
    };
    let reason = failure.code();
    let diagnostic = crate::telemetry_publication::TelemetryReceiveFailure {
        source_hex: source_hex.map(str::to_string),
        destination_hex: destination_hex.to_string(),
        message_id_hex: message_id_hex.map(str::to_string),
        command_id: metadata.and_then(|metadata| metadata.command_id.clone()),
        reason: reason.clone(),
        received_at_ms: now_ms(),
    };
    if let Ok(Some(invalidation)) = state.app_state.update_telemetry_publication(|publication| {
        publication.last_receive_failure = Some(diagnostic)
    }) {
        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    }
    bus.emit(NodeEvent::Error {
        code: format!("TelemetryReceive{reason}"),
        message: format!(
            "telemetry rejected source={} destination={} message={} command={} reason={reason}",
            source_hex.unwrap_or("-"),
            destination_hex,
            message_id_hex.unwrap_or("-"),
            metadata
                .and_then(|metadata| metadata.command_id.as_deref())
                .unwrap_or("-")
        ),
    });
    send_operational_result_if_needed(state, bus, source_hex, metadata, Some(reason.as_str()))
        .await;
    false
}

#[cfg(test)]
mod telemetry_decode_tests {
    use super::*;

    fn fields(args: Vec<(&str, MsgPackValue)>, version: Option<u64>) -> Vec<u8> {
        let mut command = vec![
            (
                MsgPackValue::from("i"),
                MsgPackValue::from("telemetry-test"),
            ),
            (
                MsgPackValue::from("t"),
                MsgPackValue::from("mission.registry.telemetry.upsert"),
            ),
            (
                MsgPackValue::from("a"),
                MsgPackValue::Map(
                    args.into_iter()
                        .map(|(key, value)| (MsgPackValue::from(key), value))
                        .collect(),
                ),
            ),
        ];
        if let Some(version) = version {
            command.push((MsgPackValue::from("v"), MsgPackValue::from(version)));
        }
        rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(command)]),
        )]))
        .unwrap()
    }

    #[test]
    fn telemetry_decode_rejections_are_typed_and_position_free() {
        use TelemetryDecodeError as Error;
        let valid = vec![
            ("cs", MsgPackValue::from("Alpha")),
            ("la", MsgPackValue::from(44.1)),
            ("lo", MsgPackValue::from(-63.1)),
        ];
        let bytes = fields(valid.clone(), None);
        let record = decode_telemetry_position(Some(&bytes), true, 123)
            .unwrap()
            .unwrap();
        assert_eq!(record.callsign, "Alpha");
        assert_eq!(record.updated_at_ms, 123);
        let legacy = rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("mission.registry.telemetry.upsert"),
                ),
                (
                    MsgPackValue::from("args"),
                    MsgPackValue::Map(vec![
                        (MsgPackValue::from("callsign"), MsgPackValue::from("Alpha")),
                        (MsgPackValue::from("lat"), MsgPackValue::from(44.1)),
                        (MsgPackValue::from("lon"), MsgPackValue::from(-63.1)),
                    ]),
                ),
            ])]),
        )]))
        .unwrap();
        assert_eq!(
            decode_telemetry_position(Some(&legacy), true, 123).unwrap(),
            Some(record)
        );
        for (key, reason) in [
            ("cs", Error::MissingCallsign),
            ("la", Error::MissingLatitude),
            ("lo", Error::MissingLongitude),
        ] {
            let args = valid
                .iter()
                .filter(|(name, _)| *name != key)
                .cloned()
                .collect();
            assert_eq!(
                decode_telemetry_position(Some(&fields(args, None)), true, 123).unwrap_err(),
                reason
            );
        }
        for (value, reason) in [
            (
                MsgPackValue::from("secret-position"),
                Error::InvalidNumericType,
            ),
            (MsgPackValue::F64(f64::NAN), Error::NonFiniteValue),
            (MsgPackValue::F64(f64::INFINITY), Error::NonFiniteValue),
            (MsgPackValue::from(91.0), Error::InvalidCoordinates),
        ] {
            let mut args = valid.clone();
            args[1].1 = value;
            let error =
                decode_telemetry_position(Some(&fields(args, None)), true, 123).unwrap_err();
            assert_eq!(error, reason);
            assert!(!error.code().contains("secret-position"));
        }
        assert_eq!(
            decode_telemetry_position(Some(&fields(valid.clone(), Some(2))), true, 123)
                .unwrap_err(),
            Error::UnsupportedVersion
        );
        assert_eq!(
            decode_telemetry_position(Some(&[0xc1]), true, 123).unwrap_err(),
            Error::CorruptMessagePack
        );
        assert!(decode_telemetry_position(Some(&[0xc1]), false, 123)
            .unwrap()
            .is_none());
        assert!(decode_telemetry_position(None, false, 123)
            .unwrap()
            .is_none());
        assert!(decode_telemetry_position(None, true, 123)
            .unwrap()
            .is_none());
        for callsign in [
            MsgPackValue::from(""),
            MsgPackValue::from("  "),
            MsgPackValue::from(42),
        ] {
            let mut args = valid.clone();
            args[0].1 = callsign;
            assert_eq!(
                decode_telemetry_position(Some(&fields(args, None)), true, 123).unwrap_err(),
                Error::InvalidCallsign
            );
        }
        let encode_commands = |commands| {
            rmp_serde::to_vec(&MsgPackValue::Map(vec![(
                MsgPackValue::from(FIELD_COMMANDS),
                commands,
            )]))
            .unwrap()
        };
        let telemetry_type = (
            MsgPackValue::from("t"),
            MsgPackValue::from("mission.registry.telemetry.upsert"),
        );
        for (commands, reason) in [
            (MsgPackValue::Nil, Error::UnsupportedEncoding),
            (MsgPackValue::Array(vec![]), Error::MissingCommandType),
            (
                MsgPackValue::Array(vec![MsgPackValue::from("wrong-command-shape")]),
                Error::UnsupportedEncoding,
            ),
            (
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![])]),
                Error::MissingCommandType,
            ),
            (
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![telemetry_type.clone()])]),
                Error::MissingArgs,
            ),
            (
                MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                    telemetry_type,
                    (MsgPackValue::from("a"), MsgPackValue::Array(vec![])),
                ])]),
                Error::MissingArgs,
            ),
        ] {
            assert_eq!(
                decode_telemetry_position(Some(&encode_commands(commands)), true, 123).unwrap_err(),
                reason
            );
        }
        let non_map = rmp_serde::to_vec(&MsgPackValue::Array(vec![])).unwrap();
        assert_eq!(
            decode_telemetry_position(Some(&non_map), true, 123).unwrap_err(),
            Error::UnsupportedEncoding
        );
        assert!(decode_telemetry_position(Some(&non_map), false, 123)
            .unwrap()
            .is_none());
        for command_type in ["unknown.command", "mission.event.upsert"] {
            let unrelated = encode_commands(MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (MsgPackValue::from("t"), MsgPackValue::from(command_type)),
                (
                    MsgPackValue::from("a"),
                    MsgPackValue::Map(
                        valid
                            .iter()
                            .cloned()
                            .map(|(key, value)| (MsgPackValue::from(key), value))
                            .collect(),
                    ),
                ),
            ])]));
            for telemetry_body in [false, true] {
                assert!(
                    decode_telemetry_position(Some(&unrelated), telemetry_body, 123)
                        .unwrap()
                        .is_none()
                );
            }
        }
        for entries in [
            vec![],
            vec![(MsgPackValue::from("GROUP"), MsgPackValue::from("Alpha"))],
        ] {
            let no_commands = rmp_serde::to_vec(&MsgPackValue::Map(entries)).unwrap();
            assert!(decode_telemetry_position(Some(&no_commands), true, 123)
                .unwrap()
                .is_none());
        }
    }
}
