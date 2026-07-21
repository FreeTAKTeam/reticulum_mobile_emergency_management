fn build_event_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let saved_destinations = saved_peers
        .iter()
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

fn build_transient_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    directory_destinations: &[String],
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen = HashSet::<String>::new();
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();

    for destination_hash in directory_destinations {
        let Some(app_destination_hex) = normalize_hex_32(destination_hash.as_str()) else {
            continue;
        };
        if self_destination_hex.as_deref() == Some(app_destination_hex.as_str()) {
            continue;
        }
        if !seen.insert(app_destination_hex.clone()) {
            continue;
        }

        let matched_peer = peers.iter().find(|peer| {
            normalize_hex_32(peer.destination_hex.as_str()).as_deref()
                == Some(app_destination_hex.as_str())
        });
        let Some(matched_peer) = matched_peer else {
            continue;
        };
        if !peer_is_current_replication_target(matched_peer)
            || !peer_supports_mission_traffic(matched_peer)
        {
            continue;
        }
        let send_mode = if !has_active_relay
            || ((matched_peer.saved
                && peer_is_mission_direct_delivery_ready(matched_peer, has_active_relay))
                || peer_can_use_direct_when_relay_route_is_missing(matched_peer, has_active_relay))
        {
            SendMode::Auto {}
        } else if peer_can_use_propagation_fallback(matched_peer) {
            SendMode::PropagationOnly {}
        } else {
            continue;
        };
        let target = MissionReplicationTarget {
            app_destination_hex,
            send_mode,
        };
        if matches!(target.send_mode, SendMode::Auto {}) {
            direct_targets.push(target);
        } else {
            relay_targets.push(target);
        }
    }

    direct_targets.extend(relay_targets);
    direct_targets
}

fn current_replication_send_mode(
    peers: &[PeerRecord],
    app_destination_hex: &str,
    active_propagation_node_hex: Option<&str>,
) -> Option<SendMode> {
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();
    let peer = peers.iter().find(|peer| {
        normalize_hex_32(peer.destination_hex.as_str()).as_deref() == Some(app_destination_hex)
            && peer_is_current_replication_target(peer)
            && peer_supports_mission_traffic(peer)
    })?;

    if peer_is_mission_direct_delivery_ready(peer, has_active_relay)
        || peer_can_use_direct_when_relay_route_is_missing(peer, has_active_relay)
    {
        Some(SendMode::Auto {})
    } else if peer_can_use_propagation_fallback(peer) && has_active_relay {
        Some(SendMode::PropagationOnly {})
    } else {
        None
    }
}

fn connected_hub_replication_target(
    peers: &[PeerRecord],
    active_propagation_node_hex: Option<&str>,
    config: &NodeConfigFingerprint,
) -> Result<MissionReplicationTarget, NodeError> {
    let app_destination_hex = configured_hub_destination(config)?;
    let send_mode = current_replication_send_mode(
        peers,
        app_destination_hex.as_str(),
        active_propagation_node_hex,
    )
    .unwrap_or(SendMode::PropagationOnly {});
    Ok(MissionReplicationTarget {
        app_destination_hex,
        send_mode,
    })
}

fn build_runtime_mission_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let active_saved_peers = active_local_saved_peers(saved_peers, hub_directory_snapshot);
    let Some(config) = active_config else {
        return Ok(build_mission_replication_targets(
            status,
            peers,
            &active_saved_peers,
            active_propagation_node_hex,
        ));
    };

    let local_targets = if !active_saved_peers.is_empty()
        || hub_directory_snapshot.is_none_or(|snapshot| snapshot.local_teams.is_empty())
    {
        build_mission_replication_targets(
            status,
            peers,
            &active_saved_peers,
            active_propagation_node_hex,
        )
    } else {
        Vec::new()
    };
    let Some(snapshot) = hub_directory_snapshot else {
        return Ok(local_targets);
    };
    let directory_destinations = active_team_directory_destinations(snapshot, None);
    let directory_targets = build_transient_replication_targets(
        status,
        peers,
        &directory_destinations,
        active_propagation_node_hex,
    );
    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Connected {} => {
            let hub_targets = if active_team_has_caller_membership(snapshot)
                && !directory_destinations.is_empty()
            {
                vec![connected_hub_replication_target(
                    peers,
                    active_propagation_node_hex,
                    config,
                )?]
            } else {
                Vec::new()
            };
            Ok(merge_replication_target_sets(local_targets, hub_targets))
        }
        HubMode::Autonomous {} | HubMode::SemiAutonomous {} => {
            Ok(merge_replication_target_sets(
                local_targets,
                directory_targets,
            ))
        }
    }
}

