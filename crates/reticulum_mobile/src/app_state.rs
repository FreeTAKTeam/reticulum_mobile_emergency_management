use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fs_err as fs;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use crate::runtime::now_ms;
use crate::types::{
    AppSettingsRecord, ChecklistCellRecord, ChecklistColumnRecord, ChecklistColumnType,
    ChecklistCreateFromTemplateRequest, ChecklistCreateOnlineRequest, ChecklistMode,
    ChecklistOriginType, ChecklistRecord, ChecklistSyncState, ChecklistTaskCellSetRequest,
    ChecklistTaskRecord, ChecklistTaskRowAddRequest, ChecklistTaskRowDeleteRequest,
    ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus, ChecklistTaskStatusSetRequest,
    ChecklistTemplateImportCsvRequest, ChecklistTemplateRecord, ChecklistUpdateRequest,
    ChecklistUserTaskStatus, ConversationRecord, EamProjectionRecord, EamTeamSummaryRecord,
    EventProjectionRecord, LegacyImportPayload, MessageDirection, MessageRecord, NodeError,
    ProjectionInvalidation, ProjectionScope, SavedPeerRecord, SosAlertRecord, SosAudioRecord,
    SosLocationRecord, SosSettingsRecord, SosStatusRecord, TelemetryPositionRecord,
    DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES,
};

const DEFAULT_STORAGE_DIR: &str = "reticulum-mobile";
const DB_FILE_NAME: &str = "app_state.db";

