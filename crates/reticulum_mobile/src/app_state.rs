use std::path::PathBuf;

use fs_err as fs;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use crate::runtime::now_ms;
use crate::types::{
    AppSettingsRecord, ConversationRecord, EamProjectionRecord, EamTeamSummaryRecord,
    EventProjectionRecord, LegacyImportPayload, MessageDirection, MessageRecord, NodeError,
    ProjectionInvalidation, ProjectionScope, SavedPeerRecord, SosAlertRecord, SosAudioRecord,
    SosLocationRecord, SosSettingsRecord, SosStatusRecord, TelemetryPositionRecord,
};

const DEFAULT_STORAGE_DIR: &str = "reticulum-mobile";
const DB_FILE_NAME: &str = "app_state.db";

#[derive(Debug, Clone)]
pub struct AppStateStore {
    db_path: PathBuf,
}

impl AppStateStore {
    pub fn new(storage_dir: Option<&str>) -> Result<Self, NodeError> {
        let base_dir = storage_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STORAGE_DIR));
        fs::create_dir_all(&base_dir).map_err(|_| NodeError::IoError {})?;
        let store = Self {
            db_path: base_dir.join(DB_FILE_NAME),
        };
        store.initialize()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, NodeError> {
        let connection = Connection::open(&self.db_path).map_err(|_| NodeError::IoError {})?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| NodeError::IoError {})?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|_| NodeError::IoError {})?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| NodeError::IoError {})?;
        Ok(connection)
    }

    fn initialize(&self) -> Result<(), NodeError> {
        let connection = self.connect()?;
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS app_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS saved_peers (
                    destination_hex TEXT PRIMARY KEY,
                    json TEXT NOT NULL,
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
                CREATE TABLE IF NOT EXISTS messages (
                    message_id_hex TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
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
            .map_err(|_| NodeError::IoError {})?;
        self.repair_message_conversations(&connection)?;
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
            .map_err(|_| NodeError::IoError {})?;
        Ok(value.as_deref() == Some("1"))
    }

    pub fn import_legacy_state(
        &self,
        payload: &LegacyImportPayload,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
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
                .map_err(|_| NodeError::IoError {})?;
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
            .map_err(|_| NodeError::IoError {})?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn set_app_settings(
        &self,
        settings: &AppSettingsRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_app_settings_tx(&transaction, settings)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::AppSettings {},
            None,
            Some("settings-updated".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

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
            .map_err(|_| NodeError::IoError {})?;
        transaction
            .execute("DELETE FROM saved_peers", [])
            .map_err(|_| NodeError::IoError {})?;
        for peer in peers {
            self.write_saved_peer_tx(&transaction, peer)?;
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::SavedPeers {},
            None,
            Some("saved-peers-updated".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

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
            .map_err(|_| NodeError::IoError {})?;
        self.write_eam_tx(&transaction, record)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Eams {},
            Some(record.callsign.to_ascii_lowercase()),
            Some("eam-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
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
            .map_err(|_| NodeError::IoError {})?;
        if let Some(raw) = transaction
            .query_row(
                "SELECT json FROM eams WHERE callsign_key = ?1",
                params![callsign.trim().to_ascii_lowercase()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?
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
        transaction.commit().map_err(|_| NodeError::IoError {})?;
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
            total: records.len() as u32,
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
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_event_tx(&transaction, record)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Events {},
            Some(record.uid.clone()),
            Some("event-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
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
            .map_err(|_| NodeError::IoError {})?;
        if let Some(raw) = transaction
            .query_row(
                "SELECT json FROM events WHERE uid = ?1",
                params![uid],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?
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
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidation)
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<&str>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let connection = self.connect()?;
        let mut records = Vec::new();
        if let Some(conversation_id) = conversation_id {
            let mut statement = connection
                .prepare(
                    "SELECT json FROM messages WHERE conversation_id = ?1
                     ORDER BY updated_at_ms ASC, message_id_hex ASC",
                )
                .map_err(|_| NodeError::IoError {})?;
            let rows = statement
                .query_map(params![conversation_id], |row| row.get::<_, String>(0))
                .map_err(|_| NodeError::IoError {})?;
            for row in rows {
                let raw: String = row.map_err(|_| NodeError::IoError {})?;
                records.push(deserialize_json(&raw)?);
            }
        } else {
            let mut statement = connection
                .prepare("SELECT json FROM messages ORDER BY updated_at_ms ASC, message_id_hex ASC")
                .map_err(|_| NodeError::IoError {})?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|_| NodeError::IoError {})?;
            for row in rows {
                let raw: String = row.map_err(|_| NodeError::IoError {})?;
                records.push(deserialize_json(&raw)?);
            }
        }
        Ok(records)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, NodeError> {
        let messages = self.list_messages(None)?;
        let labels = self
            .get_saved_peers()?
            .into_iter()
            .map(|peer| {
                (
                    normalize_message_peer_key(peer.destination_hex.as_str()),
                    peer.label,
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut conversations = std::collections::HashMap::<String, ConversationRecord>::new();

        for message in messages {
            let updated_at_ms = message
                .received_at_ms
                .or(message.sent_at_ms)
                .unwrap_or(message.updated_at_ms);
            let peer_destination_hex = message.conversation_id.clone();
            let preview = truncate_preview(message.body_utf8.as_str());
            let peer_display_name = labels.get(&peer_destination_hex).cloned().flatten();
            let next = ConversationRecord {
                conversation_id: message.conversation_id.clone(),
                peer_destination_hex,
                peer_display_name,
                last_message_preview: preview,
                last_message_at_ms: updated_at_ms,
                unread_count: 0,
                last_message_state: Some(message.state),
            };

            match conversations.get(&message.conversation_id) {
                Some(existing) if existing.last_message_at_ms > next.last_message_at_ms => {}
                _ => {
                    conversations.insert(message.conversation_id.clone(), next);
                }
            }
        }

        let mut records = conversations.into_values().collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .last_message_at_ms
                .cmp(&left.last_message_at_ms)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        Ok(records)
    }

    pub fn upsert_message(
        &self,
        message: &MessageRecord,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let canonical_message = canonicalize_chat_message(message);
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_message_tx(&transaction, &canonical_message)?;
        let messages = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Messages {},
            Some(canonical_message.conversation_id.clone()),
            Some("message-upserted".to_string()),
        )?;
        let conversations = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Conversations {},
            None,
            Some("message-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(vec![messages, conversations])
    }

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
        query_json_records(
            &self.connect()?,
            "SELECT json FROM sos_alerts ORDER BY updated_at_ms DESC, incident_id ASC",
        )
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
                "INSERT INTO saved_peers (destination_hex, json, updated_at_ms) VALUES (?1, ?2, ?3)",
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

    fn repair_message_conversations(&self, connection: &Connection) -> Result<(), NodeError> {
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
            let canonical_message = canonicalize_chat_message(&message);
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
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, NodeError> {
    serde_json::to_string(value).map_err(|_| NodeError::InternalError {})
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, NodeError> {
    serde_json::from_str(value).map_err(|_| NodeError::InternalError {})
}

pub(crate) fn canonicalize_chat_message(message: &MessageRecord) -> MessageRecord {
    let peer_key = canonical_message_peer_key(message);
    MessageRecord {
        conversation_id: peer_key,
        ..message.clone()
    }
}

fn canonical_message_peer_key(message: &MessageRecord) -> String {
    let preferred = match message.direction {
        MessageDirection::Inbound {} => message
            .source_hex
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(message.destination_hex.as_str()),
        MessageDirection::Outbound {} => message.destination_hex.as_str(),
    };
    let normalized = normalize_message_peer_key(preferred);
    if !normalized.is_empty() {
        return normalized;
    }
    normalize_message_peer_key(message.conversation_id.as_str())
}

fn normalize_message_peer_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn truncate_preview(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(120).collect())
}

fn query_json_records<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<T>, NodeError> {
    let mut statement = connection.prepare(sql).map_err(|_| NodeError::IoError {})?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| NodeError::IoError {})?;
    let mut records = Vec::new();
    for row in rows {
        records.push(deserialize_json(&row.map_err(|_| NodeError::IoError {})?)?);
    }
    Ok(records)
}

fn projection_scope_name(scope: ProjectionScope) -> &'static str {
    match scope {
        ProjectionScope::AppSettings {} => "AppSettings",
        ProjectionScope::SavedPeers {} => "SavedPeers",
        ProjectionScope::OperationalSummary {} => "OperationalSummary",
        ProjectionScope::Peers {} => "Peers",
        ProjectionScope::SyncStatus {} => "SyncStatus",
        ProjectionScope::HubRegistration {} => "HubRegistration",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
        ProjectionScope::Sos {} => "Sos",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::types::{MessageDirection, MessageMethod, MessageState};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_storage_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "reticulum-mobile-app-state-{name}-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        path
    }

    fn message(
        id: &str,
        conversation_id: &str,
        direction: MessageDirection,
        destination_hex: &str,
        source_hex: Option<&str>,
        updated_at_ms: u64,
    ) -> MessageRecord {
        MessageRecord {
            message_id_hex: id.to_string(),
            conversation_id: conversation_id.to_string(),
            direction,
            destination_hex: destination_hex.to_string(),
            source_hex: source_hex.map(str::to_string),
            title: Some("chat".to_string()),
            body_utf8: format!("body {id}"),
            method: MessageMethod::Direct {},
            state: MessageState::Received {},
            detail: None,
            sent_at_ms: Some(updated_at_ms),
            received_at_ms: None,
            updated_at_ms,
        }
    }

    #[test]
    fn messages_for_same_peer_are_repaired_into_one_canonical_thread() {
        let storage_dir = test_storage_dir("canonical-thread");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

        let outbound = message(
            "outbound-1",
            "legacy-outbound-thread",
            MessageDirection::Outbound {},
            "ABCDEF",
            Some("LOCAL"),
            10,
        );
        let inbound = message(
            "inbound-1",
            "legacy-inbound-thread",
            MessageDirection::Inbound {},
            "LOCAL",
            Some("ABCDEF"),
            20,
        );

        store.upsert_message(&outbound).expect("persist outbound");
        store.upsert_message(&inbound).expect("persist inbound");

        let conversations = store.list_conversations().expect("list conversations");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, "abcdef");
        assert_eq!(conversations[0].peer_destination_hex, "abcdef");

        let messages = store
            .list_messages(Some("abcdef"))
            .expect("list canonical messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].message_id_hex, "outbound-1");
        assert_eq!(messages[0].conversation_id, "abcdef");
        assert_eq!(messages[1].message_id_hex, "inbound-1");
        assert_eq!(messages[1].conversation_id, "abcdef");
    }

    #[test]
    fn startup_history_lists_persisted_messages_without_runtime() {
        let storage_dir = test_storage_dir("startup-history");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let outbound = message(
            "persisted-1",
            "old-thread",
            MessageDirection::Outbound {},
            "PEER-1",
            Some("LOCAL"),
            30,
        );
        store.upsert_message(&outbound).expect("persist message");

        let restarted =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("reopen store");
        let conversations = restarted
            .list_conversations()
            .expect("list conversations before runtime");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, "peer-1");

        let messages = restarted
            .list_messages(Some("peer-1"))
            .expect("list messages before runtime");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id_hex, "persisted-1");
        assert_eq!(messages[0].conversation_id, "peer-1");
    }
}
