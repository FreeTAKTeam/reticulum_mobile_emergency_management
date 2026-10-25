// Tests for plugin_map_session_api.rs, included into its test module.

const RANGEFINDER: &str = "org.freetakteam.rem.plugin.rangefinder";
const OTHER: &str = "org.freetakteam.rem.plugin.other";
const LEGACY: &str = "org.freetakteam.rem.plugin.legacy";

fn capabilities() -> PluginCapabilityRecord {
    PluginCapabilityRecord {
        map_markers_read: true,
        map_markers_write: true,
        map_observer_read: true,
        map_selection_read: true,
        ..PluginCapabilityRecord::default()
    }
}

fn discovered(plugin_id: &str, api_minor: u16) -> DiscoveredPluginRecord {
    DiscoveredPluginRecord {
        plugin_id: plugin_id.to_string(),
        display_name: plugin_id.to_string(),
        version: "1.0.0".to_string(),
        api_major: 1,
        api_minor,
        package_name: plugin_id.to_string(),
        service_class_name: ".MapPluginService".to_string(),
        publisher_fingerprint: "cd".repeat(32),
        publisher_history: Vec::new(),
        android_permissions: Vec::new(),
        declared_capabilities: capabilities(),
        messages: Vec::new(),
        configuration_entrypoint: None,
    }
}

fn node(name: &str) -> Node {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "rem-plugin-map-session-{name}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let node = Node::with_storage_dir(Some(dir.to_string_lossy().as_ref())).expect("node");
    let plugins = [
        (RANGEFINDER, 3),
        (OTHER, 3),
        (LEGACY, 2),
        ("org.example.future", 4),
    ];
    node.sync_discovered_plugins(
        plugins
            .iter()
            .map(|(id, minor)| discovered(id, *minor))
            .collect(),
    )
    .expect("discover");
    // One publisher signs every test plugin, so one approval trusts them all.
    node.approve_plugin_publisher(RANGEFINDER, Some("Test"))
        .expect("approve");
    for (id, _) in &plugins[..3] {
        node.grant_plugin_capabilities(id, capabilities())
            .expect("grant");
        node.set_plugin_enabled(id, true).expect("enable");
    }
    node
}

fn call(node: &Node, plugin_id: &str, operation: &str, payload: &Value) -> PluginMapResult {
    handle_plugin_map_operation(node, plugin_id, operation, payload).expect("map operation")
}

fn operator_marker(node: &Node, name: &str) -> String {
    node.create_map_marker(crate::types::MapMarkerCreateRequest {
        idempotency_key: None,
        marker_type: "medic".to_string(),
        symbol: "medic".to_string(),
        name: Some(name.to_string()),
        category: None,
        lat: 45.0,
        lon: 7.0,
        notes: None,
    })
    .expect("operator marker")
    .marker
    .object_destination_hash
}

fn drained(node: &Node) -> Vec<(String, Value)> {
    node.drain_plugin_map_session_events(64).expect("drain")
}

#[test]
fn fixture_flow_creates_a_marker_and_reads_only_selected_objects() {
    let node = node("fixture");
    let symbols = call(&node, RANGEFINDER, "map.symbols.list", &json!({})).expect("symbols");
    assert!(!symbols["items"].as_array().expect("catalog").is_empty());
    let created = call(
        &node,
        RANGEFINDER,
        "map.markers.create",
        &json!({ "idempotencyKey": "fix-1", "symbol": "hostile", "name": "Target", "lat": 45.2, "lon": 7.7 }),
    )
    .expect("create");
    let own = created["marker"]["object_destination_hash"]
        .as_str()
        .expect("id")
        .to_string();
    let selected = operator_marker(&node, "Selected by operator");
    let hidden = operator_marker(&node, "Not selected");

    let no_session = call(&node, RANGEFINDER, "map.session.list", &json!({})).expect("list");
    assert_eq!(no_session["items"], json!([]));
    let session = node
        .start_plugin_map_session(RANGEFINDER, std::slice::from_ref(&selected))
        .expect("operator start");
    let events = drained(&node);
    assert_eq!(events[0].0, RANGEFINDER);
    assert_eq!(events[0].1["event"], "map.session.started");

    let read = call(
        &node,
        RANGEFINDER,
        "map.session.objects.get",
        &json!({ "sessionId": session.session_id }),
    )
    .expect("objects");
    let ids = read["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .map(|object| {
            object["object_destination_hash"]
                .as_str()
                .expect("id")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![selected.clone()]);
    let unselected = call(
        &node,
        RANGEFINDER,
        "map.markers.get",
        &json!({ "objectDestinationHash": hidden }),
    )
    .expect_err("unselected operator marker");
    assert_eq!(unselected.0, "NotFound");
    let selected_write = call(
        &node,
        RANGEFINDER,
        "map.markers.delete",
        &json!({ "objectDestinationHash": selected }),
    )
    .expect_err("selection grants read only");
    assert_eq!(selected_write.0, "NotFound");
    assert!(call(
        &node,
        RANGEFINDER,
        "map.markers.get",
        &json!({ "objectDestinationHash": own })
    )
    .is_ok());

    let foreign = call(
        &node,
        OTHER,
        "map.session.objects.get",
        &json!({ "sessionId": session.session_id }),
    )
    .expect_err("other plugin");
    assert_eq!(foreign.0, "NotFound");

    node.rename_map_marker(crate::types::MapMarkerRenameRequest {
        object_destination_hash: selected.clone(),
        name: "Moved on".to_string(),
    })
    .expect("operator rename");
    let events = drained(&node);
    assert_eq!(
        events.len(),
        1,
        "only the session that selected the object hears about it"
    );
    assert_eq!(events[0].1["event"], "map.session.object.updated");
    node.move_map_marker(crate::types::MapMarkerMoveRequest {
        object_destination_hash: hidden,
        lat: 1.0,
        lon: 1.0,
    })
    .expect("move hidden");
    assert!(drained(&node).is_empty());
}

