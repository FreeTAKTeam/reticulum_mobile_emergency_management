async fn acquire_send_task_permit(
    permits: &SendTaskPermits,
    class: SendTaskClass,
) -> Result<OwnedSemaphorePermit, NodeError> {
    permits.acquire(class).await
}

async fn ensure_destination_desc(
    state: &NodeRuntimeState,
    dest: AddressHash,
    expected_name: Option<DestinationName>,
) -> Result<DestinationDesc, NodeError> {
    if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
        return Ok(desc);
    }

    state.transport.request_path(&dest, None, None).await;

    let deadline = tokio::time::Instant::now() + DEFAULT_IDENTITY_WAIT_TIMEOUT;
    loop {
        if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
            return Ok(desc);
        }

        if let Some(identity) = state.transport.destination_identity(&dest).await {
            let name = expected_name.unwrap_or_else(|| {
                DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1)
            });
            return Ok(DestinationDesc {
                identity,
                address_hash: dest,
                name,
            });
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn ensure_output_link(
    state: &NodeRuntimeState,
    desc: DestinationDesc,
) -> Result<Arc<TokioMutex<Link>>, NodeError> {
    const DEFAULT_MAX_ATTEMPTS: usize = 3;
    const RNODE_BLE_MAX_ATTEMPTS: usize = 1;
    const RETRY_DELAY: Duration = Duration::from_millis(500);
    let rnode_route = destination_uses_rnode_ble_route(state, &desc.address_hash).await;
    let max_attempts = if rnode_route {
        RNODE_BLE_MAX_ATTEMPTS
    } else {
        DEFAULT_MAX_ATTEMPTS
    };
    let connect_timeout = link_connect_timeout(rnode_route);

    for attempt in 0..max_attempts {
        let link = {
            let mut links = state.out_links.lock().await;
            if let Some(existing) = links.get(&desc.address_hash).cloned() {
                existing
            } else {
                let created = state.transport.link(desc).await;
                links.insert(desc.address_hash, created.clone());
                created
            }
        };

        match wait_for_link_active(&state.transport, &link, connect_timeout).await {
            Ok(()) => return Ok(link),
            Err(err) => {
                let stale = state.out_links.lock().await.remove(&desc.address_hash);
                if let Some(stale) = stale {
                    stale.lock().await.close();
                }
                if attempt + 1 == max_attempts {
                    return Err(err);
                }
                info!(
                    "[lxmf][events] link activation retry destination={} attempt={} timeout_ms={} reason={}",
                    address_hash_to_hex(&desc.address_hash),
                    attempt + 1,
                    connect_timeout.as_millis(),
                    err,
                );
                state
                    .transport
                    .request_path(&desc.address_hash, None, None)
                    .await;
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(NodeError::Timeout {})
}

fn managed_peer_link_target(peer: &sdkmsg::PeerRecord) -> Option<ManagedPeerLinkTarget> {
    let normalized_destination_hex = normalize_hex_32(peer.destination_hex.as_str());
    let has_rem_capabilities = peer
        .app_data
        .as_deref()
        .is_some_and(app_data_has_rem_peer_capabilities);
    let has_saved_lxmf_route_target = peer.saved
        && (peer
            .lxmf_destination_hex
            .as_deref()
            .and_then(normalize_hex_32)
            .is_some()
            || (normalized_destination_hex.is_some()
                && has_rem_capabilities
                && peer.lxmf_last_seen_at_ms.is_some()));
    if peer.stale && !has_saved_lxmf_route_target {
        return None;
    }
    if !peer.saved && !has_rem_capabilities {
        return None;
    }
    if let Some(destination_hex) = peer
        .lxmf_destination_hex
        .as_deref()
        .and_then(normalize_hex_32)
    {
        return Some(ManagedPeerLinkTarget {
            destination_hex,
            kind: ManagedPeerLinkKind::LxmfDelivery,
        });
    }
    normalized_destination_hex.map(|destination_hex| {
        let kind = if peer.saved && has_rem_capabilities && peer.lxmf_last_seen_at_ms.is_some() {
            ManagedPeerLinkKind::LxmfDelivery
        } else {
            ManagedPeerLinkKind::App
        };
        ManagedPeerLinkTarget {
            destination_hex,
            kind,
        }
    })
}

#[cfg(test)]
fn saved_peer_link_targets(peers: &[sdkmsg::PeerRecord]) -> Vec<ManagedPeerLinkTarget> {
    let mut seen = HashSet::<String>::new();
    let mut targets = Vec::<ManagedPeerLinkTarget>::new();
    for peer in peers {
        let Some(target) = managed_peer_link_target(peer) else {
            continue;
        };
        if seen.insert(target.destination_hex.clone()) {
            targets.push(target);
        }
    }
    targets
}

async fn desired_managed_peer_link_target_for_destination(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<ManagedPeerLinkTarget> {
    peer_for_any_destination_hex(state, destination_hex)
        .await
        .and_then(|peer| managed_peer_link_target(&peer))
}

async fn register_desired_managed_peer_link(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<ManagedPeerLinkTarget> {
    if !has_active_reticulum_interface(state).await {
        return None;
    }
    let target = desired_managed_peer_link_target_for_destination(state, destination_hex).await?;
    state.managed_peer_links.add_desired(target.clone()).await;
    Some(target)
}

async fn add_desired_managed_peer_link_and_schedule(
    state: &NodeRuntimeState,
    bus: &EventBus,
    target: ManagedPeerLinkTarget,
    reason: &str,
) {
    state.managed_peer_links.add_desired(target.clone()).await;
    if !has_active_reticulum_interface(state).await {
        info!(
            "[link][maintain] destination={} status=deferred reason={} detail=no-active-reticulum-interface",
            target.destination_hex, reason,
        );
        return;
    }
    if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
        if output_link_is_active(state, &destination).await {
            clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                .await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
            info!(
                "[link][maintain] destination={} status=active reason={}",
                target.destination_hex, reason,
            );
            return;
        }
    }
    state
        .managed_peer_links
        .clear_failure(target.destination_hex.as_str())
        .await;
    match state
        .managed_peer_links
        .begin_reconnect(target.destination_hex.as_str())
        .await
    {
        ManagedPeerReconnectStart::Started(target) => {
            info!(
                "[link][maintain] destination={} status=scheduled reason={}",
                target.destination_hex, reason,
            );
            spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
        }
        ManagedPeerReconnectStart::AlreadyReconnecting => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=reconnecting",
                target.destination_hex, reason,
            );
        }
        ManagedPeerReconnectStart::Backoff {
            next_retry_at_ms,
            last_failure_reason,
        } => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=backoff next_retry_at_ms={} last_failure={}",
                target.destination_hex,
                reason,
                next_retry_at_ms,
                last_failure_reason.as_deref().unwrap_or("-"),
            );
        }
        ManagedPeerReconnectStart::NotDesired => {
            info!(
                "[link][maintain] destination={} status=deferred reason={} detail=not-desired",
                target.destination_hex, reason,
            );
        }
    }
}