#[derive(Debug, Clone)]
pub struct AppStateStore {
    db_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConversationPeerResolver {
    by_alias: HashMap<String, ConversationPeerRecord>,
    by_canonical: HashMap<String, ConversationPeerRecord>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConversationPeerRecord {
    pub canonical_id: String,
    pub peer_destination_hex: String,
    pub display_name: Option<String>,
}

impl ConversationPeerResolver {
    pub(crate) fn insert(
        &mut self,
        aliases: impl IntoIterator<Item = String>,
        canonical_id: String,
        peer_destination_hex: String,
        display_name: Option<String>,
    ) {
        let canonical_id = normalize_message_peer_key(canonical_id.as_str());
        let peer_destination_hex = normalize_message_peer_key(peer_destination_hex.as_str());
        if canonical_id.is_empty() || peer_destination_hex.is_empty() {
            return;
        }
        let record = ConversationPeerRecord {
            canonical_id: canonical_id.clone(),
            peer_destination_hex,
            display_name,
        };
        self.by_canonical
            .insert(canonical_id.clone(), record.clone());
        self.by_alias.insert(canonical_id, record.clone());
        for alias in aliases {
            let alias = normalize_message_peer_key(alias.as_str());
            if !alias.is_empty() {
                self.by_alias.insert(alias, record.clone());
            }
        }
    }

    fn resolve(&self, value: &str) -> Option<&ConversationPeerRecord> {
        self.by_alias.get(&normalize_message_peer_key(value))
    }

    fn canonical_for(&self, value: &str) -> String {
        let normalized = normalize_message_peer_key(value);
        self.by_alias
            .get(&normalized)
            .map(|record| record.canonical_id.clone())
            .unwrap_or(normalized)
    }

    fn peer_for_canonical(&self, canonical_id: &str) -> Option<&ConversationPeerRecord> {
        self.by_canonical
            .get(&normalize_message_peer_key(canonical_id))
    }

    fn aliases_for_canonical(&self, canonical_id: &str) -> Vec<String> {
        let canonical_id = self.canonical_for(canonical_id);
        let mut aliases = self
            .by_alias
            .iter()
            .filter_map(|(alias, record)| {
                (record.canonical_id == canonical_id).then_some(alias.clone())
            })
            .collect::<Vec<_>>();
        aliases.push(canonical_id);
        aliases.sort();
        aliases.dedup();
        aliases
    }
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
        store.seed_default_checklist_templates()?;
        Ok(store)
    }

    pub(crate) fn storage_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STORAGE_DIR))
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

    pub fn get_active_checklists(&self) -> Result<Vec<ChecklistRecord>, NodeError> {
        Ok(query_json_records(
            &self.connect()?,
            "SELECT json FROM checklists ORDER BY updated_at_ms DESC, uid ASC",
        )?
        .into_iter()
        .filter_map(|record| sanitize_active_checklist(record))
        .collect())
    }

    pub fn get_checklist(&self, checklist_uid: &str) -> Result<Option<ChecklistRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklists WHERE uid = ?1",
                params![checklist_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value))
            .transpose()
            .map(|record| record.and_then(sanitize_active_checklist))
    }

    pub fn list_checklist_templates(&self) -> Result<Vec<ChecklistTemplateRecord>, NodeError> {
        let mut items = query_json_records(
            &self.connect()?,
            "SELECT json FROM checklist_templates ORDER BY updated_at_ms DESC, uid ASC",
        )?;
        for item in &mut items {
            normalize_checklist_template(item);
        }
        Ok(items)
    }

    pub fn get_checklist_template(
        &self,
        template_uid: &str,
    ) -> Result<Option<ChecklistTemplateRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklist_templates WHERE uid = ?1",
                params![template_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| {
            let mut record: ChecklistTemplateRecord = deserialize_json(&value)?;
            normalize_checklist_template(&mut record);
            Ok(record)
        })
        .transpose()
    }

    pub(crate) fn get_checklist_any(
        &self,
        checklist_uid: &str,
    ) -> Result<Option<ChecklistRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklists WHERE uid = ?1",
                params![checklist_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| NodeError::IoError {})?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn upsert_checklist(
        &self,
        checklist: &ChecklistRecord,
        reason: &str,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut normalized = checklist.clone();
        normalize_checklist(&mut normalized);
        self.write_checklist_tx(&transaction, &normalized)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            normalized.uid.as_str(),
            reason,
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn create_online_checklist(
        &self,
        request: &ChecklistCreateOnlineRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let timestamp = current_timestamp_rfc3339();
        let checklist_uid = request
            .checklist_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("chk-{}", now_ms()));
        let changed_by =
            normalize_optional_string(request.created_by_team_member_rns_identity.as_deref());
        let checklist = ChecklistRecord {
            uid: checklist_uid,
            mission_uid: normalize_optional_string(request.mission_uid.as_deref()),
            template_uid: Some(request.template_uid.trim().to_string()),
            template_version: None,
            template_name: None,
            name: request.name.trim().to_string(),
            description: request.description.trim().to_string(),
            start_time: Some(request.start_time.trim().to_string()),
            mode: ChecklistMode::Online {},
            sync_state: ChecklistSyncState::Synced {},
            origin_type: ChecklistOriginType::RchTemplate {},
            checklist_status: ChecklistTaskStatus::Pending {},
            created_at: Some(timestamp.clone()),
            created_by_team_member_rns_identity: request
                .created_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string(),
            created_by_team_member_display_name: normalize_optional_string(
                request.created_by_team_member_display_name.as_deref(),
            ),
            updated_at: Some(timestamp),
            last_changed_by_team_member_rns_identity: changed_by,
            deleted_at: None,
            uploaded_at: None,
            participant_rns_identities: request
                .created_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| vec![value.to_string()])
                .unwrap_or_default(),
            expected_task_count: Some(0),
            progress_percent: 0.0,
            counts: crate::types::ChecklistStatusCounts {
                pending_count: 0,
                late_count: 0,
                complete_count: 0,
            },
            columns: Vec::new(),
            tasks: Vec::new(),
            feed_publications: Vec::new(),
        };
        self.upsert_checklist(&checklist, "checklist-created")
    }

    pub fn create_checklist_from_template(
        &self,
        request: &ChecklistCreateFromTemplateRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let template = self
            .get_checklist_template(request.template_uid.trim())?
            .ok_or(NodeError::InvalidConfig {})?;
        let timestamp = current_timestamp_rfc3339();
        let checklist_uid = request
            .checklist_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("chk-{}", now_ms()));
        let created_by = request
            .created_by_team_member_rns_identity
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        let mut checklist = ChecklistRecord {
            uid: checklist_uid,
            mission_uid: normalize_optional_string(request.mission_uid.as_deref()),
            template_uid: Some(template.uid.clone()),
            template_version: Some(template.version),
            template_name: Some(template.name.clone()),
            name: request.name.trim().to_string(),
            description: request.description.trim().to_string(),
            start_time: Some(request.start_time.trim().to_string()),
            mode: ChecklistMode::Online {},
            sync_state: ChecklistSyncState::Synced {},
            origin_type: template.origin_type,
            checklist_status: ChecklistTaskStatus::Pending {},
            created_at: Some(timestamp.clone()),
            created_by_team_member_rns_identity: created_by.clone(),
            created_by_team_member_display_name: normalize_optional_string(
                request.created_by_team_member_display_name.as_deref(),
            ),
            updated_at: Some(timestamp),
            last_changed_by_team_member_rns_identity: normalize_optional_string(Some(
                created_by.as_str(),
            )),
            deleted_at: None,
            uploaded_at: None,
            participant_rns_identities: normalize_optional_string(Some(created_by.as_str()))
                .map(|value| vec![value])
                .unwrap_or_default(),
            expected_task_count: Some(
                template
                    .tasks
                    .iter()
                    .filter(|task| task.deleted_at.is_none())
                    .count() as u32,
            ),
            progress_percent: 0.0,
            counts: crate::types::ChecklistStatusCounts {
                pending_count: 0,
                late_count: 0,
                complete_count: 0,
            },
            columns: template.columns.clone(),
            tasks: template.tasks.clone(),
            feed_publications: Vec::new(),
        };
        normalize_checklist(&mut checklist);
        self.upsert_checklist(&checklist, "checklist-created-from-template")
    }

    pub fn import_checklist_template_csv(
        &self,
        request: &ChecklistTemplateImportCsvRequest,
    ) -> Result<ChecklistTemplateRecord, NodeError> {
        let due_step_minutes = self
            .get_app_settings()?
            .map(|settings| settings.checklists.default_task_due_step_minutes.max(1))
            .unwrap_or(DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES);
        let mut template = parse_checklist_template_csv(request, due_step_minutes)?;
        let timestamp = current_timestamp_rfc3339();
        if template.created_at.is_none() {
            template.created_at = Some(timestamp.clone());
        }
        template.updated_at = Some(timestamp);
        self.upsert_checklist_template(&template)?;
        Ok(template)
    }

    pub fn update_checklist(
        &self,
        request: &ChecklistUpdateRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        checklist.updated_at = Some(current_timestamp_rfc3339());
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        if let Some(mission_uid) = request.patch.mission_uid.as_deref() {
            checklist.mission_uid = normalize_optional_string(Some(mission_uid));
        }
        if let Some(template_uid) = request.patch.template_uid.as_deref() {
            checklist.template_uid = normalize_optional_string(Some(template_uid));
        }
        if let Some(name) = request.patch.name.as_deref() {
            checklist.name = name.trim().to_string();
        }
        if let Some(description) = request.patch.description.as_deref() {
            checklist.description = description.trim().to_string();
        }
        if let Some(start_time) = request.patch.start_time.as_deref() {
            checklist.start_time = normalize_optional_string(Some(start_time));
        }
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-updated",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn delete_checklist(
        &self,
        checklist_uid: &str,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        self.delete_checklist_with_actor(checklist_uid, None)
    }

    pub fn delete_checklist_with_actor(
        &self,
        checklist_uid: &str,
        changed_by_team_member_rns_identity: Option<&str>,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, checklist_uid)?;
        let timestamp = current_timestamp_rfc3339();
        checklist.deleted_at = Some(timestamp.clone());
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(&mut checklist, changed_by_team_member_rns_identity);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist_uid,
            "checklist-deleted",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_status(
        &self,
        request: &ChecklistTaskStatusSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let task = find_checklist_task_mut(&mut checklist, request.task_uid.as_str())?;
        if task.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        task.updated_at = Some(timestamp.clone());
        task.user_status = request.user_status;
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        if task.task_status.is_complete() {
            task.completed_at = Some(timestamp.clone());
            task.completed_by_team_member_rns_identity = request
                .changed_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        } else {
            task.completed_at = None;
            task.completed_by_team_member_rns_identity = None;
        }
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-status-set",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn add_checklist_task_row(
        &self,
        request: &ChecklistTaskRowAddRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        let task_uid = request
            .task_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}-task-{}-{}", checklist.uid, request.number, now_ms()));
        if checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
            let task = checklist
                .tasks
                .iter_mut()
                .find(|task| task.task_uid == task_uid)
                .ok_or(NodeError::InvalidConfig {})?;
            if task.deleted_at.is_some() {
                task.deleted_at = None;
                task.updated_at = Some(timestamp.clone());
                task.number = request.number;
                task.user_status = ChecklistUserTaskStatus::Pending {};
                task.task_status = ChecklistTaskStatus::Pending {};
                task.is_late = false;
                task.custom_status = None;
                task.due_relative_minutes = request.due_relative_minutes;
                task.due_dtg = None;
                task.notes = None;
                task.row_background_color = None;
                task.line_break_enabled = false;
                task.completed_at = None;
                task.completed_by_team_member_rns_identity = None;
                task.legacy_value = request.legacy_value.clone();
                task.cells = checklist
                    .columns
                    .iter()
                    .map(|column| ChecklistCellRecord {
                        cell_uid: format!("{}:{}", task.task_uid, column.column_uid),
                        task_uid: task.task_uid.clone(),
                        column_uid: column.column_uid.clone(),
                        value: None,
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    })
                    .collect();
                checklist.updated_at = Some(timestamp);
                set_checklist_last_changed_by(
                    &mut checklist,
                    request.changed_by_team_member_rns_identity.as_deref(),
                );
                normalize_checklist(&mut checklist);
                self.write_checklist_tx(&transaction, &checklist)?;
                let invalidations = self.bump_checklist_projection_revisions_tx(
                    &transaction,
                    checklist.uid.as_str(),
                    "checklist-task-row-added",
                )?;
                transaction.commit().map_err(|_| NodeError::IoError {})?;
                return Ok(invalidations);
            }
            return Err(NodeError::InvalidConfig {});
        }
        let cells = checklist
            .columns
            .iter()
            .map(|column| ChecklistCellRecord {
                cell_uid: format!("{task_uid}:{}", column.column_uid),
                task_uid: task_uid.clone(),
                column_uid: column.column_uid.clone(),
                value: None,
                updated_at: None,
                updated_by_team_member_rns_identity: None,
            })
            .collect::<Vec<_>>();
        checklist.tasks.push(ChecklistTaskRecord {
            task_uid,
            number: request.number,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some(timestamp.clone()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: request.due_relative_minutes,
            due_dtg: None,
            notes: None,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: request.legacy_value.clone(),
            cells,
        });
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-added",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn delete_checklist_task_row(
        &self,
        request: &ChecklistTaskRowDeleteRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        let task = checklist
            .tasks
            .iter_mut()
            .find(|task| task.task_uid == request.task_uid)
            .ok_or(NodeError::InvalidConfig {})?;
        task.deleted_at = Some(timestamp.clone());
        task.updated_at = Some(timestamp.clone());
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-deleted",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_row_style(
        &self,
        request: &ChecklistTaskRowStyleSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let task = find_checklist_task_mut(&mut checklist, request.task_uid.as_str())?;
        if task.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        task.updated_at = Some(timestamp.clone());
        if let Some(row_background_color) = request.row_background_color.as_deref() {
            task.row_background_color = normalize_optional_string(Some(row_background_color));
        }
        if let Some(line_break_enabled) = request.line_break_enabled {
            task.line_break_enabled = line_break_enabled;
        }
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-style-set",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_cell(
        &self,
        request: &ChecklistTaskCellSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        if !checklist
            .columns
            .iter()
            .any(|column| column.column_uid == request.column_uid)
        {
            let display_order = checklist.columns.len() as u32;
            checklist.columns.push(ChecklistColumnRecord {
                column_uid: request.column_uid.clone(),
                column_name: request.column_uid.clone(),
                display_order,
                column_type: ChecklistColumnType::ShortString {},
                column_editable: true,
                background_color: None,
                text_color: None,
                is_removable: true,
                system_key: None,
            });
        }
        let timestamp = current_timestamp_rfc3339();
        let task = find_checklist_task_mut(&mut checklist, request.task_uid.as_str())?;
        if task.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        task.updated_at = Some(timestamp.clone());
        if let Some(cell) = task
            .cells
            .iter_mut()
            .find(|cell| cell.column_uid == request.column_uid)
        {
            cell.value = Some(request.value.clone());
            cell.updated_at = Some(timestamp.clone());
            cell.updated_by_team_member_rns_identity = request
                .updated_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        } else {
            task.cells.push(ChecklistCellRecord {
                cell_uid: format!("{}:{}", task.task_uid, request.column_uid),
                task_uid: task.task_uid.clone(),
                column_uid: request.column_uid.clone(),
                value: Some(request.value.clone()),
                updated_at: Some(timestamp.clone()),
                updated_by_team_member_rns_identity: request
                    .updated_by_team_member_rns_identity
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            });
        }
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.updated_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-cell-set",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<&str>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        self.list_messages_resolved(conversation_id, &ConversationPeerResolver::default())
    }

    pub(crate) fn list_messages_resolved(
        &self,
        conversation_id: Option<&str>,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let connection = self.connect()?;
        self.repair_message_conversations(&connection, resolver)?;
        let mut records = Vec::new();
        if let Some(conversation_id) = conversation_id {
            let conversation_id = resolver.canonical_for(conversation_id);
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
        self.list_conversations_resolved(&ConversationPeerResolver::default())
    }

    pub(crate) fn list_conversations_resolved(
        &self,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<ConversationRecord>, NodeError> {
        let messages = self.list_messages_resolved(None, resolver)?;
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
            let resolved_peer = resolver.peer_for_canonical(message.conversation_id.as_str());
            let peer_destination_hex = resolved_peer
                .map(|peer| peer.peer_destination_hex.clone())
                .unwrap_or_else(|| message.conversation_id.clone());
            let preview = truncate_preview(message.body_utf8.as_str());
            let peer_display_name = resolved_peer
                .and_then(|peer| peer.display_name.clone())
                .or_else(|| labels.get(&peer_destination_hex).cloned().flatten());
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

    pub(crate) fn delete_conversation_resolved(
        &self,
        conversation_id: &str,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let canonical_id = resolver.canonical_for(conversation_id);
        if canonical_id.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let mut ids = resolver.aliases_for_canonical(canonical_id.as_str());
        ids.push(canonical_id.clone());
        ids.sort();
        ids.dedup();

        let mut connection = self.connect()?;
        self.repair_message_conversations(&connection, resolver)?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        for id in &ids {
            transaction
                .execute(
                    "DELETE FROM messages WHERE conversation_id = ?1",
                    params![id],
                )
                .map_err(|_| NodeError::IoError {})?;
        }
        let messages = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Messages {},
            Some(canonical_id),
            Some("conversation-deleted".to_string()),
        )?;
        let conversations = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Conversations {},
            None,
            Some("conversation-deleted".to_string()),
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

fn serialize_json<T: Serialize>(value: &T) -> Result<String, NodeError> {
    serde_json::to_string(value).map_err(|_| NodeError::InternalError {})
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, NodeError> {
    serde_json::from_str(value).map_err(|_| NodeError::InternalError {})
}

pub(crate) fn canonicalize_chat_message(message: &MessageRecord) -> MessageRecord {
    canonicalize_chat_message_with_resolver(message, &ConversationPeerResolver::default())
}

fn canonicalize_chat_message_with_resolver(
    message: &MessageRecord,
    resolver: &ConversationPeerResolver,
) -> MessageRecord {
    let peer_key = canonical_message_peer_key(message);
    let conversation_key = normalize_message_peer_key(message.conversation_id.as_str());
    let canonical_id = resolver
        .resolve(peer_key.as_str())
        .or_else(|| resolver.resolve(conversation_key.as_str()))
        .map(|record| record.canonical_id.clone())
        .unwrap_or(peer_key);
    MessageRecord {
        conversation_id: canonical_id,
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

fn parse_checklist_template_csv(
    request: &ChecklistTemplateImportCsvRequest,
    default_task_due_step_minutes: u32,
) -> Result<ChecklistTemplateRecord, NodeError> {
    let name = request.name.trim();
    if name.is_empty() || request.csv_text.trim().is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(request.csv_text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| NodeError::InvalidConfig {})?
        .clone();
    let mut rows = Vec::<Vec<String>>::new();
    for row in reader.records() {
        let row = row.map_err(|_| NodeError::InvalidConfig {})?;
        let cells = row
            .iter()
            .map(|cell| cell.replace('\u{feff}', "").trim().to_string())
            .collect::<Vec<_>>();
        if cells.iter().any(|cell| !cell.is_empty()) {
            rows.push(cells);
        }
    }
    if rows.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let max_columns = rows
        .iter()
        .fold(headers.len(), |max, row| max.max(row.len()));
    if max_columns == 0 {
        return Err(NodeError::InvalidConfig {});
    }
    let header_names = (0..max_columns)
        .map(|index| {
            headers
                .get(index)
                .map(|value| value.replace('\u{feff}', "").trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("Column {}", index + 1))
        })
        .collect::<Vec<_>>();
    let due_header_index = header_names
        .iter()
        .position(|header| is_checklist_due_header(header));

    let mut columns = Vec::new();
    columns.push(ChecklistColumnRecord {
        column_uid: "col-due-relative-dtg".to_string(),
        column_name: due_header_index
            .and_then(|index| header_names.get(index))
            .cloned()
            .unwrap_or_else(|| "CompletedDTG".to_string()),
        display_order: 0,
        column_type: ChecklistColumnType::RelativeTime {},
        column_editable: false,
        background_color: None,
        text_color: None,
        is_removable: false,
        system_key: Some(crate::types::ChecklistSystemColumnKey::DueRelativeDtg {}),
    });

    let mut used_column_uids = HashMap::<String, u32>::new();
    used_column_uids.insert("col-due-relative-dtg".to_string(), 1);
    let mut header_column_uids = HashMap::<usize, String>::new();
    for (header_index, header) in header_names.iter().enumerate() {
        if Some(header_index) == due_header_index {
            continue;
        }
        let column_uid = checklist_csv_column_uid(header, header_index, &mut used_column_uids);
        header_column_uids.insert(header_index, column_uid.clone());
        columns.push(ChecklistColumnRecord {
            column_uid,
            column_name: header.clone(),
            display_order: columns.len() as u32,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: true,
            system_key: None,
        });
    }
    if header_column_uids.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }

    let title_header_index = header_names
        .iter()
        .enumerate()
        .find(|(index, header)| {
            Some(*index) != due_header_index && is_checklist_title_header(header)
        })
        .map(|(index, _)| index);
    let description_header_index = header_names
        .iter()
        .enumerate()
        .find(|(index, header)| {
            Some(*index) != due_header_index && is_checklist_description_header(header)
        })
        .map(|(index, _)| index);
    let template_uid_seed = request
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("tmpl-import");
    let due_step = default_task_due_step_minutes.max(1);
    let mut tasks = Vec::new();
    for row in rows {
        let number = (tasks.len() + 1) as u32;
        let task_uid = format!("{template_uid_seed}-task-{number}");
        let due_relative_minutes = match due_header_index {
            Some(index) => {
                let value = csv_cell(&row, index);
                if value.is_empty() {
                    Some(number * due_step)
                } else {
                    Some(parse_checklist_due_relative_minutes(value)?)
                }
            }
            None => Some(number * due_step),
        };
        let title = title_header_index
            .map(|index| csv_cell(&row, index))
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                header_names
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| Some(*index) != due_header_index)
                    .map(|(index, _)| csv_cell(&row, index))
                    .find(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("Task {number}"));
        let notes = description_header_index
            .map(|index| csv_cell(&row, index))
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let cells = header_column_uids
            .iter()
            .map(|(header_index, column_uid)| {
                let value = csv_cell(&row, *header_index).to_string();
                ChecklistCellRecord {
                    cell_uid: format!("{task_uid}:{column_uid}"),
                    task_uid: task_uid.clone(),
                    column_uid: column_uid.clone(),
                    value: Some(value),
                    updated_at: None,
                    updated_by_team_member_rns_identity: None,
                }
            })
            .collect::<Vec<_>>();
        tasks.push(ChecklistTaskRecord {
            task_uid,
            number,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: None,
            deleted_at: None,
            custom_status: None,
            due_relative_minutes,
            due_dtg: None,
            notes,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: Some(title),
            cells,
        });
    }

    let timestamp = current_timestamp_rfc3339();
    let template_uid = request
        .template_uid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("tmpl-{}", now_ms()));
    let mut template = ChecklistTemplateRecord {
        uid: template_uid,
        name: name.to_string(),
        description: request
            .description
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        version: 1,
        origin_type: ChecklistOriginType::CsvImport {},
        created_at: Some(timestamp.clone()),
        updated_at: Some(timestamp),
        source_filename: normalize_optional_string(request.source_filename.as_deref()),
        columns,
        tasks,
    };
    normalize_checklist_template(&mut template);
    Ok(template)
}

fn csv_cell(row: &[String], index: usize) -> &str {
    row.get(index).map(String::as_str).unwrap_or_default()
}

fn normalize_checklist_csv_header(value: &str) -> String {
    value
        .replace('\u{feff}', "")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_checklist_due_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "completeddtg" | "due" | "duerelativedtg" | "duerelativeminutes" | "dueminutes"
    )
}

fn is_checklist_title_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "item" | "task" | "name" | "title"
    )
}

