const DEFAULT_R3AKT_TEAM_COLOR: &str = "YELLOW";
const TEAM_UID_YELLOW: &str = "d6b6e188b910d6bdd24d04b7a7ec5444";
const TEAM_UID_RED: &str = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9";
const TEAM_UID_BLUE: &str = "43341e5c822d99857fa6e8641f2ca9c0";
const TEAM_UID_ORANGE: &str = "a83eb640e4c4884be14831e3d7ef5ae0";
const TEAM_UID_MAGENTA: &str = "7ac50a910f42b06cd9cb68dad3def681";
const TEAM_UID_MAROON: &str = "372824ef4f15881291455562f7570233";
const TEAM_UID_PURPLE: &str = "4bf2a1d2217c8668942658137f2a6824";
const TEAM_UID_DARK_BLUE: &str = "cbb35fc9a8f5a91d7bd2b5e5b644edcd";
const TEAM_UID_CYAN: &str = "d4cd5030b68df059ec6beabe416dd6a6";
const TEAM_UID_TEAL: &str = "4d7a7a974beec395bf83491604768499";
const TEAM_UID_GREEN: &str = "612a32262163b73a80eca944c2158546";
const TEAM_UID_DARK_GREEN: &str = "341653613d4c76d56bee99c1f38177b1";
const TEAM_UID_BROWN: &str = "4efe72ac30f5b85142fdcab6d96c7631";

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
    for item in &snapshot.items {
        let Some(destination_hex) = normalize_hex_32(item.destination_hash.as_str()) else {
            continue;
        };
        if self_destination_hex == Some(destination_hex.as_str()) {
            continue;
        }
        if !item
            .announce_capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("telemetry"))
        {
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
    let Some(config) = active_config else {
        return Ok(telemetry_targets_from_peers_with_relay(
            peers,
            self_destination_hex.as_deref(),
            active_propagation_node_hex,
        ));
    };

    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Autonomous {} => Ok(telemetry_targets_from_peers_with_relay(
            peers,
            self_destination_hex.as_deref(),
            active_propagation_node_hex,
        )),
        HubMode::Connected {} => Ok(vec![connected_hub_replication_target(
            peers,
            active_propagation_node_hex,
            config,
        )?]),
        HubMode::SemiAutonomous {} => {
            if config
                .hub_identity_hash
                .as_deref()
                .and_then(normalize_hex_32)
                .is_none()
            {
                return Ok(Vec::new());
            }
            let Some(snapshot) = hub_directory_snapshot else {
                return Ok(Vec::new());
            };
            Ok(build_transient_replication_targets(
                status,
                peers,
                &telemetry_destinations_from_hub_snapshot(
                    snapshot,
                    self_destination_hex.as_deref(),
                ),
                active_propagation_node_hex,
            ))
        }
    }
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
    match effective_hub_mode(config.hub_mode, hub_directory_snapshot) {
        HubMode::Connected {} => configured_hub_destination(config),
        HubMode::Autonomous {} | HubMode::SemiAutonomous {} => Ok(requested_destination_hex),
    }
}

fn is_blank(value: Option<&str>) -> bool {
    value.is_none_or(|entry| entry.trim().is_empty())
}

fn normalize_team_color(value: &str) -> &'static str {
    match value.trim().to_ascii_uppercase().as_str() {
        "RED" => "RED",
        "BLUE" => "BLUE",
        "ORANGE" => "ORANGE",
        "MAGENTA" => "MAGENTA",
        "MAROON" => "MAROON",
        "PURPLE" => "PURPLE",
        "DARK_BLUE" => "DARK_BLUE",
        "CYAN" => "CYAN",
        "TEAL" => "TEAL",
        "GREEN" => "GREEN",
        "DARK_GREEN" => "DARK_GREEN",
        "BROWN" => "BROWN",
        _ => DEFAULT_R3AKT_TEAM_COLOR,
    }
}

fn team_uid_for_color(color: &str) -> &'static str {
    match normalize_team_color(color) {
        "RED" => TEAM_UID_RED,
        "BLUE" => TEAM_UID_BLUE,
        "ORANGE" => TEAM_UID_ORANGE,
        "MAGENTA" => TEAM_UID_MAGENTA,
        "MAROON" => TEAM_UID_MAROON,
        "PURPLE" => TEAM_UID_PURPLE,
        "DARK_BLUE" => TEAM_UID_DARK_BLUE,
        "CYAN" => TEAM_UID_CYAN,
        "TEAL" => TEAM_UID_TEAL,
        "GREEN" => TEAM_UID_GREEN,
        "DARK_GREEN" => TEAM_UID_DARK_GREEN,
        "BROWN" => TEAM_UID_BROWN,
        _ => TEAM_UID_YELLOW,
    }
}

fn populate_eam_defaults(status: &NodeStatus, record: &EamProjectionRecord) -> EamProjectionRecord {
    let mut normalized = record.clone();
    let team_color = normalize_team_color(normalized.group_name.as_str());
    normalized.group_name = team_color.to_string();
    if is_blank(normalized.team_member_uid.as_deref()) {
        let app_hash = status.app_destination_hex.trim();
        if !app_hash.is_empty() {
            normalized.team_member_uid = Some(app_hash.to_string());
        }
    }
    if is_blank(normalized.team_uid.as_deref()) {
        normalized.team_uid = Some(team_uid_for_color(team_color).to_string());
    }
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
