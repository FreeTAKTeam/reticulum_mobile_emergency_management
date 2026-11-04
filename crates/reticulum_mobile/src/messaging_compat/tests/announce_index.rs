fn indexed_announce(destination: &str, identity: &str, kind: &str, time: u64) -> AnnounceRecord {
    AnnounceRecord {
        destination_hex: destination.into(),
        identity_hex: identity.into(),
        destination_kind: kind.into(),
        app_data: "R3AKT,EMergencyMessages,Telemetry".into(),
        display_name: Some("Peer".into()),
        hops: 1,
        interface_hex: "iface".into(),
        received_at_ms: time,
    }
}

#[test]
fn announce_replacement_survives_resolution_changes_and_keeps_other_kinds() {
    for kind in ["app", "lxmf_delivery"] {
        let mut store = MessagingStore::default();
        store.record_announce(indexed_announce("old", " IDENTITY ", kind, 1));
        store.record_announce(indexed_announce("other-kind", "identity", "other", 1));
        store.record_announce(indexed_announce("other-peer", "another", kind, 1));
        store.record_resolution_result("resolved-app", "identity", "resolved-lxmf", 2);
        store.record_announce(indexed_announce("new", "identity", kind, 3));

        let records = store.list_announces();
        assert_eq!(records.len(), 3);
        assert!(!records.iter().any(|record| record.destination_hex == "old"));
        assert!(records.iter().any(|record| record.destination_hex == "new"));
        assert!(records
            .iter()
            .any(|record| record.destination_hex == "other-kind"));
        assert!(records
            .iter()
            .any(|record| record.destination_hex == "other-peer"));
    }
}

#[test]
fn announce_destination_reassignment_does_not_delete_another_identity() {
    let mut store = MessagingStore::default();
    store.record_announce(indexed_announce("shared", "first", "lxmf_delivery", 1));
    store.record_announce(indexed_announce("shared", "second", "app", 2));
    store.record_announce(indexed_announce("new", "first", "lxmf_delivery", 3));
    assert_eq!(store.list_announces().len(), 2);
    assert_eq!(store.announce_records["shared"].identity_hex, "second");
}

#[test]
fn announce_replacement_preserves_metadata_and_rejects_older_updates() {
    let mut store = MessagingStore::default();
    let original = indexed_announce(" DEST ", " IDENTITY ", "lxmf_delivery", 2);
    store.record_announce(original.clone());
    let mut update = indexed_announce("dest", "identity", "lxmf_delivery", 3);
    update.app_data.clear();
    update.display_name = None;
    store.record_announce(update);
    store.record_announce(indexed_announce("dest", "other", "app", 1));

    let record = &store.announce_records["dest"];
    assert_eq!(record.app_data, original.app_data);
    assert_eq!(record.display_name, original.display_name);
    assert_eq!(record.received_at_ms, 3);
    store.record_announce(indexed_announce("next", "identity", "lxmf_delivery", 4));
    assert_eq!(store.list_announces().len(), 1);
    assert!(store.announce_records.contains_key("next"));
}

#[test]
#[ignore = "manual release-mode announce history performance measurement"]
fn announce_history_performance() {
    let mut store = MessagingStore::default();
    let records = (0..20_000)
        .map(|index| {
            indexed_announce(
                &format!("{index:032x}"),
                &format!("{index:032x}"),
                "lxmf_delivery",
                1,
            )
        })
        .collect::<Vec<_>>();
    let started = std::time::Instant::now();
    for record in &records {
        store.record_announce(record.clone());
    }
    let replay = started.elapsed();
    let started = std::time::Instant::now();
    for record in records.iter().take(1_000) {
        let mut update = record.clone();
        update.received_at_ms = 2;
        store.record_announce(update);
    }
    eprintln!(
        "20,000 announce replay: {replay:?}; 1,000 updates: {:?}",
        started.elapsed()
    );
    assert_eq!(store.list_announces().len(), records.len());
}

#[test]
fn peer_announce_membership_tracks_metadata_changes_and_replacement() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    let mut record = indexed_announce("old", "identity", "lxmf_delivery", now);
    store.record_announce(record.clone());
    assert_eq!(store.list_peers().len(), 1);
    record.app_data.clear();
    store.record_announce(record.clone());
    assert_eq!(store.list_peers().len(), 1);
    record.app_data = "chat".into();
    store.record_announce(record.clone());
    assert!(store.list_peers().is_empty());
    record.app_data = "R3AKT,EMergencyMessages,Telemetry".into();
    record.destination_hex = "new".into();
    store.record_announce(record);
    let peers = store.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].destination_hex, "new");
    store.record_announce(indexed_announce("new", "different", "other", now + 1));
    assert!(store.list_peers().is_empty());
}

#[test]
#[ignore = "manual release-mode peer projection performance measurement"]
fn peer_projection_history_performance() {
    let mut store = MessagingStore::default();
    let now = current_time_ms();
    for index in 0..20_000 {
        let mut record = indexed_announce(
            &format!("{index:032x}"),
            &format!("{index:032x}"),
            "lxmf_delivery",
            now,
        );
        if index >= 10 {
            record.app_data = "93c40a54333535302d4465636bc09100".into();
        }
        store.record_announce(record);
    }
    let started = std::time::Instant::now();
    for _ in 0..100 {
        assert_eq!(std::hint::black_box(store.list_peers()).len(), 10);
    }
    eprintln!(
        "100 peer projections over 20,000 announces: {:?}",
        started.elapsed()
    );
}
