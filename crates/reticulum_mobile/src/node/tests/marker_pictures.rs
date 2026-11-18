const PICTURE_ID: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const PICTURE_THUMB: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 1, 2, 3];

fn picture_marker_payload(thumbnail_sha256: &str) -> JsonValue {
    let mut marker = r3akt_map_core::MarkerRecord::create_with_id(
        r3akt_map_core::MarkerDraft {
            marker_type: "picture".into(),
            symbol: "picture".into(),
            name: "Picture".into(),
            category: "picture".into(),
            position: r3akt_map_core::Position::new(44.5, -1.25).expect("position"),
            notes: None,
            origin_rch: "feedfeed".into(),
        },
        "0123456789abcdef0123456789abcdef".into(),
        "local".into(),
        1_763_380_800_000,
    );
    let mut picture = r3akt_map_core::marker_picture::MarkerPicture::parse(&json!({
        "thumbnail": {"media_type": "image/jpeg", "width": 160, "height": 120, "sha256": thumbnail_sha256},
        "captured_at": "2026-11-17T12:00:00Z",
        "crop": "centre"
    }))
    .expect("picture");
    picture.message_id = Some(PICTURE_ID.into());
    marker.picture = Some(picture);
    marker.api_value()
}

/// An RCH team relay: the replica event in field 13 and the JPEG in field 6.
fn picture_relay_fields(payload: &JsonValue, image: Option<&[u8]>) -> Vec<u8> {
    let event = json!({ "event_type": "mission.marker.created", "payload": payload });
    let mut fields = vec![(
        rmpv::Value::from(crate::lxmf_fields::FIELD_EVENT),
        rmpv::ext::to_value(&event).expect("event"),
    )];
    if let Some(image) = image {
        fields.push((
            rmpv::Value::from(6),
            rmpv::Value::Array(vec!["jpg".into(), rmpv::Value::Binary(image.to_vec())]),
        ));
    }
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &rmpv::Value::Map(fields)).expect("encode");
    bytes
}

fn store_relayed_picture(name: &str, image: Option<&[u8]>) -> crate::app_state::MapMarkerPictureView {
    let storage_dir = prepare_storage_dir(name);
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let payload = picture_marker_payload(&r3akt_map_core::marker_picture::sha256_hex(PICTURE_THUMB));
    store.apply_map_marker_replica(&payload).expect("replica");
    crate::runtime::store_received_marker_picture_thumbnail(
        &store,
        &picture_relay_fields(&payload, image),
    );
    let pictures = store.list_map_marker_pictures().expect("pictures");
    assert_eq!(store.list_map_markers().expect("markers").len(), 1, "the marker always shows");
    pictures
        .get("0123456789abcdef0123456789abcdef")
        .cloned()
        .expect("picture view")
}

#[test]
fn relayed_picture_marker_keeps_its_hash_checked_thumbnail() {
    let view = store_relayed_picture("marker-picture-thumbnail", Some(PICTURE_THUMB));
    assert_eq!(view.message_id.as_deref(), Some(PICTURE_ID));
    assert_eq!((view.thumbnail_width, view.thumbnail_height), (160, 120));
    assert_eq!(view.crop.as_deref(), Some("centre"));
    assert_eq!(
        view.thumbnail_data_url.as_deref(),
        Some(crate::app_state::jpeg_data_url(PICTURE_THUMB).as_str())
    );
}

#[test]
fn mismatching_or_missing_thumbnail_falls_back_but_keeps_the_marker() {
    let tampered = store_relayed_picture("marker-picture-mismatch", Some(&[0xff, 0xd8, 9]));
    assert_eq!(tampered.thumbnail_data_url, None);
    let missing = store_relayed_picture("marker-picture-missing", None);
    assert_eq!(missing.thumbnail_data_url, None);
    assert_eq!(missing.message_id.as_deref(), Some(PICTURE_ID));
}

#[test]
fn picture_get_uses_the_shared_rch_command_shape() {
    let fields = marker_picture_get_fields("abcd", PICTURE_ID).expect("fields");
    let commands = r3akt_profile_rch::decode_commands(&fields).expect("commands");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command_type, "mission.marker.picture.get");
    assert_eq!(commands[0].args, json!({ "message_id": PICTURE_ID }));
    assert_eq!(commands[0].correlation_id.as_deref(), Some(PICTURE_ID));
    assert!(commands[0].command_id.starts_with(MARKER_PICTURE_GET_PREFIX));
}

fn picture_answer_fields(command_id: &str, status: &str, result: JsonValue, image: Option<&[u8]>) -> Vec<u8> {
    let envelope = json!({
        "command_id": command_id,
        "status": status,
        "correlation_id": PICTURE_ID,
        "reason_code": (status == "rejected").then_some("unauthorized"),
        "result": result,
    });
    let mut fields = vec![(
        rmpv::Value::from(crate::lxmf_fields::FIELD_RESULTS),
        rmpv::ext::to_value(&envelope).expect("result"),
    )];
    if let Some(image) = image {
        fields.push((
            rmpv::Value::from(6),
            rmpv::Value::Array(vec!["jpg".into(), rmpv::Value::Binary(image.to_vec())]),
        ));
    }
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &rmpv::Value::Map(fields)).expect("encode");
    bytes
}

#[test]
fn picture_answers_are_read_from_field_10_and_hash_checked() {
    let id = format!("{MARKER_PICTURE_GET_PREFIX}1");
    let image: &[u8] = &[0xff, 0xd8, 0xff, 7, 7, 7];
    let sha = r3akt_map_core::marker_picture::sha256_hex(image);
    let answer = |status: &str, result: JsonValue, image: Option<&[u8]>| {
        crate::runtime::marker_picture_answers(&picture_answer_fields(&id, status, result, image))
    };

    assert_eq!(
        answer("result", json!({"status": "ok", "sha256": sha}), Some(image)),
        vec![(PICTURE_ID.to_string(), "ok".to_string(), Some(image.to_vec()))]
    );
    assert_eq!(
        answer("result", json!({"status": "ok", "sha256": sha}), Some(&[1, 2])),
        vec![(PICTURE_ID.to_string(), "invalid".to_string(), None)]
    );
    for status in ["pending", "unavailable"] {
        assert_eq!(
            answer("result", json!({"status": status, "message_id": PICTURE_ID}), None),
            vec![(PICTURE_ID.to_string(), status.to_string(), None)]
        );
    }
    assert_eq!(
        answer("rejected", json!({"status": "unauthorized"}), None),
        vec![(PICTURE_ID.to_string(), "unauthorized".to_string(), None)]
    );
    assert!(answer("accepted", JsonValue::Null, None).is_empty());
    let other = picture_answer_fields("hub-directory-1", "result", json!({"status": "ok"}), None);
    assert!(crate::runtime::marker_picture_answers(&other).is_empty());
}
