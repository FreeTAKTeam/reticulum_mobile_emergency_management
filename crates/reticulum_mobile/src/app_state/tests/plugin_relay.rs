#[test]
fn plugin_relay_ingress_requires_current_grant_and_uses_local_sequence() {
    let path = test_storage_dir("plugin-relay");
    let store = AppStateStore::new(path.to_str()).unwrap();
    let mut plugin = discovered_plugin(&"a".repeat(64));
    plugin.api_minor = 5;
    plugin.declared_capabilities.communications_relay = true;
    let id = plugin.plugin_id.clone();
    store.sync_discovered_plugins(&[plugin]).unwrap();
    assert!(store.poll_plugin_relay(&id, None, 16).is_err());
    store.approve_plugin_publisher(&id, None).unwrap();
    store
        .grant_plugin_capabilities(
            &id,
            PluginCapabilityRecord {
                communications_relay: true,
                ..Default::default()
            },
        )
        .unwrap();
    store.set_plugin_enabled(&id, true).unwrap();
    let source = "b".repeat(32);
    let position = serde_json::json!({"callsign":"REMOTE","lat":45,"lon":-63,"updatedAtMs":1});
    store
        .record_relay_ingress("position", &source, "p1", &position)
        .unwrap();
    let head = store.poll_plugin_relay(&id, None, 16).unwrap();
    assert_eq!(head["items"], serde_json::json!([]));
    let cursor = head["cursor"].as_u64().unwrap();
    store
        .record_relay_ingress(
            "text",
            &source,
            "m1",
            &serde_json::json!({"messageId":"c".repeat(64),"destinationHex":"d".repeat(32),"text":"native ingress"}),
        )
        .unwrap();
    store
        .record_relay_ingress(
            "text",
            &source,
            "m1",
            &serde_json::json!({"messageId":"c".repeat(64),"destinationHex":"d".repeat(32),"text":"native ingress"}),
        )
        .unwrap();
    let page = store.poll_plugin_relay(&id, Some(cursor), 16).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["source"], source);
    assert_eq!(page["items"][0]["kind"], "text");
    assert!(store
        .record_relay_ingress("position", "invalid", "p2", &position)
        .is_err());
    store
        .record_relay_ingress("position", &source, "p2", &position)
        .unwrap();
    if let Ok(output) = std::env::var("GOTENNA_RELAY_FIXTURE") {
        let output = std::path::Path::new(&output);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(
            output,
            store
                .poll_plugin_relay(&id, Some(cursor), 16)
                .unwrap()
                .to_string(),
        )
        .unwrap();
    }
    let reopened = AppStateStore::new(path.to_str()).unwrap();
    assert_eq!(
        reopened.poll_plugin_relay(&id, Some(cursor), 16).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let connection = reopened.connect().unwrap();
    connection
        .execute_batch(
            "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<2050)
        INSERT INTO plugin_relay_ingress(id,kind,source,received_at_ms,payload)
        SELECT 'retention-'||x,'text','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',1,'{}' FROM n",
        )
        .unwrap();
    reopened
        .record_relay_ingress("position", &source, "retention-last", &position)
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM plugin_relay_ingress", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        2048
    );
    assert_eq!(
        reopened.poll_plugin_relay(&id, Some(0), 16).unwrap()["gap"],
        true
    );
    reopened
        .grant_plugin_capabilities(&id, Default::default())
        .unwrap();
    assert!(reopened.poll_plugin_relay(&id, Some(cursor), 16).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn plugin_relay_authorization_is_separate_and_fails_closed() {
    let discovered = discovered_plugin(&"a".repeat(64));
    let mut plugin = InstalledPluginRecord {
        discovered,
        state: "Running".to_string(),
        trusted: true,
        enabled: true,
        granted_capabilities: Default::default(),
        diagnostic: None,
        updated_at_ms: 1,
    };
    plugin.discovered.api_minor = 5;
    plugin.discovered.declared_capabilities.communications_relay = true;
    plugin.granted_capabilities.lxmf_receive = true;
    assert!(!relay_allowed(&plugin));
    plugin.granted_capabilities.communications_relay = true;
    assert!(relay_allowed(&plugin));
    for state in ["Missing", "Incompatible", "Untrusted", "Disabled"] {
        plugin.state = state.to_string();
        assert!(!relay_allowed(&plugin));
    }
    plugin.state = "Running".to_string();
    plugin.discovered.api_minor = 4;
    assert!(!relay_allowed(&plugin));
    plugin.discovered.api_minor = 5;
    plugin.trusted = false;
    assert!(!relay_allowed(&plugin));
}

#[test]
fn plugin_relay_event_codes_preserve_severity_and_expand_legacy_compact_codes() {
    assert_eq!(
        relay_event_code("MECP/1/W02 notes").as_deref(),
        Some("MECP/1/W02")
    );
    assert_eq!(relay_event_code("P01 #A1").as_deref(), Some("MECP/2/P01"));
    assert_eq!(
        relay_event_code("MECP/2/P01+W02").as_deref(),
        Some("MECP/2/P01+W02")
    );
    assert!(relay_event_code("MECP/9/P01").is_none());
    assert!(relay_event_code("A human event label").is_none());
}
