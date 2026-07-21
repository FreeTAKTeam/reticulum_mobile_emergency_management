fn to_team_settings_record(input: TeamSettingsInput) -> Result<TeamSettingsRecord, NodeError> {
    let active_team_uid = input
        .active_team_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(YELLOW_TEAM_UID);
    if canonical_team_color_for_uid(active_team_uid).is_none() || input.aliases.len() > 13 {
        return Err(NodeError::InvalidConfig {});
    }
    let mut aliases = Vec::with_capacity(input.aliases.len());
    let mut seen = std::collections::HashSet::new();
    for alias in input.aliases {
        let team_uid = alias.team_uid.trim();
        let value = alias.alias.trim();
        if canonical_team_color_for_uid(team_uid).is_none()
            || value.is_empty()
            || value.chars().count() > 48
            || !seen.insert(team_uid.to_string())
        {
            return Err(NodeError::InvalidConfig {});
        }
        aliases.push(TeamAliasRecord {
            team_uid: team_uid.to_string(),
            alias: value.to_string(),
        });
    }
    if input.local_teams.len() > 13 {
        return Err(NodeError::InvalidConfig {});
    }
    let mut local_teams = Vec::with_capacity(input.local_teams.len());
    let mut seen_teams = std::collections::HashSet::new();
    for team in input.local_teams {
        let team_uid = team.team_uid.trim();
        if canonical_team_color_for_uid(team_uid).is_none()
            || !seen_teams.insert(team_uid.to_string())
        {
            return Err(NodeError::InvalidConfig {});
        }
        let mut members = Vec::with_capacity(team.member_destinations.len());
        let mut seen_members = std::collections::HashSet::new();
        for destination in team.member_destinations {
            let destination = normalize_hex_32(&destination).ok_or(NodeError::InvalidConfig {})?;
            if seen_members.insert(destination.clone()) {
                members.push(destination);
            }
        }
        local_teams.push(LocalTeamRecord {
            team_uid: team_uid.to_string(),
            member_destinations: members,
        });
    }
    if input.local_teams_initialized
        && !local_teams.iter().any(|team| team.team_uid == YELLOW_TEAM_UID)
    {
        local_teams.insert(
            0,
            LocalTeamRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                member_destinations: Vec::new(),
            },
        );
    }
    Ok(TeamSettingsRecord {
        active_team_uid: active_team_uid.to_string(),
        aliases,
        local_teams,
        local_teams_initialized: input.local_teams_initialized,
    })
}
