fn sdk_peer_is_direct_delivery_ready(peer: &sdkmsg::PeerRecord, has_active_relay: bool) -> bool {
    let _ = has_active_relay;
    delivery_policy::peer_is_direct_delivery_ready(peer)
}

fn sdk_peer_has_known_lxmf_route(peer: &sdkmsg::PeerRecord) -> bool {
    delivery_policy::peer_has_known_lxmf_route(peer)
}

fn sdk_peer_has_observed_lxmf_delivery_route(peer: &sdkmsg::PeerRecord) -> bool {
    delivery_policy::peer_has_observed_lxmf_delivery_route(
        peer,
        now_ms(),
        sdkmsg::DEFAULT_PEER_STALE_AFTER_MS,
    )
}

async fn saved_peer_prefers_propagation(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    has_active_relay: bool,
    direct_priority_hops: Option<u8>,
) -> bool {
    if !has_active_relay {
        return false;
    }

    let normalized_destination = requested_destination_hex.to_ascii_lowercase();
    let canonical_destination =
        canonical_app_destination_hex(state, normalized_destination.as_str()).await;
    if !saved_peer_matches_destination(
        state,
        normalized_destination.as_str(),
        canonical_destination.as_str(),
    )
    .await
    {
        return false;
    }
    let Some(peer) = peer_for_any_destination_hex(state, canonical_destination.as_str()).await
    else {
        return true;
    };

    if saved_peer_stored_route_prefers_propagation(&peer, has_active_relay, direct_priority_hops) {
        return true;
    }
    !sdk_peer_is_direct_delivery_ready(&peer, has_active_relay)
        && !sdk_peer_has_known_lxmf_route(&peer)
}

fn saved_peer_stored_route_prefers_propagation(
    peer: &sdkmsg::PeerRecord,
    has_active_relay: bool,
    direct_priority_hops: Option<u8>,
) -> bool {
    delivery_policy::saved_route_prefers_propagation(
        peer,
        has_active_relay,
        sdk_peer_is_directly_reachable(peer),
        direct_priority_hops,
        MISSION_DIRECT_PRIORITY_FREE_HOPS,
    )
}

async fn saved_peer_can_try_stored_lxmf_route(
    state: &NodeRuntimeState,
    normalized_destination: &str,
    canonical_destination: &str,
) -> bool {
    if !saved_peer_matches_destination(state, normalized_destination, canonical_destination).await {
        return false;
    }
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_has_known_lxmf_route(&peer))
}

async fn saved_peer_has_direct_ready_route(
    state: &NodeRuntimeState,
    canonical_destination: &str,
    has_active_relay: bool,
) -> bool {
    if !peer_direct_delivery_available(state, canonical_destination).await {
        return false;
    }
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_is_direct_delivery_ready(&peer, has_active_relay))
}

async fn saved_peer_has_current_lxmf_route(
    state: &NodeRuntimeState,
    canonical_destination: &str,
) -> bool {
    peer_for_any_destination_hex(state, canonical_destination)
        .await
        .is_some_and(|peer| sdk_peer_has_observed_lxmf_delivery_route(&peer))
}

async fn saved_peer_matches_destination(
    state: &NodeRuntimeState,
    normalized_destination: &str,
    canonical_destination: &str,
) -> bool {
    let saved_peers = match state.app_state.get_saved_peers() {
        Ok(saved_peers) => saved_peers,
        Err(_) => return false,
    };
    saved_peers.iter().any(|peer| {
        [
            normalize_hex_32(peer.destination_hex.as_str()),
            peer.lxmf_destination_hex
                .as_deref()
                .and_then(normalize_hex_32),
        ]
        .into_iter()
        .flatten()
        .any(|destination_hex| {
            destination_hex == canonical_destination || destination_hex == normalized_destination
        })
    })
}

