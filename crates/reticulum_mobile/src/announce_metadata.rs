pub(crate) use r3akt_mesh_delivery::{
    has_capability_token, normalize_rem_display_name, parse_announce_metadata,
    supports_mission_traffic,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Value as MsgPackValue;

    #[test]
    fn announce_metadata_accepts_text_and_msgpack_layouts() {
        let text = parse_announce_metadata("R3AKT;EMergencyMessages;name=Legacy+Team");
        assert_eq!(text.display_name.as_deref(), Some("Legacy Team"));
        assert!(text.capability_tokens.iter().any(|token| token == "r3akt"));
        assert!(text
            .capability_tokens
            .iter()
            .any(|token| token == "emergencymessages"));
        assert!(supports_mission_traffic(Some(
            "R3AKT;EMergencyMessages;name=Legacy+Team"
        )));

        let payload = MsgPackValue::Array(vec![
            MsgPackValue::from("Msgpack Team"),
            MsgPackValue::Map(vec![(
                MsgPackValue::from("caps"),
                MsgPackValue::Array(vec![
                    MsgPackValue::from("R3AKT"),
                    MsgPackValue::from("EMergencyMessages"),
                    MsgPackValue::from("Telemetry"),
                ]),
            )]),
        ]);
        let encoded = rmp_serde::to_vec(&payload).expect("msgpack");
        let msgpack_hex = hex::encode(encoded);
        let msgpack = parse_announce_metadata(msgpack_hex.as_str());

        assert_eq!(msgpack.display_name.as_deref(), Some("Msgpack Team"));
        assert!(msgpack
            .capability_tokens
            .iter()
            .any(|token| token == "r3akt"));
        assert!(msgpack
            .capability_tokens
            .iter()
            .any(|token| token == "emergencymessages"));
        assert!(has_capability_token(
            Some(msgpack_hex.as_str()),
            "telemetry"
        ));
        assert!(supports_mission_traffic(Some(msgpack_hex.as_str())));
    }

    #[test]
    fn malformed_hex_does_not_become_capability_text() {
        let metadata = parse_announce_metadata("fffe00");
        assert!(metadata.display_name.is_none());
        assert!(metadata.capability_tokens.is_empty());
        assert!(!supports_mission_traffic(Some("fffe00")));
    }
}
