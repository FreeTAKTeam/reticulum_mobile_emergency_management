fn log_entry_fields(args: Vec<(rmpv::Value, rmpv::Value)>) -> Vec<u8> {
    use rmpv::Value;
    let mut all_args = vec![
        (Value::from("entry_uid"), Value::from("evt-pos")),
        (Value::from("mission_uid"), Value::from("mission-1")),
        (Value::from("content"), Value::from("MECP/1/S02")),
        (Value::from("callsign"), Value::from("R4")),
        (Value::from("source_identity"), Value::from("abcd")),
    ];
    all_args.extend(args);
    let fields = Value::Map(vec![(
        Value::from(crate::lxmf_fields::FIELD_COMMANDS),
        Value::Array(vec![Value::Map(vec![
            (Value::from("command_type"), Value::from("mission.registry.log_entry.upsert")),
            (Value::from("args"), Value::Map(all_args)),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).expect("msgpack")
}

fn parse_event(args: Vec<(rmpv::Value, rmpv::Value)>) -> EventProjectionRecord {
    crate::runtime::event_projection_from_fields(&log_entry_fields(args), None, None, None, None, 1)
        .expect("event")
}

#[test]
fn inbound_event_position_needs_both_coordinates_in_range() {
    use rmpv::Value;
    let placed = parse_event(vec![
        (Value::from("latitude"), Value::from(44.0)),
        (Value::from("longitude"), Value::from(-63.0)),
    ]);
    assert_eq!((placed.latitude, placed.longitude), (Some(44.0), Some(-63.0)));

    let half = parse_event(vec![(Value::from("latitude"), Value::from(44.0))]);
    assert_eq!((half.latitude, half.longitude), (None, None));

    let out_of_range = parse_event(vec![
        (Value::from("latitude"), Value::from(95.0)),
        (Value::from("longitude"), Value::from(-63.0)),
    ]);
    assert_eq!((out_of_range.latitude, out_of_range.longitude), (None, None));
}

#[test]
fn event_update_without_position_keeps_stored_position() {
    use rmpv::Value;
    let storage_dir = test_storage_dir("event-position-keep");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let placed = parse_event(vec![
        (Value::from("latitude"), Value::from(44.0)),
        (Value::from("longitude"), Value::from(-63.0)),
    ]);
    store.upsert_event(&placed).expect("insert placed");

    let mut update = parse_event(Vec::new());
    update.content = "MECP/1/S02 updated".to_string();
    store.upsert_event(&update).expect("update without position");

    let reopened = AppStateStore::new(storage_dir.to_str()).expect("reopen store");
    let stored = reopened
        .get_events()
        .expect("events")
        .into_iter()
        .find(|event| event.uid == "evt-pos")
        .expect("stored event");
    assert_eq!(stored.content, "MECP/1/S02 updated");
    assert_eq!((stored.latitude, stored.longitude), (Some(44.0), Some(-63.0)));
}
