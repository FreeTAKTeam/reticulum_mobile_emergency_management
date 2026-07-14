#[expect(
    clippy::too_many_arguments,
    reason = "send policy boundary intentionally keeps transport, payload, metadata, and lane selection explicit"
)]
async fn send_lxmf_with_delivery_policy(
    state: &NodeRuntimeState,
    bus: &EventBus,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    send_mode: SendMode,
    send_task_class: SendTaskClass,
) -> Result<LxmfSendReport, NodeError> {
    const RETRY_DELAY: Duration = Duration::from_secs(10);
    const ACCEPTED_RESULT_RETRY_DELAY: Duration = Duration::from_secs(1);
    let has_active_relay = has_active_propagation_relay(state).await;
    let has_active_relay_transport =
        has_active_relay && has_active_relay_transport_interface(state).await;
    let rnode_only_transport = {
        let active_interfaces = state.active_interface_registry.lock().await;
        active_interfaces_are_rnode_ble_only(&active_interfaces)
    };
    let is_accepted_result = is_accepted_result_metadata(metadata.as_ref());
    let is_sos_status = is_sos_status_metadata(metadata.as_ref());
    let retry_delay = if is_accepted_result {
        ACCEPTED_RESULT_RETRY_DELAY
    } else {
        RETRY_DELAY
    };
    let normalized_requested_destination = normalize_hex_32(requested_destination_hex)
        .unwrap_or_else(|| requested_destination_hex.trim().to_ascii_lowercase());
    let canonical_requested_destination =
        canonical_app_destination_hex(state, normalized_requested_destination.as_str()).await;
    let is_saved_peer = saved_peer_matches_destination(
        state,
        normalized_requested_destination.as_str(),
        canonical_requested_destination.as_str(),
    )
    .await;
    let can_try_stored_lxmf_route = matches!(send_mode, SendMode::Auto {})
        && is_saved_peer
        && saved_peer_can_try_stored_lxmf_route(
            state,
            normalized_requested_destination.as_str(),
            canonical_requested_destination.as_str(),
        )
        .await;
    let require_current_peer = !is_accepted_result && !can_try_stored_lxmf_route;
    let direct_delivery_ready = if can_try_stored_lxmf_route {
        saved_peer_has_direct_ready_route(
            state,
            canonical_requested_destination.as_str(),
            has_active_relay_transport,
        )
        .await
    } else {
        false
    };
    let has_current_lxmf_route = if can_try_stored_lxmf_route {
        saved_peer_has_current_lxmf_route(state, canonical_requested_destination.as_str()).await
    } else {
        false
    };
    #[cfg(not(test))]
    let direct_priority_hops = if matches!(send_task_class, SendTaskClass::Mission)
        && matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && is_saved_peer
    {
        mission_direct_priority_hops(
            state,
            requested_destination_hex,
            canonical_requested_destination.as_str(),
        )
        .await
    } else {
        None
    };
    #[cfg(test)]
    let direct_priority_hops = None;
    let direct_attempts = direct_attempt_budget_for_send(
        send_mode,
        has_active_relay_transport,
        can_try_stored_lxmf_route,
        has_current_lxmf_route,
        direct_delivery_ready,
        direct_priority_hops,
    );
    let _destination_send_lock =
        if should_serialize_lxmf_destination_send(is_accepted_result, is_sos_status) {
            Some(
                state
                    .mission_destination_locks
                    .acquire(canonical_requested_destination.as_str())
                    .await?,
            )
        } else {
            None
        };
    let prefer_propagation = matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && has_active_relay_transport
        && is_saved_peer
        && saved_peer_prefers_propagation(
            state,
            requested_destination_hex,
            has_active_relay_transport,
            direct_priority_hops,
        )
        .await;

    if matches!(send_mode, SendMode::PropagationOnly {}) || prefer_propagation {
        let propagation_task_class = send_task_class.propagation_equivalent();
        if prefer_propagation {
            info!(
                "[lxmf][mission] saved peer {} is better suited for relay delivery; using propagation relay priority_hops={}",
                requested_destination_hex,
                direct_priority_hops
                    .map(|hops| hops.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
        let resolved_destination_hex =
            resolve_lxmf_destination_for_send(state, requested_destination_hex, false).await?;
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        log_send_task(
            propagation_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                propagation_task_class.label(),
                requested_destination_hex,
            ),
        );
        let _permit =
            acquire_send_task_permit(&state.send_task_permits, propagation_task_class).await?;
        log_send_task(
            propagation_task_class,
            format!(
                "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=initial-propagation",
                propagation_task_class.label(),
                requested_destination_hex,
            ),
        );
        return send_lxmf_via_propagation_candidates(
            state,
            destination,
            requested_destination_hex,
            body,
            title,
            fields_bytes,
            metadata,
        )
        .await;
    }

    #[cfg(not(test))]
    let mission_direct_admission_delay =
        mission_direct_priority_delay_for_hops(direct_priority_hops);

    let mut last_error: Option<NodeError> = None;
    let mut last_resolved_destination_hex: Option<String> = None;

    for attempt in 1..=direct_attempts {
        let resolved_destination_hex = resolve_lxmf_destination_for_send(
            state,
            requested_destination_hex,
            require_current_peer,
        )
        .await?;
        last_resolved_destination_hex = Some(resolved_destination_hex.clone());
        info!(
            "[lxmf][mission] resolved send requested_destination={} canonical_destination={} resolved_destination={} mode={:?} attempt={attempt}/{direct_attempts} require_current_peer={} saved_peer={} stored_lxmf_route={} active_relay={} relay_transport={} direct_ready={}",
            requested_destination_hex,
            canonical_requested_destination,
            resolved_destination_hex,
            send_mode,
            require_current_peer,
            is_saved_peer,
            can_try_stored_lxmf_route,
            has_active_relay,
            has_active_relay_transport,
            direct_delivery_ready,
        );
        let destination = parse_address_hash(resolved_destination_hex.as_str())?;
        let rnode_direct_route =
            rnode_only_transport || destination_uses_rnode_ble_route(state, &destination).await;
        let direct_link_connect_timeout =
            rnode_direct_route.then_some(RNODE_BLE_LINK_CONNECT_TIMEOUT);
        #[cfg(not(test))]
        if !mission_direct_admission_delay.is_zero() {
            info!(
                "[lxmf][queue] deferring {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts} priority_hops={} delay_ms={}",
                send_task_class.label(),
                requested_destination_hex,
                send_mode,
                direct_priority_hops.unwrap_or(u8::MAX),
                mission_direct_admission_delay.as_millis(),
            );
            tokio::time::sleep(mission_direct_admission_delay).await;
        }
        log_send_task(
            send_task_class,
            format!(
                "[lxmf][queue] waiting for {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts}",
                send_task_class.label(),
                requested_destination_hex,
                send_mode,
            ),
        );
        let send_result = {
            let _permit =
                acquire_send_task_permit(&state.send_task_permits, send_task_class).await?;
            log_send_task(
                send_task_class,
                format!(
                    "[lxmf][queue] acquired {} send slot destination={} mode={:?} attempt={attempt}/{direct_attempts}",
                    send_task_class.label(),
                    requested_destination_hex,
                    send_mode,
                ),
            );
            state
                .sdk
                .send_lxmf_with_direct_attempt(
                    destination,
                    body,
                    title.clone(),
                    fields_bytes.clone(),
                    metadata.clone(),
                    direct_attempt_send_mode(send_mode),
                    Some(attempt),
                    direct_link_connect_timeout,
                    rnode_direct_route.then_some(RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES),
                    None,
                )
                .await
        };
        match send_result {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                if !report.used_propagation_node {
                    if is_saved_peer && matches!(report.method, LxmfDeliveryMethod::Direct {}) {
                        register_desired_managed_peer_link(
                            state,
                            report.resolved_destination_hex.as_str(),
                        )
                        .await;
                    }
                    clear_peer_direct_delivery_unhealthy(
                        state,
                        requested_destination_hex,
                        Some(report.resolved_destination_hex.as_str()),
                    )
                    .await;
                    record_peer_link_state(
                        state,
                        bus,
                        report.resolved_destination_hex.as_str(),
                        true,
                    )
                    .await;
                }
                return Ok(report);
            }
            Ok(report) => {
                last_resolved_destination_hex = Some(report.resolved_destination_hex.clone());
                info!(
                    "[lxmf][mission] send attempt {attempt}/{direct_attempts} failed destination={} mode={:?} outcome={:?}",
                    requested_destination_hex,
                    send_mode,
                    report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                let retriable = is_retriable_lxmf_error(&err);
                info!(
                    "[lxmf][mission] send attempt {attempt}/{direct_attempts} errored destination={} mode={:?} err={}",
                    requested_destination_hex,
                    send_mode,
                    err,
                );
                last_error = Some(err);
                if !retriable {
                    break;
                }
            }
        }

        if attempt < direct_attempts {
            log_send_task(
                send_task_class,
                format!(
                    "[lxmf][queue] sleeping before retry destination={} mode={:?} next_attempt={}/{} delay_ms={}",
                    requested_destination_hex,
                    send_mode,
                    attempt + 1,
                    direct_attempts,
                    retry_delay.as_millis(),
                ),
            );
            tokio::time::sleep(retry_delay).await;
        }
    }

    if !matches!(send_mode, SendMode::Auto {}) || !has_active_relay_transport {
        return Err(last_error.unwrap_or(NodeError::NetworkError {}));
    }

    if direct_attempts == 0 {
        info!(
            "[lxmf][mission] auto delivery using propagation without direct probe destination={} saved_peer={} stored_lxmf_route={} active_relay={} relay_transport={} direct_ready={}",
            requested_destination_hex,
            is_saved_peer,
            can_try_stored_lxmf_route,
            has_active_relay,
            has_active_relay_transport,
            direct_delivery_ready,
        );
    } else {
        if !should_try_propagation_after_direct_failure(
            send_mode,
            is_accepted_result,
            has_active_relay_transport,
            is_saved_peer,
            last_error.as_ref().is_some_and(is_retriable_lxmf_error),
        ) {
            return Err(last_error.unwrap_or(NodeError::NetworkError {}));
        }
        mark_peer_direct_delivery_unhealthy(
            state,
            requested_destination_hex,
            last_resolved_destination_hex.as_deref(),
        )
        .await;
        close_output_links_for_direct_delivery_failure(
            state,
            requested_destination_hex,
            last_resolved_destination_hex.as_deref(),
        )
        .await;
        record_peer_link_state(state, bus, requested_destination_hex, false).await;
        if let Some(target) =
            register_desired_managed_peer_link(state, requested_destination_hex).await
        {
            if let ManagedPeerReconnectStart::Started(target) = state
                .managed_peer_links
                .begin_reconnect(target.destination_hex.as_str())
                .await
            {
                spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
            }
        }
        info!(
            "[lxmf][mission] auto delivery exhausted destination={}; retrying via propagation relay",
            requested_destination_hex,
        );
    }
    let resolved_destination_hex =
        resolve_lxmf_destination_for_send(state, requested_destination_hex, false).await?;
    let destination = parse_address_hash(resolved_destination_hex.as_str())?;
    let propagation_task_class = send_task_class.direct_recovery_equivalent();
    log_send_task(
        propagation_task_class,
        format!(
            "[lxmf][queue] waiting for {} send slot destination={} mode=PropagationOnly stage=fallback",
            propagation_task_class.label(),
            requested_destination_hex,
        ),
    );
    let _permit =
        acquire_send_task_permit(&state.send_task_permits, propagation_task_class).await?;
    log_send_task(
        propagation_task_class,
        format!(
            "[lxmf][queue] acquired {} send slot destination={} mode=PropagationOnly stage=fallback",
            propagation_task_class.label(),
            requested_destination_hex,
        ),
    );
    let mut report = send_lxmf_via_propagation_candidates(
        state,
        destination,
        requested_destination_hex,
        body,
        title,
        fields_bytes,
        metadata,
    )
    .await?;
    report.fallback_stage = Some(LxmfFallbackStage::AfterDirectRetryBudget {});
    Ok(report)
}

async fn send_lxmf_via_propagation_candidates(
    state: &NodeRuntimeState,
    destination: AddressHash,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
) -> Result<LxmfSendReport, NodeError> {
    let active_relay = state.active_propagation_node_hex.lock().await.clone();
    let active_relay_hex = active_relay.as_deref().unwrap_or("");
    let announces = state.messaging.lock().await.list_announces();
    let mut relay_candidates = propagation_sync_candidate_relays(
        announces.as_slice(),
        active_relay_hex,
        state.preferred_propagation_node_hex.as_deref(),
    );
    if relay_candidates.is_empty() {
        return Err(delivery_route_unavailable_error());
    }

    let mut last_error = None;
    for (index, relay_candidate) in relay_candidates.drain(..).enumerate() {
        let attempt_number = index + 1;
        info!(
            "[lxmf][mission] propagation send relay attempt relay={} attempt={}/{} destination={}",
            relay_candidate,
            attempt_number,
            PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS,
            requested_destination_hex,
        );

        match state
            .sdk
            .send_lxmf_via_propagation_relay(
                destination,
                body,
                title.clone(),
                fields_bytes.clone(),
                metadata.clone(),
                relay_candidate.clone(),
            )
            .await
        {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                return Ok(report);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={} destination={} outcome={:?}",
                    relay_candidate, requested_destination_hex, report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={} destination={} reason={}",
                    relay_candidate, requested_destination_hex, err,
                );
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or(NodeError::NetworkError {}))
}
