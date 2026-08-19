include!("runtime/core.rs");
include!("runtime/operational_ack.rs");
include!("runtime/eam_parsing.rs");
include!("runtime/eam_persistence.rs");
include!("runtime/checklist_merge.rs");
include!("runtime/checklist_wire.rs");
include!("runtime/checklist_dispatch.rs");
include!("runtime/mission_wire.rs");
include!("runtime/interfaces.rs");
include!("runtime/sdk_conversions.rs");
include!("runtime/scheduler.rs");
include!("runtime/delivery_events.rs");
include!("runtime/projections.rs");
include!("runtime/delivery_health.rs");
include!("runtime/destination_resolution.rs");
include!("runtime/propagation.rs");
include!("runtime/messages_commands.rs");
include!("runtime/managed_links.rs");
include!("runtime/ack_tracking.rs");
include!("runtime/ack_retries.rs");
include!("runtime/delivery_send.rs");
include!("runtime/receive.rs");
include!("runtime/ack_handlers.rs");
include!("runtime/hub_team_selection.rs");
include!("runtime/hub.rs");
include!("runtime/rnode_status.rs");
include!("runtime/interface_config.rs");
include!("runtime/network_interfaces.rs");
include!("runtime/background_maintenance.rs");
include!("runtime/background_announces.rs");
include!("runtime/background_receivers.rs");
include!("runtime/background_links.rs");
include!("runtime/command_executor.rs");
include!("runtime/command_send_bytes.rs");
include!("runtime/command_tasks.rs");
include!("runtime/command_queries.rs");