fn is_checklist_description_header(value: &str) -> bool {
    matches!(
        normalize_checklist_csv_header(value).as_str(),
        "description" | "detail" | "details" | "notes"
    )
}

fn checklist_csv_column_uid(header: &str, index: usize, used: &mut HashMap<String, u32>) -> String {
    let mut slug = header
        .replace('\u{feff}', "")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = format!("column-{}", index + 1);
    }
    let base = format!("col-{slug}");
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{count}")
    }
}

fn parse_checklist_due_relative_minutes(value: &str) -> Result<u32, NodeError> {
    let mut text = value.trim().to_ascii_lowercase();
    if text.is_empty() || text.starts_with('-') {
        return Err(NodeError::InvalidConfig {});
    }
    if let Some(stripped) = text.strip_prefix('+') {
        text = stripped.trim().to_string();
    }
    if let Some((hours, minutes)) = text.split_once(':') {
        let hours = hours
            .trim()
            .parse::<u32>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        let minutes = minutes
            .trim()
            .parse::<u32>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        if minutes >= 60 {
            return Err(NodeError::InvalidConfig {});
        }
        return Ok(hours * 60 + minutes);
    }
    if let Some(hours) = text.strip_suffix('h') {
        return hours
            .trim()
            .parse::<u32>()
            .map(|value| value * 60)
            .map_err(|_| NodeError::InvalidConfig {});
    }
    for suffix in ["hours", "hour"] {
        if let Some(hours) = text.strip_suffix(suffix) {
            return hours
                .trim()
                .parse::<u32>()
                .map(|value| value * 60)
                .map_err(|_| NodeError::InvalidConfig {});
        }
    }
    text.parse::<u32>().map_err(|_| NodeError::InvalidConfig {})
}

