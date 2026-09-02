fn to_app_settings_record(input: AppSettingsInput) -> Result<AppSettingsRecord, NodeError> {
    Ok(AppSettingsRecord {
        display_name: input.display_name,
        auto_connect_saved: input.auto_connect_saved,
        announce_capabilities: input.announce_capabilities,
        tcp_clients: input.tcp_clients,
        broadcast: input.broadcast,
        transport_node_enabled: input.transport_node_enabled,
        announce_interval_seconds: input.announce_interval_seconds,
        telemetry: TelemetrySettingsRecord {
            enabled: input.telemetry.enabled,
            publish_interval_seconds: input.telemetry.publish_interval_seconds,
            accuracy_threshold_meters: input.telemetry.accuracy_threshold_meters,
            stale_after_minutes: input.telemetry.stale_after_minutes,
            expire_after_minutes: input.telemetry.expire_after_minutes,
        },
        hub: HubSettingsRecord {
            mode: parse_hub_mode(Some(input.hub.mode.as_str())),
            identity_hash: input.hub.identity_hash,
            api_base_url: input.hub.api_base_url,
            api_key: input.hub.api_key,
            refresh_interval_seconds: input.hub.refresh_interval_seconds,
        },
        teams: to_team_settings_record(input.teams)?,
        checklists: ChecklistSettingsRecord {
            default_task_due_step_minutes: input
                .checklists
                .default_task_due_step_minutes
                .unwrap_or(crate::types::DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES)
                .max(1),
        },
        rnode: to_rnode_settings_record(Some(input.rnode))?,
        community: CommunitySettingsRecord {
            household_id: input.community.household_id.unwrap_or_default().trim().to_string(),
            household_name: input.community.household_name.unwrap_or_default().trim().to_string(),
            adults: input.community.adults.unwrap_or(0).min(20),
            children: input.community.children.unwrap_or(0).min(20),
            pets: input.community.pets.unwrap_or(0).min(20),
            role_badges: input
                .community
                .role_badges
                .into_iter()
                .map(|role| role.trim().to_string())
                .filter(|role| !role.is_empty())
                .take(5)
                .collect(),
            status: parse_household_status(input.community.status.as_deref())?,
            preferred_map_layer: parse_preferred_map_layer(
                input.community.preferred_map_layer.as_deref(),
            )?,
        },
        power: PowerPolicyRecord {
            enabled: input.power.enabled.unwrap_or(true),
            threshold_percent: parse_power_threshold(input.power.threshold_percent)?,
        },
    })
}

fn operational_notice_json(notice: &crate::types::OperationalNotice) -> serde_json::Value {
    json!({
        "level": log_level_to_str(notice.level),
        "message": notice.message,
        "atMs": notice.at_ms
    })
}

fn hub_settings_json(settings: &HubSettingsRecord) -> serde_json::Value {
    json!({
        "mode": settings.mode.as_str(),
        "identityHash": settings.identity_hash,
        "apiBaseUrl": settings.api_base_url,
        "apiKey": settings.api_key,
        "refreshIntervalSeconds": settings.refresh_interval_seconds
    })
}

fn telemetry_settings_json(settings: &TelemetrySettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "publishIntervalSeconds": settings.publish_interval_seconds,
        "accuracyThresholdMeters": settings.accuracy_threshold_meters,
        "staleAfterMinutes": settings.stale_after_minutes,
        "expireAfterMinutes": settings.expire_after_minutes
    })
}

fn rnode_settings_json(settings: &RnodeSettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "connectionMode": settings.connection_mode,
        "peripheralId": settings.peripheral_id,
        "displayName": settings.display_name,
        "region": settings.region,
        "profile": settings.profile,
        "frequencyHz": settings.frequency_hz
    })
}

fn app_settings_json(settings: &AppSettingsRecord) -> serde_json::Value {
    json!({
        "displayName": settings.display_name,
        "autoConnectSaved": settings.auto_connect_saved,
        "announceCapabilities": settings.announce_capabilities,
        "tcpClients": settings.tcp_clients,
        "broadcast": settings.broadcast,
        "transportNodeEnabled": settings.transport_node_enabled,
        "announceIntervalSeconds": settings.announce_interval_seconds,
        "telemetry": telemetry_settings_json(&settings.telemetry),
        "hub": hub_settings_json(&settings.hub),
        "teams": {
            "activeTeamUid": settings.teams.active_team_uid,
            "aliases": settings.teams.aliases.iter().map(|alias| json!({
                "teamUid": alias.team_uid,
                "alias": alias.alias,
            })).collect::<Vec<_>>(),
            "localTeams": settings.teams.local_teams.iter().map(|team| json!({
                "teamUid": team.team_uid,
                "memberDestinations": team.member_destinations,
            })).collect::<Vec<_>>(),
            "localTeamsInitialized": settings.teams.local_teams_initialized
        },
        "checklists": {
            "defaultTaskDueStepMinutes": settings.checklists.default_task_due_step_minutes
        },
        "rnode": rnode_settings_json(&settings.rnode),
        "community": {
            "householdId": settings.community.household_id,
            "householdName": settings.community.household_name,
            "adults": settings.community.adults,
            "children": settings.community.children,
            "pets": settings.community.pets,
            "roleBadges": settings.community.role_badges,
            "status": settings.community.status.as_str(),
            "preferredMapLayer": settings.community.preferred_map_layer.as_str(),
        },
        "power": {
            "enabled": settings.power.enabled,
            "thresholdPercent": settings.power.threshold_percent,
        }
    })
}
