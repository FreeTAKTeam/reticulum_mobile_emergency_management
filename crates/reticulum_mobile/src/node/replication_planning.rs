fn emit_replication_delivery_failure(bus: &EventBus, message: String, err: &NodeError) {
    bus.emit(NodeEvent::Error {
        code: crate::error_context::node_error_code(err).to_string(),
        message,
    });
}

fn emit_replication_planning_error(bus: &EventBus, operation: &str, detail: &str, err: NodeError) {
    bus.emit(NodeEvent::Error {
        code: crate::error_context::node_error_code(&err).to_string(),
        message: format!("{operation} replication planning skipped {detail} reason={err}"),
    });
}

fn saved_peers_for_replication(
    app_state: &AppStateStore,
    bus: &EventBus,
    operation: &str,
) -> Vec<SavedPeerRecord> {
    match app_state.get_saved_peers() {
        Ok(peers) => peers,
        Err(err) => {
            emit_replication_planning_error(bus, operation, "saved-peers", err);
            Vec::new()
        }
    }
}

fn route_hops_for_replication(
    app_state: &AppStateStore,
    bus: &EventBus,
    operation: &str,
) -> HashMap<String, u8> {
    match app_state.list_announces() {
        Ok(announces) => announce_route_hops(announces.as_slice()),
        Err(err) => {
            emit_replication_planning_error(bus, operation, "announce-route-hops", err);
            HashMap::new()
        }
    }
}

fn peer_for_target<'a>(
    peers: &'a [PeerRecord],
    target: &MissionReplicationTarget,
) -> Option<&'a PeerRecord> {
    peers.iter().find(|peer| {
        normalize_hex_32(peer.destination_hex.as_str()).as_deref()
            == Some(target.app_destination_hex.as_str())
    })
}

fn target_route_hops(
    target: &MissionReplicationTarget,
    peers: &[PeerRecord],
    route_hops: &HashMap<String, u8>,
) -> u8 {
    route_hops
        .get(target.app_destination_hex.as_str())
        .copied()
        .or_else(|| {
            peer_for_target(peers, target)
                .and_then(|peer| peer.lxmf_destination_hex.as_deref())
                .and_then(normalize_hex_32)
                .and_then(|destination_hex| route_hops.get(destination_hex.as_str()).copied())
        })
        .unwrap_or(u8::MAX)
}

fn target_liveness_priority(target: &MissionReplicationTarget, peers: &[PeerRecord]) -> u8 {
    match peer_for_target(peers, target) {
        Some(peer) if peer_is_directly_reachable(peer) => 0,
        Some(peer) if peer_is_current_replication_target(peer) => 1,
        Some(peer) if peer_has_observed_lxmf_delivery_route(peer) => 2,
        Some(_) => 3,
        None => 4,
    }
}

fn prioritize_replication_targets_by_route_hops(
    targets: &mut [MissionReplicationTarget],
    peers: &[PeerRecord],
    route_hops: &HashMap<String, u8>,
) {
    if route_hops.is_empty() {
        return;
    }

    let sequence_by_destination = targets
        .iter()
        .enumerate()
        .map(|(index, target)| (target.app_destination_hex.clone(), index))
        .collect::<HashMap<_, _>>();

    targets.sort_by_key(|target| {
        (
            matches!(target.send_mode, SendMode::PropagationOnly {}) as u8,
            target_liveness_priority(target, peers),
            target_route_hops(target, peers, route_hops),
            sequence_by_destination
                .get(target.app_destination_hex.as_str())
                .copied()
                .unwrap_or(usize::MAX),
        )
    });
}

fn target_is_saved(
    target: &MissionReplicationTarget,
    peers: &[PeerRecord],
    saved_destination_set: &HashSet<String>,
) -> bool {
    saved_destination_set.contains(target.app_destination_hex.as_str())
        || peer_for_target(peers, target).is_some_and(|peer| peer.saved)
}

fn prioritize_sos_replication_targets(
    targets: &mut [MissionReplicationTarget],
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    route_hops: &HashMap<String, u8>,
) {
    let saved_destination_set = saved_peers
        .iter()
        .filter_map(saved_peer_target_destination)
        .collect::<HashSet<_>>();
    let sequence_by_destination = targets
        .iter()
        .enumerate()
        .map(|(index, target)| (target.app_destination_hex.clone(), index))
        .collect::<HashMap<_, _>>();

    targets.sort_by_key(|target| {
        (
            matches!(target.send_mode, SendMode::PropagationOnly {}) as u8,
            !target_is_saved(target, peers, &saved_destination_set),
            target_route_hops(target, peers, route_hops),
            target_liveness_priority(target, peers),
            sequence_by_destination
                .get(target.app_destination_hex.as_str())
                .copied()
                .unwrap_or(usize::MAX),
        )
    });
}

