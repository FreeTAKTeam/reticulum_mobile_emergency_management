async fn run_propagation_sync_job(
    state: NodeRuntimeState,
    bus: EventBus,
    limit: Option<u32>,
    requested_at_ms: u64,
    relay_hex: String,
) {
    let mut announces = state.messaging.lock().await.list_announces();
    let mut relay_candidates = propagation_sync_candidate_relays(
        announces.as_slice(),
        relay_hex.as_str(),
        state.preferred_propagation_node_hex.as_deref(),
    );

    info!(
        "[sync] propagation sync started relay={} candidates={} limit={}",
        relay_hex,
        relay_candidates.len(),
        limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    let mut last_failure: Option<(String, NodeError)> = None;
    let mut attempted_relays = HashSet::new();
    while attempted_relays.len() < PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS {
        let Some(relay_candidate) = relay_candidates
            .iter()
            .find(|candidate| !attempted_relays.contains(candidate.as_str()))
            .cloned()
        else {
            break;
        };
        attempted_relays.insert(relay_candidate.clone());
        let attempt_number = attempted_relays.len();
        *state.active_propagation_node_hex.lock().await = Some(relay_candidate.clone());
        let active_status = from_sdk_sync_status(
            state
                .messaging
                .lock()
                .await
                .set_active_propagation_node(Some(relay_candidate.clone())),
        );
        if refresh_sync_status_snapshot(&state, &active_status) {
            bus.emit(NodeEvent::SyncUpdated {
                status: active_status,
            });
        }

        info!(
            "[sync] propagation sync relay attempt relay={relay_candidate} attempt={attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS}"
        );
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::PathRequested,
            requested_at_ms,
            0,
            Some(format!(
                "requesting path to propagation relay {relay_candidate} ({attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS})"
            )),
            false,
        )
        .await;
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::LinkEstablishing,
            requested_at_ms,
            0,
            Some(format!(
                "establishing propagation link {relay_candidate} ({attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS})"
            )),
            false,
        )
        .await;
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::RequestSent,
            requested_at_ms,
            0,
            Some(format!(
                "requesting propagated messages ({attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS})"
            )),
            false,
        )
        .await;

        let result = match state
            .sdk
            .fetch_propagated_lxmf_from_relay(relay_candidate.as_str(), limit, None)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                info!(
                    "[sync] propagation sync relay attempt failed relay={relay_candidate} attempt={attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS} reason={err}"
                );
                last_failure = Some((relay_candidate.clone(), err));
                sync_auto_propagation_node(&state, &bus).await;
                announces = state.messaging.lock().await.list_announces();
                let active_relay = state
                    .active_propagation_node_hex
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_else(|| relay_hex.clone());
                for refreshed_candidate in propagation_sync_candidate_relays(
                    announces.as_slice(),
                    active_relay.as_str(),
                    state.preferred_propagation_node_hex.as_deref(),
                ) {
                    if !attempted_relays.contains(refreshed_candidate.as_str())
                        && !relay_candidates.contains(&refreshed_candidate)
                    {
                        relay_candidates.push(refreshed_candidate);
                    }
                }
                continue;
            }
        };

        let destination_hex = result.destination_hex.clone();
        let available_count = result.available_count;
        let fetched_count = result.fetched_count;
        let fetched_entry_count = result.fetched_entry_count;
        let extracted_payload_count = result.extracted_payload_count;
        let failed_count = result.failed_count;
        let malformed_count = result.malformed_count;
        let decrypt_failed_count = result.decrypt_failed_count;
        let imported_count =
            crate::numeric::usize_to_u32_saturating(result.imported_wires.len());
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::Receiving,
            requested_at_ms,
            0,
            Some(format!(
                "available={available_count} fetched_entries={fetched_entry_count} extracted_payloads={extracted_payload_count} decrypt_failed={decrypt_failed_count}"
            )),
            false,
        )
        .await;
        for wire in result.imported_wires {
            emit_received_payload(
                &state,
                &bus,
                &state.sdk,
                destination_hex.clone(),
                wire,
                None,
                true,
            )
            .await;
        }
        let detail = format!(
            "available={available_count} fetched={fetched_count} fetched_entries={fetched_entry_count} extracted_payloads={extracted_payload_count} imported={imported_count} malformed={malformed_count} decrypt_failed={decrypt_failed_count} failed={failed_count}"
        );
        emit_sync_status_update(
            &state,
            &bus,
            sdkmsg::SyncPhase::Complete,
            requested_at_ms,
            imported_count,
            Some(detail.clone()),
            true,
        )
        .await;
        info!(
            "[sync] propagation sync complete relay={relay_candidate} {detail}"
        );
        state
            .propagation_sync_inflight
            .store(false, Ordering::Release);
        return;
    }

    let (failed_relay, err) =
        last_failure.unwrap_or_else(|| (relay_hex.clone(), NodeError::InvalidConfig {}));
    let detail = format!(
        "propagation sync failed: all relay attempts failed (last relay {failed_relay}: {err})"
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
    state
        .propagation_sync_inflight
        .store(false, Ordering::Release);
}

