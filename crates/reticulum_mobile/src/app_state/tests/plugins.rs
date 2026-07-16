#[test]
fn plugin_trust_enablement_and_sensors_persist() {
    let storage_dir = test_storage_dir("plugin-state");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let fingerprint = "ab".repeat(32);
    store
        .sync_discovered_plugins(&[discovered_plugin(fingerprint.as_str())])
        .expect("discover plugin");
    let plugin = store
        .get_plugin("org.freetakteam.rem.plugin.test")
        .expect("plugin lookup")
        .expect("plugin");
    assert_eq!(plugin.state, "Untrusted");

    store
        .approve_plugin_publisher("org.freetakteam.rem.plugin.test", Some("Test Publisher"))
        .expect("approve publisher");
    store
        .grant_plugin_capabilities(
            "org.freetakteam.rem.plugin.test",
            PluginCapabilityRecord {
                sensors_publish: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant sensors");
    store
        .set_plugin_enabled("org.freetakteam.rem.plugin.test", true)
        .expect("enable plugin");
    let (sensor, _) = store
        .record_plugin_sensor(
            "org.freetakteam.rem.plugin.test",
            PluginSensorSampleRequest {
                device_id: "sensor-1".to_string(),
                sensor_type: "heart_rate_bpm".to_string(),
                display_name: "Operator HR".to_string(),
                value: json!(82),
                unit: Some("bpm".to_string()),
                operator_rns_identity: Some("aabb".to_string()),
                confidence: Some(0.95),
                connection_state: Some("SUBSCRIBED".to_string()),
                timestamp_ms: now_ms(),
                stale_after_ms: 30_000,
                origin: "local".to_string(),
            },
        )
        .expect("record sensor");
    assert_eq!(sensor.status, "Active");

    let restored = AppStateStore::new(storage_dir.to_str()).expect("restored store");
    assert!(
        restored
            .get_plugin("org.freetakteam.rem.plugin.test")
            .expect("restored plugin")
            .expect("plugin")
            .enabled
    );
    assert_eq!(restored.list_plugin_sensors().expect("sensors").len(), 1);
}

#[test]
fn plugin_operational_read_defaults_false_and_is_part_of_capability_subset() {
    let legacy: PluginCapabilityRecord = serde_json::from_value(json!({
        "eventsPublish": true
    }))
    .expect("legacy capabilities");
    assert!(!legacy.operational_read);

    let requested = PluginCapabilityRecord {
        operational_read: true,
        ..PluginCapabilityRecord::default()
    };
    assert!(!requested.is_subset_of(&PluginCapabilityRecord::default()));
    assert!(requested.is_subset_of(&PluginCapabilityRecord {
        operational_read: true,
        ..PluginCapabilityRecord::default()
    }));
}

#[test]
fn plugin_operational_read_grant_persists() {
    let storage_dir = test_storage_dir("plugin-operational-read-persistence");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let mut discovered = discovered_plugin("ab".repeat(32).as_str());
    discovered.api_minor = 1;
    discovered.declared_capabilities.operational_read = true;
    store
        .sync_discovered_plugins(&[discovered])
        .expect("discover plugin");
    store
        .approve_plugin_publisher("org.freetakteam.rem.plugin.test", None)
        .expect("approve publisher");
    store
        .grant_plugin_capabilities(
            "org.freetakteam.rem.plugin.test",
            PluginCapabilityRecord {
                operational_read: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant operational read");

    let restored = AppStateStore::new(storage_dir.to_str()).expect("restored store");
    let plugin = restored
        .get_plugin("org.freetakteam.rem.plugin.test")
        .expect("restored plugin lookup")
        .expect("restored plugin");
    assert!(plugin.discovered.declared_capabilities.operational_read);
    assert!(plugin.granted_capabilities.operational_read);
}

#[test]
fn plugin_api_minor_newer_than_host_is_incompatible() {
    let storage_dir = test_storage_dir("plugin-api-minor");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let mut plugin = discovered_plugin("ef".repeat(32).as_str());
    plugin.api_minor = 2;
    store.sync_discovered_plugins(&[plugin]).expect("discover plugin");
    let stored = store
        .get_plugin("org.freetakteam.rem.plugin.test")
        .expect("plugin lookup")
        .expect("plugin");
    assert_eq!(stored.state, "Incompatible");
    assert!(!stored.enabled);
}

#[test]
fn plugin_sensor_status_transitions_from_active_to_stale_and_offline() {
    assert_eq!(
        sensor_status(Some("SUBSCRIBED"), 1_000, 30_000, 30_000),
        "Active"
    );
    assert_eq!(
        sensor_status(Some("SUBSCRIBED"), 1_000, 30_000, 31_001),
        "Stale"
    );
    assert_eq!(
        sensor_status(Some("SUBSCRIBED"), 1_000, 30_000, 61_001),
        "Offline"
    );
    assert_eq!(
        sensor_status(Some("DISCONNECTED"), 60_000, 30_000, 60_001),
        "Offline"
    );
}

#[test]
fn plugin_publisher_approval_applies_to_every_matching_package() {
    let storage_dir = test_storage_dir("plugin-publisher-scope");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let fingerprint = "cd".repeat(32);
    let first = discovered_plugin(fingerprint.as_str());
    let mut second = first.clone();
    second.plugin_id = "org.freetakteam.rem.plugin.second".to_string();
    second.package_name = "org.freetakteam.rem.plugin.second".to_string();
    store
        .sync_discovered_plugins(&[first, second])
        .expect("discover publisher plugins");
    store
        .approve_plugin_publisher("org.freetakteam.rem.plugin.test", None)
        .expect("approve publisher");
    let plugins = store.list_plugins().expect("plugins");
    assert_eq!(plugins.len(), 2);
    assert!(plugins.iter().all(|plugin| plugin.trusted));
    assert!(plugins.iter().all(|plugin| !plugin.enabled));
}

#[test]
fn plugin_certificate_change_revokes_enablement_and_grants() {
    let storage_dir = test_storage_dir("plugin-certificate-change");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let first = "ab".repeat(32);
    store
        .sync_discovered_plugins(&[discovered_plugin(first.as_str())])
        .expect("discover plugin");
    store
        .approve_plugin_publisher("org.freetakteam.rem.plugin.test", None)
        .expect("approve publisher");
    store
        .grant_plugin_capabilities(
            "org.freetakteam.rem.plugin.test",
            PluginCapabilityRecord {
                sensors_publish: true,
                ..PluginCapabilityRecord::default()
            },
        )
        .expect("grant sensors");
    store
        .set_plugin_enabled("org.freetakteam.rem.plugin.test", true)
        .expect("enable plugin");

    let second = "cd".repeat(32);
    store
        .sync_discovered_plugins(&[discovered_plugin(second.as_str())])
        .expect("replace signer");
    let plugin = store
        .get_plugin("org.freetakteam.rem.plugin.test")
        .expect("plugin lookup")
        .expect("plugin");
    assert!(!plugin.trusted);
    assert!(!plugin.enabled);
    assert_eq!(plugin.state, "Untrusted");
    assert_eq!(
        plugin.granted_capabilities,
        PluginCapabilityRecord::default()
    );
}

#[test]
fn plugin_signing_lineage_preserves_trust_and_revokes_as_one_publisher() {
    let storage_dir = test_storage_dir("plugin-signing-lineage");
    let store = AppStateStore::new(storage_dir.to_str()).expect("store");
    let old_fingerprint = "ab".repeat(32);
    store
        .sync_discovered_plugins(&[discovered_plugin(old_fingerprint.as_str())])
        .expect("discover old signer");
    store
        .approve_plugin_publisher("org.freetakteam.rem.plugin.test", None)
        .expect("approve old signer");

    let new_fingerprint = "cd".repeat(32);
    let mut rotated = discovered_plugin(new_fingerprint.as_str());
    rotated.publisher_history = vec![old_fingerprint.clone()];
    store
        .sync_discovered_plugins(&[rotated])
        .expect("discover rotated signer");
    assert!(
        store
            .get_plugin("org.freetakteam.rem.plugin.test")
            .expect("plugin lookup")
            .expect("plugin")
            .trusted
    );

    store
        .revoke_plugin_publisher(new_fingerprint.as_str())
        .expect("revoke signer lineage");
    assert!(store
        .list_trusted_plugin_publishers()
        .expect("publishers")
        .is_empty());
    store
        .sync_discovered_plugins(&[{
            let mut plugin = discovered_plugin(new_fingerprint.as_str());
            plugin.publisher_history = vec![old_fingerprint];
            plugin
        }])
        .expect("rediscover revoked lineage");
    assert!(
        !store
            .get_plugin("org.freetakteam.rem.plugin.test")
            .expect("plugin lookup")
            .expect("plugin")
            .trusted
    );
}

fn readiness_eam(
    callsign: &str,
    statuses: [&str; 6],
    updated_at_ms: u64,
    deleted_at_ms: Option<u64>,
) -> EamProjectionRecord {
    EamProjectionRecord {
        callsign: callsign.to_string(),
        group_name: "Yellow".to_string(),
        security_status: statuses[0].to_string(),
        capability_status: statuses[1].to_string(),
        preparedness_status: statuses[2].to_string(),
        medical_status: statuses[3].to_string(),
        mobility_status: statuses[4].to_string(),
        comms_status: statuses[5].to_string(),
        notes: None,
        updated_at_ms,
        deleted_at_ms,
        eam_uid: None,
        team_member_uid: None,
        team_uid: Some("team-yellow".to_string()),
        reported_at: None,
        reported_by: None,
        overall_status: None,
        confidence: None,
        ttl_seconds: None,
        source: None,
        sync_state: None,
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: None,
    }
}