fn build_mission_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let saved_destinations = saved_peers
        .iter()
        .filter(|peer| matches!(peer.circle_tier, CircleTier::Inner {}))
        .filter_map(saved_peer_target_destination)
        .collect::<Vec<_>>();
    let saved_destination_set = saved_destinations.iter().cloned().collect::<HashSet<_>>();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
    let mut direct_destination_set = HashSet::<String>::new();
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();

    for peer in peers {
        let Some(app_destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen_app_destinations.insert(app_destination_hex.clone()) {
            continue;
        }
        if !saved_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        if !peer_supports_mission_traffic(peer) {
            continue;
        }
        let saved_stored_route = saved_peer_can_try_stored_lxmf_route(peer, true);
        let connectivity = peer_connectivity_model(peer, has_active_relay, true);
        if !peer_is_current_replication_target(peer)
            && !peer_has_observed_lxmf_delivery_route(peer)
            && !saved_stored_route
        {
            continue;
        }
        let direct_ready = peer_is_mission_direct_delivery_ready(peer, has_active_relay)
            || peer_can_use_direct_when_relay_route_is_missing(peer, has_active_relay);
        if direct_ready {
            direct_destination_set.insert(app_destination_hex.clone());
            let sequence = direct_targets.len();
            direct_targets.push(build_prioritized_direct_target(
                peer,
                app_destination_hex,
                sequence,
            ));
        } else if !connectivity.current_or_stored_route_available() {
            continue;
        }
    }

    for app_destination_hex in saved_destinations {
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if direct_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        if has_active_relay {
            let relay_ready = saved_peers.iter().any(|peer| {
                saved_peer_target_destination(peer).as_deref() == Some(app_destination_hex.as_str())
                    && saved_peer_has_stored_propagation_route(peer)
            }) || peers.iter().any(|peer| {
                normalize_hex_32(peer.destination_hex.as_str()).as_deref()
                    == Some(app_destination_hex.as_str())
                    && peer_supports_mission_traffic(peer)
                    && peer_has_stored_propagation_route(peer)
            });
            if !relay_ready {
                continue;
            }
        } else {
            continue;
        }
        relay_targets.push(MissionReplicationTarget {
            app_destination_hex,
            send_mode: if has_active_relay {
                SendMode::PropagationOnly {}
            } else {
                SendMode::Auto {}
            },
        });
    }

    finish_replication_targets(direct_targets, relay_targets)
}

#[cfg(test)]
fn build_sos_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let saved_destinations = saved_peers
        .iter()
        .filter_map(saved_peer_target_destination)
        .collect::<Vec<_>>();
    let saved_destination_set = saved_destinations.iter().cloned().collect::<HashSet<_>>();
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen_app_destinations = HashSet::<String>::new();
    let mut direct_destination_set = HashSet::<String>::new();
    let mut relay_destination_set = HashSet::<String>::new();

    for peer in peers {
        let Some(app_destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen_app_destinations.insert(app_destination_hex.clone()) {
            continue;
        }
        if !peer_supports_mission_traffic(peer) {
            continue;
        }
        let saved_peer = peer.saved || saved_destination_set.contains(app_destination_hex.as_str());
        let saved_stored_route = saved_peer_can_try_stored_lxmf_route(peer, saved_peer);
        let connectivity = peer_connectivity_model(peer, has_active_relay, saved_peer);
        if !saved_peer && !peer.active_link {
            continue;
        }
        if !peer_is_current_replication_target(peer)
            && !peer_has_observed_lxmf_delivery_route(peer)
            && !saved_stored_route
        {
            continue;
        }

        let direct_ready = peer_is_mission_direct_delivery_ready(peer, has_active_relay)
            || peer_can_use_direct_when_relay_route_is_missing(peer, has_active_relay);
        if direct_ready {
            direct_destination_set.insert(app_destination_hex.clone());
            let sequence = direct_targets.len();
            direct_targets.push(build_prioritized_direct_target(
                peer,
                app_destination_hex,
                sequence,
            ));
        } else if connectivity.stored_propagation_available()
            || (has_active_relay && peer_can_use_propagation_fallback(peer))
        {
            relay_destination_set.insert(app_destination_hex.clone());
            relay_targets.push(MissionReplicationTarget {
                app_destination_hex,
                send_mode: SendMode::PropagationOnly {},
            });
        }
    }

    for app_destination_hex in saved_destinations {
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if direct_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        if relay_destination_set.contains(app_destination_hex.as_str()) {
            continue;
        }
        if !has_active_relay {
            continue;
        }
        let relay_ready = saved_peers.iter().any(|peer| {
            saved_peer_target_destination(peer).as_deref() == Some(app_destination_hex.as_str())
                && saved_peer_has_stored_propagation_route(peer)
        }) || peers.iter().any(|peer| {
            normalize_hex_32(peer.destination_hex.as_str()).as_deref()
                == Some(app_destination_hex.as_str())
                && peer_supports_mission_traffic(peer)
                && peer_has_stored_propagation_route(peer)
        });
        if !relay_ready {
            continue;
        }
        relay_targets.push(MissionReplicationTarget {
            app_destination_hex,
            send_mode: SendMode::PropagationOnly {},
        });
    }

    finish_replication_targets(direct_targets, relay_targets)
}
