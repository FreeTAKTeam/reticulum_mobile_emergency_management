/// Additional local recipients only; the release's online target selection runs first.
fn offline_saved_targets(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    online: &[MissionReplicationTarget],
    telemetry: bool,
) -> Vec<MissionReplicationTarget> {
    saved
        .iter()
        .filter_map(|saved_peer| {
            let destination = saved_peer_target_destination(saved_peer)?;
            let aliases = |candidate: &str| {
                crate::team_membership::destinations_include_peer(
                    &[candidate.to_owned()],
                    &saved_peer.destination_hex,
                    saved_peer.lxmf_destination_hex.as_deref(),
                    saved_peer.identity_hex.as_deref(),
                )
            };
            if aliases(&status.app_destination_hex)
                || aliases(&status.lxmf_destination_hex)
                || online
                    .iter()
                    .any(|target| aliases(&target.app_destination_hex))
            {
                return None;
            }
            let peer = peers
                .iter()
                .find(|peer| peer_destination_hashes(peer).any(|hash| aliases(&hash)));
            if !status.interfaces.is_empty()
                && peer.is_some_and(|peer| {
                    peer.active_link
                        || peer_is_current_replication_target(peer)
                        || peer_has_current_known_lxmf_route(peer)
                })
            {
                return None;
            }
            let app_data = peer
                .and_then(|peer| peer.app_data.as_deref())
                .or(saved_peer.app_data.as_deref());
            let allowed = if telemetry {
                has_capability_token(app_data, "telemetry")
            } else {
                has_capability_token(app_data, "r3akt")
                    && has_capability_token(app_data, "emergencymessages")
            };
            allowed.then_some(MissionReplicationTarget {
                app_destination_hex: destination,
                send_mode: SendMode::Auto {},
            })
        })
        .collect()
}

fn mission_targets_with_offline(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    relay: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let online = build_mission_replication_targets(status, peers, saved, relay);
    let offline = offline_saved_targets(status, peers, saved, &online, false);
    merge_replication_target_sets(online, offline)
}

fn event_targets_with_offline(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    relay: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let online = build_event_replication_targets(status, peers, saved, relay);
    let offline = offline_saved_targets(status, peers, saved, &online, false);
    merge_replication_target_sets(online, offline)
}

fn telemetry_targets_with_offline(
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    relay: Option<&str>,
) -> Vec<MissionReplicationTarget> {
    let online = telemetry_targets_from_peers_with_relay(
        peers,
        normalize_hex_32(&status.app_destination_hex).as_deref(),
        relay,
    );
    let offline = offline_saved_targets(status, peers, saved, &online, true);
    merge_replication_target_sets(online, offline)
}
