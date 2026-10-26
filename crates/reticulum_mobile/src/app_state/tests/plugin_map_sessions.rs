const SESSION_PLUGIN: &str = "org.freetakteam.rem.plugin.test";

fn session_plugin(fingerprint: &str, api_minor: u16) -> DiscoveredPluginRecord {
    let mut plugin = discovered_plugin(fingerprint);
    plugin.api_minor = api_minor;
    plugin.declared_capabilities = PluginCapabilityRecord {
        map_markers_write: true,
        map_selection_read: true,
        ..PluginCapabilityRecord::default()
    };
    plugin
}

fn session_store(name: &str) -> AppStateStore {
    let store = AppStateStore::new(test_storage_dir(name).to_str()).expect("store");
    store
        .sync_discovered_plugins(&[session_plugin(&"ab".repeat(32), 3)])
        .expect("discover");
    store
        .approve_plugin_publisher(SESSION_PLUGIN, Some("Test"))
        .expect("approve");
    store
        .grant_plugin_capabilities(
            SESSION_PLUGIN,
            PluginCapabilityRecord {
                map_markers_write: true,
                map_selection_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant");
    store
        .set_plugin_enabled(SESSION_PLUGIN, true)
        .expect("enable");
    store
}

fn session_marker(store: &AppStateStore, name: &str) -> String {
    let (created, _) = store
        .create_map_marker(&crate::types::MapMarkerCreateRequest {
            idempotency_key: None,
            marker_type: "medic".to_string(),
            symbol: "medic".to_string(),
            name: Some(name.to_string()),
            category: None,
            lat: 45.0,
            lon: 7.0,
            notes: None,
        })
        .expect("marker");
    created.marker.object_destination_hash
}

fn drained_events(store: &AppStateStore) -> Vec<(String, String)> {
    store
        .drain_plugin_map_session_events(64)
        .expect("drain")
        .into_iter()
        .map(|(plugin_id, event)| {
            (
                plugin_id,
                event["event"].as_str().expect("name").to_string(),
            )
        })
        .collect()
}

#[test]
fn plugin_map_session_start_is_bounded_and_queues_selected_objects_only() {
    let store = session_store("map-session-start");
    let selected = session_marker(&store, "Selected");
    let other = session_marker(&store, "Not selected");

    let (session, invalidation) = store
        .start_plugin_map_session(SESSION_PLUGIN, &[selected.to_uppercase(), selected.clone()])
        .expect("start");
    assert_eq!(invalidation.scope, ProjectionScope::Plugins {});
    assert_eq!(
        session.object_destination_hashes,
        vec![selected.clone()],
        "ids are normalized and deduplicated"
    );
    assert_eq!(session.started_by, "operator");

    let events = store.drain_plugin_map_session_events(64).expect("drain");
    assert_eq!(events.len(), 1);
    let (plugin_id, started) = &events[0];
    assert_eq!(plugin_id, SESSION_PLUGIN);
    assert_eq!(started["event"], "map.session.started");
    let objects = started["payload"]["objects"].as_array().expect("objects");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0]["object_destination_hash"], selected.as_str());
    assert!(
        !started.to_string().contains(&other),
        "unselected objects never reach the plugin"
    );
    assert!(store
        .drain_plugin_map_session_events(64)
        .expect("drain again")
        .is_empty());

    let too_many = (0..=PLUGIN_MAP_SESSION_MAX_OBJECTS)
        .map(|index| format!("{index:032x}"))
        .collect::<Vec<_>>();
    assert!(store
        .start_plugin_map_session(SESSION_PLUGIN, &too_many)
        .is_err());
    assert!(store.start_plugin_map_session(SESSION_PLUGIN, &[]).is_err());
    assert!(
        store
            .start_plugin_map_session(
                SESSION_PLUGIN,
                &["0123456789abcdef0123456789abcdef".to_string()]
            )
            .is_err(),
        "unknown objects cannot be selected"
    );
    for _ in 1..PLUGIN_MAP_SESSION_MAX_PER_PLUGIN {
        store
            .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&other))
            .expect("more sessions");
    }
    assert!(
        store
            .start_plugin_map_session(SESSION_PLUGIN, &[other])
            .is_err(),
        "per-plugin session cap"
    );
}

