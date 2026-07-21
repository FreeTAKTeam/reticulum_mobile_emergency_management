#[derive(Debug, Clone)]
struct MissionReplicationTarget {
    app_destination_hex: String,
    send_mode: SendMode,
}

#[derive(Debug, Clone)]
struct PrioritizedMissionReplicationTarget {
    priority: u8,
    sequence: usize,
    target: MissionReplicationTarget,
}

type ScheduledMissionSend = (String, Vec<u8>, Vec<u8>, SendMode);
const CHECKLIST_INITIAL_TASK_SEND_INTERVAL: Duration = Duration::from_millis(100);

fn effective_hub_mode(
    configured_mode: HubMode,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> HubMode {
    match configured_mode {
        HubMode::Autonomous {} => HubMode::Autonomous {},
        HubMode::Connected {} => HubMode::Connected {},
        HubMode::SemiAutonomous {} => {
            if hub_directory_snapshot.is_some_and(|snapshot| snapshot.effective_connected_mode) {
                HubMode::Connected {}
            } else {
                HubMode::SemiAutonomous {}
            }
        }
    }
}

fn active_team_directory_destinations(
    snapshot: &HubDirectorySnapshot,
    required_capability: Option<&str>,
) -> Vec<String> {
    let active_team_uid = active_team_uid(Some(snapshot));
    let mut destinations = Vec::new();
    let mut seen = HashSet::new();
    for member in &snapshot.members {
        if member.team_uid != active_team_uid {
            continue;
        }
        if required_capability.is_some_and(|required| {
            !member
                .announce_capabilities
                .iter()
                .any(|capability| capability.eq_ignore_ascii_case(required))
        }) {
            continue;
        }
        if let Some(destination) = normalize_hex_32(&member.destination_hash) {
            if seen.insert(destination.clone()) {
                destinations.push(destination);
            }
        }
    }
    // Directly constructed and legacy snapshots may only carry `items`.
    if active_team_uid == YELLOW_TEAM_UID && destinations.is_empty() {
        for item in &snapshot.items {
            if required_capability.is_some_and(|required| {
                !item
                    .announce_capabilities
                    .iter()
                    .any(|capability| capability.eq_ignore_ascii_case(required))
            }) {
                continue;
            }
            if let Some(destination) = normalize_hex_32(&item.destination_hash) {
                if seen.insert(destination.clone()) {
                    destinations.push(destination);
                }
            }
        }
    }
    destinations
}

fn active_team_has_caller_membership(snapshot: &HubDirectorySnapshot) -> bool {
    let team_uid = active_team_uid(Some(snapshot));
    (team_uid == YELLOW_TEAM_UID && snapshot.schema_version < HUB_DIRECTORY_SCHEMA_VERSION)
        || snapshot
            .caller_memberships
            .iter()
            .any(|membership| membership.team_uid == team_uid)
}

fn merge_replication_target_sets(
    primary: Vec<MissionReplicationTarget>,
    additional: Vec<MissionReplicationTarget>,
) -> Vec<MissionReplicationTarget> {
    let mut seen = HashSet::new();
    primary
        .into_iter()
        .chain(additional)
        .filter(|target| seen.insert(target.app_destination_hex.clone()))
        .collect()
}

fn telemetry_targets_from_peers_with_relay(
    peers: &[PeerRecord],
    self_destination_hex: Option<&str>,
    active_propagation_node_hex: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let has_active_relay = active_propagation_node_hex
        .and_then(normalize_hex_32)
        .is_some();
    let mut direct_targets = Vec::new();
    let mut relay_targets = Vec::new();
    let mut seen = HashSet::<String>::new();
    for peer in peers {
        let Some(destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if self_destination_hex == Some(destination_hex.as_str()) {
            continue;
        }
        if !has_capability_token(peer.app_data.as_deref(), "telemetry") {
            continue;
        }
        if !peer_is_current_replication_target(peer) {
            continue;
        }
        if !peer.saved && !peer.active_link {
            continue;
        }

        let relay_ready = has_active_relay && peer_can_use_propagation_fallback(peer);
        let direct_ready = peer_is_direct_delivery_ready(peer)
            || peer_can_use_direct_when_relay_route_is_missing(peer, has_active_relay);
        if !direct_ready && !relay_ready {
            continue;
        }
        if seen.insert(destination_hex.clone()) {
            let target = MissionReplicationTarget {
                app_destination_hex: destination_hex,
                send_mode: if direct_ready {
                    SendMode::Auto {}
                } else {
                    SendMode::PropagationOnly {}
                },
            };
            if matches!(target.send_mode, SendMode::Auto {}) {
                direct_targets.push(target);
            } else {
                relay_targets.push(target);
            }
        }
    }
    direct_targets.extend(relay_targets);
    direct_targets
}

fn telemetry_destinations_from_hub_snapshot(
    snapshot: &HubDirectorySnapshot,
    self_destination_hex: Option<&str>,
) -> Vec<String> {
    let mut destinations = Vec::new();
    let mut seen = HashSet::<String>::new();
    for destination in active_team_directory_destinations(snapshot, Some("telemetry")) {
        let Some(destination_hex) = normalize_hex_32(destination.as_str()) else {
            continue;
        };
        if self_destination_hex == Some(destination_hex.as_str()) {
            continue;
        }
        if seen.insert(destination_hex.clone()) {
            destinations.push(destination_hex);
        }
    }
    destinations
}

fn build_runtime_telemetry_destinations(
    status: &NodeStatus,
    peers: &[PeerRecord],
    active_propagation_node_hex: Option<&str>,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let self_destination_hex = normalize_hex_32(status.app_destination_hex.as_str());
    let local_destinations = hub_directory_snapshot
        .map(active_local_team_destinations)
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let local_peers = if hub_directory_snapshot.is_some_and(|snapshot| !snapshot.local_teams.is_empty()) {
        peers
            .iter()
            .filter(|peer| {
                local_destinations.contains(&peer.destination_hex.to_ascii_lowercase())
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        peers.to_vec()
    };
    let Some(config) = active_config else {
        return Ok(telemetry_targets_from_peers_with_relay(
            &local_peers,
            self_destination_hex.as_deref(),
            active_propagation_node_hex,
        ));
    };

    let mode = effective_hub_mode(config.hub_mode, hub_directory_snapshot);
    let local_targets = if !local_destinations.is_empty()
        || hub_directory_snapshot.is_none_or(|snapshot| snapshot.local_teams.is_empty())
    {
        telemetry_targets_from_peers_with_relay(
            &local_peers,
            self_destination_hex.as_deref(),
            active_propagation_node_hex,
        )
    } else {
        Vec::new()
    };
    let Some(snapshot) = hub_directory_snapshot else {
        return Ok(local_targets);
    };
    let directory_targets = build_transient_replication_targets(
        status,
        peers,
        &telemetry_destinations_from_hub_snapshot(snapshot, self_destination_hex.as_deref()),
        active_propagation_node_hex,
    );
    if matches!(mode, HubMode::Connected {}) {
        let hub_targets = if active_team_has_caller_membership(snapshot)
            && !active_team_directory_destinations(snapshot, Some("telemetry")).is_empty()
        {
            vec![connected_hub_replication_target(
                peers,
                active_propagation_node_hex,
                config,
            )?]
        } else {
            Vec::new()
        };
        return Ok(merge_replication_target_sets(local_targets, hub_targets));
    }
    Ok(merge_replication_target_sets(local_targets, directory_targets))
}

fn configured_hub_destination(config: &NodeConfigFingerprint) -> Result<String, NodeError> {
    config
        .hub_identity_hash
        .as_deref()
        .and_then(normalize_hex_32)
        .ok_or(NodeError::InvalidConfig {})
}

fn routed_destination_hex(
    requested_destination_hex: String,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> Result<String, NodeError> {
    let Some(config) = active_config else {
        return Ok(requested_destination_hex);
    };
    let mode = effective_hub_mode(config.hub_mode, hub_directory_snapshot);
    if matches!(mode, HubMode::Connected {}) {
        let snapshot = hub_directory_snapshot.ok_or(NodeError::InvalidConfig {})?;
        if active_local_team_destinations(snapshot).contains(&requested_destination_hex) {
            return Ok(requested_destination_hex);
        }
        let destinations = active_team_directory_destinations(snapshot, None);
        if !active_team_has_caller_membership(snapshot) || destinations.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let configured_hub = configured_hub_destination(config)?;
        if requested_destination_hex != configured_hub
            && !destinations.contains(&requested_destination_hex)
        {
            return Err(NodeError::InvalidConfig {});
        }
        return Ok(configured_hub);
    }
    if hub_directory_snapshot.is_some_and(|snapshot| {
        !snapshot.local_teams.is_empty()
            && !active_local_team_destinations(snapshot).contains(&requested_destination_hex)
            && !active_team_directory_destinations(snapshot, None)
                .contains(&requested_destination_hex)
    })
    {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(requested_destination_hex)
}

fn is_blank(value: Option<&str>) -> bool {
    value.is_none_or(|entry| entry.trim().is_empty())
}

fn populate_eam_defaults(
    status: &NodeStatus,
    record: &EamProjectionRecord,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
) -> EamProjectionRecord {
    let mut normalized = record.clone();
    let active_team_uid = active_team_uid(hub_directory_snapshot);
    let team_color = canonical_team_color_for_uid(active_team_uid).unwrap_or("YELLOW");
    normalized.group_name = team_color.to_string();
    normalized.team_uid = Some(active_team_uid.to_string());
    let active_team_member_uid = hub_directory_snapshot.and_then(|snapshot| {
        snapshot
            .caller_memberships
            .iter()
            .find(|membership| membership.team_uid == active_team_uid)
            .map(|membership| membership.team_member_uid.clone())
    });
    normalized.team_member_uid = active_team_member_uid.or_else(|| {
        let app_hash = status.app_destination_hex.trim();
        (!app_hash.is_empty()).then(|| app_hash.to_string())
    });
    if is_blank(normalized.reported_by.as_deref()) && !status.name.trim().is_empty() {
        normalized.reported_by = Some(status.name.trim().to_string());
    }
    if normalized.source.is_none() && !status.identity_hex.trim().is_empty() {
        normalized.source = Some(EamSourceRecord {
            rns_identity: status.identity_hex.clone(),
            display_name: (!status.name.trim().is_empty()).then(|| status.name.trim().to_string()),
        });
    }
    if normalized.overall_status.is_none() {
        normalized.overall_status = derive_eam_overall_status(&normalized);
    }
    normalized
}

#[cfg(test)]
fn has_known_lxmf_route(peer: &PeerRecord) -> bool {
    delivery_policy::peer_has_known_lxmf_route(peer)
}

fn peer_is_directly_reachable(peer: &PeerRecord) -> bool {
    delivery_policy::peer_is_directly_reachable(peer)
}

fn peer_has_observed_lxmf_delivery_route(peer: &PeerRecord) -> bool {
    delivery_policy::peer_has_observed_lxmf_delivery_route(
        peer,
        now_ms(),
        sdkmsg::DEFAULT_PEER_STALE_AFTER_MS,
    )
}

fn peer_is_current_replication_target(peer: &PeerRecord) -> bool {
    delivery_policy::peer_is_current_replication_target(peer)
}

fn peer_supports_mission_traffic(peer: &PeerRecord) -> bool {
    has_capability_token(peer.app_data.as_deref(), "r3akt")
        && has_capability_token(peer.app_data.as_deref(), "emergencymessages")
}

fn peer_is_direct_delivery_ready(peer: &PeerRecord) -> bool {
    peer_connectivity_model(peer, false, peer.saved).direct_delivery_available()
}

fn peer_has_current_known_lxmf_route(peer: &PeerRecord) -> bool {
    delivery_policy::peer_has_current_known_lxmf_route(peer)
}

fn peer_connectivity_model(
    peer: &PeerRecord,
    has_active_relay: bool,
    saved: bool,
) -> delivery_policy::PeerConnectivityModel {
    delivery_policy::PeerConnectivityModel::from_peer_with_saved(
        peer,
        saved,
        has_active_relay,
        true,
        false,
        now_ms(),
        sdkmsg::DEFAULT_PEER_STALE_AFTER_MS,
    )
}

fn peer_is_mission_direct_delivery_ready(peer: &PeerRecord, has_active_relay: bool) -> bool {
    let connectivity = peer_connectivity_model(peer, has_active_relay, peer.saved);
    if has_active_relay {
        return connectivity.direct_delivery_available();
    }
    connectivity.direct_delivery_available() || peer_has_current_known_lxmf_route(peer)
}

fn peer_can_use_propagation_fallback(peer: &PeerRecord) -> bool {
    delivery_policy::peer_can_use_propagation_fallback(peer)
}

fn peer_has_stored_propagation_route(peer: &PeerRecord) -> bool {
    peer_connectivity_model(peer, true, true).stored_propagation_available()
}

fn saved_peer_target_destination(peer: &SavedPeerRecord) -> Option<String> {
    peer.lxmf_destination_hex
        .as_deref()
        .and_then(normalize_hex_32)
        .or_else(|| normalize_hex_32(peer.destination_hex.as_str()))
}

fn saved_peer_supports_mission_traffic(peer: &SavedPeerRecord) -> bool {
    has_capability_token(peer.app_data.as_deref(), "r3akt")
        && has_capability_token(peer.app_data.as_deref(), "emergencymessages")
}

fn saved_peer_has_stored_propagation_route(peer: &SavedPeerRecord) -> bool {
    saved_peer_supports_mission_traffic(peer)
        && normalize_hex_32(peer.destination_hex.as_str()).is_some()
        && saved_peer_target_destination(peer).is_some()
}

fn saved_peer_can_try_stored_lxmf_route(peer: &PeerRecord, saved: bool) -> bool {
    peer_supports_mission_traffic(peer)
        && peer_connectivity_model(peer, true, saved).stored_propagation_available()
}

fn peer_has_usable_propagation_route(peer: &PeerRecord, has_active_relay: bool) -> bool {
    peer_connectivity_model(peer, has_active_relay, true).propagation_eligible
        && peer_can_use_propagation_fallback(peer)
}

fn peer_can_use_direct_when_relay_route_is_missing(
    peer: &PeerRecord,
    has_active_relay: bool,
) -> bool {
    !peer_has_usable_propagation_route(peer, has_active_relay) && peer_is_directly_reachable(peer)
}

fn direct_replication_target_priority(peer: &PeerRecord) -> u8 {
    if peer_is_current_replication_target(peer) || peer_has_observed_lxmf_delivery_route(peer) {
        0
    } else {
        1
    }
}

fn build_prioritized_direct_target(
    peer: &PeerRecord,
    app_destination_hex: String,
    sequence: usize,
) -> PrioritizedMissionReplicationTarget {
    PrioritizedMissionReplicationTarget {
        priority: direct_replication_target_priority(peer),
        sequence,
        target: MissionReplicationTarget {
            app_destination_hex,
            send_mode: SendMode::Auto {},
        },
    }
}

fn finish_replication_targets(
    mut direct_targets: Vec<PrioritizedMissionReplicationTarget>,
    mut relay_targets: Vec<MissionReplicationTarget>,
) -> Vec<MissionReplicationTarget> {
    direct_targets.sort_by_key(|target| (target.priority, target.sequence));
    let mut targets = direct_targets
        .into_iter()
        .map(|target| target.target)
        .collect::<Vec<_>>();
    targets.append(&mut relay_targets);
    targets
}

fn announce_route_hops(announces: &[AnnounceRecord]) -> HashMap<String, u8> {
    let mut route_hops = HashMap::<String, u8>::new();
    for announce in announces {
        let Some(destination_hex) = normalize_hex_32(announce.destination_hex.as_str()) else {
            continue;
        };
        route_hops
            .entry(destination_hex)
            .and_modify(|hops| *hops = (*hops).min(announce.hops))
            .or_insert(announce.hops);
    }
    route_hops
}
#[cfg(test)]
const TEAM_UID_YELLOW: &str = YELLOW_TEAM_UID;
