impl AppStateStore {
    pub(crate) fn telemetry_publication(
        &self,
    ) -> Result<crate::telemetry_publication::TelemetryPublication, NodeError> {
        let value: Option<String> = self
            .connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key = 'telemetry_publication'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        value
            .map(|json| deserialize_json(&json))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    pub(crate) fn update_telemetry_publication(
        &self,
        update: impl FnOnce(&mut crate::telemetry_publication::TelemetryPublication),
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let old: Option<String> = transaction
            .query_row(
                "SELECT value FROM metadata WHERE key = 'telemetry_publication'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let mut publication = old
            .as_deref()
            .map(deserialize_json)
            .transpose()?
            .unwrap_or_default();
        update(&mut publication);
        let json = serialize_json(&publication)?;
        if old.as_deref() == Some(&json) {
            return Ok(None);
        }
        transaction.execute(
            "INSERT INTO metadata (key, value) VALUES ('telemetry_publication', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![json],
        ).map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Telemetry {},
            None,
            Some("telemetry-publication".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(Some(invalidation))
    }

    pub(crate) fn record_telemetry_delivery(
        &self,
        update: &crate::types::LxmfDeliveryUpdate,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        use crate::telemetry_publication::TelemetryPublicationState as State;
        if update.command_type.as_deref() != Some("mission.registry.telemetry.upsert") {
            return Ok(None);
        }
        self.update_telemetry_publication(|publication| {
            let Some(recipient) = publication.recipients.iter_mut().find(|recipient| {
                Some(recipient.command_id.as_str()) == update.command_id.as_deref()
            }) else {
                return;
            };
            // Receipts arriving after an application result must not demote that result.
            if matches!(recipient.state, State::Acknowledged | State::Rejected) {
                return;
            }
            recipient.state = match update.application_ack_state {
                crate::types::ApplicationAckState::Accepted {}
                | crate::types::ApplicationAckState::Completed {} => State::Acknowledged,
                crate::types::ApplicationAckState::Rejected {} => State::Rejected,
                crate::types::ApplicationAckState::Failed {}
                    if matches!(
                        update.status,
                        crate::types::LxmfDeliveryStatus::Acknowledged {}
                    ) =>
                {
                    State::Failed
                }
                _ => match update.status {
                    crate::types::LxmfDeliveryStatus::StoredLocally {} => State::StoredLocally,
                    crate::types::LxmfDeliveryStatus::Uploading {} => State::QueuedForDelivery,
                    crate::types::LxmfDeliveryStatus::Expired {}
                    | crate::types::LxmfDeliveryStatus::Evicted {} => State::Failed,
                    crate::types::LxmfDeliveryStatus::TimedOut {} => State::TimedOut,
                    crate::types::LxmfDeliveryStatus::Failed {} => State::Failed,
                    crate::types::LxmfDeliveryStatus::Delivered {} => State::TransportDelivered,
                    _ => State::Transmitted,
                },
            };
            recipient.message_id_hex = Some(update.message_id_hex.clone());
            recipient.representation = Some(
                match update.representation {
                    crate::types::LxmfDeliveryRepresentation::Packet {} => "Packet",
                    crate::types::LxmfDeliveryRepresentation::Resource {} => "Resource",
                }
                .to_string(),
            );
            recipient.detail = update.detail.clone();
            recipient.updated_at_ms = update.updated_at_ms;
        })
    }
}

impl AppStateStore {
    pub(crate) fn record_local_telemetry_custody(
        &self,
        command_id: &str,
        entry: &crate::types::LocalPropagationRecord,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        use crate::telemetry_publication::TelemetryPublicationState as State;
        use crate::types::LocalPropagationState as Custody;
        self.update_telemetry_publication(|publication| {
            let Some(row) = publication
                .recipients
                .iter_mut()
                .find(|row| row.command_id == command_id)
            else {
                return;
            };
            if matches!(row.state, State::Acknowledged | State::Rejected) {
                return;
            }
            let (state, detail) = match entry.state {
                Custody::StoredLocally => (State::StoredLocally, "Stored locally"),
                Custody::Uploading => (State::QueuedForDelivery, "Uploading"),
                Custody::UploadedUnverified => (State::Transmitted, "Uploaded (unverified)"),
                Custody::RelayHolds => (State::Transmitted, "Relay holds message"),
                Custody::Delivered => (State::TransportDelivered, "Delivered"),
                Custody::Acknowledged => (State::Acknowledged, "Acknowledged"),
                Custody::Expired => (State::Failed, "Expired"),
                Custody::Evicted => (State::Failed, "Evicted from local storage"),
            };
            let detail = if entry.may_exist_on_relay {
                format!("{detail}; a copy may remain on the relay after local expiry")
            } else {
                detail.to_owned()
            };
            if row.state == state
                && row.custody_message_id_hex.as_deref() == Some(entry.message_id_hex.as_str())
                && row.detail.as_deref() == Some(detail.as_str())
            {
                return;
            }
            row.state = state;
            row.custody_message_id_hex = Some(entry.message_id_hex.clone());
            row.message_id_hex = Some(entry.message_id_hex.clone());
            row.detail = Some(detail);
            row.updated_at_ms = now_ms();
        })
    }
}
