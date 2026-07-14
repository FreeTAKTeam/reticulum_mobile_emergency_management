impl AppStateStore {
    pub fn get_telemetry_positions(&self) -> Result<Vec<TelemetryPositionRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM telemetry_positions ORDER BY updated_at_ms DESC",
        )
    }

    pub fn record_local_telemetry_fix(
        &self,
        position: &TelemetryPositionRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_telemetry_tx(&transaction, position)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Telemetry {},
            Some(position.callsign.to_ascii_lowercase()),
            Some("telemetry-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn delete_local_telemetry(
        &self,
        callsign: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        transaction
            .execute(
                "DELETE FROM telemetry_positions WHERE callsign_key = ?1",
                params![callsign.trim().to_ascii_lowercase()],
            )
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Telemetry {},
            Some(callsign.trim().to_ascii_lowercase()),
            Some("telemetry-deleted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn get_sos_settings(&self) -> Result<Option<SosSettingsRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row("SELECT json FROM sos_settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn set_sos_settings(
        &self,
        settings: &SosSettingsRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let json = serialize_json(settings)?;
        transaction
            .execute(
                "INSERT INTO sos_settings (id, json, updated_at_ms) VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json, updated_at_ms = excluded.updated_at_ms",
                params![json, now_ms() as i64],
            )
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Sos {},
            Some("settings".to_string()),
            Some("sos-settings-updated".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn get_sos_status(&self) -> Result<Option<SosStatusRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row("SELECT json FROM sos_state WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn set_sos_status(
        &self,
        status: &SosStatusRecord,
        reason: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let json = serialize_json(status)?;
        transaction
            .execute(
                "INSERT INTO sos_state (id, json, updated_at_ms) VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json, updated_at_ms = excluded.updated_at_ms",
                params![json, status.updated_at_ms as i64],
            )
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Sos {},
            Some("status".to_string()),
            Some(reason.to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn list_sos_alerts(&self) -> Result<Vec<SosAlertRecord>, NodeError> {
        let connection = self.connect()?;
        let records: Vec<SosAlertRecord> = query_json_records(
            &connection,
            "SELECT json FROM sos_alerts ORDER BY updated_at_ms DESC, incident_id ASC",
        )?;
        let mut filtered = Vec::new();
        for alert in records {
            if alert.active {
                if matches!(
                    sos_kind_from_text(alert.body_utf8.as_str()),
                    Some(crate::types::SosMessageKind::Cancelled {})
                ) {
                    continue;
                }
                if !conversation_has_messages(&connection, alert.conversation_id.as_str())?
                    || conversation_has_sos_cancellation(
                        &connection,
                        alert.conversation_id.as_str(),
                        alert.updated_at_ms,
                    )?
                {
                    continue;
                }
            }
            filtered.push(alert);
        }
        Ok(filtered)
    }

    pub(crate) fn latest_active_sos_alert_for_source(
        &self,
        source_hex: &str,
    ) -> Result<Option<SosAlertRecord>, NodeError> {
        let normalized = source_hex.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Ok(None);
        }
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM sos_alerts
                 WHERE source_hex = ?1 AND active = 1
                 ORDER BY updated_at_ms DESC, incident_id ASC
                 LIMIT 1",
                params![normalized],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn upsert_sos_alert(
        &self,
        alert: &SosAlertRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_sos_alert_tx(&transaction, alert)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Sos {},
            Some(alert.incident_id.clone()),
            Some("sos-alert-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn list_sos_locations(&self) -> Result<Vec<SosLocationRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM sos_locations ORDER BY recorded_at_ms ASC",
        )
    }

    pub fn upsert_sos_location(
        &self,
        location: &SosLocationRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let json = serialize_json(location)?;
        transaction
            .execute(
                "INSERT INTO sos_locations (incident_id, source_hex, recorded_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(incident_id, source_hex, recorded_at_ms) DO UPDATE SET json = excluded.json",
                params![
                    location.incident_id,
                    location.source_hex,
                    location.recorded_at_ms as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Sos {},
            Some(location.incident_id.clone()),
            Some("sos-location-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn list_sos_audio(&self) -> Result<Vec<SosAudioRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM sos_audio ORDER BY created_at_ms DESC",
        )
    }

    pub fn upsert_sos_audio(
        &self,
        audio: &SosAudioRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let json = serialize_json(audio)?;
        transaction
            .execute(
                "INSERT INTO sos_audio (audio_id, incident_id, source_hex, created_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(audio_id) DO UPDATE SET
                    incident_id = excluded.incident_id,
                    source_hex = excluded.source_hex,
                    created_at_ms = excluded.created_at_ms,
                    json = excluded.json",
                params![
                    audio.audio_id,
                    audio.incident_id,
                    audio.source_hex,
                    audio.created_at_ms as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Sos {},
            Some(audio.incident_id.clone()),
            Some("sos-audio-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

}
