impl AppStateStore {
    pub fn get_hub_directory(
        &self,
        hub_identity_hash: &str,
    ) -> Result<Option<HubDirectorySnapshot>, NodeError> {
        let hub_identity_hash = normalize_hub_identity(hub_identity_hash)?;
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM hub_directories WHERE hub_identity_hash = ?1",
                params![hub_identity_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn set_hub_directory(
        &self,
        hub_identity_hash: &str,
        snapshot: &HubDirectorySnapshot,
    ) -> Result<(), NodeError> {
        let hub_identity_hash = normalize_hub_identity(hub_identity_hash)?;
        let json = serialize_json(snapshot)?;
        let connection = self.connect()?;
        connection
            .execute(
                "INSERT INTO hub_directories (hub_identity_hash, json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(hub_identity_hash) DO UPDATE SET
                    json = excluded.json,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    hub_identity_hash,
                    json,
                    crate::numeric::u64_to_i64_saturating(now_ms())
                ],
            )
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        Ok(())
    }
}

fn normalize_hub_identity(value: &str) -> Result<String, NodeError> {
    crate::delivery_policy::normalize_hex_32(value).ok_or(NodeError::InvalidConfig {})
}
