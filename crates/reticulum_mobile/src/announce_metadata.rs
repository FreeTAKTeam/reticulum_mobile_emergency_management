use lxmf::announce::{
    capabilities_from_delivery_app_data, encode_delivery_announce_app_data_with_capabilities,
    AnnounceEncodeError,
};
use rmpv::Value as MsgPackValue;

pub(crate) const STANDARD_LXMF_RECEIPTS_CAPABILITY: &str = "rem.standard_lxmf_receipts.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnnounceWireFormat {
    LegacyText,
    StructuredLxmf,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnounceMetadata {
    pub(crate) display_name: Option<String>,
    pub(crate) capability_tokens: Vec<String>,
    pub(crate) wire_format: AnnounceWireFormat,
    pub(crate) has_legacy_name_token: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnounceProfile {
    display_name: String,
    capability_tokens: Vec<String>,
}

impl AnnounceProfile {
    pub(crate) fn new(display_name: &str, capability_text: &str) -> Self {
        let display_name =
            normalize_rem_display_name(display_name).unwrap_or_else(|| "REM".to_string());
        let mut profile = Self {
            display_name,
            capability_tokens: Vec::new(),
        };
        profile.set_capabilities(capability_text);
        profile
    }

    pub(crate) fn set_capabilities(&mut self, capability_text: &str) {
        self.capability_tokens = parse_announce_metadata(capability_text).capability_tokens;
        if !self
            .capability_tokens
            .iter()
            .any(|token| token == STANDARD_LXMF_RECEIPTS_CAPABILITY)
        {
            self.capability_tokens
                .push(STANDARD_LXMF_RECEIPTS_CAPABILITY.to_string());
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, AnnounceEncodeError> {
        encode_delivery_announce_app_data_with_capabilities(
            self.display_name.as_str(),
            None,
            self.capability_tokens.as_slice(),
        )
    }
}

pub(crate) fn parse_announce_metadata(app_data: &str) -> AnnounceMetadata {
    let has_legacy_name_token = app_data
        .split([',', ';'])
        .map(str::trim)
        .any(|token| token.to_ascii_lowercase().starts_with("name="));
    let display_name = app_data
        .split([',', ';'])
        .map(str::trim)
        .find_map(|token| {
            token
                .get(..5)
                .filter(|prefix| prefix.eq_ignore_ascii_case("name="))
                .map(|_| &token[5..])
        })
        .and_then(decode_percent_component)
        .as_deref()
        .and_then(normalize_rem_display_name);
    let text_tokens = parse_capability_tokens(app_data);

    if let Some(bytes) = decode_hex_announce_app_data(app_data) {
        if let Some(payload) = parse_announce_payload_msgpack(bytes.as_slice()) {
            let msgpack_display_name = extract_msgpack_announce_display_name(&payload);
            let msgpack_tokens =
                capabilities_from_delivery_app_data(bytes.as_slice()).unwrap_or_default();
            if msgpack_display_name.is_some() || !msgpack_tokens.is_empty() {
                return AnnounceMetadata {
                    display_name: msgpack_display_name,
                    capability_tokens: msgpack_tokens,
                    wire_format: AnnounceWireFormat::StructuredLxmf,
                    has_legacy_name_token: false,
                };
            }
        }
        if display_name.is_none() {
            return AnnounceMetadata {
                display_name: None,
                capability_tokens: Vec::new(),
                wire_format: AnnounceWireFormat::Unknown,
                has_legacy_name_token: false,
            };
        }
    }

    AnnounceMetadata {
        display_name,
        capability_tokens: text_tokens,
        wire_format: AnnounceWireFormat::LegacyText,
        has_legacy_name_token,
    }
}

pub(crate) fn normalize_rem_display_name(value: &str) -> Option<String> {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(64).collect())
    }
}

pub(crate) fn has_capability_token(app_data: Option<&str>, capability: &str) -> bool {
    let requested = capability.trim().to_ascii_lowercase();
    if requested.is_empty() {
        return false;
    }

    app_data.is_some_and(|value| {
        parse_announce_metadata(value)
            .capability_tokens
            .iter()
            .any(|token| token == &requested)
    })
}

pub(crate) fn supports_mission_traffic(app_data: Option<&str>) -> bool {
    has_capability_token(app_data, "r3akt") && has_capability_token(app_data, "emergencymessages")
}

pub(crate) fn requires_legacy_rem_chat_ack(app_data: Option<&str>) -> bool {
    app_data.is_some_and(|value| {
        let metadata = parse_announce_metadata(value);
        metadata.wire_format == AnnounceWireFormat::LegacyText
            && metadata.has_legacy_name_token
            && metadata
                .capability_tokens
                .iter()
                .any(|token| token == "r3akt")
            && metadata
                .capability_tokens
                .iter()
                .any(|token| token == "emergencymessages")
    })
}

fn parse_capability_tokens(app_data: &str) -> Vec<String> {
    app_data
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .filter(|token| !token.to_ascii_lowercase().starts_with("name="))
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn decode_percent_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
                let byte = u8::from_str_radix(hex, 16).ok()?;
                decoded.push(byte);
                index += 3;
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            value => {
                decoded.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn announce_display_name_from_msgpack_value(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().and_then(normalize_rem_display_name),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone())
            .ok()
            .as_deref()
            .and_then(normalize_rem_display_name),
        _ => None,
    }
}

fn parse_announce_payload_msgpack(bytes: &[u8]) -> Option<MsgPackValue> {
    rmp_serde::from_slice::<MsgPackValue>(bytes).ok()
}

fn extract_msgpack_announce_display_name(value: &MsgPackValue) -> Option<String> {
    let MsgPackValue::Array(entries) = value else {
        return None;
    };
    entries
        .first()
        .and_then(announce_display_name_from_msgpack_value)
}

fn decode_hex_announce_app_data(app_data: &str) -> Option<Vec<u8>> {
    let trimmed = app_data.trim();
    if trimmed.len() < 2 || !trimmed.len().is_multiple_of(2) {
        return None;
    }
    if !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    hex::decode(trimmed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announce_metadata_accepts_text_and_msgpack_layouts() {
        let text = parse_announce_metadata("R3AKT;EMergencyMessages;name=Legacy+Team");
        assert_eq!(text.display_name.as_deref(), Some("Legacy Team"));
        assert_eq!(text.wire_format, AnnounceWireFormat::LegacyText);
        assert!(text.has_legacy_name_token);
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
        assert_eq!(msgpack.wire_format, AnnounceWireFormat::StructuredLxmf);
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

    #[test]
    fn announce_profile_emits_standard_metadata_and_receipt_capability() {
        let profile = AnnounceProfile::new("Alpha 123", "R3AKT,EMergencyMessages;name=Legacy+Name");
        let encoded = profile.encode().expect("structured announce");
        let parsed = parse_announce_metadata(hex::encode(encoded).as_str());

        assert_eq!(parsed.display_name.as_deref(), Some("Alpha 123"));
        assert_eq!(parsed.wire_format, AnnounceWireFormat::StructuredLxmf);
        assert!(parsed
            .capability_tokens
            .iter()
            .any(|token| token == STANDARD_LXMF_RECEIPTS_CAPABILITY));
        assert!(!parsed.has_legacy_name_token);
    }

    #[test]
    fn legacy_ack_compatibility_requires_a_legacy_rem_named_announce() {
        assert!(requires_legacy_rem_chat_ack(Some(
            "R3AKT,EMergencyMessages;name=Alpha123"
        )));
        assert!(!requires_legacy_rem_chat_ack(Some(
            "R3AKT,EMergencyMessages"
        )));

        let structured = AnnounceProfile::new("Alpha123", "R3AKT,EMergencyMessages")
            .encode()
            .expect("structured announce");
        assert!(!requires_legacy_rem_chat_ack(Some(
            hex::encode(structured).as_str()
        )));
    }
}