fn checklist_template_columns() -> Vec<ChecklistColumnRecord> {
    vec![
        ChecklistColumnRecord {
            column_uid: "col-due-relative-dtg".to_string(),
            column_name: "CompletedDTG".to_string(),
            display_order: 0,
            column_type: ChecklistColumnType::RelativeTime {},
            column_editable: false,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: Some(crate::types::ChecklistSystemColumnKey::DueRelativeDtg {}),
        },
        ChecklistColumnRecord {
            column_uid: "col-item".to_string(),
            column_name: "Item".to_string(),
            display_order: 1,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-description".to_string(),
            column_name: "Description".to_string(),
            display_order: 2,
            column_type: ChecklistColumnType::LongString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-category".to_string(),
            column_name: "Category".to_string(),
            display_order: 3,
            column_type: ChecklistColumnType::ShortString {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
        ChecklistColumnRecord {
            column_uid: "col-quantity".to_string(),
            column_name: "Quantity".to_string(),
            display_order: 4,
            column_type: ChecklistColumnType::Integer {},
            column_editable: true,
            background_color: None,
            text_color: None,
            is_removable: false,
            system_key: None,
        },
    ]
}

fn checklist_template_from_rows(
    uid: &str,
    name: &str,
    description: &str,
    rows: &[(&str, &str, &str, u32)],
) -> ChecklistTemplateRecord {
    let timestamp = current_timestamp_rfc3339();
    let tasks = rows
        .iter()
        .enumerate()
        .map(|(index, (item, description, category, quantity))| {
            let task_uid = format!("{uid}-task-{}", index + 1);
            ChecklistTaskRecord {
                task_uid: task_uid.clone(),
                number: (index + 1) as u32,
                user_status: ChecklistUserTaskStatus::Pending {},
                task_status: ChecklistTaskStatus::Pending {},
                is_late: false,
                updated_at: None,
                deleted_at: None,
                custom_status: None,
                due_relative_minutes: Some(
                    (index as u32 + 1) * DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES,
                ),
                due_dtg: None,
                notes: Some((*description).to_string()),
                row_background_color: None,
                line_break_enabled: false,
                completed_at: None,
                completed_by_team_member_rns_identity: None,
                legacy_value: Some((*item).to_string()),
                cells: vec![
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-item"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-item".to_string(),
                        value: Some((*item).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-description"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-description".to_string(),
                        value: Some((*description).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-category"),
                        task_uid: task_uid.clone(),
                        column_uid: "col-category".to_string(),
                        value: Some((*category).to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                    ChecklistCellRecord {
                        cell_uid: format!("{task_uid}:col-quantity"),
                        task_uid,
                        column_uid: "col-quantity".to_string(),
                        value: Some(quantity.to_string()),
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    },
                ],
            }
        })
        .collect();
    let mut template = ChecklistTemplateRecord {
        uid: uid.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        version: 1,
        origin_type: ChecklistOriginType::RchTemplate {},
        created_at: Some(timestamp.clone()),
        updated_at: Some(timestamp),
        source_filename: None,
        columns: checklist_template_columns(),
        tasks,
    };
    normalize_checklist_template(&mut template);
    template
}

fn default_checklist_templates() -> Vec<ChecklistTemplateRecord> {
    vec![
        checklist_template_from_rows(
            "tmpl-24-hour-survival-pack",
            "24 Hour Survival Pack",
            "Personal 24-hour emergency loadout for rapid deployment and sustainment.",
            &[
                ("Mini Bic or waterproof lighter", "Reliable ignition source", "Fire & Light", 1),
                ("Compact headlamp", "Hands-free light source", "Fire & Light", 1),
                ("1L Nalgene bottle or canteen", "Durable water container", "Water", 1),
                ("Water purification tabs", "Lightweight and effective", "Water", 10),
                ("MRE or freeze-dried meal", "Primary field ration", "Food", 1),
                ("Energy bars", "High-calorie snack", "Food", 4),
                ("Multitool", "Repair and utility tool", "Tools & Utility", 1),
                ("550 Paracord", "Shelter and lashing", "Tools & Utility", 1),
                ("Emergency mylar blanket", "Warmth and signaling", "Clothing / Warmth", 1),
                ("IFAK", "Trauma bandage, gloves, tourniquet", "Medical / Hygiene", 1),
                ("Compass", "Reliable navigation tool", "Navigation / Communication", 1),
                ("Printed map", "Area-specific map", "Navigation / Communication", 1),
            ],
        ),
        checklist_template_from_rows(
            "tmpl-72-hour-home-preparedness",
            "72 Hour Home Preparedness",
            "Household emergency readiness checklist for shelter-in-place and temporary disruption.",
            &[
                ("Stored drinking water", "Three-day reserve for household use", "Water", 12),
                ("Shelf-stable meals", "Ready-to-eat or low-prep food", "Food", 9),
                ("Manual can opener", "Access canned food during outage", "Food", 1),
                ("Flashlights", "Area lighting during power loss", "Power / Lighting", 2),
                ("Battery bank", "Phone and radio charging backup", "Power / Lighting", 1),
                ("AA/AAA batteries", "Spare cells for lights and radios", "Power / Lighting", 12),
                ("First aid kit", "Household medical supplies", "Medical", 1),
                ("Prescription refill copy", "Medication continuity reference", "Medical", 1),
                ("Hygiene kit", "Soap, wipes, sanitation bags", "Hygiene", 1),
                ("Printed contact sheet", "Emergency contacts and rally info", "Communications", 1),
                ("Weather or crank radio", "Situation updates during outage", "Communications", 1),
                ("Copies of IDs and insurance", "Critical documents protected", "Documents", 1),
            ],
        ),
        checklist_template_from_rows(
            "tmpl-vehicle-emergency-preparedness",
            "Vehicle Emergency Preparedness",
            "Vehicle-based emergency kit for evacuation, roadside survival, and communications continuity.",
            &[
                ("Vehicle first aid kit", "Trauma and minor wound support", "Medical", 1),
                ("Jumper cables", "Battery recovery", "Vehicle Recovery", 1),
                ("Tire inflator or sealant", "Flat tire contingency", "Vehicle Recovery", 1),
                ("Tow strap", "Recovery from mud or ditch", "Vehicle Recovery", 1),
                ("Reflective triangles", "Roadside visibility", "Safety", 3),
                ("High-visibility vest", "Roadside operator visibility", "Safety", 1),
                ("Blanket", "Cold-weather warmth", "Shelter / Warmth", 2),
                ("Stored water bottles", "Occupant hydration", "Water", 6),
                ("Shelf-stable snacks", "Travel sustainment", "Food", 6),
                ("Paper map", "Navigation backup if devices fail", "Navigation", 1),
                ("12V phone charger", "Device power continuity", "Communications", 1),
                ("Handheld radio", "Backup local comms", "Communications", 1),
            ],
        ),
    ]
}

fn projection_scope_name(scope: ProjectionScope) -> &'static str {
    match scope {
        ProjectionScope::AppSettings {} => "AppSettings",
        ProjectionScope::SavedPeers {} => "SavedPeers",
        ProjectionScope::OperationalSummary {} => "OperationalSummary",
        ProjectionScope::Peers {} => "Peers",
        ProjectionScope::SyncStatus {} => "SyncStatus",
        ProjectionScope::HubRegistration {} => "HubRegistration",
        ProjectionScope::Checklists {} => "Checklists",
        ProjectionScope::ChecklistDetail {} => "ChecklistDetail",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
        ProjectionScope::Plugins {} => "Plugins",
        ProjectionScope::Sos {} => "Sos",
    }
}

pub(crate) fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn set_checklist_last_changed_by(
    checklist: &mut ChecklistRecord,
    identity: Option<&str>,
) {
    checklist.last_changed_by_team_member_rns_identity = normalize_optional_string(identity);
}

fn sanitize_active_checklist(mut checklist: ChecklistRecord) -> Option<ChecklistRecord> {
    if checklist.deleted_at.is_some() {
        return None;
    }
    checklist.tasks.retain(|task| task.deleted_at.is_none());
    Some(checklist)
}

pub(crate) fn normalize_checklist_record(checklist: &mut ChecklistRecord) {
    let start_epoch_seconds = checklist
        .start_time
        .as_deref()
        .and_then(parse_rfc3339_epoch_seconds);
    let now_epoch_seconds = unix_seconds_now();
    for task in &mut checklist.tasks {
        let due_epoch_seconds = start_epoch_seconds.and_then(|start| {
            task.due_relative_minutes
                .map(|minutes| start.saturating_add(i64::from(minutes) * 60))
        });
        task.due_dtg = due_epoch_seconds.map(format_rfc3339_from_epoch_seconds);
        task.is_late =
            checklist_task_is_late_for_due_dtg(task, due_epoch_seconds, now_epoch_seconds);
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        task.cells.sort_by(|left, right| {
            left.column_uid
                .cmp(&right.column_uid)
                .then_with(|| left.cell_uid.cmp(&right.cell_uid))
        });
    }
    checklist.columns.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.column_uid.cmp(&right.column_uid))
    });
    checklist.tasks.sort_by(|left, right| {
        left.number
            .cmp(&right.number)
            .then_with(|| left.task_uid.cmp(&right.task_uid))
    });

    let active_tasks = checklist
        .tasks
        .iter()
        .filter(|task| task.deleted_at.is_none())
        .collect::<Vec<_>>();
    let pending_count = active_tasks
        .iter()
        .copied()
        .filter(|task| matches!(task.task_status, ChecklistTaskStatus::Pending {}))
        .count() as u32;
    let late_count = active_tasks
        .iter()
        .copied()
        .filter(|task| matches!(task.task_status, ChecklistTaskStatus::Late {}))
        .count() as u32;
    let complete_count = active_tasks
        .iter()
        .copied()
        .filter(|task| task.task_status.is_complete())
        .count() as u32;
    checklist.counts.pending_count = pending_count;
    checklist.counts.late_count = late_count;
    checklist.counts.complete_count = complete_count;
    let total = active_tasks.len() as u32;
    if checklist.expected_task_count.is_none() {
        checklist.expected_task_count = Some(total);
    }
    checklist.progress_percent = if total == 0 {
        0.0
    } else {
        (f64::from(complete_count) * 100.0) / f64::from(total)
    };
    checklist.checklist_status = if late_count > 0 {
        ChecklistTaskStatus::Late {}
    } else if pending_count > 0 || total == 0 {
        ChecklistTaskStatus::Pending {}
    } else if active_tasks
        .iter()
        .copied()
        .any(|task| matches!(task.task_status, ChecklistTaskStatus::CompleteLate {}))
    {
        ChecklistTaskStatus::CompleteLate {}
    } else {
        ChecklistTaskStatus::Complete {}
    };
}

fn normalize_checklist_template(template: &mut ChecklistTemplateRecord) {
    template.name = template.name.trim().to_string();
    template.description = template.description.trim().to_string();
    template
        .source_filename
        .clone_from(&normalize_optional_string(
            template.source_filename.as_deref(),
        ));
    for task in &mut template.tasks {
        task.is_late = task.task_status.is_late();
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        task.cells.sort_by(|left, right| {
            left.column_uid
                .cmp(&right.column_uid)
                .then_with(|| left.cell_uid.cmp(&right.cell_uid))
        });
    }
    template.columns.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.column_uid.cmp(&right.column_uid))
    });
    template.tasks.sort_by(|left, right| {
        left.number
            .cmp(&right.number)
            .then_with(|| left.task_uid.cmp(&right.task_uid))
    });
}

