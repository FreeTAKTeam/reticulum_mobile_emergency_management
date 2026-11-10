#[test]
fn sensor_connected_hub_queue_rechecks_grants_and_team_without_allowing_ordinary_positions() {
    let (node, writer) = setup();
    let id = origin(&node);
    let now = now_ms();
    node.sensor_geometry_operation(
        A,
        "map.origins.telemetry.upsert",
        &json!({"originId":id,"position":position(now)}),
    )
    .unwrap();
    node.sensor_geometry_operation(
        A,
        "map.sensors.upsert",
        &json!({"originId":id,"sensor":state("eo",now)}),
    )
    .unwrap();
    let hub = "ef".repeat(16);
    let config =
        super::tests::build_config_fingerprint_for_tests(HubMode::Connected {}, Some(&hub));
    let mut status = super::tests::build_status_for_tests();
    status.lxmf_destination_hex = writer;
    let mut directory = HubDirectorySnapshot::yellow_only(now);
    directory.schema_version = 1;
    let (tx, mut rx) = mpsc::channel(16);
    let store = {
        let mut inner = node.inner.lock().unwrap();
        inner
            .app_state
            .set_app_settings(&super::tests::build_app_settings())
            .unwrap();
        inner.active_config = Some(config.clone());
        inner.cmd_tx = Some(tx);
        *inner.status.lock().unwrap() = status.clone();
        *inner.hub_directory_snapshot.lock().unwrap() = Some(directory.clone());
        inner.app_state.clone()
    };
    let snapshot = sensor_snapshot(&store, store.sensor_origin(&id).unwrap().unwrap())
        .unwrap()
        .unwrap();
    let command = sensor_command(
        &snapshot,
        &hub,
        &sensor_scope(Some(&config), Some(&directory)),
        &sensor_route(&status, &[]).unwrap(),
    )
    .unwrap();
    let fields =
        sensor_fields(&snapshot, &command, true, active_team_uid(Some(&directory))).unwrap();
    assert!(routed_outbound_destination_hex(
        hub.clone(),
        OutboundTrafficClass::Telemetry {},
        Some(&config),
        Some(&directory),
        false,
        true
    )
    .is_err());
    node.queue_bytes_with_class(
        hub.clone(),
        b"R3S/1".to_vec(),
        Some(fields.clone()),
        SendMode::Auto {},
        OutboundTrafficClass::Telemetry {},
    )
    .unwrap();
    let Command::SendBytes {
        destination_hex, ..
    } = rx.try_recv().unwrap()
    else {
        panic!("sensor send");
    };
    assert_eq!(destination_hex, hub);
    node.grant_plugin_capabilities(
        A,
        PluginCapabilityRecord {
            sensor_geometry_share: false,
            ..capabilities()
        },
    )
    .unwrap();
    assert!(node
        .queue_bytes_with_class(
            hub.clone(),
            vec![],
            Some(fields.clone()),
            SendMode::Auto {},
            OutboundTrafficClass::Telemetry {}
        )
        .is_err());
    node.grant_plugin_capabilities(A, capabilities()).unwrap();
    directory.schema_version = HUB_DIRECTORY_SCHEMA_VERSION;
    *node
        .inner
        .lock()
        .unwrap()
        .hub_directory_snapshot
        .lock()
        .unwrap() = Some(directory);
    assert!(node
        .queue_bytes_with_class(
            hub,
            vec![],
            Some(fields),
            SendMode::Auto {},
            OutboundTrafficClass::Telemetry {}
        )
        .is_err());
}
