fn active_team_uid(snapshot: Option<&HubDirectorySnapshot>) -> &str {
    snapshot
        .map(|snapshot| snapshot.active_team_uid.trim())
        .filter(|team_uid| canonical_team_color_for_uid(team_uid).is_some())
        .unwrap_or(YELLOW_TEAM_UID)
}

fn active_local_team_destinations(snapshot: &HubDirectorySnapshot) -> Vec<String> {
    let active_team_uid = active_team_uid(Some(snapshot));
    snapshot
        .local_teams
        .iter()
        .find(|team| team.team_uid == active_team_uid)
        .map(|team| {
            let mut seen = HashSet::new();
            team.member_destinations
                .iter()
                .filter_map(|destination| normalize_hex_32(destination))
                .filter(|destination| seen.insert(destination.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn active_local_saved_peers(
    saved_peers: &[SavedPeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
) -> Vec<SavedPeerRecord> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let active_uid = active_team_uid(Some(snapshot));
    let teams = snapshot
        .local_teams
        .iter()
        .filter(|team| team.team_uid == active_uid)
        .cloned()
        .collect::<Vec<_>>();
    saved_peers
        .iter()
        .filter(|peer| crate::team_membership::saved_peer_is_local_team_member(&teams, peer))
        .cloned()
        .collect()
}

fn destination_is_active_local_member(
    destination: &str,
    snapshot: Option<&HubDirectorySnapshot>,
    saved_peers: &[SavedPeerRecord],
    peers: &[PeerRecord],
) -> bool {
    let Some(snapshot) = snapshot else {
        return false;
    };
    let members = active_local_team_destinations(snapshot);
    let requested = vec![destination.to_string()];
    members
        .iter()
        .any(|member| normalize_hex_32(destination).as_ref() == Some(member))
        || saved_peers.iter().any(|peer| {
            crate::team_membership::destinations_include_peer(
                &requested,
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            ) && crate::team_membership::destinations_include_peer(
                &members,
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            )
        })
        || peers.iter().any(|peer| {
            crate::team_membership::destinations_include_peer(
                &requested,
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            ) && crate::team_membership::destinations_include_peer(
                &members,
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            )
        })
}

fn destination_is_configured_hub(
    destination: &str,
    config: Option<&NodeConfigFingerprint>,
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
) -> bool {
    let Some(hub) = config
        .and_then(|config| config.hub_identity_hash.as_deref())
        .and_then(normalize_hex_32)
    else {
        return false;
    };
    let matches_hub = |peer_destination: &str, lxmf: Option<&str>, identity: Option<&str>| {
        crate::team_membership::destinations_include_peer(
            std::slice::from_ref(&hub),
            peer_destination,
            lxmf,
            identity,
        ) && crate::team_membership::destinations_include_peer(
            &[destination.to_string()],
            peer_destination,
            lxmf,
            identity,
        )
    };
    normalize_hex_32(destination).as_deref() == Some(hub.as_str())
        || peers.iter().any(|peer| {
            matches_hub(
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            )
        })
        || saved_peers.iter().any(|peer| {
            matches_hub(
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            )
        })
}

fn routed_outbound_destination_hex(
    destination: String,
    class: OutboundTrafficClass,
    config: Option<&NodeConfigFingerprint>,
    snapshot: Option<&HubDirectorySnapshot>,
    local_target: bool,
    hub_target: bool,
) -> Result<String, NodeError> {
    if matches!(
        class,
        OutboundTrafficClass::CommunityStatus {} | OutboundTrafficClass::Sos {}
    ) {
        return Ok(destination);
    }
    let connected = config.is_some_and(|config| {
        matches!(
            effective_hub_mode(config.hub_mode, snapshot),
            HubMode::Connected {}
        )
    });
    if matches!(class, OutboundTrafficClass::Telemetry {}) && connected && hub_target {
        return Err(NodeError::InvalidConfig {});
    }
    if local_target {
        return Ok(destination);
    }
    let routed = routed_destination_hex(destination, config, snapshot)?;
    if matches!(class, OutboundTrafficClass::Telemetry {}) && connected {
        // A roster-only target would be forwarded through RCH, which has no position policy.
        return Err(NodeError::InvalidConfig {});
    }
    Ok(routed)
}
