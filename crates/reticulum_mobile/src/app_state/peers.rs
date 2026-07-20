impl AppStateStore {
    pub fn get_saved_peers(&self) -> Result<Vec<SavedPeerRecord>, NodeError> {
        query_json_records(
            &self.connect()?,
            "SELECT json FROM saved_peers ORDER BY updated_at_ms DESC",
        )
    }

    pub fn set_saved_peers(
        &self,
        peers: &[SavedPeerRecord],
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        transaction
            .execute("DELETE FROM saved_peers", [])
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        for peer in peers {
            self.write_saved_peer_tx(&transaction, peer)?;
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::SavedPeers {},
            None,
            Some("saved-peers-updated".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

    pub fn upsert_saved_peer(
        &self,
        peer: &SavedPeerRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        self.write_saved_peer_tx(&transaction, peer)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::SavedPeers {},
            None,
            Some("saved-peer-upserted".to_string()),
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidation)
    }

    pub(crate) fn get_ignored_peer_destinations(&self) -> Result<Vec<String>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT destination_hex FROM ignored_peers ORDER BY updated_at_ms DESC")
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut destinations = Vec::new();
        for row in rows {
            destinations.push(row.map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?);
        }
        Ok(destinations)
    }

    pub(crate) fn add_ignored_peer_destinations(
        &self,
        destinations: &[String],
    ) -> Result<(), NodeError> {
        if destinations.is_empty() {
            return Ok(());
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let updated_at_ms = crate::numeric::u64_to_i64_saturating(now_ms());
        for destination in destinations {
            let normalized = destination.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                continue;
            }
            transaction
                .execute(
                    "INSERT INTO ignored_peers (destination_hex, updated_at_ms)
                     VALUES (?1, ?2)
                     ON CONFLICT(destination_hex) DO UPDATE SET
                        updated_at_ms = excluded.updated_at_ms",
                    params![normalized, updated_at_ms],
                )
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        }
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(())
    }

    pub(crate) fn remove_ignored_peer_destinations(
        &self,
        destinations: &[String],
    ) -> Result<(), NodeError> {
        if destinations.is_empty() {
            return Ok(());
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        for destination in destinations {
            let normalized = destination.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                continue;
            }
            transaction
                .execute(
                    "DELETE FROM ignored_peers WHERE destination_hex = ?1",
                    params![normalized],
                )
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        }
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(())
    }

    pub fn upsert_announce(&self, record: &AnnounceRecord) -> Result<(), NodeError> {
        let destination_hex = record.destination_hex.trim().to_ascii_lowercase();
        if destination_hex.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let connection = self.connect()?;
        let existing: Option<(i64, Option<String>)> = connection
            .query_row(
                "SELECT received_at_ms, display_name FROM announces WHERE destination_hex = ?1",
                params![destination_hex],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        if existing
            .as_ref()
            .is_some_and(|(received_at_ms, _)| {
                *received_at_ms
                    > crate::numeric::u64_to_i64_saturating(record.received_at_ms)
            })
        {
            return Ok(());
        }

        let mut normalized = record.clone();
        normalized.destination_hex = destination_hex;
        normalized.identity_hex = record.identity_hex.trim().to_ascii_lowercase();
        normalized.destination_kind = record.destination_kind.trim().to_ascii_lowercase();
        normalized.interface_hex = record.interface_hex.trim().to_ascii_lowercase();
        normalized.display_name = normalize_optional_string(record.display_name.as_deref())
            .or_else(|| {
                existing.as_ref().and_then(|(_, display_name)| {
                    normalize_optional_string(display_name.as_deref())
                })
            });
        let announce_class = announce_class_name(normalized.announce_class);
        let json = serialize_json(&normalized)?;
        connection
            .execute(
                "INSERT INTO announces (
                    destination_hex,
                    identity_hex,
                    destination_kind,
                    announce_class,
                    app_data,
                    display_name,
                    hops,
                    interface_hex,
                    received_at_ms,
                    json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(destination_hex) DO UPDATE SET
                    identity_hex = excluded.identity_hex,
                    destination_kind = excluded.destination_kind,
                    announce_class = excluded.announce_class,
                    app_data = excluded.app_data,
                    display_name = excluded.display_name,
                    hops = excluded.hops,
                    interface_hex = excluded.interface_hex,
                    received_at_ms = excluded.received_at_ms,
                    json = excluded.json",
                params![
                    normalized.destination_hex,
                    normalized.identity_hex,
                    normalized.destination_kind,
                    announce_class,
                    normalized.app_data,
                    normalized.display_name,
                    i64::from(normalized.hops),
                    normalized.interface_hex,
                    crate::numeric::u64_to_i64_saturating(normalized.received_at_ms),
                    json,
                ],
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(())
    }

    pub fn list_announces(&self) -> Result<Vec<AnnounceRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT
                    destination_hex,
                    identity_hex,
                    destination_kind,
                    announce_class,
                    app_data,
                    display_name,
                    hops,
                    interface_hex,
                    received_at_ms
                 FROM announces
                 ORDER BY received_at_ms DESC, destination_hex ASC",
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let rows = statement
            .query_map([], |row| {
                let announce_class: String = row.get(3)?;
                let hops: i64 = row.get(6)?;
                let received_at_ms: i64 = row.get(8)?;
                Ok(AnnounceRecord {
                    destination_hex: row.get(0)?,
                    identity_hex: row.get(1)?,
                    destination_kind: row.get(2)?,
                    announce_class: announce_class_from_name(announce_class.as_str()),
                    app_data: row.get(4)?,
                    display_name: row.get(5)?,
                    hops: crate::numeric::i64_to_u8_saturating(hops),
                    interface_hex: row.get(7)?,
                    received_at_ms: crate::numeric::i64_to_u64_saturating(received_at_ms),
                })
            })
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?);
        }
        Ok(records)
    }

}