fn should_include_checklist_participant_targets(
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> bool {
    hub_directory_snapshot.is_none_or(|snapshot| snapshot.local_teams.is_empty())
        && active_team_uid(hub_directory_snapshot) == YELLOW_TEAM_UID
        && active_config.is_none_or(|config| {
            matches!(
                effective_hub_mode(config.hub_mode, hub_directory_snapshot),
                HubMode::Autonomous {}
            )
        })
}

fn append_checklist_participant_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    participant_rns_identities: &[String],
    active_propagation_node_hex: Option<&str>,
    targets: &mut Vec<MissionReplicationTarget>,
) {
    let self_destinations = [
        normalize_hex_32(status.identity_hex.as_str()),
        normalize_hex_32(status.app_destination_hex.as_str()),
        normalize_hex_32(status.lxmf_destination_hex.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();
    let mut seen_destinations = targets
        .iter()
        .map(|target| target.app_destination_hex.clone())
        .collect::<HashSet<_>>();
    for participant in participant_rns_identities {
        let Some(app_destination_hex) = normalize_hex_32(participant.as_str()) else {
            continue;
        };
        if self_destinations.contains(app_destination_hex.as_str())
            || !seen_destinations.insert(app_destination_hex.clone())
        {
            continue;
        }
        let Some(send_mode) = current_replication_send_mode(
            peers,
            app_destination_hex.as_str(),
            active_propagation_node_hex,
        ) else {
            continue;
        };
        targets.push(MissionReplicationTarget {
            send_mode,
            app_destination_hex,
        });
    }
}

fn build_runtime_checklist_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
    checklist: Option<&ChecklistRecord>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let mut targets = build_runtime_mission_replication_targets(
        status,
        peers,
        saved_peers,
        active_propagation_node_hex,
        active_config,
        hub_directory_snapshot,
    )?;
    if should_include_checklist_participant_targets(active_config, hub_directory_snapshot) {
        if let Some(checklist) = checklist {
            append_checklist_participant_replication_targets(
                status,
                peers,
                checklist.participant_rns_identities.as_slice(),
                active_propagation_node_hex,
                &mut targets,
            );
        }
    }
    Ok(targets)
}

fn build_runtime_event_replication_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let active_saved_peers = active_local_saved_peers(saved_peers, hub_directory_snapshot);
    let Some(config) = active_config else {
        return Ok(build_event_replication_targets(
            status,
            peers,
            &active_saved_peers,
            active_propagation_node_hex,
        ));
    };

    let local_targets = if !active_saved_peers.is_empty()
        || hub_directory_snapshot.is_none_or(|snapshot| snapshot.local_teams.is_empty())
    {
        build_event_replication_targets(
            status,
            peers,
            &active_saved_peers,
            active_propagation_node_hex,
        )
    } else {
        Vec::new()
    };
    let Some(snapshot) = hub_directory_snapshot else {
        return Ok(local_targets);
    };
    let directory_destinations = active_team_directory_destinations(snapshot, None);
    let directory_targets = build_transient_replication_targets(
        status,
        peers,
        &directory_destinations,
        active_propagation_node_hex,
    );
    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Connected {} => {
            let hub_targets = if active_team_has_caller_membership(snapshot)
                && !directory_destinations.is_empty()
            {
                vec![connected_hub_replication_target(
                    peers,
                    active_propagation_node_hex,
                    config,
                )?]
            } else {
                Vec::new()
            };
            Ok(merge_replication_target_sets(local_targets, hub_targets))
        }
        HubMode::Autonomous {} | HubMode::SemiAutonomous {} => {
            Ok(merge_replication_target_sets(
                local_targets,
                directory_targets,
            ))
        }
    }
}