async fn output_link_is_active(state: &NodeRuntimeState, destination: &AddressHash) -> bool {
    let link = state.out_links.lock().await.get(destination).cloned();
    let Some(link) = link else {
        return false;
    };
    let active = link.lock().await.status() == LinkStatus::Active;
    active
}

async fn ensure_managed_peer_link(
    state: &NodeRuntimeState,
    bus: &EventBus,
    target: ManagedPeerLinkTarget,
) -> Result<(), NodeError> {
    if !has_active_reticulum_interface(state).await {
        return Err(NodeError::NetworkError {});
    }
    let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) else {
        return Err(NodeError::InvalidConfig {});
    };
    if output_link_is_active(state, &destination).await {
        clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None).await;
        record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
        return Ok(());
    }
    info!(
        "[link][maintain] destination={} status=connecting kind={:?}",
        target.destination_hex, target.kind,
    );
    let desc =
        match ensure_destination_desc(state, destination, Some(target.kind.destination_name()))
            .await
        {
            Ok(desc) => desc,
            Err(err) => {
                mark_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(state, bus, target.destination_hex.as_str(), false).await;
                info!(
                    "[link][maintain] destination={} status=resolve-failed kind={:?} reason={}",
                    target.destination_hex, target.kind, err,
                );
                return Err(err);
            }
        };
    match ensure_output_link(state, desc).await {
        Ok(_) => {
            clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                .await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
            info!(
                "[link][maintain] destination={} status=active kind={:?}",
                target.destination_hex, target.kind,
            );
            Ok(())
        }
        Err(err) => {
            mark_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None).await;
            record_peer_link_state(state, bus, target.destination_hex.as_str(), false).await;
            info!(
                "[link][maintain] destination={} status=failed kind={:?} reason={}",
                target.destination_hex, target.kind, err,
            );
            Err(err)
        }
    }
}

