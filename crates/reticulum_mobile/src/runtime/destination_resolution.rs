async fn emit_peer_changed(state: &NodeRuntimeState, bus: &EventBus, destination_hex: &str) {
    if !refresh_peer_snapshot(state).await {
        return;
    }
    if let Some(change) = state
        .messaging
        .lock()
        .await
        .peer_change_for_destination(destination_hex)
        .map(from_sdk_peer_change)
    {
        bus.emit(NodeEvent::PeerChanged { change });
    }
}

fn peer_matches_hex(peer: &sdkmsg::PeerRecord, normalized_hex: &str) -> bool {
    peer.destination_hex == normalized_hex
        || peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value == normalized_hex)
        || peer
            .identity_hex
            .as_deref()
            .is_some_and(|value| value == normalized_hex)
}

fn equivalent_peer_destinations(peer: &sdkmsg::PeerRecord) -> impl Iterator<Item = &str> {
    [
        Some(peer.destination_hex.as_str()),
        peer.lxmf_destination_hex.as_deref(),
        peer.identity_hex.as_deref(),
    ]
    .into_iter()
    .flatten()
}

fn peer_is_current_send_target(peer: &sdkmsg::PeerRecord) -> bool {
    !peer.stale && (peer.active_link || peer.announce_last_seen_at_ms.is_some())
}

fn delivery_route_unavailable_error() -> NodeError {
    NodeError::NetworkError {}
}

