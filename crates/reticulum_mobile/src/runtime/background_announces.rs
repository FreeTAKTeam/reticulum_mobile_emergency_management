fn spawn_announce_tasks(
    config: &NodeConfig,
    state: &NodeRuntimeState,
    bus: &EventBus,
    app_destination: &Arc<TokioMutex<SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<String>>,
) {
    {
        let transport = state.transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = state.lxmf_destination.clone();
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
        let transport = state.transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = state.lxmf_destination.clone();
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

    spawn_announce_receiver(state, bus);
}

fn spawn_announce_receiver(state: &NodeRuntimeState, bus: &EventBus) {
    let transport = state.transport.clone();
    let bus = bus.clone();
    let sdk = state.sdk.clone();
    let known_destinations = state.known_destinations.clone();
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
                        emit_peer_resolved_for_destination(&state, &bus, &destination_hex).await;
                        spawn_passive_peer_resolution(
                            state.clone(),
                            bus.clone(),
                            destination_hex.clone(),
                        );
                    } else if destination_kind == DESTINATION_KIND_LXMF_DELIVERY {
                        let app_destination_hex = SingleOutputDestination::new(
                            desc.identity,
                            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
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
                        emit_peer_resolved_for_destination(&state, &bus, &destination_hex).await;
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
