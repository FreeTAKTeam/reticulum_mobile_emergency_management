fn library_source(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(format!("picked-{name}"));
    std::fs::write(&path, bytes).expect("write picked file");
    path
}

#[test]
fn library_import_persists_across_restart_and_dedupes_content() {
    let storage_dir = test_storage_dir("library-restart");
    let storage = storage_dir.to_string_lossy().into_owned();
    let store = AppStateStore::new(Some(&storage)).expect("store");
    let jpeg = library_source(&storage_dir, "photo.jpg", &[0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]);
    let pdf = library_source(&storage_dir, "plan.pdf", b"%PDF-1.7\n...");
    let image = store.library_import(&jpeg, "photo.jpg", Some("image/jpeg"), 10).expect("jpeg");
    let doc = store.library_import(&pdf, "plan.pdf", Some("application/pdf"), 20).expect("pdf");
    assert_eq!(image.kind, r3akt_library_core::ItemKind::Image);
    assert_eq!(doc.kind, r3akt_library_core::ItemKind::File);
    assert_eq!(
        doc.content_sha256.as_deref(),
        Some(hex::encode(<sha2::Sha256 as sha2::Digest>::digest(b"%PDF-1.7\n...")).as_str())
    );
    let again = store.library_import(&pdf, "plan-copy.pdf", None, 30).expect("repeat");
    assert_eq!(again.item_id, doc.item_id, "identical content is not duplicated");
    drop(store);

    let restarted = AppStateStore::new(Some(&storage)).expect("restart");
    let entries = restarted.library_entries().expect("entries");
    assert_eq!(entries.len(), 2);
    let stored = restarted.library_entry(&image.item_id).expect("entry").expect("present");
    assert_eq!(
        std::fs::read(stored.path.expect("bytes on disk")).expect("read"),
        [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]
    );
}

#[test]
fn library_rejects_unsafe_names_and_oversized_files() {
    let storage_dir = test_storage_dir("library-invalid");
    let store = AppStateStore::new(Some(&storage_dir.to_string_lossy())).expect("store");
    let file = library_source(&storage_dir, "x.bin", b"abc");
    for name in ["", "../escape.pdf", "a/b.pdf", "line\nbreak.pdf"] {
        assert!(store.library_import(&file, name, None, 1).is_err(), "{name:?}");
    }
    assert!(store
        .library_import(&file, "bad.jpg", Some("image jpeg"), 1)
        .is_err(), "malformed media type");
    let big = storage_dir.join("big.bin");
    std::fs::File::create(&big)
        .and_then(|file| file.set_len(LIBRARY_MAX_IMPORT_BYTES + 1))
        .expect("sparse file");
    assert!(store.library_import(&big, "big.bin", None, 1).is_err());
    assert!(store.library_entries().expect("entries").is_empty(), "nothing indexed");
}

#[test]
fn library_delete_and_missions_and_partial_recovery() {
    let storage_dir = test_storage_dir("library-delete");
    let storage = storage_dir.to_string_lossy().into_owned();
    let store = AppStateStore::new(Some(&storage)).expect("store");
    let file = library_source(&storage_dir, "notes.txt", b"hello");
    let item = store.library_import(&file, "notes.txt", Some("text/plain"), 1).expect("import");
    let linked = store
        .library_set_missions(&item.item_id, vec![" alpha ".into(), "alpha".into(), String::new()], 2)
        .expect("missions")
        .expect("item");
    assert_eq!(linked.mission_uids, ["alpha"]);
    let path = store.library_entry(&item.item_id).expect("entry").expect("present").path.expect("path");
    assert!(store.library_delete(&item.item_id).expect("delete"));
    assert!(!path.exists(), "bytes removed with the last reference");
    assert!(!store.library_delete(&item.item_id).expect("repeat delete"));

    std::fs::write(store.library_dir().join("deadbeef.part"), b"half").expect("partial");
    drop(store);
    let restarted = AppStateStore::new(Some(&storage)).expect("restart");
    assert!(!restarted.library_dir().join("deadbeef.part").exists(), "interrupted import cleaned");
}

