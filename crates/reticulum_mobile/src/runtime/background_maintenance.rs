fn spawn_peer_maintenance_tasks(
    state: &NodeRuntimeState,
    bus: &EventBus,
    power_saver_rx: watch::Receiver<bool>,
) {
    // Peer freshness/relay maintenance.
    {
        let bus = bus.clone();
        let state = state.clone();
        let power_saver_rx = power_saver_rx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                refresh_peer_snapshot(&state).await;
                if !*power_saver_rx.borrow() {
                    sync_auto_propagation_node(&state, &bus).await;
                }
            }
        });
    }

    // Saved peer route maintenance. Passive announces are opportunistic; keep
    // asking the transport for managed peers so late or asymmetric mesh routes
    // can still be resolved without changing global node readiness.
    {
        let bus = bus.clone();
        let state = state.clone();
        let power_saver_rx = power_saver_rx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_ROUTE_REFRESH_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                if *power_saver_rx.borrow() {
                    continue;
                }
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
        let power_saver_rx = power_saver_rx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAVED_PEER_LINK_MAINTENANCE_INTERVAL);
            loop {
                interval.tick().await;
                if !*power_saver_rx.borrow() {
                    maintain_managed_peer_links_once(&state, &bus).await;
                }
            }
        });
    }
}

fn spawn_propagation_maintenance_task(
    state: &NodeRuntimeState,
    bus: &EventBus,
    power_saver_rx: watch::Receiver<bool>,
) {
    // Propagation receive maintenance. Relay sends are store-and-forward, so
    // receivers must poll the selected relay even when nobody taps Sync.
    let bus = bus.clone();
    let state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(AUTO_PROPAGATION_SYNC_INTERVAL);
        loop {
            interval.tick().await;
            if *power_saver_rx.borrow() {
                continue;
            }
            // Automatic relay polling can fan out across several candidates.
            // Do not put that background workload on an RNode-only LoRa link;
            // explicit propagation sync and direct sends remain available; a
            // relay-capable interface resumes polling as soon as it is active.
            if !has_active_relay_transport_interface(&state).await {
                continue;
            }
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
                "[sync] automatic propagation sync scheduled relay={relay_hex} limit={AUTO_PROPAGATION_SYNC_LIMIT}"
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

fn spawn_receipt_listener(
    state: &NodeRuntimeState,
    bus: &EventBus,
    mut receipt_rx: mpsc::UnboundedReceiver<String>,
) {
    let bus = bus.clone();
    let sdk = state.sdk.clone();
    let state = state.clone();
    tokio::spawn(async move {
        while let Some(message_id_hex) = receipt_rx.recv().await {
            let maybe_record = state
                .messaging
                .lock()
                .await
                .update_message_delivery_state(sdkmsg::MessageDeliveryUpdate {
                    message_id_hex: message_id_hex.as_str(),
                    state: Some(sdkmsg::MessageState::Delivered),
                    transport_state: Some(sdkmsg::TransportDeliveryState::TransportDelivered),
                    application_ack_state: None,
                    detail: Some("transport receipt".to_string()),
                    last_wire_message_id_hex: None,
                    updated_at_ms: now_ms(),
                })
                .map(|record| from_sdk_message_record_with_persisted(&state, record));

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

            let pending = state
                .pending_lxmf_deliveries
                .lock()
                .await
                .values()
                .find(|pending| pending.message_id_hex == message_id_hex)
                .cloned();
            if let Some(pending) = pending {
                sdk.record_delivery_acknowledged(
                    &pending.message_id_hex,
                    &pending.destination_hex,
                    None,
                    pending.correlation_id.as_deref(),
                    pending.command_id.as_deref(),
                    pending.command_type.as_deref(),
                    pending.event_uid.as_deref(),
                    pending.mission_uid.as_deref(),
                    Some("transport receipt"),
                );
                emit_lxmf_delivery_with_source(
                    &bus,
                    &pending,
                    None,
                    LxmfDeliveryStatus::Delivered {},
                    ApplicationAckState::Waiting {},
                    Some("transport receipt".to_string()),
                );
            }
        }
    });
}
