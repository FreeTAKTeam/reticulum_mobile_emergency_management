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
                if let Err(error)=publish_local_custody(&state,&bus).await {debug!("[local-propagation] projection pending: {error}");}
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
        let mut last_lora_poll:Option<tokio::time::Instant>=None;
        loop {
            interval.tick().await;
            if *power_saver_rx.borrow() {
                continue;
            }
            if !has_active_reticulum_interface(&state).await {continue}
            let lora=active_interfaces_are_rnode_ble_only(&*state.active_interface_registry.lock().await);
            if lora && last_lora_poll.is_some_and(|last|last.elapsed()<Duration::from_secs(u64::from(crate::sdk_bridge::LOCAL_PROPAGATION_LORA_INTERVAL_SECONDS))) {continue}
            sync_auto_propagation_node(&state, &bus).await;
            let Some(relay_hex) = state.active_propagation_node_hex.lock().await.clone() else {
                spawn_local_custody_recovery(&state,&bus,None);
                continue;
            };
            let Some(relay_hex) = routed_automatic_propagation_relay(&state, &relay_hex).await else {
                spawn_local_custody_recovery(&state, &bus, None);
                continue;
            };
            if state
                .propagation_sync_inflight
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                continue;
            }
            if lora { last_lora_poll = Some(tokio::time::Instant::now()); }
            let requested_at_ms = now_ms();
            info!(
                "[sync] automatic propagation sync scheduled relay={relay_hex} limit={AUTO_PROPAGATION_SYNC_LIMIT}"
            );
            let recovery_state=state.clone();let recovery_bus=bus.clone();
            tokio::spawn(async move {
                run_propagation_sync_job(recovery_state.clone(),recovery_bus.clone(),Some(AUTO_PROPAGATION_SYNC_LIMIT),requested_at_ms,relay_hex,true).await;
                spawn_local_custody_recovery(&recovery_state,&recovery_bus,None);
            });
        }
    });
}

async fn automatic_propagation_poll_is_routed(state: &NodeRuntimeState, relay: &str) -> bool {
    let lora = active_interfaces_are_rnode_ble_only(&*state.active_interface_registry.lock().await);
    if !lora {
        return true;
    }
    let Ok(destination) = parse_address_hash(relay) else { return false; };
    state.transport.has_path(&destination).await
}

async fn routed_automatic_propagation_relay(state: &NodeRuntimeState, active: &str) -> Option<String> {
    let announces = state.messaging.lock().await.list_announces();
    for relay in propagation_sync_candidate_relays(&announces, active,
        state.preferred_propagation_node_hex.as_deref()) {
        if automatic_propagation_poll_is_routed(state, &relay).await {
            return Some(relay);
        }
    }
    None
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
            if let Some(store)=&state.sdk.local_propagation {
                if let Ok(Some(record))=store.message(&message_id_hex) {
                    let _=store.delivered(&record.transient_id,&record.message_id,false,crate::numeric::u64_to_i64_saturating(now_ms() / 1000));
                    let _=publish_local_custody(&state,&bus).await;
                    // The custody owner also decides whether a late proof is still admissible.
                    continue;
                }
            }
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
                    &state.app_state, &bus,
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
