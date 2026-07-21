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
        return saved_peers.to_vec();
    };
    if snapshot.local_teams.is_empty() && active_team_uid(Some(snapshot)) == YELLOW_TEAM_UID {
        return saved_peers.to_vec();
    }
    let destinations = active_local_team_destinations(snapshot)
        .into_iter()
        .collect::<HashSet<_>>();
    saved_peers
        .iter()
        .filter(|peer| {
            normalize_hex_32(&peer.destination_hex)
                .is_some_and(|destination| destinations.contains(&destination))
        })
        .cloned()
        .collect()
}