#[test]
fn plugin_map_session_requires_api_1_3_and_the_selection_capability() {
    let store = session_store("map-session-denied");
    let marker = session_marker(&store, "Target");
    store
        .grant_plugin_capabilities(
            SESSION_PLUGIN,
            PluginCapabilityRecord {
                map_markers_write: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant without selection");
    assert!(store
        .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
        .is_err());

    store
        .sync_discovered_plugins(&[session_plugin(&"ab".repeat(32), 2)])
        .expect("downgrade");
    store
        .grant_plugin_capabilities(
            SESSION_PLUGIN,
            PluginCapabilityRecord {
                map_selection_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant");
    assert!(
        store
            .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
            .is_err(),
        "API 1.2 plugin"
    );
    assert!(store
        .start_plugin_map_session("org.example.unknown", &[marker])
        .is_err());
}

#[test]
fn plugin_map_sessions_end_when_capability_is_revoked_or_plugin_disabled() {
    let store = session_store("map-session-revoke");
    let marker = session_marker(&store, "Target");
    store
        .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
        .expect("start");
    drained_events(&store);
    assert!(store
        .reconcile_plugin_map_sessions()
        .expect("no-op")
        .is_none());

    store
        .grant_plugin_capabilities(SESSION_PLUGIN, PluginCapabilityRecord::default())
        .expect("revoke");
    assert!(store
        .reconcile_plugin_map_sessions()
        .expect("reconcile")
        .is_some());
    assert!(store
        .list_plugin_map_sessions(None)
        .expect("list")
        .is_empty());
    let events = store.drain_plugin_map_session_events(8).expect("drain");
    assert_eq!(events[0].1["event"], "map.session.stopped");
    assert_eq!(events[0].1["payload"]["reason"], "capability-revoked");

    store
        .grant_plugin_capabilities(
            SESSION_PLUGIN,
            PluginCapabilityRecord {
                map_selection_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("regrant");
    store
        .start_plugin_map_session(SESSION_PLUGIN, &[marker])
        .expect("restart");
    store
        .set_plugin_enabled(SESSION_PLUGIN, false)
        .expect("disable");
    store.reconcile_plugin_map_sessions().expect("reconcile");
    let events = store.drain_plugin_map_session_events(8).expect("drain");
    assert_eq!(
        events.last().expect("stop").1["payload"]["reason"],
        "plugin-disabled"
    );
}

#[test]
fn plugin_map_session_follows_selected_object_changes_and_deletion() {
    let store = session_store("map-session-changes");
    let first = session_marker(&store, "First");
    let second = session_marker(&store, "Second");
    let unrelated = session_marker(&store, "Unrelated");
    let (session, _) = store
        .start_plugin_map_session(SESSION_PLUGIN, &[first.clone(), second.clone()])
        .expect("start");
    drained_events(&store);

    store
        .queue_plugin_map_object_change(&unrelated, "marker-moved")
        .expect("unrelated");
    assert!(
        drained_events(&store).is_empty(),
        "unselected objects raise no events"
    );

    store
        .rename_map_marker(&crate::types::MapMarkerRenameRequest {
            object_destination_hash: first.clone(),
            name: "Renamed".to_string(),
        })
        .expect("rename");
    assert!(store
        .queue_plugin_map_object_change(&first, "marker-renamed")
        .expect("update")
        .is_none());
    let events = store.drain_plugin_map_session_events(8).expect("drain");
    assert_eq!(events[0].1["event"], "map.session.object.updated");
    assert_eq!(events[0].1["payload"]["object"]["name"], "Renamed");

    store.delete_map_marker(&first).expect("delete");
    assert!(store
        .queue_plugin_map_object_change(&first, "marker-deleted")
        .expect("removed")
        .is_some());
    assert_eq!(
        drained_events(&store),
        vec![(
            SESSION_PLUGIN.to_string(),
            "map.session.object.removed".to_string()
        )]
    );
    let remaining = store
        .plugin_map_session(SESSION_PLUGIN, &session.session_id)
        .expect("get")
        .expect("alive");
    assert_eq!(remaining.object_destination_hashes, vec![second.clone()]);

    store.delete_map_marker(&second).expect("delete");
    store
        .queue_plugin_map_object_change(&second, "marker-deleted")
        .expect("last removed");
    let names = drained_events(&store)
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["map.session.object.removed", "map.session.stopped"]
    );
    assert!(store
        .list_plugin_map_sessions(None)
        .expect("list")
        .is_empty());
}

#[test]
fn plugin_map_sessions_do_not_survive_restart_and_host_stop_can_skip_notification() {
    let storage_dir = test_storage_dir("map-session-restart");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    store
        .sync_discovered_plugins(&[session_plugin(&"ab".repeat(32), 3)])
        .expect("discover");
    store
        .approve_plugin_publisher(SESSION_PLUGIN, Some("Test"))
        .expect("approve");
    store
        .grant_plugin_capabilities(
            SESSION_PLUGIN,
            PluginCapabilityRecord {
                map_selection_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant");
    store
        .set_plugin_enabled(SESSION_PLUGIN, true)
        .expect("enable");
    let marker = session_marker(&store, "Target");
    let (session, _) = store
        .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
        .expect("start");
    drained_events(&store);

    // Binder death: the connection is gone, so no event is queued for it.
    assert!(store
        .stop_plugin_map_sessions(Some(SESSION_PLUGIN), None, "binder-died", false)
        .expect("stop")
        .is_some());
    assert!(drained_events(&store).is_empty());
    assert!(store
        .plugin_map_session(SESSION_PLUGIN, &session.session_id)
        .expect("get")
        .is_none());
    assert!(store
        .stop_plugin_map_sessions(None, Some(&session.session_id), "operator", true)
        .expect("stop again")
        .is_none());

    store
        .start_plugin_map_session(SESSION_PLUGIN, &[marker])
        .expect("start again");
    let reopened = AppStateStore::new(storage_dir.to_str()).expect("reopen");
    assert!(reopened
        .list_plugin_map_sessions(None)
        .expect("list")
        .is_empty());
    assert!(reopened
        .drain_plugin_map_session_events(8)
        .expect("drain")
        .is_empty());
}

#[test]
fn plugin_map_session_event_backlog_is_bounded() {
    let store = session_store("map-session-backlog");
    let marker = session_marker(&store, "Target");
    store
        .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
        .expect("start");
    for _ in 0..300 {
        store
            .queue_plugin_map_object_change(&marker, "marker-moved")
            .expect("queue");
    }
    let events = store.drain_plugin_map_session_events(1_000).expect("drain");
    assert!(events.len() <= 257, "backlog kept {}", events.len());
    assert_eq!(
        events.last().expect("newest").1["event"],
        "map.session.object.updated"
    );
}

#[test]
fn largest_plugin_map_session_event_fits_the_binder_limit() {
    let store = session_store("map-session-size");
    let long = "\u{00e9}".repeat(4_000);
    let ids = (0..PLUGIN_MAP_SESSION_MAX_OBJECTS)
        .map(|index| {
            let id = format!("{index:032x}");
            store
                .apply_map_marker_replica(&serde_json::json!({
                    "object_destination_hash": id,
                    "origin_rch": long,
                    "type": "medic",
                    "symbol": "medic",
                    "name": long,
                    "category": long,
                    "notes": long,
                    "position": { "lat": 10.5, "lon": 20.25 },
                    "created_at": "2026-10-13T10:00:00Z",
                    "updated_at": "2026-10-13T10:00:00Z",
                }))
                .expect("replica");
            id
        })
        .collect::<Vec<_>>();
    let (session, _) = store
        .start_plugin_map_session(SESSION_PLUGIN, &ids)
        .expect("start");
    let events = store.drain_plugin_map_session_events(4).expect("drain");
    let started = serde_json::to_vec(&events[0].1).expect("encode");
    assert!(
        started.len() < 65_536,
        "started event is {} bytes",
        started.len()
    );
    assert_eq!(events[0].1["payload"]["objects"][0]["textTruncated"], true);
    let objects = store.plugin_map_session_objects(&session).expect("objects");
    let response =
        serde_json::to_vec(&serde_json::json!({ "session": session, "objects": objects }))
            .expect("encode");
    assert!(
        response.len() < 60_000,
        "objects response is {} bytes",
        response.len()
    );
}

#[test]
fn plugin_removal_keeps_shared_markers_and_a_new_signer_loses_ownership() {
    let store = session_store("map-session-removal");
    let marker = session_marker(&store, "Plugin marker");
    store
        .record_plugin_map_marker(SESSION_PLUGIN, &marker)
        .expect("own");
    store
        .start_plugin_map_session(SESSION_PLUGIN, std::slice::from_ref(&marker))
        .expect("start");

    store.sync_discovered_plugins(&[]).expect("uninstall");
    assert_eq!(
        store
            .get_plugin(SESSION_PLUGIN)
            .expect("get")
            .expect("record")
            .state,
        "Missing"
    );
    store.reconcile_plugin_map_sessions().expect("reconcile");
    assert!(store
        .list_plugin_map_sessions(None)
        .expect("list")
        .is_empty());
    assert!(
        store.get_map_marker(&marker).expect("get").is_some(),
        "removal never deletes shared markers"
    );
    assert!(
        store
            .plugin_owns_map_marker(SESSION_PLUGIN, &marker)
            .expect("owned"),
        "same signer may return"
    );

    store
        .sync_discovered_plugins(&[session_plugin(&"cd".repeat(32), 3)])
        .expect("other signer");
    assert!(!store
        .plugin_owns_map_marker(SESSION_PLUGIN, &marker)
        .expect("owned"));
    assert!(store.get_map_marker(&marker).expect("get").is_some());
}
