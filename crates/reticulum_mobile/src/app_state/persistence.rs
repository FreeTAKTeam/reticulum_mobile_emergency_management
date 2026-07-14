impl AppStateStore {
    pub fn bump_projection_revision(
        &self,
        scope: ProjectionScope,
        key: Option<String>,
        reason: Option<String>,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let invalidation = self.bump_projection_revision_tx(&transaction, scope, key, reason)?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    fn write_app_settings_tx(
        &self,
        transaction: &Transaction<'_>,
        settings: &AppSettingsRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(settings)?;
        transaction
            .execute(
                "INSERT INTO app_settings (id, json, updated_at_ms) VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json, updated_at_ms = excluded.updated_at_ms",
                params![json, now_ms() as i64],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_saved_peer_tx(
        &self,
        transaction: &Transaction<'_>,
        peer: &SavedPeerRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(peer)?;
        transaction
            .execute(
                "INSERT INTO saved_peers (destination_hex, json, updated_at_ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT(destination_hex) DO UPDATE SET
                    json = excluded.json,
                    updated_at_ms = excluded.updated_at_ms",
                params![peer.destination_hex, json, peer.saved_at_ms as i64],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_eam_tx(
        &self,
        transaction: &Transaction<'_>,
        record: &EamProjectionRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(record)?;
        transaction
            .execute(
                "INSERT INTO eams (callsign_key, team_uid, overall_status, updated_at_ms, deleted_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(callsign_key) DO UPDATE SET
                    team_uid = excluded.team_uid,
                    overall_status = excluded.overall_status,
                    updated_at_ms = excluded.updated_at_ms,
                    deleted_at_ms = excluded.deleted_at_ms,
                    json = excluded.json",
                params![
                    record.callsign.to_ascii_lowercase(),
                    record.team_uid,
                    record.overall_status,
                    record.updated_at_ms as i64,
                    record.deleted_at_ms.map(|value| value as i64),
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_event_tx(
        &self,
        transaction: &Transaction<'_>,
        record: &EventProjectionRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(record)?;
        transaction
            .execute(
                "INSERT INTO events (uid, mission_uid, updated_at_ms, deleted_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(uid) DO UPDATE SET
                    mission_uid = excluded.mission_uid,
                    updated_at_ms = excluded.updated_at_ms,
                    deleted_at_ms = excluded.deleted_at_ms,
                    json = excluded.json",
                params![
                    record.uid,
                    record.mission_uid,
                    record.updated_at_ms as i64,
                    record.deleted_at_ms.map(|value| value as i64),
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_checklist_tx(
        &self,
        transaction: &Transaction<'_>,
        checklist: &ChecklistRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(checklist)?;
        transaction
            .execute(
                "INSERT INTO checklists (uid, mission_uid, template_uid, checklist_status, updated_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(uid) DO UPDATE SET
                    mission_uid = excluded.mission_uid,
                    template_uid = excluded.template_uid,
                    checklist_status = excluded.checklist_status,
                    updated_at_ms = excluded.updated_at_ms,
                    json = excluded.json",
                params![
                    checklist.uid,
                    checklist.mission_uid,
                    checklist.template_uid,
                    checklist.checklist_status.as_str(),
                    now_ms() as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn upsert_checklist_template(
        &self,
        template: &ChecklistTemplateRecord,
    ) -> Result<(), NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_checklist_template_tx(&transaction, template)?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_checklist_template_tx(
        &self,
        transaction: &Transaction<'_>,
        template: &ChecklistTemplateRecord,
    ) -> Result<(), NodeError> {
        let mut normalized = template.clone();
        normalize_checklist_template(&mut normalized);
        let json = serialize_json(&normalized)?;
        transaction
            .execute(
                "INSERT INTO checklist_templates (uid, updated_at_ms, json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(uid) DO UPDATE SET
                    updated_at_ms = excluded.updated_at_ms,
                    json = excluded.json",
                params![normalized.uid, now_ms() as i64, json],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn seed_default_checklist_templates(&self) -> Result<(), NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        for template in default_checklist_templates() {
            self.write_checklist_template_tx(&transaction, &template)?;
        }
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn load_checklist_tx(
        &self,
        transaction: &Transaction<'_>,
        checklist_uid: &str,
    ) -> Result<ChecklistRecord, NodeError> {
        let raw: Option<String> = transaction
            .query_row(
                "SELECT json FROM checklists WHERE uid = ?1",
                params![checklist_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        match raw {
            Some(value) => deserialize_json(&value),
            None => Err(NodeError::InvalidConfig {}),
        }
    }

    fn write_message_tx(
        &self,
        transaction: &Transaction<'_>,
        message: &MessageRecord,
    ) -> Result<(), NodeError> {
        let canonical_message = canonicalize_chat_message(message);
        let json = serialize_json(&canonical_message)?;
        transaction
            .execute(
                "INSERT INTO messages (message_id_hex, conversation_id, updated_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(message_id_hex) DO UPDATE SET
                    conversation_id = excluded.conversation_id,
                    updated_at_ms = excluded.updated_at_ms,
                    json = excluded.json",
                params![
                    canonical_message.message_id_hex,
                    canonical_message.conversation_id,
                    canonical_message.updated_at_ms as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn repair_message_conversations(
        &self,
        connection: &Connection,
        resolver: &ConversationPeerResolver,
    ) -> Result<(), NodeError> {
        let mut statement = connection
            .prepare("SELECT message_id_hex, json FROM messages")
            .map_err(|_| NodeError::IoError {})?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| NodeError::IoError {})?;
        let mut repairs = Vec::new();
        for row in rows {
            let (message_id_hex, raw) = row.map_err(|_| NodeError::IoError {})?;
            let message: MessageRecord = deserialize_json(&raw)?;
            let canonical_message = canonicalize_chat_message_with_resolver(&message, resolver);
            if canonical_message.conversation_id != message.conversation_id {
                repairs.push((message_id_hex, canonical_message));
            }
        }
        drop(statement);

        if repairs.is_empty() {
            return Ok(());
        }

        for (message_id_hex, message) in repairs {
            let json = serialize_json(&message)?;
            connection
                .execute(
                    "UPDATE messages
                     SET conversation_id = ?1, updated_at_ms = ?2, json = ?3
                     WHERE message_id_hex = ?4",
                    params![
                        message.conversation_id,
                        message.updated_at_ms as i64,
                        json,
                        message_id_hex,
                    ],
                )
                .map_err(|_| NodeError::IoError {})?;
        }
        Ok(())
    }

    fn write_telemetry_tx(
        &self,
        transaction: &Transaction<'_>,
        position: &TelemetryPositionRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(position)?;
        transaction
            .execute(
                "INSERT INTO telemetry_positions (callsign_key, updated_at_ms, json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(callsign_key) DO UPDATE SET
                    updated_at_ms = excluded.updated_at_ms,
                    json = excluded.json",
                params![
                    position.callsign.to_ascii_lowercase(),
                    position.updated_at_ms as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn write_sos_alert_tx(
        &self,
        transaction: &Transaction<'_>,
        alert: &SosAlertRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(alert)?;
        transaction
            .execute(
                "INSERT INTO sos_alerts (incident_id, source_hex, active, updated_at_ms, json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(incident_id, source_hex) DO UPDATE SET
                    active = excluded.active,
                    updated_at_ms = excluded.updated_at_ms,
                    json = excluded.json",
                params![
                    alert.incident_id,
                    alert.source_hex,
                    if alert.active { 1_i64 } else { 0_i64 },
                    alert.updated_at_ms as i64,
                    json
                ],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }

    fn delete_sos_records_for_conversations_tx(
        &self,
        transaction: &Transaction<'_>,
        conversation_ids: &[String],
    ) -> Result<bool, NodeError> {
        if conversation_ids.is_empty() {
            return Ok(false);
        }

        let mut statement = transaction
            .prepare("SELECT json FROM sos_alerts")
            .map_err(|_| NodeError::IoError {})?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| NodeError::IoError {})?;
        let mut records_to_delete = Vec::<(String, String)>::new();
        for row in rows {
            let alert: SosAlertRecord = deserialize_json(&row.map_err(|_| NodeError::IoError {})?)?;
            let alert_conversation = normalize_message_peer_key(alert.conversation_id.as_str());
            if conversation_ids.iter().any(|id| id == &alert_conversation) {
                records_to_delete.push((alert.incident_id, alert.source_hex));
            }
        }
        drop(statement);

        records_to_delete.sort();
        records_to_delete.dedup();
        for (incident_id, source_hex) in &records_to_delete {
            transaction
                .execute(
                    "DELETE FROM sos_locations WHERE incident_id = ?1 AND source_hex = ?2",
                    params![incident_id, source_hex],
                )
                .map_err(|_| NodeError::IoError {})?;
            transaction
                .execute(
                    "DELETE FROM sos_alerts WHERE incident_id = ?1 AND source_hex = ?2",
                    params![incident_id, source_hex],
                )
                .map_err(|_| NodeError::IoError {})?;
        }

        Ok(!records_to_delete.is_empty())
    }

    fn bump_projection_revision_tx(
        &self,
        transaction: &Transaction<'_>,
        scope: ProjectionScope,
        key: Option<String>,
        reason: Option<String>,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let scope_name = projection_scope_name(scope);
        let current: Option<i64> = transaction
            .query_row(
                "SELECT revision FROM projection_versions WHERE scope = ?1",
                params![scope_name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        let updated_at_ms = now_ms();
        let revision = current.unwrap_or(0) + 1;
        transaction
            .execute(
                "INSERT INTO projection_versions (scope, revision, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(scope) DO UPDATE SET revision = excluded.revision, updated_at_ms = excluded.updated_at_ms",
                params![scope_name, revision, updated_at_ms as i64],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(ProjectionInvalidation {
            scope,
            key,
            revision: revision as u64,
            updated_at_ms,
            reason,
        })
    }

    fn bump_checklist_projection_revisions_tx(
        &self,
        transaction: &Transaction<'_>,
        checklist_uid: &str,
        reason: &str,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let list = self.bump_projection_revision_tx(
            transaction,
            ProjectionScope::Checklists {},
            None,
            Some(reason.to_string()),
        )?;
        let detail = self.bump_projection_revision_tx(
            transaction,
            ProjectionScope::ChecklistDetail {},
            Some(checklist_uid.to_string()),
            Some(reason.to_string()),
        )?;
        Ok(vec![list, detail])
    }
}
