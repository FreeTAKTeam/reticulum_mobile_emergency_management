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
