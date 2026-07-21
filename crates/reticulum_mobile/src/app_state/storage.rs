impl AppStateStore {
    pub fn new(storage_dir: Option<&str>) -> Result<Self, NodeError> {
        let base_dir = storage_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STORAGE_DIR));
        fs::create_dir_all(&base_dir).map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let store = Self {
            db_path: base_dir.join(DB_FILE_NAME),
        };
        store.initialize()?;
        store.seed_default_checklist_templates()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, NodeError> {
        let connection = Connection::open(&self.db_path).map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        connection
            .busy_timeout(SQLITE_BUSY_TIMEOUT)
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(connection)
    }

    fn initialize(&self) -> Result<(), NodeError> {
        let connection = self.connect()?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS app_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS hub_directories (
                    hub_identity_hash TEXT PRIMARY KEY,
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS saved_peers (
                    destination_hex TEXT PRIMARY KEY,
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS ignored_peers (
                    destination_hex TEXT PRIMARY KEY,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS eams (
                    callsign_key TEXT PRIMARY KEY,
                    team_uid TEXT,
                    overall_status TEXT,
                    updated_at_ms INTEGER NOT NULL,
                    deleted_at_ms INTEGER,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS events (
                    uid TEXT PRIMARY KEY,
                    mission_uid TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    deleted_at_ms INTEGER,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS checklists (
                    uid TEXT PRIMARY KEY,
                    mission_uid TEXT,
                    template_uid TEXT,
                    checklist_status TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS checklist_templates (
                    uid TEXT PRIMARY KEY,
                    updated_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS messages (
                    message_id_hex TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_messages_conversation_updated
                    ON messages (conversation_id, updated_at_ms, message_id_hex);
                CREATE TABLE IF NOT EXISTS announces (
                    destination_hex TEXT PRIMARY KEY,
                    identity_hex TEXT NOT NULL,
                    destination_kind TEXT NOT NULL,
                    announce_class TEXT NOT NULL,
                    app_data TEXT NOT NULL,
                    display_name TEXT,
                    hops INTEGER NOT NULL,
                    interface_hex TEXT NOT NULL,
                    received_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS telemetry_positions (
                    callsign_key TEXT PRIMARY KEY,
                    updated_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS sos_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS sos_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS sos_alerts (
                    incident_id TEXT NOT NULL,
                    source_hex TEXT NOT NULL,
                    active INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL,
                    PRIMARY KEY (incident_id, source_hex)
                );
                CREATE TABLE IF NOT EXISTS sos_locations (
                    incident_id TEXT NOT NULL,
                    source_hex TEXT NOT NULL,
                    recorded_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL,
                    PRIMARY KEY (incident_id, source_hex, recorded_at_ms)
                );
                CREATE TABLE IF NOT EXISTS sos_audio (
                    audio_id TEXT PRIMARY KEY,
                    incident_id TEXT NOT NULL,
                    source_hex TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS plugin_publishers (
                    fingerprint TEXT PRIMARY KEY,
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS plugins (
                    plugin_id TEXT PRIMARY KEY,
                    package_name TEXT NOT NULL,
                    publisher_fingerprint TEXT NOT NULL,
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS plugin_sensors (
                    plugin_id TEXT NOT NULL,
                    device_id TEXT NOT NULL,
                    sensor_type TEXT NOT NULL,
                    sample_at_ms INTEGER NOT NULL,
                    json TEXT NOT NULL,
                    PRIMARY KEY (plugin_id, device_id, sensor_type)
                );
                CREATE TABLE IF NOT EXISTS projection_versions (
                    scope TEXT PRIMARY KEY,
                    revision INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                ",
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        self.repair_message_conversations(&connection, &ConversationPeerResolver::default())?;
        Ok(())
    }

    pub fn legacy_import_completed(&self) -> Result<bool, NodeError> {
        let connection = self.connect()?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'legacy_import_completed'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(value.as_deref() == Some("1"))
    }

    pub fn import_legacy_state(
        &self,
        payload: &LegacyImportPayload,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut invalidations = Vec::new();

        if let Some(settings) = payload.settings.as_ref() {
            self.write_app_settings_tx(&transaction, settings)?;
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::AppSettings {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        if !payload.saved_peers.is_empty() {
            transaction
                .execute("DELETE FROM saved_peers", [])
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
            for peer in &payload.saved_peers {
                self.write_saved_peer_tx(&transaction, peer)?;
            }
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::SavedPeers {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        if !payload.eams.is_empty() {
            for eam in &payload.eams {
                self.write_eam_tx(&transaction, eam)?;
            }
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Eams {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        if !payload.events.is_empty() {
            for event in &payload.events {
                self.write_event_tx(&transaction, event)?;
            }
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Events {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        if !payload.messages.is_empty() {
            for message in &payload.messages {
                self.write_message_tx(&transaction, message)?;
            }
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Messages {},
                None,
                Some("legacy-import".to_string()),
            )?);
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Conversations {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        if !payload.telemetry_positions.is_empty() {
            for position in &payload.telemetry_positions {
                self.write_telemetry_tx(&transaction, position)?;
            }
            invalidations.push(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Telemetry {},
                None,
                Some("legacy-import".to_string()),
            )?);
        }

        transaction
            .execute(
                "INSERT INTO metadata (key, value) VALUES ('legacy_import_completed', '1')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }

}