#[test]
fn library_offers_list_as_remote_items_and_receipt_adds_a_verified_peer_item() {
    let storage_dir = test_storage_dir("library-offers");
    let store = AppStateStore::new(Some(&storage_dir.to_string_lossy())).expect("store");
    let package = "11".repeat(16);
    let sender = "22".repeat(16);
    store.library_record_offer(&package, &sender, &"33".repeat(32), 5).expect("offer");
    store.library_record_offer(&package, &sender, &"33".repeat(32), 6).expect("repeat offer");
    let offers: Vec<_> = store
        .library_entries()
        .expect("entries")
        .into_iter()
        .filter(|entry| entry.item.source == r3akt_library_core::Source::Peer)
        .collect();
    assert_eq!(offers.len(), 1, "a repeated Offer is listed once");
    assert_eq!(offers[0].item.availability, r3akt_library_core::Availability::Remote);
    assert!(offers[0].path.is_none(), "no bytes before download");
    assert_eq!(library_offer_package(&offers[0].item.item_id), Some(package.as_str()));

    // A verified fetch, as rpp-core returns it from the transfer store.
    let owner = rpp_core::SigningIdentity::from_private_bytes(&[7u8; 64]);
    let fetched_dir = storage_dir.join("library").join("rpp").join("packages").join("h");
    std::fs::create_dir_all(&fetched_dir).expect("dir");
    let path = fetched_dir.join("plan.pdf");
    std::fs::write(&path, b"%PDF-1.7").expect("file");
    let manifest = rpp_core::ManifestDraft {
        package_destination: [0x11; 16],
        manifest_destination: [0x44; 16],
        files: vec![rpp_core::FileDraft::from_chunks("plan.pdf", [0x55; 16], &[b"%PDF-1.7"])],
        ..rpp_core::ManifestDraft::default()
    }
    .sign(&owner)
    .expect("manifest");
    let fetched = rpp_core::rns_share::FetchedFile {
        path,
        name: "plan.pdf".into(),
        media_type: Some("application/pdf".into()),
        manifest,
    };
    let item = store.library_receive(&package, &fetched).expect("receive");
    assert_eq!(item.source, r3akt_library_core::Source::Peer);
    assert_eq!(item.publisher.as_deref(), Some(sender.as_str()));
    assert_eq!(item.rpp.as_ref().map(|r| r.file_destination.as_str()), Some("55".repeat(16).as_str()));
    assert!(!fetched_dir.exists(), "transfer copy removed after import");
    let entries = store.library_entries().expect("entries");
    assert!(entries.iter().all(|e| library_offer_package(&e.item.item_id).is_none()), "offer retired");
    assert!(entries.iter().any(|e| e.item.item_id == item.item_id && e.path.is_some()));
}

#[test]
fn library_hub_items_cache_metadata_and_record_status() {
    let storage_dir = test_storage_dir("library-hub");
    let store = AppStateStore::new(Some(&storage_dir.to_string_lossy())).expect("store");
    let listed = r3akt_library_core::LibraryItem {
        item_id: "rch:7".into(),
        legacy_file_id: Some(7),
        kind: r3akt_library_core::ItemKind::Image,
        source: r3akt_library_core::Source::Rch,
        availability: r3akt_library_core::Availability::Local,
        name: "site.jpg".into(),
        media_type: Some("image/jpeg".into()),
        size: 9,
        content_sha256: None,
        rpp: None,
        publisher: None,
        mission_uids: vec!["alpha".into()],
        team_uid: None,
        created_ts_ms: 1,
        updated_ts_ms: 1,
    };
    let mut bad = listed.clone();
    bad.item_id = "rch:8".into();
    bad.name = "../escape".into();
    store.library_cache_hub_items(&[listed.clone(), bad]).expect("cache");
    let hub = store.library_hub_items().expect("hub items");
    assert_eq!(hub.len(), 1, "invalid hub metadata is dropped");
    assert_eq!(hub[0].availability, r3akt_library_core::Availability::Remote, "metadata only");
    store.library_set_hub_item_state("rch:7", "downloading").expect("state");
    assert_eq!(
        store.library_hub_item("rch:7").expect("item").expect("present").availability,
        r3akt_library_core::Availability::Downloading
    );
    store.library_cache_hub_items(&[listed]).expect("refresh");
    assert_eq!(store.library_hub_items().expect("items").len(), 1, "refresh does not duplicate");
    assert_eq!(store.library_hub_status().expect("status"), None);
    store.library_set_hub_status("unknown_command").expect("status");
    assert_eq!(store.library_hub_status().expect("status").as_deref(), Some("unknown_command"));
}

