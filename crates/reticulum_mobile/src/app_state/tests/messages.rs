#[test]
fn eam_readiness_summary_derives_per_message_scores_in_rust() {
    let record = readiness_eam(
        "Atlas",
        ["Green", "Yellow", "Red", "Unknown", "Green", "Yellow"],
        100,
        None,
    );

    let summary = build_eam_readiness_summary(vec![record]);

    assert_eq!(summary.active_total, 1);
    assert_eq!(summary.messages.len(), 1);
    assert_eq!(summary.messages[0].callsign, "Atlas");
    assert_eq!(summary.messages[0].overall_score, 54);
    assert_eq!(summary.messages[0].overall_band, "Yellow");
    assert!(!summary.messages[0].overall_ring_color.is_empty());
}

#[test]
fn messages_for_same_peer_are_repaired_into_one_canonical_thread() {
    let storage_dir = test_storage_dir("canonical-thread");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

    let outbound = message(
        "outbound-1",
        "legacy-outbound-thread",
        MessageDirection::Outbound {},
        "ABCDEF",
        Some("LOCAL"),
        10,
    );
    let inbound = message(
        "inbound-1",
        "legacy-inbound-thread",
        MessageDirection::Inbound {},
        "LOCAL",
        Some("ABCDEF"),
        20,
    );

    store.upsert_message(&outbound).expect("persist outbound");
    store.upsert_message(&inbound).expect("persist inbound");

    let conversations = store.list_conversations().expect("list conversations");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].conversation_id, "abcdef");
    assert_eq!(conversations[0].peer_destination_hex, "abcdef");

    let messages = store
        .list_messages(Some("abcdef"))
        .expect("list canonical messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message_id_hex, "outbound-1");
    assert_eq!(messages[0].conversation_id, "abcdef");
    assert_eq!(messages[1].message_id_hex, "inbound-1");
    assert_eq!(messages[1].conversation_id, "abcdef");
}

#[test]
fn conversation_message_query_uses_the_additive_sort_index() {
    let storage_dir = test_storage_dir("message-query-index");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let connection = store.connect().expect("open store");
    let plan: String = connection
        .query_row(
            "EXPLAIN QUERY PLAN
             SELECT json FROM messages WHERE conversation_id = ?1
             ORDER BY updated_at_ms ASC, message_id_hex ASC",
            ["peer-a"],
            |row| row.get(3),
        )
        .expect("query plan");

    assert!(
        plan.contains("idx_messages_conversation_updated"),
        "unexpected query plan: {plan}"
    );
}

#[test]
fn startup_history_lists_persisted_messages_without_runtime() {
    let storage_dir = test_storage_dir("startup-history");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let outbound = message(
        "persisted-1",
        "old-thread",
        MessageDirection::Outbound {},
        "PEER-1",
        Some("LOCAL"),
        30,
    );
    store.upsert_message(&outbound).expect("persist message");

    let restarted =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("reopen store");
    let conversations = restarted
        .list_conversations()
        .expect("list conversations before runtime");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].conversation_id, "peer-1");

    let messages = restarted
        .list_messages(Some("peer-1"))
        .expect("list messages before runtime");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id_hex, "persisted-1");
    assert_eq!(messages[0].conversation_id, "peer-1");
}

#[test]
fn delete_conversation_removes_canonical_peer_alias_thread() {
    let storage_dir = test_storage_dir("delete-alias-thread");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let outbound = message(
        "delete-outbound",
        "app-thread",
        MessageDirection::Outbound {},
        "APPDEST",
        Some("LOCAL"),
        10,
    );
    let inbound = message(
        "delete-inbound",
        "lxmf-thread",
        MessageDirection::Inbound {},
        "LOCAL",
        Some("LXMFDest"),
        20,
    );
    let unrelated = message(
        "delete-unrelated",
        "other-thread",
        MessageDirection::Outbound {},
        "OTHER",
        Some("LOCAL"),
        30,
    );

    store.upsert_message(&outbound).expect("persist outbound");
    store.upsert_message(&inbound).expect("persist inbound");
    store.upsert_message(&unrelated).expect("persist unrelated");

    let mut resolver = ConversationPeerResolver::default();
    resolver.insert(
        vec!["APPDEST".to_string(), "LXMFDest".to_string()],
        "IDENTITY".to_string(),
        "LXMFDest".to_string(),
        Some("Poco".to_string()),
    );

    store
        .delete_conversation_resolved("APPDEST", &resolver)
        .expect("delete alias conversation");

    let conversations = store
        .list_conversations_resolved(&resolver)
        .expect("list after delete");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].conversation_id, "other");
    let messages = store
        .list_messages_resolved(None, &resolver)
        .expect("list remaining messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id_hex, "delete-unrelated");
}

