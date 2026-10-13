fn replicated_marker() -> r3akt_map_core::MarkerRecord {
    let draft = r3akt_map_core::MarkerDraft {
        marker_type: "fire".into(),
        symbol: "fire".into(),
        name: "Ridge".into(),
        category: "incident".into(),
        position: r3akt_map_core::Position::new(44.5, -1.25).expect("position"),
        notes: None,
        origin_rch: String::new(),
    };
    r3akt_map_core::MarkerRecord::create(draft, "local".into(), 1_700_000_000_000)
}

#[test]
fn map_marker_payload_carries_rch_command_and_rem_replica() {
    let marker = replicated_marker();
    let status = build_status_for_tests();
    let (body, fields) =
        map_marker_replication_payload(&status, &MapMarkerChange::created(marker.clone()))
            .expect("payload");
    assert_eq!(String::from_utf8(body).expect("utf8"), "Map marker created: Ridge");

    let metadata = parse_mission_sync_metadata(&fields).expect("mission metadata");
    assert_eq!(metadata.command_type.as_deref(), Some("mission.marker.create"));
    assert!(metadata.event_present);

    let decoded: MsgPackValue = rmp_serde::from_slice(&fields).expect("fields");
    let command = crate::msgpack_values::msgpack_map_entries(&decoded)
        .and_then(|entries| crate::msgpack_values::msgpack_get_indexed(entries, FIELD_COMMANDS))
        .cloned()
        .expect("commands");
    let command: JsonValue = rmpv::ext::from_value(command).expect("json command");
    let args = &command[0]["args"];
    assert_eq!(args["object_destination_hash"], marker.object_destination_hash);
    assert_eq!(args["symbol"], "fire");
    assert!(args.get("notes").is_none(), "null args are omitted");
    assert_eq!(command[0]["source"]["rns_identity"], status.identity_hex);

    let events = crate::runtime::map_marker_events(&fields);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "mission.marker.created");
    let replica = r3akt_map_core::replica::from_api_value(&events[0].1, "peer".into())
        .expect("replica");
    assert_eq!(replica.object_destination_hash, marker.object_destination_hash);
    assert!(!crate::runtime::is_marker_telemetry(&fields));
}

#[test]
fn map_marker_changes_map_to_rch_commands() {
    let marker = replicated_marker();
    let cases = [
        (MapMarkerChange::renamed(marker.clone()), "mission.marker.patch", "mission.marker.updated"),
        (
            MapMarkerChange::moved(marker.clone()),
            "mission.marker.position.patch",
            "mission.marker.position.updated",
        ),
        (MapMarkerChange::deleted(marker), "mission.marker.delete", "mission.marker.deleted"),
    ];
    for (change, command_type, event_type) in cases {
        assert_eq!(change.command_type, command_type);
        assert_eq!(change.event_type, event_type);
        assert!(change.args.get("object_destination_hash").is_some());
    }
}

#[test]
fn rch_marker_telemetry_is_recognized_and_not_stored_as_telemetry() {
    let fields = serde_json::json!({
        "9": [{
            "command_type": "mission.registry.telemetry.upsert",
            "args": {
                "callsign": "Ridge",
                "lat": 44.5,
                "lon": -1.25,
                "object_destination_hash": "0123456789abcdef0123456789abcdef",
                "marker_type": "fire",
                "symbol": "fire",
            },
        }],
    });
    let bytes = rmp_serde::to_vec(&rmpv::ext::to_value(&fields).expect("msgpack")).expect("bytes");
    assert!(crate::runtime::is_marker_telemetry(&bytes));

    let mut peer = fields.clone();
    peer["9"][0]["args"]
        .as_object_mut()
        .expect("args")
        .retain(|key, _| !matches!(key.as_str(), "object_destination_hash" | "marker_type"));
    let bytes = rmp_serde::to_vec(&rmpv::ext::to_value(&peer).expect("msgpack")).expect("bytes");
    assert!(!crate::runtime::is_marker_telemetry(&bytes));
}
