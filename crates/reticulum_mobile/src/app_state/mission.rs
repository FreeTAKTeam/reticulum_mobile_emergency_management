impl AppStateStore {
    pub fn get_eams(&self) -> Result<Vec<EamProjectionRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM eams ORDER BY updated_at_ms DESC",
        )
    }

    pub fn upsert_eam(
        &self,
        record: &EamProjectionRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        self.write_eam_tx(&transaction, record)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Eams {},
            Some(record.callsign.to_ascii_lowercase()),
            Some("eam-upserted".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

    pub fn delete_eam(
        &self,
        callsign: &str,
        deleted_at_ms: u64,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        if let Some(raw) = transaction
            .query_row(
                "SELECT json FROM eams WHERE callsign_key = ?1",
                params![callsign.trim().to_ascii_lowercase()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?
        {
            let mut record: EamProjectionRecord = deserialize_json(&raw)?;
            record.deleted_at_ms = Some(deleted_at_ms);
            record.updated_at_ms = deleted_at_ms;
            self.write_eam_tx(&transaction, &record)?;
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Eams {},
            Some(callsign.trim().to_ascii_lowercase()),
            Some("eam-deleted".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

    pub fn get_eam_team_summary(
        &self,
        team_uid: &str,
    ) -> Result<Option<EamTeamSummaryRecord>, NodeError> {
        let records: Vec<EamProjectionRecord> = self
            .get_eams()?
            .into_iter()
            .filter(|record| record.team_uid.as_deref() == Some(team_uid))
            .collect();
        if records.is_empty() {
            return Ok(None);
        }
        let mut summary = EamTeamSummaryRecord {
            team_uid: team_uid.to_string(),
            total: crate::numeric::usize_to_u32_saturating(records.len()),
            active_total: 0,
            deleted_total: 0,
            overall_status: None,
            green_total: 0,
            yellow_total: 0,
            red_total: 0,
            updated_at_ms: 0,
        };
        for record in records {
            summary.updated_at_ms = summary.updated_at_ms.max(record.updated_at_ms);
            if record.deleted_at_ms.is_some() {
                summary.deleted_total += 1;
                continue;
            }
            summary.active_total += 1;
            match record.overall_status.as_deref() {
                Some("Green") => summary.green_total += 1,
                Some("Yellow") => summary.yellow_total += 1,
                Some("Red") => summary.red_total += 1,
                _ => {}
            }
        }
        summary.overall_status = if summary.red_total > 0 {
            Some("Red".to_string())
        } else if summary.yellow_total > 0 {
            Some("Yellow".to_string())
        } else if summary.green_total > 0 {
            Some("Green".to_string())
        } else {
            None
        };
        Ok(Some(summary))
    }

    pub fn get_eam_readiness_summary(&self) -> Result<EamReadinessSummaryRecord, NodeError> {
        Ok(build_eam_readiness_summary(self.get_eams()?))
    }

    pub fn get_events(&self) -> Result<Vec<EventProjectionRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM events ORDER BY updated_at_ms DESC",
        )
    }

    pub fn upsert_event(
        &self,
        record: &EventProjectionRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut normalized = record.clone();
        if record.mission_uid == crate::node::COMMUNITY_MISSION_UID
            || record.content.starts_with(crate::node::COMMUNITY_PREFIX)
        {
            let existing = self
                .get_events()?
                .into_iter()
                .find(|candidate| candidate.uid == record.uid);
            match crate::node::community_event_is_newer(existing.as_ref(), record, now_ms()) {
                Ok(false) => {
                    return self.bump_projection_revision(
                        ProjectionScope::Events {},
                        Some(record.uid.clone()),
                        Some("community-replay-ignored".to_string()),
                    );
                }
                Ok(true) => {}
                Err(_) => {
                    normalized.uid = format!("generic:{}:{}", record.uid, record.updated_at_ms);
                    normalized.mission_uid = "r3akt-default-mission".to_string();
                    normalized
                        .topics
                        .retain(|topic| topic != crate::node::COMMUNITY_TOPIC);
                }
            }
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        self.write_event_tx(&transaction, &normalized)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Events {},
            Some(normalized.uid.clone()),
            Some("event-upserted".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

    pub fn delete_event(
        &self,
        uid: &str,
        deleted_at_ms: u64,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        if let Some(raw) = transaction
            .query_row(
                "SELECT json FROM events WHERE uid = ?1",
                params![uid],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?
        {
            let mut record: EventProjectionRecord = deserialize_json(&raw)?;
            record.deleted_at_ms = Some(deleted_at_ms);
            record.updated_at_ms = deleted_at_ms;
            self.write_event_tx(&transaction, &record)?;
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Events {},
            Some(uid.to_string()),
            Some("event-deleted".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

}