#[test]
fn active_sos_alert_requires_existing_conversation_messages() {
    let storage_dir = test_storage_dir("sos-alert-orphan");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let alert = sos_alert(
        "incident-orphan",
        "source-orphan",
        "orphan-thread",
        true,
        10,
    );

    store.upsert_sos_alert(&alert).expect("persist sos alert");
    assert!(store
        .list_sos_alerts()
        .expect("list orphan sos alerts")
        .is_empty());

    let message = message(
        "sos-message",
        "orphan-thread",
        MessageDirection::Inbound {},
        "LOCAL",
        Some("orphan-thread"),
        11,
    );
    store.upsert_message(&message).expect("persist sos message");
    assert_eq!(
        store
            .list_sos_alerts()
            .expect("list attached sos alerts")
            .len(),
        1
    );
}

#[test]
fn active_sos_alert_is_hidden_after_legacy_cancel_message() {
    let storage_dir = test_storage_dir("sos-alert-legacy-cancel");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let active_message = message(
        "sos-active-message",
        "legacy-thread",
        MessageDirection::Outbound {},
        "legacy-thread",
        Some("LOCAL"),
        10,
    );
    let mut cancel_message = message(
        "sos-cancel-message",
        "legacy-thread",
        MessageDirection::Outbound {},
        "legacy-thread",
        Some("LOCAL"),
        12,
    );
    cancel_message.body_utf8 = "SOS Cancelled - I am safe.".to_string();
    let alert = sos_alert("incident-legacy", "legacypeer", "legacy-thread", true, 11);

    store
        .upsert_message(&active_message)
        .expect("persist active message");
    store.upsert_sos_alert(&alert).expect("persist sos alert");
    assert_eq!(store.list_sos_alerts().expect("list active alert").len(), 1);

    store
        .upsert_message(&cancel_message)
        .expect("persist cancel message");
    assert!(store
        .list_sos_alerts()
        .expect("list after legacy cancel")
        .is_empty());
    assert_eq!(
        store
            .latest_active_sos_alert_for_source("legacypeer")
            .expect("latest active")
            .expect("active row remains for runtime reconciliation")
            .incident_id,
        "incident-legacy"
    );
}

#[test]
fn delete_conversation_removes_sos_alert_and_locations() {
    let storage_dir = test_storage_dir("sos-delete-conversation");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let message = message(
        "sos-delete-message",
        "sos-thread",
        MessageDirection::Outbound {},
        "sos-thread",
        Some("LOCAL"),
        20,
    );
    let alert = sos_alert("incident-delete", "sospeer", "sos-thread", true, 21);
    let location = sos_location("incident-delete", "sospeer", 22);

    store.upsert_message(&message).expect("persist sos message");
    store.upsert_sos_alert(&alert).expect("persist sos alert");
    store
        .upsert_sos_location(&location)
        .expect("persist sos location");
    assert_eq!(store.list_sos_alerts().expect("list alerts").len(), 1);
    assert_eq!(store.list_sos_locations().expect("list locations").len(), 1);

    let invalidations = store
        .delete_conversation_resolved("sos-thread", &ConversationPeerResolver::default())
        .expect("delete sos conversation");

    assert!(invalidations
        .iter()
        .any(|invalidation| matches!(invalidation.scope, ProjectionScope::Sos {})));
    assert!(store
        .list_sos_alerts()
        .expect("list alerts after delete")
        .is_empty());
    assert!(store
        .list_sos_locations()
        .expect("list locations after delete")
        .is_empty());
}
