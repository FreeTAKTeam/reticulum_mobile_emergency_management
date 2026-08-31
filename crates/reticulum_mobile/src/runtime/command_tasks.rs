fn spawn_manual_announce(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    transport: &Arc<Transport>,
    app_destination: &Arc<TokioMutex<SingleInputDestination>>,
    lxmf_destination: &Arc<TokioMutex<SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<AnnounceProfile>>,
) {
    let transport = transport.clone();
    let app_destination = app_destination.clone();
    let lxmf_destination = lxmf_destination.clone();
    let announce_capabilities = announce_capabilities.clone();
    executor.spawn_detached(lane, RuntimeCommandClass::Control, async move {
        announce_destinations(
            &transport,
            &app_destination,
            &lxmf_destination,
            &announce_capabilities,
            "manual",
        )
        .await;
    });
}

fn spawn_announce_capability_update(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    transport: &Arc<Transport>,
    destinations: (
        &Arc<TokioMutex<SingleInputDestination>>,
        &Arc<TokioMutex<SingleInputDestination>>,
    ),
    announce_capabilities: &Arc<TokioMutex<AnnounceProfile>>,
    capability_string: String,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let transport = transport.clone();
    let app_destination = destinations.0.clone();
    let lxmf_destination = destinations.1.clone();
    let announce_capabilities = announce_capabilities.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        announce_capabilities
            .lock()
            .await
            .set_capabilities(capability_string.as_str());
        announce_destinations(
            &transport,
            &app_destination,
            &lxmf_destination,
            &announce_capabilities,
            "capabilities-updated",
        )
        .await;
        Ok(())
    });
}

fn spawn_connect_peer_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: String,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        let destination_hex_copy = destination_hex.clone();
        let result = async {
            let dest = parse_address_hash(&destination_hex)?;
            let directory_member = state
                .hub_directory_snapshot
                .lock()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(
                        NodeError::InternalError {},
                        error,
                    )
                })?
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot.members.iter().any(|member| {
                        normalize_hex_32(member.destination_hash.as_str()).as_deref()
                            == Some(destination_hex.as_str())
                    })
                });
            if !directory_member {
                let saved_peer =
                    persist_selected_peer_destination(&state, &bus, destination_hex.as_str())
                        .await?;
                emit_peer_changed(&state, &bus, saved_peer.destination_hex.as_str()).await;
            }
            clear_ignored_peer_destinations(&state, std::slice::from_ref(&destination_hex)).await;
            state.sdk.record_peer_changed(
                destination_hex.as_str(),
                PeerState::Connecting {},
                None,
            );
            resolve_peer_route(&state, &bus, &destination_hex).await?;
            let target = match register_desired_managed_peer_link(&state, &destination_hex).await {
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
            record_peer_link_state(&state, &bus, target.destination_hex.as_str(), true).await;
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
        result
    });
}

fn spawn_disconnect_peer_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: String,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        let dest = parse_address_hash(&destination_hex)?;
        let mut destinations = vec![destination_hex.clone()];
        if let Some(peer) = peer_for_any_destination_hex(&state, &destination_hex).await {
            destinations.extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
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
        state.connected_peers.lock().await.remove(&dest);
        for destination in &destinations {
            if let Ok(destination) = parse_address_hash(destination.as_str()) {
                state.connected_peers.lock().await.remove(&destination);
                if let Some(link) = state.out_links.lock().await.remove(&destination) {
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
        Ok(())
    });
}

fn spawn_saved_peer_projection_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    peers: Vec<SavedPeerRecord>,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        apply_saved_peer_management_projection(&state, &bus, &peers).await?;
        refresh_peer_snapshot(&state).await;
        Ok(())
    });
}

fn spawn_sync_request_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    limit: Option<u32>,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        let requested_at_ms = now_ms();
        if state.propagation_sync_inflight.load(Ordering::Acquire) {
            info!("[sync] propagation sync request ignored reason=inflight");
            return Ok(());
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
            return Err(NodeError::InvalidConfig {});
        };
        if state
            .propagation_sync_inflight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            info!("[sync] propagation sync request ignored reason=inflight");
            return Ok(());
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
        Ok(())
    });
}

fn spawn_broadcast_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    bytes: Vec<u8>,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Work, resp, async move {
        let peers = state
            .connected_peers
            .lock()
            .await
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let mut sent_any = false;
        for dest in peers {
            let outcome =
                send_transport_packet_with_path_retry(&state.transport, dest, &bytes).await;
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
            Ok(())
        } else {
            let error = NodeError::NetworkError {};
            bus.emit(NodeEvent::Error {
                code: node_error_code(&error).to_string(),
                message: format!("broadcast_bytes failed reason={error}"),
            });
            Err(error)
        }
    });
}
fn mark_runtime_stopped(status: &Arc<Mutex<NodeStatus>>, bus: &EventBus) {
    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
}
