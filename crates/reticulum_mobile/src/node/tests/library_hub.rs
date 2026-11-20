#[test]
fn library_fetch_answers_find_their_item_from_the_command_id() {
    let command_id = library_fetch_command_id("rch:43");
    assert!(command_id.starts_with(LIBRARY_FETCH_PREFIX));
    // RCH answers with REM's command ID, and echoes it as the correlation too.
    assert_eq!(library_fetch_item_id(&command_id).as_deref(), Some("rch:43"));
    assert_eq!(library_fetch_item_id("library-fetch-marker-c8020fab"), None);
    assert_eq!(library_fetch_item_id("rch:43"), None);
}