async fn sync_auto_propagation_node(state: &NodeRuntimeState, bus: &EventBus) {
    let announces = {
        let messaging = state.messaging.lock().await;
        messaging.list_announces()
    };
    let current_destination = state.active_propagation_node_hex.lock().await.clone();
    // This helper only selects the relay. Automatic polling remains gated in
    // `spawn_propagation_maintenance_task`; explicit sync must still select a
    // relay when an RNode is the only active transport.
    let desired_destination = announces
        .iter()
        .filter(|record| record.destination_kind == "lxmf_propagation")
        .min_by_key(|record| {
            propagation_candidate_sort_key(
                record,
                state.preferred_propagation_node_hex.as_deref(),
                current_destination.as_deref(),
            )
        })
        .map(|record| record.destination_hex.clone());

    let mut active_guard = state.active_propagation_node_hex.lock().await;
    if *active_guard == desired_destination {
        return;
    }
    info!(
        "[sync] auto propagation relay {}",
        desired_destination
            .as_deref()
            .map(|value| format!("selected {value}"))
            .unwrap_or_else(|| "cleared".to_string())
    );
    *active_guard = desired_destination.clone();
    drop(active_guard);

    let status = from_sdk_sync_status(
        state
            .messaging
            .lock()
            .await
            .set_active_propagation_node(desired_destination),
    );
    if refresh_sync_status_snapshot(state, &status) {
        bus.emit(NodeEvent::SyncUpdated { status });
    }
}

async fn wait_for_active_propagation_relay(
    state: &NodeRuntimeState,
    bus: &EventBus,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + PROPAGATION_SYNC_RELAY_SELECTION_WAIT;
    loop {
        sync_auto_propagation_node(state, bus).await;
        if let Some(relay_hex) = state
            .active_propagation_node_hex
            .lock()
            .await
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            return Some(relay_hex);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(PROPAGATION_SYNC_RELAY_SELECTION_POLL).await;
    }
}

async fn resolve_peer_route(
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: &str,
) -> Result<(), NodeError> {
    if *state.power_saver_rx.borrow() {
        return Ok(());
    }
    let destination = parse_address_hash(destination_hex)?;
    let attempted_at_ms = now_ms();
    {
        let mut messaging = state.messaging.lock().await;
        messaging.record_resolution_attempt(destination_hex, attempted_at_ms);
    }
    emit_peer_changed(state, bus, destination_hex).await;

    state.transport.request_path(&destination, None, None).await;
    let desc = ensure_destination_desc(state, destination, None).await?;
    let identity_hex = desc.identity.address_hash.to_hex_string();
    let lxmf_desc = SingleOutputDestination::new(
        desc.identity,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    )
    .desc;
    let lxmf_destination_hex = lxmf_desc.address_hash.to_hex_string();
    {
        let mut messaging = state.messaging.lock().await;
        messaging.record_resolution_result(
            destination_hex,
            identity_hex.as_str(),
            lxmf_destination_hex.as_str(),
            now_ms(),
        );
    }
    let desired_link_target = {
        let messaging = state.messaging.lock().await;
        if messaging.is_peer_saved(destination_hex) {
            messaging
                .peer_by_destination(destination_hex)
                .and_then(|peer| managed_peer_link_target(&peer))
        } else {
            None
        }
    };
    emit_peer_changed(state, bus, destination_hex).await;
    emit_peer_resolved_for_destination(state, bus, destination_hex).await;
    if !*state.power_saver_rx.borrow() {
        if let Some(target) = desired_link_target {
        add_desired_managed_peer_link_and_schedule(state, bus, target, "saved-peer-resolution")
            .await;
        }
        sync_auto_propagation_node(state, bus).await;
    }
    Ok(())
}