fn hub_test_item(item_id: &str) -> r3akt_library_core::LibraryItem {
    r3akt_library_core::LibraryItem {
        item_id: item_id.into(),
        legacy_file_id: None,
        kind: r3akt_library_core::ItemKind::File,
        source: r3akt_library_core::Source::Rch,
        availability: r3akt_library_core::Availability::Remote,
        name: "plan.pdf".into(),
        media_type: None,
        size: 1,
        content_sha256: None,
        rpp: None,
        publisher: None,
        mission_uids: Vec::new(),
        team_uid: None,
        created_ts_ms: 1,
        updated_ts_ms: 1,
    }
}

#[test]
fn library_downloads_left_over_from_a_previous_run_restart_as_failed() {
    let storage_dir = test_storage_dir("library-stuck-restart");
    let storage = storage_dir.to_string_lossy().into_owned();
    let store = AppStateStore::new(Some(&storage)).expect("store");
    store.library_cache_hub_items(&[hub_test_item("rch:1"), hub_test_item("rch:2")]).expect("cache");
    store.library_set_hub_item_state("rch:1", "downloading").expect("state");
    let package = "44".repeat(16);
    store.library_record_offer(&package, &"55".repeat(16), &"66".repeat(32), 5).expect("offer");
    store.library_set_offer_state(&package, "downloading").expect("offer state");
    drop(store);

    let store = AppStateStore::new(Some(&storage)).expect("reopen");
    let hub = store.library_hub_items().expect("hub items");
    let availability = |id: &str| hub.iter().find(|item| item.item_id == id).expect(id).availability;
    assert_eq!(availability("rch:1"), r3akt_library_core::Availability::Failed);
    assert_eq!(availability("rch:2"), r3akt_library_core::Availability::Remote, "idle items untouched");
    let offers = store.library_offer_items().expect("offers");
    assert_eq!(offers.len(), 1, "offer kept");
    assert_eq!(offers[0].availability, r3akt_library_core::Availability::Failed);
}

#[test]
fn library_hub_download_without_an_answer_fails_after_the_deadline_and_retries() {
    let storage_dir = test_storage_dir("library-stuck-deadline");
    let store = AppStateStore::new(Some(&storage_dir.to_string_lossy())).expect("store");
    store.library_cache_hub_items(&[hub_test_item("rch:1")]).expect("cache");
    store.library_set_hub_item_state("rch:1", "downloading").expect("state");
    let availability = || store.library_hub_item("rch:1").expect("item").expect("present").availability;
    assert_eq!(availability(), r3akt_library_core::Availability::Downloading, "inside the deadline");

    let started = crate::numeric::u64_to_i64_saturating(now_ms()) - LIBRARY_HUB_DOWNLOAD_DEADLINE_MS - 1;
    store
        .connect()
        .expect("connect")
        .execute("UPDATE rem_library_hub_items SET state_ts_ms = ?1", params![started])
        .expect("backdate");
    assert_eq!(availability(), r3akt_library_core::Availability::Failed, "lost answer can be retried");
    store.library_cache_hub_items(&[hub_test_item("rch:1")]).expect("refresh");
    assert_eq!(availability(), r3akt_library_core::Availability::Failed, "refresh keeps it retryable");

    // Retry, or a late hub answer, sets `downloading` again with a fresh clock.
    store.library_set_hub_item_state("rch:1", "downloading").expect("retry");
    assert_eq!(availability(), r3akt_library_core::Availability::Downloading);
}
