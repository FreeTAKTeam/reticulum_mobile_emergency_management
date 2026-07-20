use rmpv::Value as MsgPackValue;

pub(crate) fn msgpack_map_entries(value: &MsgPackValue) -> Option<&[(MsgPackValue, MsgPackValue)]> {
    match value {
        MsgPackValue::Map(entries) => Some(entries.as_slice()),
        _ => None,
    }
}

pub(crate) fn msgpack_get_indexed(
    entries: &[(MsgPackValue, MsgPackValue)],
    key: i64,
) -> Option<&MsgPackValue> {
    let key_string = key.to_string();
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| match entry_key {
            MsgPackValue::Integer(value) if value.as_i64() == Some(key) => Some(entry_value),
            MsgPackValue::String(value) if value.as_str() == Some(key_string.as_str()) => {
                Some(entry_value)
            }
            _ => None,
        })
}

pub(crate) fn msgpack_get_named<'a>(
    entries: &'a [(MsgPackValue, MsgPackValue)],
    keys: &[&str],
) -> Option<&'a MsgPackValue> {
    keys.iter().find_map(|wanted| {
        entries.iter().find_map(|(entry_key, entry_value)| {
            matches!(entry_key, MsgPackValue::String(actual) if actual.as_str() == Some(*wanted))
                .then_some(entry_value)
        })
    })
}

pub(crate) fn msgpack_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::String(value) => value.as_str().map(str::to_string),
        MsgPackValue::Binary(value) => String::from_utf8(value.clone()).ok(),
        _ => None,
    }
}

pub(crate) fn msgpack_hex_or_string(value: &MsgPackValue) -> Option<String> {
    match value {
        MsgPackValue::Binary(value) if value.len() == 16 => Some(hex::encode(value)),
        _ => msgpack_string(value),
    }
}

pub(crate) fn msgpack_bool(value: &MsgPackValue) -> Option<bool> {
    match value {
        MsgPackValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

pub(crate) fn msgpack_f64(value: &MsgPackValue) -> Option<f64> {
    match value {
        MsgPackValue::F32(value) => Some(f64::from(*value)),
        MsgPackValue::F64(value) => Some(*value),
        MsgPackValue::Integer(value) => value.as_i64().and_then(crate::numeric::i64_to_f64_exact),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        msgpack_bool, msgpack_f64, msgpack_get_indexed, msgpack_get_named, msgpack_hex_or_string,
        msgpack_map_entries, msgpack_string,
    };
    use rmpv::Value as MsgPackValue;

    #[test]
    fn reads_named_and_compact_map_keys() {
        let value = MsgPackValue::Map(vec![
            (MsgPackValue::from(9), MsgPackValue::from("numeric")),
            (
                MsgPackValue::from("10"),
                MsgPackValue::from("string-number"),
            ),
            (MsgPackValue::from("name"), MsgPackValue::from("named")),
        ]);
        let entries = msgpack_map_entries(&value).expect("map");

        assert_eq!(
            msgpack_get_indexed(entries, 9).and_then(msgpack_string),
            Some("numeric".to_string())
        );
        assert_eq!(
            msgpack_get_indexed(entries, 10).and_then(msgpack_string),
            Some("string-number".to_string())
        );
        assert_eq!(
            msgpack_get_named(entries, &["missing", "name"]).and_then(msgpack_string),
            Some("named".to_string())
        );
    }

    #[test]
    fn preserves_binary_and_scalar_conversion_rules() {
        let identity = MsgPackValue::Binary(vec![0xAB; 16]);
        assert_eq!(msgpack_hex_or_string(&identity), Some("ab".repeat(16)));
        assert_eq!(
            msgpack_string(&MsgPackValue::Binary(b"REM".to_vec())),
            Some("REM".to_string())
        );
        assert_eq!(msgpack_bool(&MsgPackValue::Boolean(true)), Some(true));
        assert_eq!(msgpack_f64(&MsgPackValue::from(42)), Some(42.0));
    }
}
