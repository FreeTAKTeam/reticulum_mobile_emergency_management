impl AppStateStore {
    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let mut settings = deserialize_json::<AppSettingsRecord>(&raw)?;
        if !settings.teams.local_teams_initialized {
            let mut statement = connection
                .prepare("SELECT destination_hex FROM saved_peers ORDER BY destination_hex")
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
            let destinations = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
            drop(statement);
            if initialize_local_team_settings(&mut settings, destinations) {
                connection
                    .execute(
                        "UPDATE app_settings SET json = ?1, updated_at_ms = ?2 WHERE id = 1",
                        params![
                            serialize_json(&settings)?,
                            crate::numeric::u64_to_i64_saturating(now_ms())
                        ],
                    )
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
            }
        }
        Ok(Some(settings))
    }

    pub fn set_app_settings(
        &self,
        settings: &AppSettingsRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        self.write_app_settings_tx(&transaction, settings)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::AppSettings {},
            None,
            Some("settings-updated".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }
}
