fn hub_directory_contains_active_team(
    snapshot: &HubDirectorySnapshot,
    selected_team_uid: &str,
) -> bool {
    selected_team_uid == YELLOW_TEAM_UID
        || snapshot
            .local_teams
            .iter()
            .any(|team| team.team_uid == selected_team_uid)
        || (snapshot.schema_version >= HUB_DIRECTORY_SCHEMA_VERSION
            && snapshot
                .caller_memberships
                .iter()
                .any(|membership| membership.team_uid == selected_team_uid))
}
