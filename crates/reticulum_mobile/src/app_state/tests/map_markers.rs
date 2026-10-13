fn marker_request(symbol: &str, lat: f64, lon: f64) -> crate::types::MapMarkerCreateRequest {
    crate::types::MapMarkerCreateRequest {
        idempotency_key: None,
        marker_type: symbol.to_string(),
        symbol: symbol.to_string(),
        name: Some(format!("{symbol} point")),
        category: None,
        lat,
        lon,
        notes: Some("bounded note".to_string()),
    }
}

fn invalid_marker_cause(error: NodeError) -> String {
    assert!(matches!(error, NodeError::InvalidConfig {}), "{error:?}");
    crate::error_context::take_internal_failure("InvalidConfig")
        .expect("validation cause")
        .cause
}

#[test]
fn map_markers_persist_across_restart_with_shared_symbols() {
    let storage_dir = test_storage_dir("map-markers-restart");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let (fire, invalidation) = store
        .create_map_marker(&marker_request("Wildfire", 44.5, -1.25))
        .expect("create fire");
    assert!(fire.created);
    assert_eq!(invalidation.scope, ProjectionScope::MapMarkers {});
    assert_eq!(fire.marker.symbol, "fire", "aliases normalize through the shared catalog");
    assert_eq!(fire.marker.category, "fire", "category defaults to the symbol");
    store
        .create_map_marker(&marker_request("medic", -33.0, 151.0))
        .expect("create medic");

    let reopened = AppStateStore::new(storage_dir.to_str()).expect("reopen store");
    let markers = reopened.list_map_markers().expect("list markers");
    assert_eq!(markers.len(), 2);
    let restored = reopened
        .get_map_marker(&fire.marker.object_destination_hash)
        .expect("get marker")
        .expect("marker exists");
    assert_eq!(restored, fire.marker);
    assert_eq!(restored.notes.as_deref(), Some("bounded note"));
}

#[test]
fn map_markers_rename_move_and_delete_through_shared_rules() {
    let storage_dir = test_storage_dir("map-markers-edit");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let (created, _) = store
        .create_map_marker(&marker_request("vehicle", 10.0, 20.0))
        .expect("create");
    let key = created.marker.object_destination_hash.clone();

    let (renamed, _) = store
        .rename_map_marker(&crate::types::MapMarkerRenameRequest {
            object_destination_hash: key.clone(),
            name: "  Convoy  ".to_string(),
        })
        .expect("rename");
    assert_eq!(renamed.name, "Convoy");
    let blank = store.rename_map_marker(&crate::types::MapMarkerRenameRequest {
        object_destination_hash: key.clone(),
        name: "   ".to_string(),
    });
    assert_eq!(invalid_marker_cause(blank.expect_err("blank name")), "Marker name is required");

    let (moved, _) = store
        .move_map_marker(&crate::types::MapMarkerMoveRequest {
            object_destination_hash: key.clone(),
            lat: 11.0,
            lon: 21.0,
        })
        .expect("move");
    assert_eq!((moved.lat, moved.lon), (11.0, 21.0));

    assert!(store.delete_map_marker(&key).expect("delete").is_some());
    assert!(store.delete_map_marker(&key).expect("delete again").is_none());
    assert!(store.get_map_marker(&key).expect("get").is_none());
}

#[test]
fn map_markers_reject_invalid_input_with_rch_messages() {
    let storage_dir = test_storage_dir("map-markers-invalid");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let cases = [
        (marker_request("fire", 91.0, 0.0), "lat must be between -90 and 90"),
        (marker_request("fire", 0.0, -180.5), "lon must be between -180 and 180"),
        (marker_request("fire", f64::NAN, 0.0), "lat must be between -90 and 90"),
        (marker_request("spaceship", 0.0, 0.0), "Unsupported marker type"),
    ];
    for (request, expected) in cases {
        let error = store.create_map_marker(&request).expect_err(expected);
        assert_eq!(invalid_marker_cause(error), expected);
    }
    let missing_symbol = crate::types::MapMarkerCreateRequest {
        symbol: " ".to_string(),
        ..marker_request("fire", 0.0, 0.0)
    };
    let error = store.create_map_marker(&missing_symbol).expect_err("missing symbol");
    assert_eq!(invalid_marker_cause(error), "symbol is required");

    let moved = store.move_map_marker(&crate::types::MapMarkerMoveRequest {
        object_destination_hash: "missing".to_string(),
        lat: 0.0,
        lon: 0.0,
    });
    assert_eq!(invalid_marker_cause(moved.expect_err("missing marker")), "Marker not found");
    assert!(store.list_map_markers().expect("list").is_empty());
}

#[test]
fn map_marker_idempotency_replays_and_detects_conflicts() {
    let storage_dir = test_storage_dir("map-markers-idempotency");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let keyed = crate::types::MapMarkerCreateRequest {
        idempotency_key: Some("observation-0001".to_string()),
        ..marker_request("sensor", 1.0, 2.0)
    };
    let (first, _) = store.create_map_marker(&keyed).expect("first");
    let (replayed, invalidation) = store.create_map_marker(&keyed).expect("replay");
    assert!(first.created);
    assert!(!replayed.created);
    assert_eq!(replayed.marker, first.marker);
    assert_eq!(invalidation.reason.as_deref(), Some("marker-replayed"));
    assert_eq!(store.list_map_markers().expect("list").len(), 1);

    let conflicting = crate::types::MapMarkerCreateRequest {
        lat: 3.0,
        ..keyed.clone()
    };
    let error = store.create_map_marker(&conflicting).expect_err("conflict");
    assert_eq!(
        invalid_marker_cause(error),
        "Idempotency-Key was already used with different marker fields"
    );
    let bad_key = crate::types::MapMarkerCreateRequest {
        idempotency_key: Some("has space".to_string()),
        ..keyed
    };
    let error = store.create_map_marker(&bad_key).expect_err("invalid key");
    assert_eq!(
        invalid_marker_cause(error),
        "Idempotency-Key must contain 1 to 128 visible ASCII characters"
    );
    assert_eq!(store.list_map_markers().expect("list").len(), 1);
}

#[test]
fn map_marker_symbol_catalog_matches_shared_crate() {
    let symbols = map_marker_symbols();
    assert_eq!(symbols.len(), r3akt_map_core::symbols::CATALOG.len());
    let fire = symbols.iter().find(|symbol| symbol.id == "fire").expect("fire symbol");
    assert_eq!(fire.set, "mdi");
    assert_eq!(fire.mdi.as_deref(), Some("fire"));
    assert!(fire.aliases.contains(&"wildfire".to_string()));
    let mut without_aliases = serde_json::to_value(&symbols).expect("symbols json");
    for entry in without_aliases.as_array_mut().expect("array") {
        entry.as_object_mut().expect("object").remove("aliases");
    }
    assert_eq!(
        without_aliases,
        r3akt_map_core::symbols::catalog_json(),
        "REM exposes the same catalog as RCH /api/markers/symbols"
    );
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../packages/node-client/src/fixtures/map-marker-catalog.json"
    ))
    .expect("node-client fixture");
    assert_eq!(
        fixture["symbols"],
        r3akt_map_core::symbols::catalog_json(),
        "node-client test double catalog must match the shared crate"
    );
    let alias_count = symbols.iter().map(|symbol| symbol.aliases.len()).sum::<usize>();
    assert_eq!(fixture["aliases"].as_object().expect("aliases").len(), alias_count);
}