async fn maintain_managed_peer_links_once(state: &NodeRuntimeState, bus: &EventBus) {
    if !has_active_reticulum_interface(state).await {
        return;
    }
    let targets = state.managed_peer_links.desired_targets().await;
    for target in targets {
        if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
            if output_link_is_active(state, &destination).await {
                clear_peer_direct_delivery_unhealthy(state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(state, bus, target.destination_hex.as_str(), true).await;
                continue;
            }
        }
        let still_saved_and_current =
            peer_for_any_destination_hex(state, target.destination_hex.as_str())
                .await
                .is_some_and(|peer| managed_peer_link_target(&peer).is_some());
        if still_saved_and_current {
            match state
                .managed_peer_links
                .begin_reconnect(target.destination_hex.as_str())
                .await
            {
                ManagedPeerReconnectStart::Started(target) => {
                    info!(
                        "[link][maintain] destination={} status=scheduled reason=periodic-maintenance",
                        target.destination_hex,
                    );
                    spawn_managed_peer_link_reconnect(state.clone(), bus.clone(), target);
                }
                ManagedPeerReconnectStart::AlreadyReconnecting
                | ManagedPeerReconnectStart::Backoff { .. }
                | ManagedPeerReconnectStart::NotDesired => {}
            }
        } else {
            state
                .managed_peer_links
                .remove_desired([target.destination_hex.as_str()])
                .await;
        }
    }
}

fn spawn_managed_peer_link_reconnect(
    state: NodeRuntimeState,
    bus: EventBus,
    target: ManagedPeerLinkTarget,
) {
    tokio::spawn(async move {
        tokio::time::sleep(SAVED_PEER_LINK_RECONNECT_DELAY).await;
        let result = match tokio::time::timeout(
            MANAGED_PEER_LINK_RECONNECT_TIMEOUT,
            ensure_managed_peer_link(&state, &bus, target.clone()),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                mark_peer_direct_delivery_unhealthy(&state, target.destination_hex.as_str(), None)
                    .await;
                record_peer_link_state(&state, &bus, target.destination_hex.as_str(), false).await;
                if let Ok(destination) = parse_address_hash(target.destination_hex.as_str()) {
                    if let Some(stale) = state.out_links.lock().await.remove(&destination) {
                        stale.lock().await.close();
                    }
                }
                info!(
                    "[link][maintain] destination={} status=failed kind={:?} reason=reconnect-timeout timeout_ms={}",
                    target.destination_hex,
                    target.kind,
                    MANAGED_PEER_LINK_RECONNECT_TIMEOUT.as_millis(),
                );
                Err(NodeError::Timeout {})
            }
        };
        state
            .managed_peer_links
            .finish_reconnect(
                &target,
                result.as_ref().map(|_| ()).map_err(ToString::to_string),
            )
            .await;
        if let Err(err) = result {
            info!(
                "[link][maintain] destination={} status=reconnect-backoff reason={}",
                target.destination_hex, err,
            );
        }
    });
}

async fn register_pending_lxmf_delivery(
    state: &NodeRuntimeState,
    report: &LxmfSendReport,
    resend: Option<PendingLxmfResend>,
    message_id_override: Option<String>,
) -> Option<RegisteredPendingLxmfDelivery> {
    if !report.track_delivery_timeout {
        return None;
    }
    let metadata = report.metadata.as_ref()?;
    let tracking_key = metadata.tracking_key()?.to_string();
    let pending = PendingLxmfDelivery {
        message_id_hex: message_id_override.unwrap_or_else(|| report.message_id_hex.clone()),
        destination_hex: report.resolved_destination_hex.clone(),
        correlation_id: metadata.correlation_id.clone(),
        command_id: metadata.command_id.clone(),
        command_type: metadata.command_type.clone(),
        event_uid: metadata.event_uid.clone(),
        mission_uid: metadata.mission_uid.clone(),
        method: report.method,
        representation: report.representation,
        relay_destination_hex: report.relay_destination_hex.clone(),
        fallback_stage: report.fallback_stage,
        resend,
        sent_at_ms: now_ms(),
    };

    state
        .pending_lxmf_deliveries
        .lock()
        .await
        .insert(tracking_key.clone(), pending.clone());
    let buffered_ack = state
        .pending_lxmf_acknowledgements
        .lock()
        .await
        .remove(&tracking_key);
    Some(RegisteredPendingLxmfDelivery {
        pending,
        buffered_ack,
    })
}
