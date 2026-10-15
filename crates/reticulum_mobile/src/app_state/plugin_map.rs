// Plugin access to shared map markers (API 1.2). Markers stay ordinary
// shared markers; REM only records which plugin created which marker so a
// plugin can read and edit its own markers and never the whole map.

const LOCAL_OBSERVER_FIX_KEY: &str = "local_observer_fix";

impl AppStateStore {
    pub(crate) fn record_plugin_map_marker(
        &self,
        plugin_id: &str,
        object_destination_hash: &str,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT OR IGNORE INTO plugin_map_markers
                 (plugin_id, object_destination_hash, created_at_ms) VALUES (?1, ?2, ?3)",
                params![
                    plugin_id,
                    object_destination_hash,
                    crate::numeric::u64_to_i64_saturating(now_ms())
                ],
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(())
    }

    pub(crate) fn plugin_owns_map_marker(
        &self,
        plugin_id: &str,
        object_destination_hash: &str,
    ) -> Result<bool, NodeError> {
        self.connect()?
            .prepare(
                "SELECT 1 FROM plugin_map_markers WHERE plugin_id = ?1 AND object_destination_hash = ?2",
            )
            .and_then(|mut statement| statement.exists(params![plugin_id, object_destination_hash.trim()]))
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))
    }

    /// Markers this plugin created that still exist, newest first, at most `limit`.
    pub(crate) fn plugin_map_markers(
        &self,
        plugin_id: &str,
        limit: usize,
    ) -> Result<Vec<r3akt_map_core::MarkerRecord>, NodeError> {
        let connection = self.connect()?;
        let ids = connection
            .prepare(
                "SELECT object_destination_hash FROM plugin_map_markers
                 WHERE plugin_id = ?1 ORDER BY created_at_ms DESC",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(params![plugin_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut markers = Vec::new();
        for id in ids {
            if markers.len() >= limit {
                break;
            }
            if let Some(marker) = r3akt_map_core::sqlite::get(&connection, &id)
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?
            {
                markers.push(marker);
            }
        }
        Ok(markers)
    }

    /// Stores this device's own fix. Only the local location path writes it,
    /// so plugins never receive a peer's telemetry as the observer position.
    pub(crate) fn record_local_observer_fix(
        &self,
        position: &TelemetryPositionRecord,
    ) -> Result<(), NodeError> {
        let json = serialize_json(position)?;
        self.connect()?
            .execute(
                "INSERT INTO metadata (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![LOCAL_OBSERVER_FIX_KEY, json],
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(())
    }

    pub(crate) fn local_observer_fix(&self) -> Result<Option<TelemetryPositionRecord>, NodeError> {
        let value: Option<String> = self
            .connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![LOCAL_OBSERVER_FIX_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        value
            .map(|json| {
                serde_json::from_str(&json).map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
                })
            })
            .transpose()
    }
}
