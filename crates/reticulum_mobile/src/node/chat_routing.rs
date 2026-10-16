fn routed_chat_destination_hex<F>(
    requested_destination_hex: String,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
    load_routing_context: F,
) -> Result<String, NodeError>
where
    F: FnOnce() -> Result<Vec<PeerRecord>, NodeError>,
{
    let connected = active_config.is_some_and(|config| {
        matches!(
            effective_hub_mode(config.hub_mode, hub_directory_snapshot),
            HubMode::Connected {}
        )
    });
    if connected {
        if let Some(snapshot) = hub_directory_snapshot {
            let roster_only = active_team_has_caller_membership(snapshot)
                && active_team_directory_destinations(snapshot, None)
                    .contains(&requested_destination_hex);
            if roster_only {
                let peers = load_routing_context()?;
                let has_own_route = peers.iter().any(|peer| {
                    crate::team_membership::destinations_include_peer(
                        std::slice::from_ref(&requested_destination_hex),
                        &peer.destination_hex,
                        peer.lxmf_destination_hex.as_deref(),
                        peer.identity_hex.as_deref(),
                    ) && (peer.active_link || peer_has_observed_lxmf_delivery_route(peer))
                });
                let locally_known = snapshot.local_teams.iter().any(|team| {
                    team.member_destinations.iter().any(|member| {
                        normalize_hex_32(member).as_deref() == Some(&requested_destination_hex)
                    })
                });
                if !has_own_route && !locally_known {
                    if let Some(config) = active_config {
                        return configured_hub_destination(config);
                    }
                }
            }
        }
    }
    Ok(requested_destination_hex)
}
