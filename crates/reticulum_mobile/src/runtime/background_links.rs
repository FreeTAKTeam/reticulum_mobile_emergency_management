fn spawn_link_event_listener(state: &NodeRuntimeState, bus: &EventBus) {
    let transport = state.transport.clone();
    let bus = bus.clone();
    let connected_peers = state.connected_peers.clone();
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

fn spawn_periodic_hub_refresh(config: &NodeConfig, state: &NodeRuntimeState, bus: &EventBus) {
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
                match refresh_hub_directory_lxmf(&config, &state).await {
                    Ok(snapshot) => publish_hub_directory_snapshot(&state, &bus, snapshot).await,
                    Err(error) => {
                        publish_failed_hub_directory_refresh(&state, &bus, &error).await;
                    }
                }
            }
        });
    }
}