fn normalize_checklist(checklist: &mut ChecklistRecord) {
    normalize_checklist_record(checklist);
}

fn checklist_task_is_late_for_due_dtg(
    task: &ChecklistTaskRecord,
    due_epoch_seconds: Option<i64>,
    now_epoch_seconds: i64,
) -> bool {
    let Some(due_epoch_seconds) = due_epoch_seconds else {
        return task.task_status.is_late();
    };
    match task.user_status {
        ChecklistUserTaskStatus::Pending {} => now_epoch_seconds > due_epoch_seconds,
        ChecklistUserTaskStatus::Complete {} => task
            .completed_at
            .as_deref()
            .and_then(parse_rfc3339_epoch_seconds)
            .map(|completed_epoch_seconds| completed_epoch_seconds > due_epoch_seconds)
            .unwrap_or(false),
    }
}

pub(crate) fn checklist_task_status_for(
    user_status: ChecklistUserTaskStatus,
    is_late: bool,
) -> ChecklistTaskStatus {
    match user_status {
        ChecklistUserTaskStatus::Pending {} => {
            if is_late {
                ChecklistTaskStatus::Late {}
            } else {
                ChecklistTaskStatus::Pending {}
            }
        }
        ChecklistUserTaskStatus::Complete {} => {
            if is_late {
                ChecklistTaskStatus::CompleteLate {}
            } else {
                ChecklistTaskStatus::Complete {}
            }
        }
    }
}

