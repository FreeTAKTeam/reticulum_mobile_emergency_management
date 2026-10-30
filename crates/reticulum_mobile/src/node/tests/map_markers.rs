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

#[test]
fn create_map_marker_returns_without_waiting_for_replication_delivery() {
    let storage_dir = prepare_storage_dir("map-marker-create-non-blocking");
    let storage_dir_text = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(storage_dir_text.as_str())).expect("node storage");
    let (tx, mut rx) = mpsc::channel(8);
    {
        let mut inner = node.inner.lock().expect("node lock");
        inner
            .app_state
            .set_saved_peers(&[build_saved_peer()])
            .expect("persist saved peer");
        *inner.status.lock().expect("status lock") = build_status_for_tests();
        *inner.peers_snapshot.lock().expect("peers lock") = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        )];
        // Replication targets are the active local team's members.
        *inner.hub_directory_snapshot.lock().expect("directory lock") = Some(HubDirectorySnapshot {
            local_teams: vec![crate::types::LocalTeamRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                member_destinations: vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()],
            }],
            ..HubDirectorySnapshot::yellow_only(123)
        });
        inner.cmd_tx = Some(tx);
    }

    let started = std::time::Instant::now();
    let result = node
        .create_map_marker(crate::types::MapMarkerCreateRequest {
            idempotency_key: None,
            marker_type: "fire".into(),
            symbol: "fire".into(),
            name: Some("Ridge".into()),
            category: None,
            lat: 44.5,
            lon: -1.25,
            notes: None,
        })
        .expect("create marker");
    // Nobody answers the queued send; the create must not wait for it.
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "create blocked on replication delivery for {:?}",
        started.elapsed()
    );
    assert!(result.created);

    let command = rx.try_recv().expect("expected queued marker send");
    let Command::SendBytes { traffic_class, .. } = command else {
        panic!("expected SendBytes command");
    };
    assert!(matches!(traffic_class, OutboundTrafficClass::Event {}));
    assert_eq!(
        node.list_map_markers().expect("markers").len(),
        1,
        "marker is stored before replication completes"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn markers_exchange_both_directions_and_accept_rch_shared_wire() {
    let _guard = test_lock().lock().await;
    let (relay, a, b) = start_node_pair("marker-wire-interop").await;
    for (sender, receiver, source) in [(&a, &b, "REM"), (&b, &a, "REM"), (&a, &b, "RCH")] {
        let mut marker = replicated_marker();
        marker.updated_ts_ms = crate::numeric::u64_to_i64_saturating(now_ms());
        let fields = if source == "REM" {
            map_marker_replication_payload(&sender.get_status(), &MapMarkerChange::created(marker.clone())).unwrap().1
        } else {
            r3akt_map_core::marker_wire::encode_fields(
                "mission.marker.updated", &marker, &sender.get_status().identity_hex, None,
            ).unwrap()
        };
        sender.send_bytes(receiver.get_status().lxmf_destination_hex,
            b"Synthetic marker interop".to_vec(), Some(fields), SendMode::Auto {}).unwrap();
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            if let Some(received) = receiver.get_map_marker(marker.object_destination_hash.clone()).unwrap() {
                assert_eq!(received.name, marker.name);
                assert_eq!(received.lat, marker.lat);
                assert_eq!(received.lon, marker.lon);
                break;
            }
            assert!(Instant::now() < deadline, "{source} marker was not persisted");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(receiver.get_telemetry_positions().unwrap().is_empty(), "marker must not become sender GPS telemetry");
    }
    stop_node(a).await;
    stop_node(b).await;
    relay.shutdown().await;
}

#[test]
fn markers_persist_without_tcp_or_enabled_telemetry() {
    let storage_dir = prepare_storage_dir("map-markers-offline");
    let node = Node::with_storage_dir(storage_dir.to_str()).expect("offline node");
    let mut settings = sample_app_settings();
    settings.tcp_clients.clear();
    settings.telemetry.enabled = false;
    node.set_app_settings(settings).expect("offline settings");
    let created = node.create_map_marker(MapMarkerCreateRequest {
        idempotency_key: None,
        marker_type: "friendly".into(),
        symbol: "friendly".into(),
        name: Some("Offline marker".into()),
        category: None,
        lat: 44.5,
        lon: -1.25,
        notes: None,
    }).expect("create offline");
    let key = created.marker.object_destination_hash;
    node.rename_map_marker(MapMarkerRenameRequest {
        object_destination_hash: key.clone(), name: "Offline renamed".into(),
    }).expect("rename offline");
    node.move_map_marker(MapMarkerMoveRequest {
        object_destination_hash: key.clone(), lat: 44.51, lon: -1.26,
    }).expect("move offline");
    drop(node);
    let reopened = Node::with_storage_dir(storage_dir.to_str()).expect("reopen");
    let marker = reopened.get_map_marker(key.clone()).expect("read").expect("persisted");
    assert_eq!(marker.name, "Offline renamed");
    assert_eq!((marker.lat, marker.lon), (44.51, -1.26));
    assert!(reopened.delete_map_marker(key).expect("delete offline"));
    assert!(reopened.list_map_markers().expect("list").is_empty());
}
