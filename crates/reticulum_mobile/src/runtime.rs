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
include!("runtime/hub.rs");
include!("runtime/interface_config.rs");
include!("runtime/network_interfaces.rs");

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
    let (receipt_tx, mut receipt_rx) = mpsc::unbounded_channel::<String>();
    transport
        .set_receipt_handler(Box::new(RuntimeReceiptBridge {
            receipt_message_ids: receipt_message_ids.clone(),
            tx: receipt_tx,
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

    let announce_capabilities = Arc::new(TokioMutex::new(config.announce_capabilities.clone()));
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
        config.stale_after_minutes,
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

    // Peer freshness/relay maintenance.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                refresh_peer_snapshot(&state).await;
                sync_auto_propagation_node(&state, &bus).await;
            }
        });
    }

    // Saved peer route maintenance. Passive announces are opportunistic; keep
    // asking the transport for managed peers so late or asymmetric mesh routes
    // can still be resolved without changing global node readiness.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_ROUTE_REFRESH_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                let destinations = {
                    let messaging = state.messaging.lock().await;
                    saved_peer_destinations_needing_route_refresh(&messaging)
                };
                if !destinations.is_empty() {
                    info!(
                        "[announce] saved peer route refresh destinations={}",
                        destinations.join(","),
                    );
                }
                for destination_hex in destinations {
                    if let Ok(destination) = parse_address_hash(destination_hex.as_str()) {
                        state.transport.request_path(&destination, None, None).await;
                    }
                    spawn_managed_peer_resolution(state.clone(), bus.clone(), destination_hex);
                }
            }
        });
    }

    // Keep desired peer links warm. Fresh REM-capable LXMF delivery announces
    // add desired link targets; explicit disconnect removes them.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_LINK_MAINTENANCE_INTERVAL);
            loop {
                interval.tick().await;
                maintain_managed_peer_links_once(&state, &bus).await;
            }
        });
    }

    // Propagation receive maintenance. Relay sends are store-and-forward, so
    // receivers must poll the selected relay even when nobody taps Sync.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(AUTO_PROPAGATION_SYNC_INTERVAL);
            loop {
                interval.tick().await;
                sync_auto_propagation_node(&state, &bus).await;
                let Some(relay_hex) = state.active_propagation_node_hex.lock().await.clone() else {
                    continue;
                };
                if state
                    .propagation_sync_inflight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    continue;
                }
                let requested_at_ms = now_ms();
                info!(
                    "[sync] automatic propagation sync scheduled relay={} limit={}",
                    relay_hex, AUTO_PROPAGATION_SYNC_LIMIT
                );
                tokio::spawn(run_propagation_sync_job(
                    state.clone(),
                    bus.clone(),
                    Some(AUTO_PROPAGATION_SYNC_LIMIT),
                    requested_at_ms,
                    relay_hex,
                ));
            }
        });
    }

    // Transport delivery receipts.
    {
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            while let Some(message_id_hex) = receipt_rx.recv().await {
                let maybe_record = state
                    .messaging
                    .lock()
                    .await
                    .update_message_delivery_state(
                        message_id_hex.as_str(),
                        None,
                        Some(sdkmsg::TransportDeliveryState::TransportDelivered),
                        None,
                        Some("transport receipt".to_string()),
                        None,
                        now_ms(),
                    )
                    .map(from_sdk_message_record);

                if let Some(record) = maybe_record {
                    sdk.record_delivery_acknowledged(
                        &record.message_id_hex,
                        &record.destination_hex,
                        record.source_hex.as_deref(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        record.detail.as_deref(),
                    );
                    bus.emit(NodeEvent::MessageUpdated {
                        message: record.clone(),
                    });
                }
            }
        });
    }

    // Announces.
    {
        let transport = transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = lxmf_destination.clone();
        let announce_capabilities = announce_capabilities.clone();
        tokio::spawn(async move {
            for delay_secs in STARTUP_ANNOUNCE_DELAYS_SECS {
                if delay_secs > 0 {
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "startup-burst",
                )
                .await;
            }
        });
    }

    {
        let transport = transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = lxmf_destination.clone();
        let announce_capabilities = announce_capabilities.clone();
        let interval_secs = effective_announce_interval_seconds(config.announce_interval_seconds);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            interval.tick().await;
            loop {
                interval.tick().await;
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "periodic",
                )
                .await;
            }
        });
    }

    // Announce receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let sdk = sdk.clone();
        let known_destinations = known_destinations.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.recv_announces().await;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let desc = event.destination.lock().await.desc;
                        known_destinations
                            .lock()
                            .await
                            .insert(desc.address_hash, desc);
                        let destination_hex = address_hash_to_hex(&desc.address_hash);
                        let identity_hex = desc.identity.address_hash.to_hex_string();
                        let destination_kind =
                            announce_destination_kind_from_name_hash(event.name_hash.as_slice())
                                .to_string();
                        let interface_hex = hex::encode(event.interface);
                        let received_at_ms = now_ms();
                        let sdk_announce_record = lxmf_sdk_announce_record_from_raw(
                            destination_hex.clone(),
                            identity_hex.clone(),
                            destination_kind.clone(),
                            event.app_data.as_slice(),
                            event.hops,
                            interface_hex.clone(),
                            received_at_ms,
                        );
                        let announce_record =
                            from_lxmf_sdk_announce_record(sdk_announce_record.clone());
                        let announce_class = announce_record.announce_class;
                        let app_data = announce_record.app_data.clone();
                        let is_rem_capable_lxmf_delivery = destination_kind
                            == DESTINATION_KIND_LXMF_DELIVERY
                            && app_data_has_rem_peer_capabilities(&app_data);
                        let display_name = announce_record.display_name.clone();
                        state
                            .messaging
                            .lock()
                            .await
                            .record_announce(to_compat_announce_record(&sdk_announce_record));
                        if let Err(err) = state.app_state.upsert_announce(&announce_record) {
                            bus.emit(NodeEvent::Error {
                                code: "IoError".to_string(),
                                message: format!(
                                    "failed to persist announce destination={} reason={}",
                                    destination_hex, err
                                ),
                            });
                        }
                        sdk.record_announce_received(
                            &destination_hex,
                            &identity_hex,
                            &destination_kind,
                            &announce_record.app_data,
                            event.hops,
                            &interface_hex,
                        );
                        bus.emit(NodeEvent::AnnounceReceived {
                            destination_hex: destination_hex.clone(),
                            identity_hex: identity_hex.clone(),
                            destination_kind: destination_kind.clone(),
                            announce_class,
                            app_data,
                            display_name: display_name.clone(),
                            hops: event.hops,
                            interface_hex,
                            received_at_ms,
                        });
                        if let Some(message) = operator_announce_message(
                            announce_class,
                            is_rem_capable_lxmf_delivery,
                            display_name.as_deref(),
                            destination_hex.as_str(),
                            identity_hex.as_str(),
                            event.hops,
                        ) {
                            emit_operational_notice(&bus, LogLevel::Info {}, message);
                        }
                        if destination_kind == DESTINATION_KIND_APP {
                            let lxmf_destination_hex = SingleOutputDestination::new(
                                desc.identity,
                                DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
                            )
                            .desc
                            .address_hash
                            .to_hex_string();
                            state.messaging.lock().await.record_resolution_result(
                                destination_hex.as_str(),
                                identity_hex.as_str(),
                                lxmf_destination_hex.as_str(),
                                received_at_ms,
                            );
                            emit_peer_changed(&state, &bus, &destination_hex).await;
                            emit_peer_resolved_for_destination(&state, &bus, &destination_hex)
                                .await;
                            spawn_passive_peer_resolution(
                                state.clone(),
                                bus.clone(),
                                destination_hex.clone(),
                            );
                        } else if destination_kind == DESTINATION_KIND_LXMF_DELIVERY {
                            let app_destination_hex = SingleOutputDestination::new(
                                desc.identity,
                                DestinationName::new(
                                    APP_DESTINATION_NAME.0,
                                    APP_DESTINATION_NAME.1,
                                ),
                            )
                            .desc
                            .address_hash
                            .to_hex_string();
                            debug!(
                                "[announce] derived app route from lxmf_delivery app={} lxmf={} identity={} display={} hops={}",
                                app_destination_hex,
                                destination_hex,
                                identity_hex,
                                display_name.as_deref().unwrap_or(""),
                                event.hops,
                            );
                            state.messaging.lock().await.record_resolution_result(
                                app_destination_hex.as_str(),
                                identity_hex.as_str(),
                                destination_hex.as_str(),
                                received_at_ms,
                            );
                            emit_peer_changed(&state, &bus, &destination_hex).await;
                            emit_peer_resolved_for_destination(&state, &bus, &destination_hex)
                                .await;
                            let ignored = peer_destinations_are_ignored(
                                &state,
                                [destination_hex.clone(), app_destination_hex.clone()],
                            )
                            .await;
                            if is_rem_capable_lxmf_delivery && !ignored {
                                add_desired_managed_peer_link_and_schedule(
                                    &state,
                                    &bus,
                                    ManagedPeerLinkTarget {
                                        destination_hex: destination_hex.clone(),
                                        kind: ManagedPeerLinkKind::LxmfDelivery,
                                    },
                                    "rem-lxmf-announce",
                                )
                                .await;
                            } else if is_rem_capable_lxmf_delivery {
                                debug!(
                                    "[link][maintain] destination={} status=ignored reason=rem-lxmf-announce",
                                    destination_hex,
                                );
                            }
                        }
                        let pruned_saved_destinations = {
                            let mut messaging = state.messaging.lock().await;
                            messaging.prune_saved_destinations_with_non_rem_announce_evidence()
                        };
                        if !pruned_saved_destinations.is_empty() {
                            info!(
                                "[peers] pruned saved peers with non-rem lxmf announce evidence destinations={}",
                                pruned_saved_destinations.join(",")
                            );
                            cleanup_removed_saved_destinations(
                                &state,
                                pruned_saved_destinations.as_slice(),
                            )
                            .await;
                            for destination in &pruned_saved_destinations {
                                emit_peer_changed(&state, &bus, destination).await;
                            }
                        }
                        sync_auto_propagation_node(&state, &bus).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Data receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            let mut rx = transport.received_data_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.destination);
                        let expected_lxmf = {
                            let local_lxmf_destination = state
                                .lxmf_destination
                                .lock()
                                .await
                                .desc
                                .address_hash
                                .to_hex_string();
                            destination_hex == local_lxmf_destination
                        };
                        info!(
                            "[lxmf][rx] data_event destination={} bytes={}",
                            destination_hex,
                            event.data.as_slice().len(),
                        );
                        emit_received_payload(
                            &state,
                            &bus,
                            &sdk,
                            destination_hex,
                            event.data.as_slice().to_vec(),
                            None,
                            expected_lxmf,
                        )
                        .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Resource receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let state = state.clone();
        let sdk = sdk.clone();
        tokio::spawn(async move {
            let mut rx = transport.resource_events();
            loop {
                match rx.recv().await {
                    Ok(event) => match event.kind {
                        ResourceEventKind::Complete(complete) => {
                            let destination_hex = if let Some(link) =
                                transport.find_in_link(&event.link_id).await
                            {
                                address_hash_to_hex(&link.lock().await.destination().address_hash)
                            } else if let Some(link) = transport.find_out_link(&event.link_id).await
                            {
                                address_hash_to_hex(&link.lock().await.destination().address_hash)
                            } else {
                                address_hash_to_hex(&event.link_id)
                            };
                            info!(
                                "[lxmf][events] resource complete link_id={} destination={} bytes={} metadata_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                destination_hex,
                                complete.data.len(),
                                complete.metadata.as_ref().map(Vec::len).unwrap_or(0),
                            );
                            emit_received_payload(
                                &state,
                                &bus,
                                &sdk,
                                destination_hex,
                                complete.data,
                                complete.metadata,
                                true,
                            )
                            .await;
                        }
                        ResourceEventKind::Progress(progress) => {
                            debug!(
                                "[lxmf][debug] resource progress link_id={} received_bytes={} total_bytes={} received_parts={} total_parts={}",
                                address_hash_to_hex(&event.link_id),
                                progress.received_bytes,
                                progress.total_bytes,
                                progress.received_parts,
                                progress.total_parts,
                            );
                        }
                        ResourceEventKind::OutboundComplete => {
                            info!(
                                "[lxmf][events] resource outbound complete link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::OutboundFailed => {
                            info!(
                                "[lxmf][events] resource outbound failed link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::OutboundCancelled => {
                            info!(
                                "[lxmf][events] resource outbound cancelled link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::InboundFailed(failure) => {
                            warn!(
                                "[lxmf][events] resource inbound failed link_id={} hash={} reason={} received_parts={} total_parts={} received_bytes={} total_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                                failure.reason,
                                failure.progress.received_parts,
                                failure.progress.total_parts,
                                failure.progress.received_bytes,
                                failure.progress.total_bytes,
                            );
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Pending LXMF acknowledgement timeout watcher.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = now_ms();
                let mut expired = Vec::<PendingLxmfDelivery>::new();
                {
                    let mut guard = state.pending_lxmf_deliveries.lock().await;
                    let expired_keys = guard
                        .iter()
                        .filter(|(_, pending)| pending_ack_timeout_elapsed(pending, now))
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for key in expired_keys {
                        if let Some(pending) = guard.remove(&key) {
                            expired.push(pending);
                        }
                    }
                }
                for pending in expired {
                    match retry_pending_ack_timeout_via_propagation(&state, &bus, &pending).await {
                        Ok(true) => continue,
                        Ok(false) => {
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                "ack timeout",
                            );
                        }
                        Err(err) => {
                            let detail = format!("ack timeout; propagation retry failed: {err}");
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                detail.as_str(),
                            );
                        }
                    }
                }
            }
        });
    }

    // Cleanup stale buffered acknowledgements and receipt tracking.
    {
        let pending_lxmf_acknowledgements = pending_lxmf_acknowledgements.clone();
        let receipt_message_ids = receipt_message_ids.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let now = now_ms();
                let pruned_acks = {
                    let mut guard = pending_lxmf_acknowledgements.lock().await;
                    prune_expired_buffered_acknowledgements(&mut guard, now)
                };
                let pruned_receipts = if let Ok(mut guard) = receipt_message_ids.lock() {
                    prune_expired_receipt_tracking(&mut guard, now)
                } else {
                    0
                };
                if pruned_acks > 0 || pruned_receipts > 0 {
                    debug!(
                        "[runtime] pruned stale state buffered_acks={} receipt_tracking={}",
                        pruned_acks, pruned_receipts,
                    );
                }
            }
        });
    }

    // Link events.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let connected_peers = connected_peers.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.out_link_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.address_hash);
                        match event.event {
                            LinkEvent::Activated => {
                                debug!(
                                    "[link][event] kind=activated destination={} link_id={}",
                                    destination_hex,
                                    address_hash_to_hex(&event.id),
                                );
                                connected_peers.lock().await.insert(event.address_hash);
                                record_peer_link_state(&state, &bus, &destination_hex, true).await;
                            }
                            LinkEvent::Closed => {
                                debug!(
                                    "[link][event] kind=closed destination={} link_id={}",
                                    destination_hex,
                                    address_hash_to_hex(&event.id),
                                );
                                state.out_links.lock().await.remove(&event.address_hash);
                                connected_peers.lock().await.remove(&event.address_hash);
                                record_peer_link_state(&state, &bus, &destination_hex, false).await;
                                mark_peer_direct_delivery_unhealthy(
                                    &state,
                                    destination_hex.as_str(),
                                    None,
                                )
                                .await;
                                match state
                                    .managed_peer_links
                                    .begin_reconnect(destination_hex.as_str())
                                    .await
                                {
                                    ManagedPeerReconnectStart::Started(target) => {
                                        info!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-scheduled",
                                            destination_hex,
                                        );
                                        spawn_managed_peer_link_reconnect(
                                            state.clone(),
                                            bus.clone(),
                                            target,
                                        );
                                    }
                                    ManagedPeerReconnectStart::Backoff {
                                        next_retry_at_ms,
                                        last_failure_reason,
                                    } => {
                                        debug!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-deferred detail=backoff next_retry_at_ms={} last_failure={}",
                                            destination_hex,
                                            next_retry_at_ms,
                                            last_failure_reason.as_deref().unwrap_or("-"),
                                        );
                                    }
                                    ManagedPeerReconnectStart::AlreadyReconnecting => {
                                        debug!(
                                            "[link][event] kind=closed destination={} desired=true status=reconnect-deferred detail=reconnecting",
                                            destination_hex,
                                        );
                                    }
                                    ManagedPeerReconnectStart::NotDesired => {}
                                }
                            }
                            LinkEvent::Data(_) => {}
                            LinkEvent::PeerIdentified(_) => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Optional periodic hub refresh.
    if matches!(
        config.hub_mode,
        HubMode::SemiAutonomous {} | HubMode::Connected {}
    ) && config.hub_refresh_interval_seconds > 0
    {
        let bus = bus.clone();
        let config = config.clone();
        let state = state.clone();
        let interval_secs = config.hub_refresh_interval_seconds;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                if let Ok(snapshot) = refresh_hub_directory_lxmf(&config, &state).await {
                    publish_hub_directory_snapshot(&state, &bus, snapshot).await;
                }
            }
        });
    }

    loop {
        let cmd = tokio::select! {
            biased;
            Some(cmd) = priority_cmd_rx.recv() => cmd,
            Some(cmd) = cmd_rx.recv() => cmd,
            else => break,
        };
        match cmd {
            Command::Stop { resp } => {
                if let Ok(mut guard) = status.lock() {
                    guard.running = false;
                    guard.refresh_readiness();
                    bus.emit(NodeEvent::StatusChanged {
                        status: guard.clone(),
                    });
                }
                let _ = resp.send(Ok(()));
                break;
            }
            Command::AnnounceNow {} => {
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "manual",
                )
                .await;
            }
            Command::SetLogLevel { level } => {
                crate::logger::NodeLogger::global().set_level(level);
            }
            Command::RequestPeerIdentity {
                destination_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                tokio::spawn(async move {
                    let result = resolve_peer_route(&state, &bus, destination_hex.as_str()).await;
                    if let Err(err) = &result {
                        state.messaging.lock().await.record_resolution_error(
                            destination_hex.as_str(),
                            Some(err.to_string()),
                        );
                        emit_peer_changed(&state, &bus, destination_hex.as_str()).await;
                    }
                    let _ = resp.send(result);
                });
            }
            Command::SetAnnounceCapabilities {
                capability_string,
                resp,
            } => {
                *announce_capabilities.lock().await = capability_string;
                announce_destinations(
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                    "capabilities-updated",
                )
                .await;
                let _ = resp.send(Ok(()));
            }
            Command::ConnectPeer {
                destination_hex,
                resp,
            } => {
                let destination_hex_copy = destination_hex.clone();
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    let saved_peer =
                        persist_selected_peer_destination(&state, &bus, destination_hex.as_str())
                            .await?;
                    clear_ignored_peer_destinations(&state, std::slice::from_ref(&destination_hex))
                        .await;
                    emit_peer_changed(&state, &bus, saved_peer.destination_hex.as_str()).await;
                    state.sdk.record_peer_changed(
                        saved_peer.destination_hex.as_str(),
                        PeerState::Connecting {},
                        None,
                    );
                    resolve_peer_route(&state, &bus, &destination_hex).await?;
                    let target =
                        match register_desired_managed_peer_link(&state, &destination_hex).await {
                            Some(target) => target,
                            None => {
                                let target = ManagedPeerLinkTarget {
                                    destination_hex: address_hash_to_hex(&dest),
                                    kind: ManagedPeerLinkKind::App,
                                };
                                state.managed_peer_links.add_desired(target.clone()).await;
                                target
                            }
                        };
                    let target_destination = parse_address_hash(target.destination_hex.as_str())?;
                    let desc = ensure_destination_desc(
                        &state,
                        target_destination,
                        Some(target.kind.destination_name()),
                    )
                    .await?;
                    let _link = ensure_output_link(&state, desc).await?;
                    record_peer_link_state(&state, &bus, target.destination_hex.as_str(), true)
                        .await;
                    Ok::<(), NodeError>(())
                }
                .await;
                if let Err(err) = &result {
                    state.messaging.lock().await.record_resolution_error(
                        destination_hex_copy.as_str(),
                        Some(err.to_string()),
                    );
                    emit_peer_changed(&state, &bus, &destination_hex_copy).await;
                    state.sdk.record_peer_changed(
                        &destination_hex_copy,
                        PeerState::Disconnected {},
                        Some(err.to_string().as_str()),
                    );
                }
                let _ = resp.send(result);
            }
            Command::DisconnectPeer {
                destination_hex,
                resp,
            } => {
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    let mut destinations = vec![destination_hex.clone()];
                    if let Some(peer) = peer_for_any_destination_hex(&state, &destination_hex).await
                    {
                        destinations
                            .extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
                    }
                    destinations.sort();
                    destinations.dedup();
                    {
                        let now = now_ms();
                        let mut messaging = state.messaging.lock().await;
                        for destination in &destinations {
                            messaging.set_peer_active_link(destination.as_str(), false, now);
                        }
                    }
                    state
                        .direct_delivery_health
                        .clear(destinations.iter().map(String::as_str));
                    state
                        .managed_peer_links
                        .remove_desired(destinations.iter().map(String::as_str))
                        .await;
                    mark_peer_destinations_ignored(&state, destinations.as_slice()).await;
                    connected_peers.lock().await.remove(&dest);
                    for destination in &destinations {
                        if let Ok(destination) = parse_address_hash(destination.as_str()) {
                            connected_peers.lock().await.remove(&destination);
                            if let Some(link) = out_links.lock().await.remove(&destination) {
                                link.lock().await.close();
                            }
                        }
                    }
                    emit_peer_changed(&state, &bus, &destination_hex).await;
                    state.sdk.record_peer_changed(
                        &address_hash_to_hex(&dest),
                        PeerState::Disconnected {},
                        None,
                    );
                    sync_auto_propagation_node(&state, &bus).await;
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SetSavedPeers { peers, resp } => {
                let result = apply_saved_peer_management_projection(&state, &bus, &peers).await;
                let _ = resp.send(result);
            }
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                let transport = transport.clone();
                let metadata = fields_bytes
                    .as_deref()
                    .and_then(parse_mission_sync_metadata);
                let send_task_class = SendTaskClass::from_lxmf_request(
                    fields_bytes.is_some(),
                    metadata.as_ref(),
                    &send_mode,
                );
                log_send_task(
                    send_task_class,
                    format!(
                        "[lxmf][queue] enqueued {} send destination={} mode={:?} has_fields={}",
                        send_task_class.label(),
                        destination_hex,
                        send_mode,
                        fields_bytes.is_some(),
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let lxmf_report = if fields_bytes.is_some() {
                            Some(
                                send_lxmf_with_delivery_policy(
                                    &state,
                                    &bus,
                                    &destination_hex,
                                    &bytes,
                                    None,
                                    fields_bytes.clone(),
                                    metadata.clone(),
                                    send_mode,
                                    send_task_class,
                                )
                                .await?,
                            )
                        } else {
                            None
                        };
                        let outcome = if let Some(report) = lxmf_report.as_ref() {
                            report.outcome
                        } else {
                            log_send_task(
                                SendTaskClass::General,
                                format!(
                                    "[lxmf][queue] waiting for general send slot destination={} mode=transport-bytes",
                                    destination_hex,
                                ),
                            );
                            let _permit = acquire_send_task_permit(
                                &state.send_task_permits,
                                SendTaskClass::General,
                            )
                            .await?;
                            log_send_task(
                                SendTaskClass::General,
                                format!(
                                    "[lxmf][queue] acquired general send slot destination={} mode=transport-bytes",
                                    destination_hex,
                                ),
                            );
                            let dest = parse_address_hash(&destination_hex)?;
                            send_transport_packet_with_path_retry(&transport, dest, &bytes).await
                        };
                        let mapped = send_outcome_to_udl(outcome);
                        bus.emit(NodeEvent::PacketSent {
                            destination_hex: destination_hex.clone(),
                            bytes: bytes.clone(),
                            outcome: mapped,
                        });

                        if let Some(report) = lxmf_report.as_ref() {
                            if let Some(metadata) = report.metadata.as_ref() {
                                if metadata.is_mission_related() {
                                    info!(
                                        "[lxmf][mission] outbound kind={} name={} destination={} message_id={} event_uid={} mission_uid={} correlation={}",
                                        metadata.primary_kind(),
                                        metadata.primary_name().unwrap_or("-"),
                                        report.resolved_destination_hex.as_str(),
                                        report.message_id_hex,
                                        metadata.event_uid.as_deref().unwrap_or("-"),
                                        metadata.mission_uid.as_deref().unwrap_or("-"),
                                        metadata.correlation_id.as_deref().unwrap_or("-"),
                                    );
                                }
                            }

                            let resend = build_pending_lxmf_resend(
                                report,
                                destination_hex.as_str(),
                                bytes.as_slice(),
                                None,
                                fields_bytes.clone(),
                                metadata.clone(),
                                send_mode,
                                send_task_class,
                            );
                            if let Some(registered) =
                                register_pending_lxmf_delivery(&state, report, resend, None).await
                            {
                                let pending = &registered.pending;
                                if matches!(
                                    report.outcome,
                                    RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                                ) {
                                    state.sdk.record_delivery_sent(
                                        &pending.message_id_hex,
                                        &pending.destination_hex,
                                        pending.correlation_id.as_deref(),
                                        pending.command_id.as_deref(),
                                        pending.command_type.as_deref(),
                                        pending.event_uid.as_deref(),
                                        pending.mission_uid.as_deref(),
                                    );
                                    emit_lxmf_delivery(
                                        &bus,
                                        pending,
                                        lxmf_delivery_status_for(report),
                                        None,
                                    );
                                    info!(
                                        "[lxmf][mission] sent message_id={} destination={} command={} correlation={}",
                                        pending.message_id_hex,
                                        pending.destination_hex,
                                        pending.command_type.as_deref().unwrap_or("-"),
                                        pending.correlation_id.as_deref().unwrap_or("-"),
                                    );
                                    if let Some(buffered_ack) = registered.buffered_ack {
                                        acknowledge_pending_with_buffered_ack(
                                            &state,
                                            &bus,
                                            pending,
                                            buffered_ack,
                                        )
                                        .await;
                                    }
                                } else {
                                    let failure_detail = format!("{mapped:?}");
                                    {
                                        let tracking_key = pending_tracking_key(pending);
                                        if let Some(tracking_key) = tracking_key {
                                            state.pending_lxmf_deliveries.lock().await.remove(&tracking_key);
                                        }
                                    }
                                    state.sdk.record_delivery_failed(
                                        &pending.message_id_hex,
                                        &pending.destination_hex,
                                        pending.correlation_id.as_deref(),
                                        pending.command_id.as_deref(),
                                        pending.command_type.as_deref(),
                                        pending.event_uid.as_deref(),
                                        pending.mission_uid.as_deref(),
                                        Some(failure_detail.as_str()),
                                    );
                                    emit_lxmf_delivery(
                                        &bus,
                                        pending,
                                        LxmfDeliveryStatus::Failed {},
                                        Some(failure_detail.clone()),
                                    );
                                    info!(
                                        "[lxmf][mission] failed message_id={} destination={} command={} correlation={} outcome={:?}",
                                        pending.message_id_hex,
                                        pending.destination_hex,
                                        pending.command_type.as_deref().unwrap_or("-"),
                                        pending.correlation_id.as_deref().unwrap_or("-"),
                                        mapped,
                                    );
                                }
                            }
                        }

                        if matches!(
                            outcome,
                            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                        ) {
                            Ok(())
                        } else {
                            Err(NodeError::NetworkError {})
                        }
                    }
                    .await;
                    if let Err(err) = &result {
                        if !should_emit_global_send_bytes_error(send_task_class) {
                            info!(
                                "[lxmf][mission] propagation send exhausted destination={} reason={}",
                                destination_hex, err
                            );
                            let _ = resp.send(result);
                            return;
                        }
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "send_bytes failed destination={} reason={}",
                                destination_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
            }
            Command::SendLxmf { request, resp } => {
                let state = state.clone();
                let bus = bus.clone();
                let receipt_message_ids = receipt_message_ids.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general send destination={} mode={:?} has_fields=false",
                        request.destination_hex,
                        request.send_mode,
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let body_bytes = request.body_utf8.as_bytes().to_vec();
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            request.destination_hex.as_str(),
                            body_bytes.as_slice(),
                            request.title.clone(),
                            None,
                            None,
                            request.send_mode,
                            SendTaskClass::General,
                        )
                        .await?;
                        let method = match (report.method, report.representation) {
                            (LxmfDeliveryMethod::Propagated {}, _) => MessageMethod::Propagated {},
                            (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                MessageMethod::Opportunistic {}
                            }
                            (_, LxmfDeliveryRepresentation::Resource {}) => {
                                MessageMethod::Resource {}
                            }
                            _ => MessageMethod::Direct {},
                        };
                        let state_value = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else if matches!(
                            report.outcome,
                            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                        ) {
                            MessageState::SentDirect {}
                        } else {
                            MessageState::Failed {}
                        };
                        let detail = if matches!(state_value, MessageState::Failed {}) {
                            Some(format!("{:?}", send_outcome_to_udl(report.outcome)))
                        } else {
                            None
                        };
                        let conversation_id =
                            conversation_id_for(report.resolved_destination_hex.as_str());
                        let record = MessageRecord {
                            message_id_hex: report.message_id_hex.clone(),
                            conversation_id,
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(request.destination_hex.clone()),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: request.title.clone(),
                            body_utf8: request.body_utf8.clone(),
                            method,
                            state: state_value,
                            transport_state: transport_state_for_message_state(state_value),
                            application_ack_state: if matches!(state_value, MessageState::Failed {})
                            {
                                ApplicationAckState::Failed {}
                            } else {
                                ApplicationAckState::Waiting {}
                            },
                            detail: detail.clone(),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, record, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: to_sdk_send_request(&request),
                                message_id_hex: report.message_id_hex.clone(),
                            },
                        );
                        if let Some(receipt_hash_hex) = report.receipt_hash_hex.as_ref() {
                            if let Ok(mut guard) = receipt_message_ids.lock() {
                                guard.insert(
                                    receipt_hash_hex.clone(),
                                    ReceiptMessageTracking {
                                        message_id_hex: report.message_id_hex.clone(),
                                        recorded_at_ms: now_ms(),
                                    },
                                );
                            }
                        }
                        Ok::<String, NodeError>(report.message_id_hex)
                    }
                    .await;
                    if let Err(err) = &result {
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "send_lxmf failed destination={} reason={}",
                                request.destination_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
            }
            Command::RetryLxmf {
                message_id_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general retry message_id={}",
                        message_id_hex,
                    ),
                );
                tokio::spawn(async move {
                    let result = async {
                        let outbound = state
                            .messaging
                            .lock()
                            .await
                            .outbound(message_id_hex.as_str())
                            .ok_or(NodeError::InvalidConfig {})?;
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            outbound.request.destination_hex.as_str(),
                            outbound.request.body_utf8.as_bytes(),
                            outbound.request.title.clone(),
                            None,
                            None,
                            match outbound.request.effective_send_mode() {
                                sdkmsg::SendMode::Auto => SendMode::Auto {},
                                sdkmsg::SendMode::DirectOnly => SendMode::DirectOnly {},
                                sdkmsg::SendMode::PropagationOnly => SendMode::PropagationOnly {},
                            },
                            SendTaskClass::General,
                        )
                        .await?;
                        let retried_state = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else {
                            MessageState::SentDirect {}
                        };
                        let retried = MessageRecord {
                            message_id_hex: outbound.message_id_hex.clone(),
                            conversation_id: conversation_id_for(
                                report.resolved_destination_hex.as_str(),
                            ),
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(
                                outbound.request.destination_hex.clone(),
                            ),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: outbound.request.title.clone(),
                            body_utf8: outbound.request.body_utf8.clone(),
                            method: match (report.method, report.representation) {
                                (LxmfDeliveryMethod::Propagated {}, _) => {
                                    MessageMethod::Propagated {}
                                }
                                (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                    MessageMethod::Opportunistic {}
                                }
                                (_, LxmfDeliveryRepresentation::Resource {}) => {
                                    MessageMethod::Resource {}
                                }
                                _ => MessageMethod::Direct {},
                            },
                            state: retried_state,
                            transport_state: transport_state_for_message_state(retried_state),
                            application_ack_state: ApplicationAckState::Waiting {},
                            detail: Some(format!("retry of {}", outbound.message_id_hex)),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, retried, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: outbound.request,
                                message_id_hex: outbound.message_id_hex.clone(),
                            },
                        );
                        Ok::<(), NodeError>(())
                    }
                    .await;
                    if let Err(err) = &result {
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "retry_lxmf failed message_id={} reason={}",
                                message_id_hex, err
                            ),
                        });
                    }
                    let _ = resp.send(result);
                });
            }
            Command::CancelLxmf {
                message_id_hex,
                resp,
            } => {
                let result = async {
                    let updated = state
                        .messaging
                        .lock()
                        .await
                        .update_message_delivery_state(
                            message_id_hex.as_str(),
                            Some(sdkmsg::MessageState::Cancelled),
                            Some(sdkmsg::TransportDeliveryState::Cancelled),
                            Some(sdkmsg::ApplicationAckState::Failed),
                            Some("cancelled locally".to_string()),
                            None,
                            now_ms(),
                        )
                        .map(from_sdk_message_record)
                        .ok_or(NodeError::InvalidConfig {})?;
                    upsert_message_record(&state, &bus, updated, false).await;
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SetActivePropagationNode {
                destination_hex,
                resp,
            } => {
                *state.active_propagation_node_hex.lock().await = destination_hex.clone();
                let status_update = from_sdk_sync_status(
                    state
                        .messaging
                        .lock()
                        .await
                        .set_active_propagation_node(destination_hex),
                );
                if refresh_sync_status_snapshot(&state, &status_update) {
                    bus.emit(NodeEvent::SyncUpdated {
                        status: status_update,
                    });
                }
                let _ = resp.send(Ok(()));
            }
            Command::RequestLxmfSync { limit, resp } => {
                let requested_at_ms = now_ms();
                if state.propagation_sync_inflight.load(Ordering::Acquire) {
                    info!("[sync] propagation sync request ignored reason=inflight");
                    let _ = resp.send(Ok(()));
                    continue;
                }
                emit_sync_status_update(
                    &state,
                    &bus,
                    sdkmsg::SyncPhase::PathRequested,
                    requested_at_ms,
                    0,
                    Some("waiting for propagation relay selection".to_string()),
                    false,
                )
                .await;
                let Some(relay_hex) = wait_for_active_propagation_relay(&state, &bus).await else {
                    let detail = format!(
                        "no active propagation relay selected after {}s",
                        PROPAGATION_SYNC_RELAY_SELECTION_WAIT.as_secs()
                    );
                    emit_sync_status_update(
                        &state,
                        &bus,
                        sdkmsg::SyncPhase::Failed,
                        requested_at_ms,
                        0,
                        Some(detail.clone()),
                        true,
                    )
                    .await;
                    info!("[sync] propagation sync failed reason={detail}");
                    let _ = resp.send(Err(NodeError::InvalidConfig {}));
                    continue;
                };
                if state
                    .propagation_sync_inflight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    info!("[sync] propagation sync request ignored reason=inflight");
                    let _ = resp.send(Ok(()));
                    continue;
                }
                info!(
                    "[sync] propagation sync scheduled relay={} limit={}",
                    relay_hex,
                    limit
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string())
                );
                tokio::spawn(run_propagation_sync_job(
                    state.clone(),
                    bus.clone(),
                    limit,
                    requested_at_ms,
                    relay_hex,
                ));
                let _ = resp.send(Ok(()));
            }
            Command::ListAnnounces { resp } => {
                let records = state
                    .messaging
                    .lock()
                    .await
                    .list_announces()
                    .into_iter()
                    .map(from_sdk_announce_record)
                    .collect::<Vec<_>>();
                let _ = resp.send(Ok(records));
            }
            Command::ListPeers { resp } => {
                let _ = resp.send(Ok(snapshot_peer_records(&state).await));
            }
            Command::ListConversations { resp } => {
                let _ = resp.send(Ok(conversation_records_snapshot(&state).await));
            }
            Command::ListMessages {
                conversation_id,
                resp,
            } => {
                let _ = resp.send(Ok(message_records_snapshot(
                    &state,
                    conversation_id.as_deref(),
                )
                .await));
            }
            Command::DeleteConversation {
                conversation_id,
                resp,
            } => {
                let _ = resp.send(
                    delete_conversation_records(&state, &bus, conversation_id.as_str()).await,
                );
            }
            Command::GetLxmfSyncStatus { resp } => {
                let _ = resp.send(Ok(from_sdk_sync_status(
                    state.messaging.lock().await.sync_status(),
                )));
            }
            Command::BroadcastBytes { bytes, resp } => {
                let result = async {
                    let peers = connected_peers
                        .lock()
                        .await
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    let mut sent_any = false;
                    for dest in peers {
                        let outcome =
                            send_transport_packet_with_path_retry(&transport, dest, &bytes).await;
                        bus.emit(NodeEvent::PacketSent {
                            destination_hex: address_hash_to_hex(&dest),
                            bytes: bytes.clone(),
                            outcome: send_outcome_to_udl(outcome),
                        });
                        if matches!(
                            outcome,
                            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                        ) {
                            sent_any = true;
                        }
                    }

                    if sent_any {
                        Ok::<(), NodeError>(())
                    } else {
                        Err(NodeError::NetworkError {})
                    }
                }
                .await;
                if let Err(err) = &result {
                    bus.emit(NodeEvent::Error {
                        code: node_error_code(err).to_string(),
                        message: format!("broadcast_bytes failed reason={}", err),
                    });
                }
                let _ = resp.send(result);
            }
            Command::RefreshHubDirectory { resp } => {
                let state = state.clone();
                let bus = bus.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    let result = match config.hub_mode {
                        HubMode::Autonomous {} => Err(NodeError::InvalidConfig {}),
                        HubMode::SemiAutonomous {} | HubMode::Connected {} => {
                            refresh_hub_directory_lxmf(&config, &state).await
                        }
                    }
                    .map(|snapshot| async {
                        publish_hub_directory_snapshot(&state, &bus, snapshot).await;
                    });
                    let _ = resp.send(match result {
                        Ok(publish) => {
                            publish.await;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    });
                });
            }
        }
    }

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
    include!("runtime/tests/core.rs");
    include!("runtime/tests/delivery_compact_eam_fields_derive_sender_identity_.rs");
    include!("runtime/tests/delivery_mission_recovery_sends_do_not_wait_on_satu.rs");
    include!("runtime/tests/interfaces.rs");
    include!("runtime/tests/mission_events.rs");
    include!("runtime/tests/peers_routes_direct_delivery_health_blocks_and_restores.rs");
    include!("runtime/tests/peers_routes_lxmf_delivery_announce_mapping_uses_lxmf_s.rs");
    include!("runtime/tests/peers_routes_rem_lxmf_announce_path_response_keeps_capa.rs");
    include!("runtime/tests/propagation.rs");
    include!("runtime/tests/sos.rs");
}
