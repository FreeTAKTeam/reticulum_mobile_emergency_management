const HOUSEHOLD_SITUATION_KEY: &str = "household_situation_v1";

/// Everything one situation change stores, written in a single transaction so the
/// device's own screens never show half of a situation.
pub(crate) struct HouseholdSituationWrite<'a> {
    pub state_json: &'a str,
    pub settings: Option<&'a AppSettingsRecord>,
    pub eam: Option<&'a EamProjectionRecord>,
    pub events: &'a [EventProjectionRecord],
}

impl AppStateStore {
    pub(crate) fn household_situation_json(&self) -> Result<Option<String>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![HOUSEHOLD_SITUATION_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))
    }

    pub(crate) fn commit_household_situation(
        &self,
        write: &HouseholdSituationWrite<'_>,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        transaction
            .execute(
                "INSERT INTO metadata (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![HOUSEHOLD_SITUATION_KEY, write.state_json],
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let reason = || Some("household-situation".to_string());
        let mut invalidations = Vec::new();
        if let Some(settings) = write.settings {
            self.write_app_settings_tx(&transaction, settings)?;
        }
        // The situation itself lives beside the settings, so its readers follow AppSettings.
        invalidations.push(self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::AppSettings {},
            Some(HOUSEHOLD_SITUATION_KEY.to_string()),
            reason(),
        )?);
        if let Some(eam) = write.eam {
            self.write_eam_tx(&transaction, eam)?;
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Eams {},
                Some(eam.callsign.to_ascii_lowercase()),
                reason(),
            )?);
        }
        for event in write.events {
            self.write_event_tx(&transaction, event)?;
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Events {},
                Some(event.uid.clone()),
                reason(),
            )?);
        }
        if write.eam.is_some() || !write.events.is_empty() {
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::OperationalSummary {},
                None,
                reason(),
            )?);
        }
        transaction
            .commit()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }
}
