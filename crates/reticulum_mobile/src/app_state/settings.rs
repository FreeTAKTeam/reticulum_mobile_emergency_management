impl AppStateStore {
    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let raw: Option<String> = transaction
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let mut settings = deserialize_json::<AppSettingsRecord>(&raw)?;
        // Storage-only marker: adding a field to TeamSettingsRecord changes UniFFI.
        let completed: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata WHERE key = 'circle_migration_completed' AND value = '1')",
            [], |row| row.get(0),
        ).map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        if !completed {
            let mut peers: Vec<SavedPeerRecord> = query_json_records(
                &transaction,
                "SELECT json FROM saved_peers ORDER BY destination_hex",
            )?;
            initialize_local_team_settings(&mut settings, std::iter::empty());
            let additions = peers
                .iter()
                .filter(|peer| matches!(peer.circle_tier, CircleTier::Inner {}))
                .filter(|peer| {
                    !crate::team_membership::saved_peer_is_local_team_member(
                        &settings.teams.local_teams,
                        peer,
                    )
                })
                .filter_map(|peer| crate::delivery_policy::normalize_hex_32(&peer.destination_hex))
                .collect::<Vec<_>>();
            if !additions.is_empty() {
                if !settings
                    .teams
                    .local_teams
                    .iter()
                    .any(|team| team.team_uid == YELLOW_TEAM_UID)
                {
                    settings.teams.local_teams.push(LocalTeamRecord {
                        team_uid: YELLOW_TEAM_UID.to_string(),
                        member_destinations: Vec::new(),
                    });
                }
                if let Some(yellow) = settings
                    .teams
                    .local_teams
                    .iter_mut()
                    .find(|team| team.team_uid == YELLOW_TEAM_UID)
                {
                    yellow.member_destinations.extend(additions);
                    yellow.member_destinations.sort();
                    yellow.member_destinations.dedup();
                }
            }
            crate::team_membership::apply_derived_circle_tiers(
                &mut peers,
                &settings.teams.local_teams,
            );
            self.write_app_settings_tx(&transaction, &settings)?;
            for peer in &peers {
                self.write_saved_peer_tx(&transaction, peer)?;
            }
            transaction.execute(
                "INSERT INTO metadata (key, value) VALUES ('circle_migration_completed', '1') ON CONFLICT(key) DO UPDATE SET value = '1'", [],
            ).map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
            for scope in [
                ProjectionScope::AppSettings {},
                ProjectionScope::SavedPeers {},
            ] {
                self.bump_projection_revision_tx(
                    &transaction,
                    scope,
                    None,
                    Some("circle-migration".to_string()),
                )?;
            }
        }
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(Some(settings))
    }

    pub fn set_app_settings(
        &self,
        settings: &AppSettingsRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        self.write_app_settings_tx(&transaction, settings)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::AppSettings {},
            None,
            Some("settings-updated".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(invalidation)
    }
}