fn spawn_managed_peer_resolution(state: NodeRuntimeState, bus: EventBus, destination_hex: String) {
    tokio::spawn(async move {
        let Some(destination_hex) = normalize_hex_32(destination_hex.as_str()) else {
            return;
        };
        {
            let mut inflight = state.peer_resolution_inflight.lock().await;
            if !inflight.insert(destination_hex.clone()) {
                return;
            }
        }

        let retry_delays_secs = [0_u64, 3, 8, 15, 30];
        for delay_secs in retry_delays_secs {
            if *state.power_saver_rx.borrow() {
                state
                    .peer_resolution_inflight
                    .lock()
                    .await
                    .remove(destination_hex.as_str());
                return;
            }
            if delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }

            let (should_retry, cached_target) = {
                let messaging = state.messaging.lock().await;
                if !messaging.is_peer_saved(destination_hex.as_str()) {
                    (false, None)
                } else {
                    let peer = messaging.peer_by_destination(destination_hex.as_str());
                    let should_retry = peer
                        .as_ref()
                        .is_none_or(|peer| !sdk_peer_has_known_delivery_route(peer));
                    let cached_target = if should_retry {
                        None
                    } else {
                        peer.as_ref().and_then(managed_peer_link_target)
                    };
                    (should_retry, cached_target)
                }
            };

            if !should_retry {
                if let Some(target) = cached_target {
                    add_desired_managed_peer_link_and_schedule(
                        &state,
                        &bus,
                        target,
                        "saved-peer-resolution-cached",
                    )
                    .await;
                }
                state
                    .peer_resolution_inflight
                    .lock()
                    .await
                    .remove(destination_hex.as_str());
                return;
            }

            if let Err(err) = resolve_peer_route(&state, &bus, destination_hex.as_str()).await {
                state
                    .messaging
                    .lock()
                    .await
                    .record_resolution_error(destination_hex.as_str(), Some(err.to_string()));
                emit_peer_changed(&state, &bus, destination_hex.as_str()).await;
            } else {
                state
                    .peer_resolution_inflight
                    .lock()
                    .await
                    .remove(destination_hex.as_str());
                return;
            }
        }
        state
            .peer_resolution_inflight
            .lock()
            .await
            .remove(destination_hex.as_str());
    });
}

fn saved_peer_destinations_needing_route_refresh(
    messaging: &sdkmsg::MessagingStore,
) -> Vec<String> {
    let mut destinations = messaging
        .list_peers()
        .into_iter()
        .filter(|peer| peer.saved && !sdk_peer_has_known_delivery_route(peer))
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    destinations.sort();
    destinations.dedup();
    destinations
}

fn spawn_passive_peer_resolution(state: NodeRuntimeState, bus: EventBus, destination_hex: String) {
    tokio::spawn(async move {
        let should_resolve = {
            let messaging = state.messaging.lock().await;
            match messaging.peer_by_destination(destination_hex.as_str()) {
                Some(peer) => {
                    (peer.identity_hex.is_none() || peer.lxmf_destination_hex.is_none())
                        && peer
                            .last_resolution_attempt_at_ms
                            .is_none_or(|attempted_at_ms| {
                                now_ms().saturating_sub(attempted_at_ms)
                                    >= PASSIVE_PEER_RESOLUTION_MIN_INTERVAL_MS
                            })
                }
                None => false,
            }
        };
        if !should_resolve {
            return;
        }

        {
            let mut inflight = state.peer_resolution_inflight.lock().await;
            if !inflight.insert(destination_hex.clone()) {
                return;
            }
        }

        let _ = resolve_peer_route(&state, &bus, destination_hex.as_str()).await;
        state
            .peer_resolution_inflight
            .lock()
            .await
            .remove(destination_hex.as_str());
    });
}