#[test]
fn sessions_stop_from_operator_plugin_revocation_and_binder_death() {
    let node = node("stop");
    let marker = operator_marker(&node, "Target");
    let first = node
        .start_plugin_map_session(RANGEFINDER, std::slice::from_ref(&marker))
        .expect("start");
    drained(&node);

    // Operator stop from the REM UI notifies the plugin.
    assert!(node
        .stop_plugin_map_sessions(None, Some(&first.session_id), "operator", true)
        .expect("operator stop"));
    let events = drained(&node);
    assert_eq!(events[0].1["event"], "map.session.stopped");
    assert_eq!(events[0].1["payload"]["reason"], "operator");
    let gone = call(
        &node,
        RANGEFINDER,
        "map.session.objects.get",
        &json!({ "sessionId": first.session_id }),
    )
    .expect_err("stopped");
    assert_eq!(gone.0, "NotFound");

    // The plugin stops its own session.
    let second = node
        .start_plugin_map_session(RANGEFINDER, std::slice::from_ref(&marker))
        .expect("start");
    drained(&node);
    let stopped = call(
        &node,
        RANGEFINDER,
        "map.session.stop",
        &json!({ "sessionId": second.session_id }),
    )
    .expect("plugin stop");
    assert_eq!(stopped["stopped"], true);
    assert!(
        drained(&node).is_empty(),
        "the plugin is not told about its own stop"
    );

    // Revoking mapSelectionRead ends the session and denies further reads.
    let third = node
        .start_plugin_map_session(RANGEFINDER, std::slice::from_ref(&marker))
        .expect("start");
    drained(&node);
    node.grant_plugin_capabilities(
        RANGEFINDER,
        PluginCapabilityRecord {
            map_markers_read: true,
            ..PluginCapabilityRecord::default()
        },
    )
    .expect("revoke");
    let events = drained(&node);
    assert_eq!(events[0].1["payload"]["reason"], "capability-revoked");
    let denied = call(
        &node,
        RANGEFINDER,
        "map.session.objects.get",
        &json!({ "sessionId": third.session_id }),
    )
    .expect_err("revoked");
    assert_eq!(denied.0, "PermissionDenied");
    node.grant_plugin_capabilities(RANGEFINDER, capabilities())
        .expect("regrant");

    // Binder death: the host ends sessions without queueing events for a dead connection.
    node.start_plugin_map_session(RANGEFINDER, std::slice::from_ref(&marker))
        .expect("start");
    node.start_plugin_map_session(OTHER, &[marker])
        .expect("other start");
    drained(&node);
    assert!(node
        .stop_plugin_map_sessions(Some(RANGEFINDER), None, "binder-died", false)
        .expect("death"));
    assert!(drained(&node).is_empty());
    let remaining = node.list_plugin_map_sessions(None).expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].plugin_id, OTHER,
        "other plugins keep their sessions"
    );

    // Disabling or uninstalling ends the rest.
    node.set_plugin_enabled(OTHER, false).expect("disable");
    assert_eq!(drained(&node)[0].1["payload"]["reason"], "plugin-disabled");
    assert!(node
        .list_plugin_map_sessions(None)
        .expect("list")
        .is_empty());
}

#[test]
fn session_operations_negotiate_api_1_3_and_capability() {
    let node = node("negotiation");
    let marker = operator_marker(&node, "Target");
    let legacy = call(&node, LEGACY, "map.session.list", &json!({})).expect_err("api 1.2");
    assert_eq!(legacy.0, "PermissionDenied");
    assert!(
        call(&node, LEGACY, "map.symbols.list", &json!({})).is_ok(),
        "1.2 operations still work"
    );
    assert!(node
        .start_plugin_map_session(LEGACY, std::slice::from_ref(&marker))
        .is_err());

    let future = node
        .list_plugins()
        .expect("plugins")
        .into_iter()
        .find(|plugin| plugin.discovered.plugin_id == "org.example.future")
        .expect("future plugin");
    assert_eq!(
        future.state, "Incompatible",
        "API 1.4 is newer than this host"
    );
    assert!(node
        .start_plugin_map_session("org.example.future", &[marker])
        .is_err());

    let missing =
        call(&node, RANGEFINDER, "map.session.objects.get", &json!({})).expect_err("payload");
    assert_eq!(
        missing,
        ("InvalidRequest", "sessionId is required".to_string())
    );
}

#[test]
fn oversize_responses_become_a_typed_error() {
    let response = plugin_host_oversize_response("req-1");
    assert_eq!(response["ok"], false);
    assert_eq!(response["requestId"], "req-1");
    assert_eq!(response["error"]["code"], "ResponseTooLarge");
    assert!(plugin_host_response_within_limit(&response));
    assert!(!plugin_host_response_within_limit(
        &json!({ "value": "x".repeat(65_536) })
    ));
}
