pub(crate) fn apply_local_team_settings(
    snapshot: &mut HubDirectorySnapshot,
    settings: &TeamSettingsRecord,
) {
    snapshot.active_team_uid = settings.active_team_uid.clone();
    snapshot.local_teams = settings.local_teams.clone();
    for local_team in &settings.local_teams {
        if snapshot.teams.iter().any(|team| team.uid == local_team.team_uid) {
            continue;
        }
        if let Some(color) = canonical_team_color_for_uid(&local_team.team_uid) {
            snapshot.teams.push(crate::types::HubTeamRecord {
                uid: local_team.team_uid.clone(),
                color: color.to_string(),
                team_name: color.to_string(),
            });
        }
    }
    snapshot
        .teams
        .sort_by_key(|team| (team.uid != YELLOW_TEAM_UID, team.color.clone()));
}

fn normalize_team_settings(settings: &mut TeamSettingsRecord) -> Result<(), NodeError> {
    if canonical_team_color_for_uid(settings.active_team_uid.trim()).is_none()
        || settings.aliases.len() > 13
        || settings.local_teams.len() > 13
    {
        return Err(NodeError::InvalidConfig {});
    }
    let mut seen_aliases = HashSet::new();
    settings.aliases.retain_mut(|alias| {
        alias.team_uid = alias.team_uid.trim().to_ascii_lowercase();
        alias.alias = alias.alias.trim().chars().take(48).collect();
        canonical_team_color_for_uid(&alias.team_uid).is_some()
            && !alias.alias.is_empty()
            && seen_aliases.insert(alias.team_uid.clone())
    });
    let mut seen_teams = HashSet::new();
    for team in &mut settings.local_teams {
        team.team_uid = team.team_uid.trim().to_ascii_lowercase();
        if canonical_team_color_for_uid(&team.team_uid).is_none()
            || !seen_teams.insert(team.team_uid.clone())
        {
            return Err(NodeError::InvalidConfig {});
        }
        let mut seen_members = HashSet::new();
        team.member_destinations = team
            .member_destinations
            .iter()
            .filter_map(|destination| normalize_hex_32(destination))
            .filter(|destination| seen_members.insert(destination.clone()))
            .collect();
    }
    if settings.local_teams_initialized
        && !settings
            .local_teams
            .iter()
            .any(|team| team.team_uid == YELLOW_TEAM_UID)
    {
        settings.local_teams.insert(
            0,
            crate::types::LocalTeamRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                member_destinations: Vec::new(),
            },
        );
    }
    Ok(())
}

fn restored_hub_directory_for_config(
    app_state: &AppStateStore,
    config: &NodeConfig,
) -> Result<Option<HubDirectorySnapshot>, NodeError> {
    let hub_identity_hash = config
        .hub_identity_hash
        .as_deref()
        .and_then(normalize_hex_32);
    let mut snapshot = if let Some(hub_identity_hash) = hub_identity_hash.as_deref() {
        app_state
            .get_hub_directory(hub_identity_hash)?
            .unwrap_or_else(|| HubDirectorySnapshot::yellow_only(crate::runtime::now_ms()))
    } else {
        HubDirectorySnapshot::yellow_only(crate::runtime::now_ms())
    };
    snapshot.hub_identity_hash = hub_identity_hash;
    if let Some(settings) = app_state.get_app_settings()? {
        let selected_available = settings.teams.active_team_uid == YELLOW_TEAM_UID
            || settings
                .teams
                .local_teams
                .iter()
                .any(|team| team.team_uid == settings.teams.active_team_uid)
            || snapshot.caller_memberships.iter().any(|membership| {
                membership.team_uid == settings.teams.active_team_uid
            });
        let mut team_settings = settings.teams;
        if !selected_available {
            team_settings.active_team_uid = YELLOW_TEAM_UID.to_string();
        }
        apply_local_team_settings(&mut snapshot, &team_settings);
    }
    Ok(Some(snapshot))
}