pub(crate) fn find_checklist_task_mut<'a>(
    checklist: &'a mut ChecklistRecord,
    task_uid: &str,
) -> Result<&'a mut ChecklistTaskRecord, NodeError> {
    checklist
        .tasks
        .iter_mut()
        .find(|task| task.task_uid == task_uid && task.deleted_at.is_none())
        .ok_or(NodeError::InvalidConfig {})
}

pub(crate) fn current_timestamp_rfc3339() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds_since_epoch = duration.as_secs() as i64;
    let nanos = duration.subsec_nanos();
    let days_since_epoch = seconds_since_epoch.div_euclid(86_400);
    let seconds_of_day = seconds_since_epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn parse_rfc3339_epoch_seconds(timestamp: &str) -> Option<i64> {
    let trimmed = timestamp.trim();
    let suffix = trimmed.strip_suffix('Z')?;
    let (date, time) = suffix.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second_part = time_parts.next().unwrap_or("0");
    if time_parts.next().is_some() {
        return None;
    }
    let second_text = second_part
        .split_once('.')
        .map_or(second_part, |(whole, _)| whole);
    let second = second_text.parse::<i64>().ok()?;

    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn format_rfc3339_from_epoch_seconds(epoch_seconds: i64) -> String {
    let days_since_epoch = epoch_seconds.div_euclid(86_400);
    let seconds_of_day = epoch_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::types::{
        AppSettingsRecord, ChecklistCellRecord, ChecklistColumnRecord, ChecklistColumnType,
        ChecklistCreateFromTemplateRequest, ChecklistMode, ChecklistOriginType, ChecklistRecord,
        ChecklistSettingsRecord, ChecklistStatusCounts, ChecklistSystemColumnKey,
        ChecklistTaskCellSetRequest, ChecklistTaskRecord, ChecklistTaskRowAddRequest,
        ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus,
        ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistUpdatePatch,
        ChecklistUpdateRequest, ChecklistUserTaskStatus, HubMode, HubSettingsRecord,
        MessageDirection, MessageMethod, MessageState, ProjectionScope, TelemetrySettingsRecord,
    };

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

    fn app_settings_with_due_step(default_task_due_step_minutes: u32) -> AppSettingsRecord {
        AppSettingsRecord {
            display_name: "Test Operator".to_string(),
            auto_connect_saved: true,
            announce_capabilities: "R3AKT,EMergencyMessages".to_string(),
            tcp_clients: Vec::new(),
            broadcast: true,
            announce_interval_seconds: 1800,
            telemetry: TelemetrySettingsRecord {
                enabled: false,
                publish_interval_seconds: 60,
                accuracy_threshold_meters: None,
                stale_after_minutes: 30,
                expire_after_minutes: 180,
            },
            hub: HubSettingsRecord {
                mode: HubMode::Autonomous {},
                identity_hash: String::new(),
                api_base_url: String::new(),
                api_key: String::new(),
                refresh_interval_seconds: 3600,
            },
            checklists: ChecklistSettingsRecord {
                default_task_due_step_minutes,
            },
        }
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

    fn checklist(uid: &str) -> ChecklistRecord {
        ChecklistRecord {
            uid: uid.to_string(),
            mission_uid: Some("mission-alpha".to_string()),
            template_uid: Some("tmpl-alpha".to_string()),
            template_version: Some(1),
            template_name: Some("Alpha Template".to_string()),
            name: "Alpha Checklist".to_string(),
            description: "Shared alpha checklist".to_string(),
            start_time: Some("2099-04-22T12:00:00Z".to_string()),
            mode: ChecklistMode::Online {},
            sync_state: crate::types::ChecklistSyncState::Synced {},
            origin_type: ChecklistOriginType::RchTemplate {},
            checklist_status: ChecklistTaskStatus::Pending {},
            created_at: Some("2026-04-22T12:00:00Z".to_string()),
            created_by_team_member_rns_identity: "abcd1234".to_string(),
            created_by_team_member_display_name: Some("Alpha Operator".to_string()),
            updated_at: Some("2026-04-22T12:00:00Z".to_string()),
            last_changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            deleted_at: None,
            uploaded_at: None,
            participant_rns_identities: vec!["abcd1234".to_string()],
            expected_task_count: Some(1),
            progress_percent: 0.0,
            counts: ChecklistStatusCounts {
                pending_count: 1,
                late_count: 0,
                complete_count: 0,
            },
            columns: vec![
                ChecklistColumnRecord {
                    column_uid: "col-due".to_string(),
                    column_name: "Due".to_string(),
                    display_order: 0,
                    column_type: ChecklistColumnType::RelativeTime {},
                    column_editable: false,
                    background_color: None,
                    text_color: None,
                    is_removable: false,
                    system_key: Some(ChecklistSystemColumnKey::DueRelativeDtg {}),
                },
                ChecklistColumnRecord {
                    column_uid: "col-task".to_string(),
                    column_name: "Task".to_string(),
                    display_order: 1,
                    column_type: ChecklistColumnType::ShortString {},
                    column_editable: true,
                    background_color: None,
                    text_color: None,
                    is_removable: true,
                    system_key: None,
                },
            ],
            tasks: vec![ChecklistTaskRecord {
                task_uid: "task-1".to_string(),
                number: 1,
                user_status: ChecklistUserTaskStatus::Pending {},
                task_status: ChecklistTaskStatus::Pending {},
                is_late: false,
                updated_at: None,
                deleted_at: None,
                custom_status: None,
                due_relative_minutes: Some(15),
                due_dtg: None,
                notes: None,
                row_background_color: None,
                line_break_enabled: false,
                completed_at: None,
                completed_by_team_member_rns_identity: None,
                legacy_value: Some("Check in".to_string()),
                cells: vec![ChecklistCellRecord {
                    cell_uid: "task-1:col-task".to_string(),
                    task_uid: "task-1".to_string(),
                    column_uid: "col-task".to_string(),
                    value: Some("Check in".to_string()),
                    updated_at: None,
                    updated_by_team_member_rns_identity: None,
                }],
            }],
            feed_publications: Vec::new(),
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

    #[test]
    fn peer_identity_aliases_fold_existing_split_threads() {
        let storage_dir = test_storage_dir("identity-alias-thread");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let outbound = message(
            "outbound-alias",
            "app-thread",
            MessageDirection::Outbound {},
            "APPDEST",
            Some("LOCAL"),
            10,
        );
        let inbound = message(
            "inbound-alias",
            "lxmf-thread",
            MessageDirection::Inbound {},
            "LOCAL",
            Some("LXMFDest"),
            20,
        );

        store.upsert_message(&outbound).expect("persist outbound");
        store.upsert_message(&inbound).expect("persist inbound");
        assert_eq!(
            store
                .list_conversations()
                .expect("list before aliases")
                .len(),
            2
        );

        let mut resolver = ConversationPeerResolver::default();
        resolver.insert(
            vec!["APPDEST".to_string(), "LXMFDest".to_string()],
            "IDENTITY".to_string(),
            "LXMFDest".to_string(),
            Some("Poco".to_string()),
        );

        let conversations = store
            .list_conversations_resolved(&resolver)
            .expect("list after aliases");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, "identity");
        assert_eq!(conversations[0].peer_destination_hex, "lxmfdest");
        assert_eq!(conversations[0].peer_display_name.as_deref(), Some("Poco"));

        let messages = store
            .list_messages_resolved(Some("APPDEST"), &resolver)
            .expect("list canonical messages");
        assert_eq!(messages.len(), 2);
        assert!(messages
            .iter()
            .all(|message| message.conversation_id == "identity"));
    }

    #[test]
    fn delete_conversation_removes_canonical_peer_alias_thread() {
        let storage_dir = test_storage_dir("delete-alias-thread");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let outbound = message(
            "delete-outbound",
            "app-thread",
            MessageDirection::Outbound {},
            "APPDEST",
            Some("LOCAL"),
            10,
        );
        let inbound = message(
            "delete-inbound",
            "lxmf-thread",
            MessageDirection::Inbound {},
            "LOCAL",
            Some("LXMFDest"),
            20,
        );
        let unrelated = message(
            "delete-unrelated",
            "other-thread",
            MessageDirection::Outbound {},
            "OTHER",
            Some("LOCAL"),
            30,
        );

        store.upsert_message(&outbound).expect("persist outbound");
        store.upsert_message(&inbound).expect("persist inbound");
        store.upsert_message(&unrelated).expect("persist unrelated");

        let mut resolver = ConversationPeerResolver::default();
        resolver.insert(
            vec!["APPDEST".to_string(), "LXMFDest".to_string()],
            "IDENTITY".to_string(),
            "LXMFDest".to_string(),
            Some("Poco".to_string()),
        );

        store
            .delete_conversation_resolved("APPDEST", &resolver)
            .expect("delete alias conversation");

        let conversations = store
            .list_conversations_resolved(&resolver)
            .expect("list after delete");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].conversation_id, "other");
        let messages = store
            .list_messages_resolved(None, &resolver)
            .expect("list remaining messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id_hex, "delete-unrelated");
    }

    #[test]
    fn checklist_lifecycle_persists_and_invalidates_list_and_detail_scopes() {
        let storage_dir = test_storage_dir("checklist-lifecycle");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let checklist = checklist("chk-1");

        let invalidations = store
            .upsert_checklist(&checklist, "checklist-upserted")
            .expect("upsert checklist");
        assert_eq!(invalidations.len(), 2);
        assert!(matches!(
            invalidations[0].scope,
            ProjectionScope::Checklists {}
        ));
        assert!(matches!(
            invalidations[1].scope,
            ProjectionScope::ChecklistDetail {}
        ));
        assert_eq!(invalidations[1].key.as_deref(), Some("chk-1"));

        let list = store
            .get_active_checklists()
            .expect("get active checklists");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].uid, "chk-1");

        let fetched = store
            .get_checklist("chk-1")
            .expect("get checklist")
            .expect("checklist exists");
        assert_eq!(fetched.name, "Alpha Checklist");

        let updated = store
            .update_checklist(&ChecklistUpdateRequest {
                checklist_uid: "chk-1".to_string(),
                patch: ChecklistUpdatePatch {
                    mission_uid: Some("mission-bravo".to_string()),
                    template_uid: None,
                    name: Some("Bravo Checklist".to_string()),
                    description: Some("Updated after briefing".to_string()),
                    start_time: None,
                },
                changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("update checklist");
        assert_eq!(updated.len(), 2);
        let fetched = store
            .get_checklist("chk-1")
            .expect("get updated checklist")
            .expect("updated checklist exists");
        assert_eq!(fetched.mission_uid.as_deref(), Some("mission-bravo"));
        assert_eq!(fetched.name, "Bravo Checklist");
        assert_eq!(
            fetched.last_changed_by_team_member_rns_identity.as_deref(),
            Some("abcd1234")
        );

        let deleted = store.delete_checklist("chk-1").expect("delete checklist");
        assert_eq!(deleted.len(), 2);
        assert!(store
            .get_checklist("chk-1")
            .expect("query deleted checklist")
            .is_none());
    }

    #[test]
    fn checklist_task_mutations_update_counts_and_cells() {
        let storage_dir = test_storage_dir("checklist-task-mutations");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        store
            .upsert_checklist(&checklist("chk-2"), "seed-checklist")
            .expect("seed checklist");

        store
            .add_checklist_task_row(&ChecklistTaskRowAddRequest {
                checklist_uid: "chk-2".to_string(),
                task_uid: Some("task-2".to_string()),
                number: 2,
                due_relative_minutes: Some(30),
                legacy_value: Some("Confirm rally point".to_string()),
                changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("add task row");
        store
            .set_checklist_task_row_style(&ChecklistTaskRowStyleSetRequest {
                checklist_uid: "chk-2".to_string(),
                task_uid: "task-2".to_string(),
                row_background_color: Some("#402020".to_string()),
                line_break_enabled: Some(true),
                changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("set row style");
        store
            .set_checklist_task_cell(&ChecklistTaskCellSetRequest {
                checklist_uid: "chk-2".to_string(),
                task_uid: "task-2".to_string(),
                column_uid: "col-task".to_string(),
                value: "Move to alternate pickup".to_string(),
                updated_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("set task cell");
        store
            .set_checklist_task_status(&ChecklistTaskStatusSetRequest {
                checklist_uid: "chk-2".to_string(),
                task_uid: "task-2".to_string(),
                user_status: ChecklistUserTaskStatus::Complete {},
                changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("set task status");

        let checklist = store
            .get_checklist("chk-2")
            .expect("get checklist")
            .expect("checklist exists");
        assert_eq!(checklist.tasks.len(), 2);
        assert_eq!(checklist.counts.pending_count, 1);
        assert_eq!(checklist.counts.complete_count, 1);
        assert_eq!(checklist.progress_percent, 50.0);
        assert_eq!(
            checklist
                .last_changed_by_team_member_rns_identity
                .as_deref(),
            Some("abcd1234")
        );
        let second_task = checklist
            .tasks
            .iter()
            .find(|task| task.task_uid == "task-2")
            .expect("second task exists");
        assert!(matches!(
            second_task.task_status,
            ChecklistTaskStatus::Complete {}
        ));
        assert_eq!(second_task.row_background_color.as_deref(), Some("#402020"));
        assert!(second_task.line_break_enabled);
        assert_eq!(
            second_task
                .cells
                .iter()
                .find(|cell| cell.column_uid == "col-task")
                .and_then(|cell| cell.value.as_deref()),
            Some("Move to alternate pickup")
        );

        store
            .delete_checklist_task_row(&ChecklistTaskRowDeleteRequest {
                checklist_uid: "chk-2".to_string(),
                task_uid: "task-2".to_string(),
                changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
            })
            .expect("delete task row");
        let checklist = store
            .get_checklist("chk-2")
            .expect("get checklist after delete")
            .expect("checklist still exists");
        assert_eq!(checklist.tasks.len(), 1);
        assert_eq!(checklist.counts.pending_count, 1);
        assert_eq!(checklist.counts.complete_count, 0);
    }

    #[test]
    fn deleted_checklists_reject_local_mutations() {
        let storage_dir = test_storage_dir("checklist-deleted-mutations");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        store
            .upsert_checklist(&checklist("chk-3"), "seed-checklist")
            .expect("seed checklist");
        store.delete_checklist("chk-3").expect("delete checklist");

        let update = store.update_checklist(&ChecklistUpdateRequest {
            checklist_uid: "chk-3".to_string(),
            patch: ChecklistUpdatePatch {
                mission_uid: Some("mission-bravo".to_string()),
                template_uid: None,
                name: Some("Bravo Checklist".to_string()),
                description: None,
                start_time: None,
            },
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        });
        assert!(matches!(update, Err(NodeError::InvalidConfig {})));

        let add_row = store.add_checklist_task_row(&ChecklistTaskRowAddRequest {
            checklist_uid: "chk-3".to_string(),
            task_uid: Some("task-2".to_string()),
            number: 2,
            due_relative_minutes: Some(30),
            legacy_value: Some("Confirm rally point".to_string()),
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        });
        assert!(matches!(add_row, Err(NodeError::InvalidConfig {})));
    }

    #[test]
    fn default_checklist_templates_are_seeded() {
        let storage_dir = test_storage_dir("checklist-template-seed");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

        let templates = store
            .list_checklist_templates()
            .expect("list checklist templates");
        assert_eq!(templates.len(), 3);
        assert!(templates
            .iter()
            .any(|template| template.uid == "tmpl-24-hour-survival-pack"));
        assert!(templates.iter().all(|template| {
            template.columns.iter().any(|column| {
                column.system_key == Some(ChecklistSystemColumnKey::DueRelativeDtg {})
                    && column.column_type == ChecklistColumnType::RelativeTime {}
            }) && template
                .tasks
                .iter()
                .all(|task| task.due_relative_minutes.is_some())
        }));
    }

    #[test]
    fn checklist_late_status_is_calculated_from_due_dtg() {
        let storage_dir = test_storage_dir("checklist-due-dtg-late");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        let mut pending_late = checklist("chk-pending-late");
        pending_late.start_time = Some("1970-01-01T00:00:00Z".to_string());
        pending_late.tasks[0].due_relative_minutes = Some(15);
        store
            .upsert_checklist(&pending_late, "seed-late-checklist")
            .expect("seed late checklist");

        let pending_late = store
            .get_checklist("chk-pending-late")
            .expect("get checklist")
            .expect("checklist exists");
        assert_eq!(
            pending_late.tasks[0].due_dtg.as_deref(),
            Some("1970-01-01T00:15:00Z")
        );
        assert!(pending_late.tasks[0].is_late);
        assert_eq!(
            pending_late.tasks[0].task_status,
            ChecklistTaskStatus::Late {}
        );
        assert_eq!(pending_late.counts.late_count, 1);

        let mut completed_on_time = checklist("chk-completed-on-time");
        completed_on_time.start_time = Some("1970-01-01T00:00:00Z".to_string());
        completed_on_time.tasks[0].due_relative_minutes = Some(15);
        completed_on_time.tasks[0].user_status = ChecklistUserTaskStatus::Complete {};
        completed_on_time.tasks[0].completed_at = Some("1970-01-01T00:10:00Z".to_string());
        store
            .upsert_checklist(&completed_on_time, "seed-complete-checklist")
            .expect("seed complete checklist");

        let completed_on_time = store
            .get_checklist("chk-completed-on-time")
            .expect("get checklist")
            .expect("checklist exists");
        assert!(!completed_on_time.tasks[0].is_late);
        assert_eq!(
            completed_on_time.tasks[0].task_status,
            ChecklistTaskStatus::Complete {}
        );
    }

    #[test]
    fn csv_template_import_can_spawn_checklist_from_template() {
        let storage_dir = test_storage_dir("checklist-template-import");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

        let template = store
            .import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
                template_uid: Some("tmpl-imported".to_string()),
                name: "Imported Preparedness".to_string(),
                description: Some("CSV import".to_string()),
                csv_text: "Task,CompletedDTG,Description,Owner,Radio Channel\nTorch,+1 hour,Portable light,Logistics,1\nRadio,+01:30,Receive alerts,Comms,7\n".to_string(),
                source_filename: Some("preparedness.csv".to_string()),
            })
            .expect("import csv template");
        assert_eq!(template.origin_type.as_str(), "CSV_IMPORT");
        assert_eq!(template.tasks.len(), 2);
        assert!(template.columns.iter().any(|column| {
            column.system_key == Some(ChecklistSystemColumnKey::DueRelativeDtg {})
                && column.column_type == ChecklistColumnType::RelativeTime {}
        }));
        assert_eq!(template.tasks[0].due_relative_minutes, Some(60));
        assert_eq!(template.tasks[1].due_relative_minutes, Some(90));
        assert_eq!(template.tasks[0].cells.len(), 4);

        store
            .create_checklist_from_template(&ChecklistCreateFromTemplateRequest {
                checklist_uid: Some("chk-imported".to_string()),
                mission_uid: Some("mission-ready".to_string()),
                template_uid: template.uid.clone(),
                name: "Imported Checklist".to_string(),
                description: "Generated from imported template".to_string(),
                start_time: "2099-04-23T12:00:00Z".to_string(),
                created_by_team_member_rns_identity: Some("alpha".to_string()),
                created_by_team_member_display_name: Some("Alpha Operator".to_string()),
            })
            .expect("create from template");

        let checklist = store
            .get_checklist("chk-imported")
            .expect("get checklist")
            .expect("checklist exists");
        assert_eq!(checklist.template_uid.as_deref(), Some("tmpl-imported"));
        assert_eq!(checklist.tasks.len(), 2);
        assert_eq!(checklist.counts.pending_count, 2);
        assert_eq!(checklist.tasks[0].due_relative_minutes, Some(60));
        assert_eq!(checklist.tasks[1].cells.len(), 4);
    }

    #[test]
    fn csv_template_import_generates_default_completed_dtg_when_missing() {
        let storage_dir = test_storage_dir("checklist-template-import-default-due");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
        store
            .set_app_settings(&app_settings_with_due_step(45))
            .expect("set app settings");

        let template = store
            .import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
                template_uid: Some("tmpl-import-default-due".to_string()),
                name: "Imported Default Due".to_string(),
                description: None,
                csv_text: "Title,Details,Owner\nStage kits,Prepare rescue kits,Alpha\nOpen channel,Start radio watch,Bravo\n".to_string(),
                source_filename: Some("default-due.csv".to_string()),
            })
            .expect("import csv template");

        assert_eq!(template.tasks[0].due_relative_minutes, Some(45));
        assert_eq!(template.tasks[1].due_relative_minutes, Some(90));
        assert_eq!(template.columns[0].column_name, "CompletedDTG");
        assert_eq!(
            template.columns[0].system_key,
            Some(ChecklistSystemColumnKey::DueRelativeDtg {})
        );
    }

    #[test]
    fn csv_template_import_rejects_invalid_completed_dtg() {
        let storage_dir = test_storage_dir("checklist-template-import-invalid-due");
        let store =
            AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

        let result = store.import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
            template_uid: Some("tmpl-import-invalid-due".to_string()),
            name: "Invalid Due".to_string(),
            description: None,
            csv_text: "Task,CompletedDTG\nOpen channel,tomorrow\n".to_string(),
            source_filename: Some("invalid-due.csv".to_string()),
        });

        assert!(matches!(result, Err(NodeError::InvalidConfig {})));
    }
}
