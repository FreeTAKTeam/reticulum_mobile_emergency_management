fn decode_received_position(
    fields_bytes: Option<&[u8]>,
    telemetry_body: bool,
    received_at_ms: u64,
    source: Option<&str>,
    columba_mps: bool,
) -> Result<Option<TelemetryPositionRecord>, TelemetryDecodeError> {
    let Some(bytes) = fields_bytes else {
        return Ok(None);
    };
    if is_marker_telemetry(bytes) {
        return Ok(None);
    }
    // Validate the raw field map before traversing attacker-controlled values.
    if bytes.len() > r3akt_map_core::telemetry::MAX_SENSOR_BYTES
        || r3akt_map_core::msgpack::to_json(bytes).is_none()
    {
        return Err(TelemetryDecodeError::CorruptMessagePack);
    }
    let value: MsgPackValue =
        rmp_serde::from_slice(bytes).map_err(|_| TelemetryDecodeError::CorruptMessagePack)?;
    let standard =
        msgpack_map_entries(&value).and_then(|entries| msgpack_get_indexed(entries, 0x02));
    if let Some(standard) = standard {
        let MsgPackValue::Binary(packed) = standard else {
            return Err(TelemetryDecodeError::InvalidTelemeter);
        };
        let fix = crate::telemeter::decode(packed, columba_mps)
            .map_err(|_| TelemetryDecodeError::InvalidTelemeter)?;
        let (Some(lat), Some(lon)) = (fix.lat, fix.lon) else {
            return Ok(None);
        };
        // T1 supplies the existing REM display key only; field 2 owns the readings.
        let callsign = decode_telemetry_position(fields_bytes, false, received_at_ms)
            .ok()
            .flatten()
            .map(|position| position.callsign)
            .or_else(|| source.map(|source| source.to_string()))
            .ok_or(TelemetryDecodeError::MissingSource)?;
        return Ok(Some(TelemetryPositionRecord {
            callsign,
            lat,
            lon,
            alt: fix.alt,
            speed: fix.speed,
            course: fix.course,
            accuracy: fix.accuracy,
            updated_at_ms: fix.updated_at_ms,
        }));
    }
    decode_telemetry_position(fields_bytes, telemetry_body, received_at_ms)
}

#[cfg(test)]
mod standard_telemetry_tests {
    use super::*;

    fn field(hex: &str) -> Vec<u8> {
        rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(2),
            MsgPackValue::Binary(hex::decode(hex).unwrap()),
        )]))
        .unwrap()
    }

    #[test]
    fn sideband_pyxis_and_columba_source_bytes_reach_rem_with_source_age_and_units() {
        // Sideband/Pyxis signed-bounds fixture: sensor time=128, fix time=127.
        let fields = field(
            "8201cc800297c404faa2b580c4040aba9500c404ffffcfc7c40400000000c404ffffee29c402ffff7f",
        );
        let position =
            decode_received_position(Some(&fields), false, 999_000, Some("synthetic-peer"), false)
                .unwrap()
                .unwrap();
        assert_eq!(
            (position.lat, position.lon, position.alt),
            (-90.0, 180.0, Some(-123.45))
        );
        assert_eq!(position.updated_at_ms, 127_000);
        assert_eq!(position.callsign, "synthetic-peer");
        // Columba 2.2.6 unmodified JVM codec fixture.
        let fields = field("8201ce6553f1000297c40400000000c40400000000c404fffffb1ec4040000007dc404ffffff6ac40201f4ce6553f100");
        let position = decode_received_position(
            Some(&fields),
            false,
            1_700_000_500_000,
            Some("columba-peer"),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(position.speed, Some(1.25));
        assert_eq!(position.alt, Some(-12.5));
        assert_eq!(position.updated_at_ms, 1_700_000_000_000);
        let standard =
            decode_received_position(Some(&fields), false, 1, Some("sideband-peer"), false)
                .unwrap()
                .unwrap();
        assert_eq!(standard.speed, Some(1.25 / 3.6));
    }

    #[test]
    fn numeric_collector_request_and_t1_are_distinct() {
        let fields = rmp_serde::to_vec(&MsgPackValue::Map(vec![(
            MsgPackValue::from(9),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![(
                MsgPackValue::from(1),
                MsgPackValue::Array(vec![MsgPackValue::from(0), MsgPackValue::Boolean(true)]),
            )])]),
        )]))
        .unwrap();
        assert!(
            decode_received_position(Some(&fields), false, 1000, Some("peer"), false)
                .unwrap()
                .is_none()
        );
        assert!(
            decode_received_position(Some(&field("c1")), false, 1000, Some("peer"), false).is_err()
        );
    }
}