impl Node {
    pub fn refresh_hub_directory(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(
                inner.power_state.saver_active,
                OutboundTrafficClass::Control {},
            )?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };
        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::RefreshHubDirectory { resp: resp_tx })?;
        resp_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn get_hub_directory_snapshot(&self) -> Result<HubDirectorySnapshot, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let team_settings = inner
            .app_state
            .get_app_settings()?
            .map(|settings| settings.teams)
            .unwrap_or_default();
        let mut snapshot = inner
            .hub_directory_snapshot
            .lock()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?
            .clone()
            .unwrap_or_else(|| HubDirectorySnapshot::yellow_only(crate::runtime::now_ms()));
        apply_local_team_settings(&mut snapshot, &team_settings);
        Ok(snapshot)
    }

    pub fn set_active_team(&self, team_uid: String) -> Result<(), NodeError> {
        let team_uid = team_uid.trim();
        if canonical_team_color_for_uid(team_uid).is_none() {
            return Err(NodeError::InvalidConfig {});
        }
        let (previous_team_uid, local_eams) = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let settings = inner
                .app_state
                .get_app_settings()?
                .ok_or(NodeError::InvalidConfig {})?;
            let snapshot = inner.hub_directory_snapshot.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let is_local_team = settings
                .teams
                .local_teams
                .iter()
                .any(|team| team.team_uid == team_uid);
            if team_uid != YELLOW_TEAM_UID && !is_local_team
                && !snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.schema_version >= HUB_DIRECTORY_SCHEMA_VERSION
                        && snapshot
                            .caller_memberships
                            .iter()
                            .any(|membership| membership.team_uid == team_uid)
                })
            {
                return Err(NodeError::InvalidConfig {});
            }
            let previous_team_uid = settings.teams.active_team_uid;
            if previous_team_uid == team_uid {
                return Ok(());
            }
            let status = inner.status.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let local_identity = normalize_hex_32(&status.identity_hex);
            let local_destination = normalize_hex_32(&status.app_destination_hex);
            let local_eams = inner
                .app_state
                .get_eams()?
                .into_iter()
                .filter(|record| record.deleted_at_ms.is_none())
                .filter(|record| {
                    record
                        .source
                        .as_ref()
                        .and_then(|source| normalize_hex_32(&source.rns_identity))
                        .is_some_and(|identity| Some(identity) == local_identity)
                        || record
                            .team_member_uid
                            .as_deref()
                            .and_then(normalize_hex_32)
                            .is_some_and(|destination| Some(destination) == local_destination)
                })
                .collect::<Vec<_>>();
            (previous_team_uid, local_eams)
        };

        let moved_at_ms = crate::runtime::now_ms();
        for record in &local_eams {
            self.delete_eam(record.callsign.clone(), moved_at_ms)?;
        }
        {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let mut settings = inner
                .app_state
                .get_app_settings()?
                .ok_or(NodeError::InvalidConfig {})?;
            settings.teams.active_team_uid = team_uid.to_string();
            let invalidation = inner.app_state.set_app_settings(&settings)?;
            inner.bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            let mut snapshot_guard = inner.hub_directory_snapshot.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let mut snapshot = snapshot_guard
                .clone()
                .unwrap_or_else(|| HubDirectorySnapshot::yellow_only(crate::runtime::now_ms()));
            snapshot.active_team_uid = team_uid.to_string();
            snapshot.local_teams = settings.teams.local_teams.clone();
            if let Some(hub_identity_hash) = snapshot.hub_identity_hash.as_deref() {
                inner
                    .app_state
                    .set_hub_directory(hub_identity_hash, &snapshot)?;
            }
            *snapshot_guard = Some(snapshot.clone());
            inner
                .bus
                .emit(NodeEvent::HubDirectoryUpdated { snapshot });
            inner.bus.emit(NodeEvent::OperationalNotice {
                notice: OperationalNotice {
                    level: LogLevel::Info {},
                    message: format!(
                        "Active TEAM changed from {previous_team_uid} to {team_uid}"
                    ),
                    at_ms: moved_at_ms,
                },
            });
        }
        for mut record in local_eams {
            record.deleted_at_ms = None;
            record.team_uid = None;
            record.team_member_uid = None;
            record.updated_at_ms = moved_at_ms;
            self.upsert_eam(record)?;
        }
        Ok(())
    }
}