fn inbound_message_matches_destinations(
    message: &MessageRecord,
    destinations: &HashSet<String>,
) -> bool {
    if !matches!(message.direction, MessageDirection::Inbound {}) {
        return false;
    }

    [
        Some(message.conversation_id.as_str()),
        Some(message.destination_hex.as_str()),
        message.source_hex.as_deref(),
        message.requested_destination_hex.as_deref(),
        message.delivery_destination_hex.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(normalize_hex_32)
    .any(|destination| destinations.contains(destination.as_str()))
}

async fn inbound_correspondent_matches_destination(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    canonical_destination_hex: &str,
) -> bool {
    let mut destinations = HashSet::<String>::new();
    add_normalized_destination_candidate(&mut destinations, requested_destination_hex);
    add_normalized_destination_candidate(&mut destinations, canonical_destination_hex);

    if let Some(peer) = peer_for_any_destination_hex(state, canonical_destination_hex).await {
        for destination in equivalent_peer_destinations(&peer) {
            add_normalized_destination_candidate(&mut destinations, destination);
        }
    }
    if destinations.is_empty() {
        return false;
    }

    state.app_state.list_messages(None).is_ok_and(|messages| {
        messages
            .iter()
            .any(|message| inbound_message_matches_destinations(message, &destinations))
    })
}

fn mission_direct_priority_delay_for_hops(hops: Option<u8>) -> Duration {
    let Some(hops) = hops else {
        return Duration::ZERO;
    };
    if hops <= MISSION_DIRECT_PRIORITY_FREE_HOPS {
        return Duration::ZERO;
    }

    let delay_units = u32::from(hops - MISSION_DIRECT_PRIORITY_FREE_HOPS);
    (MISSION_DIRECT_PRIORITY_DELAY_PER_HOP * delay_units).min(MISSION_DIRECT_PRIORITY_MAX_DELAY)
}

fn add_normalized_destination_candidate(candidates: &mut HashSet<String>, destination_hex: &str) {
    if let Some(normalized) = normalize_hex_32(destination_hex) {
        candidates.insert(normalized);
    }
}

#[cfg(not(test))]
async fn mission_direct_priority_hops(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    canonical_destination_hex: &str,
) -> Option<u8> {
    let mut candidates = HashSet::<String>::new();
    add_normalized_destination_candidate(&mut candidates, requested_destination_hex);
    add_normalized_destination_candidate(&mut candidates, canonical_destination_hex);

    if let Some(peer) = peer_for_any_destination_hex(state, canonical_destination_hex).await {
        add_normalized_destination_candidate(&mut candidates, peer.destination_hex.as_str());
        if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.as_deref() {
            add_normalized_destination_candidate(&mut candidates, lxmf_destination_hex);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let announces = state.app_state.list_announces().ok()?;
    announces
        .iter()
        .filter_map(|announce| {
            let destination_hex = normalize_hex_32(announce.destination_hex.as_str())?;
            candidates
                .contains(destination_hex.as_str())
                .then_some(announce.hops)
        })
        .min()
}

fn direct_attempt_budget_for_send(
    send_mode: SendMode,
    has_active_relay: bool,
    can_try_stored_lxmf_route: bool,
    has_current_lxmf_route: bool,
    direct_delivery_ready: bool,
    direct_priority_hops: Option<u8>,
) -> usize {
    delivery_policy::direct_attempt_budget_for_send(
        delivery_policy::DirectAttemptBudget {
            send_mode,
            has_active_relay,
            can_try_stored_lxmf_route,
            has_current_lxmf_route,
            direct_delivery_ready,
            direct_priority_hops,
            direct_priority_free_hops: MISSION_DIRECT_PRIORITY_FREE_HOPS,
            lxmf_direct_attempts: LXMF_DIRECT_ATTEMPTS,
        },
    )
}

fn should_skip_direct_for_inbound_correspondent(
    send_mode: SendMode,
    has_active_relay: bool,
    is_inbound_correspondent: bool,
    has_current_lxmf_route: bool,
) -> bool {
    matches!(send_mode, SendMode::Auto {})
        && has_active_relay
        && is_inbound_correspondent
        && !has_current_lxmf_route
}

fn direct_attempt_send_mode(send_mode: SendMode) -> SendMode {
    match send_mode {
        SendMode::Auto {} | SendMode::DirectOnly {} => SendMode::DirectOnly {},
        SendMode::PropagationOnly {} => SendMode::PropagationOnly {},
    }
}

fn should_try_propagation_after_direct_failure(
    send_mode: SendMode,
    is_accepted_result: bool,
    has_active_relay: bool,
    propagation_fallback_allowed: bool,
    retriable: bool,
) -> bool {
    matches!(send_mode, SendMode::Auto {})
        && !is_accepted_result
        && has_active_relay
        && propagation_fallback_allowed
        && retriable
}

async fn equivalent_direct_delivery_destinations(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) -> Vec<String> {
    let mut destinations = Vec::<String>::new();
    for destination in [Some(requested_destination_hex), resolved_destination_hex]
        .into_iter()
        .flatten()
    {
        if let Some(normalized) = normalize_hex_32(destination) {
            destinations.push(normalized);
        }
    }

    if let Some(peer) = peer_for_any_destination_hex(state, requested_destination_hex).await {
        destinations.extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
    }
    if let Some(resolved_destination_hex) = resolved_destination_hex {
        if let Some(peer) = peer_for_any_destination_hex(state, resolved_destination_hex).await {
            destinations.extend(equivalent_peer_destinations(&peer).map(ToOwned::to_owned));
        }
    }

    destinations.sort();
    destinations.dedup();

    if destinations.is_empty() {
        return destinations;
    }

    destinations
}

async fn mark_peer_direct_delivery_unhealthy(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }
    let until_ms = now_ms().saturating_add(crate::numeric::u128_to_u64_saturating(
        DIRECT_DELIVERY_FAILURE_COOLDOWN.as_millis(),
    ));
    state
        .direct_delivery_health
        .mark_unhealthy(destinations.iter().map(String::as_str), until_ms);
    debug!(
        "[lxmf][mission] marked direct delivery cooldown destinations={} until_ms={}",
        destinations.join(","),
        until_ms,
    );
}

async fn clear_peer_direct_delivery_unhealthy(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }
    state
        .direct_delivery_health
        .clear(destinations.iter().map(String::as_str));
}

async fn close_output_links_for_direct_delivery_failure(
    state: &NodeRuntimeState,
    requested_destination_hex: &str,
    resolved_destination_hex: Option<&str>,
) {
    let destinations = equivalent_direct_delivery_destinations(
        state,
        requested_destination_hex,
        resolved_destination_hex,
    )
    .await;
    if destinations.is_empty() {
        return;
    }

    let mut stale_links = Vec::new();
    {
        let mut links = state.out_links.lock().await;
        for destination in &destinations {
            let Ok(address_hash) = parse_address_hash(destination) else {
                continue;
            };
            if let Some(link) = links.remove(&address_hash) {
                stale_links.push((destination.clone(), link));
            }
        }
    }

    for (destination, link) in stale_links {
        link.lock().await.close();
        debug!(
            "[link][maintain] destination={destination} status=closed reason=direct-delivery-failed",
        );
    }
}

async fn peer_direct_delivery_available(state: &NodeRuntimeState, destination_hex: &str) -> bool {
    let destinations = equivalent_direct_delivery_destinations(state, destination_hex, None).await;
    let now = now_ms();
    destinations
        .iter()
        .all(|destination| state.direct_delivery_health.is_available(destination, now))
}

async fn emit_peer_resolved_for_destination(
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: &str,
) {
    if !refresh_peer_snapshot(state).await {
        return;
    }
    if let Some(peer) = state
        .messaging
        .lock()
        .await
        .peer_by_destination(destination_hex)
        .map(from_sdk_peer_record)
    {
        bus.emit(NodeEvent::PeerResolved { peer });
    }
}