async fn known_lxmf_route_for_app_destination(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<String> {
    let destination_hex = normalize_hex_32(destination_hex)?;
    let known_destinations = state.known_destinations.lock().await;
    known_destinations.values().find_map(|desc| {
        let app_destination_hex = SingleOutputDestination::new(
            desc.identity,
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .desc
        .address_hash
        .to_hex_string();
        (app_destination_hex == destination_hex).then(|| {
            SingleOutputDestination::new(
                desc.identity,
                DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
            )
            .desc
            .address_hash
            .to_hex_string()
        })
    })
}

fn resolve_current_lxmf_destination_from_peers(
    peers: &[sdkmsg::PeerRecord],
    destination_hex: &str,
) -> Result<String, NodeError> {
    let normalized_destination =
        normalize_hex_32(destination_hex).ok_or(NodeError::InvalidConfig {})?;

    if let Some(peer) = peers.iter().find(|peer| {
        peer_matches_hex(peer, normalized_destination.as_str()) && peer_is_current_send_target(peer)
    }) {
        if peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value == normalized_destination)
        {
            return Ok(normalized_destination);
        }

        return Ok(peer
            .lxmf_destination_hex
            .clone()
            .unwrap_or_else(|| peer.destination_hex.clone()));
    }

    let stale_equivalent = peers
        .iter()
        .find(|peer| peer_matches_hex(peer, normalized_destination.as_str()));
    let Some(stale_equivalent) = stale_equivalent else {
        return Err(delivery_route_unavailable_error());
    };
    let identity_hex = stale_equivalent.identity_hex.as_deref();
    let lxmf_destination_hex = stale_equivalent
        .lxmf_destination_hex
        .as_deref()
        .or_else(|| {
            if normalized_destination == stale_equivalent.destination_hex {
                None
            } else {
                Some(stale_equivalent.destination_hex.as_str())
            }
        });

    peers
        .iter()
        .find(|peer| {
            peer_is_current_send_target(peer)
                && (lxmf_destination_hex.is_some_and(|destination| {
                    peer_matches_hex(peer, destination)
                        || peer.lxmf_destination_hex.as_deref() == Some(destination)
                }) || identity_hex
                    .is_some_and(|identity| peer.identity_hex.as_deref() == Some(identity)))
        })
        .map(|peer| {
            peer.lxmf_destination_hex
                .clone()
                .unwrap_or_else(|| peer.destination_hex.clone())
        })
        .ok_or_else(delivery_route_unavailable_error)
}

async fn peer_for_any_destination_hex(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Option<sdkmsg::PeerRecord> {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    let messaging = state.messaging.lock().await;
    messaging
        .peer_by_destination(normalized_destination.as_str())
        .or_else(|| {
            messaging
                .list_peers()
                .into_iter()
                .find(|peer| peer_matches_hex(peer, normalized_destination.as_str()))
        })
}

async fn resolve_current_lxmf_destination_hex(
    state: &NodeRuntimeState,
    destination_hex: &str,
) -> Result<String, NodeError> {
    let messaging = state.messaging.lock().await;
    let peers = messaging.list_peers();
    match resolve_current_lxmf_destination_from_peers(peers.as_slice(), destination_hex) {
        Ok(destination) => Ok(destination),
        Err(err) => {
            let normalized_destination =
                normalize_hex_32(destination_hex).ok_or(NodeError::InvalidConfig {})?;
            let lxmf_candidates = peers
                .iter()
                .filter(|peer| peer_matches_hex(peer, normalized_destination.as_str()))
                .flat_map(equivalent_peer_destinations)
                .chain(std::iter::once(normalized_destination.as_str()));
            for candidate in lxmf_candidates {
                if let Some(destination) = messaging.current_lxmf_announce_destination(candidate) {
                    return Ok(destination);
                }
            }
            drop(messaging);
            if let Some(destination) =
                known_lxmf_route_for_app_destination(state, destination_hex).await
            {
                return Ok(destination);
            }
            Err(err)
        }
    }
}

async fn resolve_lxmf_destination_hex(state: &NodeRuntimeState, destination_hex: &str) -> String {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    if let Ok(saved_peers) = state.app_state.get_saved_peers() {
        if let Some(peer) = saved_peers.iter().find(|peer| {
            peer.destination_hex
                .eq_ignore_ascii_case(normalized_destination.as_str())
                || peer.lxmf_destination_hex.as_deref().is_some_and(|value| {
                    value.eq_ignore_ascii_case(normalized_destination.as_str())
                })
                || peer.identity_hex.as_deref().is_some_and(|value| {
                    value.eq_ignore_ascii_case(normalized_destination.as_str())
                })
        }) {
            if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.as_deref() {
                return lxmf_destination_hex.to_ascii_lowercase();
            }
            return peer.destination_hex.to_ascii_lowercase();
        }
    }
    let Some(peer) = peer_for_any_destination_hex(state, &normalized_destination).await else {
        return known_lxmf_route_for_app_destination(state, &normalized_destination)
            .await
            .unwrap_or(normalized_destination);
    };
    if peer
        .lxmf_destination_hex
        .as_deref()
        .is_some_and(|value| value == normalized_destination)
    {
        return normalized_destination;
    }
    peer.lxmf_destination_hex.unwrap_or(peer.destination_hex)
}

async fn resolve_lxmf_destination_for_send(
    state: &NodeRuntimeState,
    destination_hex: &str,
    require_current_peer: bool,
) -> Result<String, NodeError> {
    if require_current_peer {
        resolve_current_lxmf_destination_hex(state, destination_hex).await
    } else {
        Ok(resolve_lxmf_destination_hex(state, destination_hex).await)
    }
}

async fn canonical_app_destination_hex(state: &NodeRuntimeState, destination_hex: &str) -> String {
    let normalized_destination = destination_hex.to_ascii_lowercase();
    let Some(peer) = peer_for_any_destination_hex(state, &normalized_destination).await else {
        return normalized_destination;
    };
    let Some(identity_hex) = peer.identity_hex.clone() else {
        return peer.destination_hex;
    };
    if let Some(destination_hex) = state
        .messaging
        .lock()
        .await
        .app_destination_for_identity(identity_hex.as_str())
    {
        return destination_hex;
    }
    reticulum::transport::identity::Identity::new_from_hex_string(identity_hex.as_str())
        .ok()
        .map(|identity| {
            SingleOutputDestination::new(
                identity,
                DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
            )
            .desc
            .address_hash
            .to_hex_string()
        })
        .unwrap_or(peer.destination_hex)
}

async fn peer_destinations_equivalent(
    state: &NodeRuntimeState,
    left_hex: &str,
    right_hex: &str,
) -> bool {
    let normalized_left = left_hex.to_ascii_lowercase();
    let normalized_right = right_hex.to_ascii_lowercase();
    if normalized_left == normalized_right {
        return true;
    }

    let left_peer = peer_for_any_destination_hex(state, &normalized_left).await;
    let right_peer = peer_for_any_destination_hex(state, &normalized_right).await;
    let (Some(left_peer), Some(right_peer)) = (left_peer, right_peer) else {
        return false;
    };

    if left_peer.identity_hex.is_some() && left_peer.identity_hex == right_peer.identity_hex {
        return true;
    }

    let matches = equivalent_peer_destinations(&left_peer)
        .any(|candidate| equivalent_peer_destinations(&right_peer).any(|other| candidate == other));
    matches
}

async fn has_active_propagation_relay(state: &NodeRuntimeState) -> bool {
    state
        .active_propagation_node_hex
        .lock()
        .await
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn propagation_candidate_sort_key(
    announce: &sdkmsg::AnnounceRecord,
    preferred_destination_hex: Option<&str>,
    current_destination_hex: Option<&str>,
) -> (u8, u8, u8, u64, String) {
    let preferred_rank = if preferred_destination_hex.is_some_and(|preferred| {
        preferred == announce.destination_hex || preferred == announce.identity_hex
    }) {
        0
    } else {
        1
    };
    let current_rank = if preferred_destination_hex.is_none()
        && current_destination_hex.is_some_and(|current| current == announce.destination_hex)
    {
        0
    } else {
        1
    };
    (
        preferred_rank,
        announce.hops,
        current_rank,
        u64::MAX.saturating_sub(announce.received_at_ms),
        announce.destination_hex.clone(),
    )
}

fn propagation_sync_candidate_relays(
    announces: &[sdkmsg::AnnounceRecord],
    active_relay_hex: &str,
    preferred_destination_hex: Option<&str>,
) -> Vec<String> {
    let active_relay_hex = active_relay_hex.trim();
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    let active_matches_preferred =
        preferred_destination_hex.is_some_and(|preferred| preferred == active_relay_hex);
    if !active_relay_hex.is_empty()
        && (preferred_destination_hex.is_none() || active_matches_preferred)
        && seen.insert(active_relay_hex.to_string())
    {
        candidates.push(active_relay_hex.to_string());
    }

    let mut relay_announces = announces
        .iter()
        .filter(|record| record.destination_kind == "lxmf_propagation")
        .collect::<Vec<_>>();
    relay_announces.sort_by_key(|record| {
        propagation_candidate_sort_key(record, preferred_destination_hex, Some(active_relay_hex))
    });
    for record in relay_announces {
        if candidates.len() >= PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS {
            break;
        }
        if seen.insert(record.destination_hex.clone()) {
            candidates.push(record.destination_hex.clone());
        }
    }
    if candidates.is_empty()
        && !active_relay_hex.is_empty()
        && seen.insert(active_relay_hex.to_string())
    {
        candidates.push(active_relay_hex.to_string());
    }
    candidates
}