#[expect(
    clippy::too_many_arguments,
    reason = "runtime entrypoint receives independently owned state handles and command lanes"
)]
pub async fn run_node(
    config: NodeConfig,
    identity: PrivateIdentity,
    app_state: AppStateStore,
    status: Arc<Mutex<NodeStatus>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    hub_directory_snapshot: Arc<Mutex<Option<HubDirectorySnapshot>>>,
    bus: EventBus,
    mut cmd_rx: mpsc::Receiver<Command>,
    mut priority_cmd_rx: mpsc::Receiver<Command>,
) {
    let mut transport_cfg = TransportConfig::new(config.name.clone(), &identity, config.broadcast);
    transport_cfg.set_retransmit(config.transport_node_enabled);
    if config.rnode.enabled {
        transport_cfg.set_resource_retry_interval_secs(RNODE_BLE_RESOURCE_RETRY_INTERVAL_SECS);
        transport_cfg.set_resource_retry_limit(RNODE_BLE_RESOURCE_RETRY_LIMIT);
    }

    if let Some(dir) = config
        .storage_dir
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let mut path = PathBuf::from(dir);
        path.push("ratchets.dat");
        transport_cfg.set_ratchet_store_path(path);
    }
    let mut transport = Transport::new(transport_cfg);
    let receipt_message_ids =
        Arc::new(Mutex::new(HashMap::<String, ReceiptMessageTracking>::new()));
    let (receipt_tx, receipt_rx) = mpsc::unbounded_channel::<String>();
    let receipt_tracker = ReceiptTracker {
        receipt_message_ids: receipt_message_ids.clone(),
        tx: receipt_tx,
    };
    transport
        .set_receipt_handler(Box::new(RuntimeReceiptBridge {
            tracker: receipt_tracker.clone(),
        }))
        .await;

    let app_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .await;
    let lxmf_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .await;
    lxmf_destination
        .lock()
        .await
        .set_proof_strategy(ProofStrategy::All);

    let transport = Arc::new(transport);
    let active_interface_registry: ActiveInterfaceRegistry =
        Arc::new(TokioMutex::new(HashMap::new()));
    spawn_interface_traffic_monitor(
        transport.clone(),
        active_interface_registry.clone(),
        status.clone(),
        bus.clone(),
    );
    let tcp_client_endpoints = configured_tcp_client_endpoints(config.tcp_clients.as_slice());
    for endpoint in tcp_client_endpoints.iter().cloned() {
        spawn_tcp_client_interface_manager(
            transport.clone(),
            endpoint,
            active_interface_registry.clone(),
            status.clone(),
            bus.clone(),
        );
    }
    spawn_rnode_ble_interface(
        transport.clone(),
        bus.clone(),
        config.rnode.clone(),
        active_interface_registry.clone(),
        status.clone(),
    );

    let _legacy_app_destination_hex = app_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let lxmf_destination_hex = lxmf_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    let app_destination_hex = lxmf_destination_hex.clone();

    let announce_capabilities = Arc::new(TokioMutex::new(AnnounceProfile::new(
        config.name.as_str(),
        config.announce_capabilities.as_str(),
    )));
    let known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<Link>>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let connected_peers: Arc<TokioMutex<HashSet<AddressHash>>> =
        Arc::new(TokioMutex::new(HashSet::new()));
    let peer_resolution_inflight: Arc<TokioMutex<HashSet<String>>> =
        Arc::new(TokioMutex::new(HashSet::new()));
    let pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let pending_lxmf_acknowledgements: Arc<
        TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>,
    > = Arc::new(TokioMutex::new(HashMap::new()));
    let messaging = Arc::new(TokioMutex::new(sdkmsg::MessagingStore::new(
        effective_peer_stale_after_minutes(
            config.stale_after_minutes,
            config.announce_interval_seconds,
        ),
    )));
    let active_propagation_node_hex: Arc<TokioMutex<Option<String>>> =
        Arc::new(TokioMutex::new(None));
    let propagation_sync_inflight = Arc::new(AtomicBool::new(false));
    let direct_delivery_health = DirectDeliveryHealth::default();
    let managed_peer_links = ManagedPeerLinks::default();
    let ignored_peer_destinations = Arc::new(TokioMutex::new(
        app_state
            .get_ignored_peer_destinations()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|destination| normalize_hex_32(destination.as_str()))
            .collect::<HashSet<_>>(),
    ));
    let send_task_permits = SendTaskPermits::new();
    let mission_destination_locks = MissionDestinationLocks::new();
    let projection_journal = Arc::new(RuntimeProjectionJournal::new(
        projection_journal_path(config.storage_dir.as_deref()),
        bus.clone(),
    ));
    let sdk = Arc::new(
        RuntimeLxmfSdk::new(
            identity.address_hash().to_hex_string(),
            SdkTransportState {
                identity: identity.clone(),
                transport: transport.clone(),
                lxmf_destination: lxmf_destination.clone(),
                known_destinations: known_destinations.clone(),
                out_links: out_links.clone(),
                active_propagation_node_hex: active_propagation_node_hex.clone(),
            },
        )
        .await,
    );

    let state = NodeRuntimeState {
        app_state,
        identity: identity.clone(),
        app_destination_hex,
        transport: transport.clone(),
        lxmf_destination: lxmf_destination.clone(),
        peer_resolution_inflight: peer_resolution_inflight.clone(),
        known_destinations: known_destinations.clone(),
        out_links: out_links.clone(),
        active_interface_registry: active_interface_registry.clone(),
        connected_peers: connected_peers.clone(),
        pending_lxmf_deliveries: pending_lxmf_deliveries.clone(),
        pending_lxmf_acknowledgements: pending_lxmf_acknowledgements.clone(),
        messaging: messaging.clone(),
        peers_snapshot: peers_snapshot.clone(),
        sync_status_snapshot: sync_status_snapshot.clone(),
        hub_directory_snapshot: hub_directory_snapshot.clone(),
        projection_journal: projection_journal.clone(),
        sdk: sdk.clone(),
        active_propagation_node_hex: active_propagation_node_hex.clone(),
        preferred_propagation_node_hex: config
            .hub_identity_hash
            .as_ref()
            .and_then(|value| normalize_hex_32(value)),
        propagation_sync_inflight: propagation_sync_inflight.clone(),
        direct_delivery_health: direct_delivery_health.clone(),
        managed_peer_links: managed_peer_links.clone(),
        ignored_peer_destinations: ignored_peer_destinations.clone(),
        send_task_permits: send_task_permits.clone(),
        mission_destination_locks: mission_destination_locks.clone(),
    };

    if let Some(snapshot) = projection_journal.load_snapshot() {
        let restored_snapshot = snapshot.pruned_for_restore();
        projection_journal.seed_snapshot(restored_snapshot.clone());
        if let Ok(mut guard) = peers_snapshot.lock() {
            *guard = restored_snapshot.peers();
        }
        if let Ok(mut guard) = sync_status_snapshot.lock() {
            *guard = restored_snapshot.sync_status();
        }
        seed_runtime_projection_snapshot(&state, &restored_snapshot).await;
    }

    if let Ok(announces) = state.app_state.list_announces() {
        let mut messaging = state.messaging.lock().await;
        for announce in announces {
            messaging.record_announce(to_sdk_announce_record(announce));
        }
    }

    let restored_saved_management = {
        let saved_peers = state.app_state.get_saved_peers().unwrap_or_default();
        let mut messaging = state.messaging.lock().await;
        restore_saved_peer_management(&mut messaging, saved_peers.as_slice())
    };

    if let Err(err) = sdk.start().await {
        bus.emit(NodeEvent::Error {
            code: "sdk_start_failed".to_string(),
            message: err.to_string(),
        });
    }

    refresh_peer_snapshot(&state).await;
    sync_auto_propagation_node(&state, &bus).await;
    if !restored_saved_management.pruned_destinations.is_empty() {
        info!(
            "[peers] pruned restored saved peers with non-rem lxmf announce evidence destinations={}",
            restored_saved_management.pruned_destinations.join(","),
        );
        for destination in &restored_saved_management.pruned_destinations {
            emit_peer_changed(&state, &bus, destination).await;
        }
    }
    if !restored_saved_management
        .route_request_destinations
        .is_empty()
    {
        info!(
            "[announce] restored saved peers route requests destinations={}",
            restored_saved_management
                .route_request_destinations
                .join(","),
        );
    }
    for target in restored_saved_management.link_targets {
        add_desired_managed_peer_link_and_schedule(&state, &bus, target, "saved-peer-restore")
            .await;
    }
    for destination_hex in restored_saved_management.route_request_destinations {
        if let Some(destination_hex) = normalize_hex_32(destination_hex.as_str()) {
            if let Ok(destination) = parse_address_hash(destination_hex.as_str()) {
                transport.request_path(&destination, None, None).await;
                spawn_managed_peer_resolution(state.clone(), bus.clone(), destination_hex);
            }
        }
    }
    let initial_sync_status = from_sdk_sync_status(state.messaging.lock().await.sync_status());
    refresh_sync_status_snapshot(&state, &initial_sync_status);

    if let Ok(mut guard) = status.lock() {
        guard.running = true;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
    spawn_tcp_client_readiness_monitor(
        tcp_readiness_monitor_endpoints(tcp_client_endpoints.as_slice()),
        status.clone(),
        bus.clone(),
    );

    spawn_peer_maintenance_tasks(&state, &bus);
    spawn_propagation_maintenance_task(&state, &bus);
    spawn_receipt_listener(&state, &bus, receipt_rx);
    spawn_announce_tasks(
        &config,
        &state,
        &bus,
        &app_destination,
        &announce_capabilities,
    );
    spawn_payload_receivers(&state, &bus);
    spawn_delivery_tracking_tasks(&state, &bus, &receipt_message_ids);
    spawn_link_event_listener(&state, &bus);
    spawn_periodic_hub_refresh(&config, &state, &bus);
    let command_executor = RuntimeCommandExecutor::new();

    include!("runtime/command_loop.rs");
    let _ = state.sdk.shutdown().await;
    state.projection_journal.flush_now().await;

    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
}
include!("runtime/identity.rs");

#[cfg(test)]
mod tests {
    include!("runtime/tests/support.rs");
    include!("runtime/tests/checklists_inbound_delete_marks_existing_checklist_de.rs");
    include!("runtime/tests/checklists_upload_snapshot_hydrates_hidden_placeholde.rs");
    include!("runtime/tests/command_tasks.rs");
    include!("runtime/tests/core.rs");
    include!("runtime/tests/delivery_compact_eam_fields_derive_sender_identity_.rs");
    include!("runtime/tests/delivery_receipt_tracking.rs");
    include!("runtime/tests/delivery_mission_recovery_sends_do_not_wait_on_satu.rs");
    include!("runtime/tests/hub_directory.rs");
    include!("runtime/tests/interfaces.rs");
    include!("runtime/tests/mission_events.rs");
    include!("runtime/tests/peers_routes_direct_delivery_health_blocks_and_restores.rs");
    include!("runtime/tests/peers_routes_lxmf_delivery_announce_mapping_uses_lxmf_s.rs");
    include!("runtime/tests/peers_routes_rem_lxmf_announce_path_response_keeps_capa.rs");
    include!("runtime/tests/propagation.rs");
    include!("runtime/tests/sos.rs");
}
