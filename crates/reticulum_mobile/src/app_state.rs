use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fs_err as fs;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use crate::runtime::now_ms;
use crate::sos_fields::sos_kind_from_text;
use crate::types::{
    AnnounceClass, AnnounceRecord, AppSettingsRecord, ChecklistCellRecord, ChecklistColumnRecord,
    ChecklistColumnType, ChecklistCreateFromTemplateRequest, ChecklistCreateOnlineRequest,
    ChecklistMode, ChecklistOriginType, ChecklistRecord, ChecklistSyncState,
    ChecklistTaskCellSetRequest, ChecklistTaskRecord, ChecklistTaskRowAddRequest,
    ChecklistTaskRowDeleteRequest, ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus,
    ChecklistTaskStatusSetRequest, ChecklistTemplateImportCsvRequest, ChecklistTemplateRecord,
    ChecklistUpdateRequest, ChecklistUserTaskStatus, ConversationRecord, DiscoveredPluginRecord,
    EamProjectionRecord, EamReadinessMessageRecord, EamReadinessStatusMetricRecord,
    EamReadinessSummaryRecord, EamTeamSummaryRecord, EventProjectionRecord, HubDirectorySnapshot,
    InstalledPluginRecord, LegacyImportPayload, LocalTeamRecord, MessageDirection, MessageRecord,
    NodeError, PluginCapabilityRecord, PluginSensorRecord, PluginSensorSampleRequest,
    ProjectionInvalidation, ProjectionScope, SavedPeerRecord, SosAlertRecord, SosAudioRecord,
    SosLocationRecord, SosSettingsRecord, SosStatusRecord, TelemetryPositionRecord,
    TrustedPluginPublisherRecord, DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES, YELLOW_TEAM_UID,
};

fn initialize_local_team_settings(
    settings: &mut AppSettingsRecord,
    saved_destinations: impl IntoIterator<Item = String>,
) -> bool {
    if settings.teams.local_teams_initialized {
        return false;
    }
    let mut members = saved_destinations
        .into_iter()
        .map(|destination| destination.trim().to_ascii_lowercase())
        .filter(|destination| !destination.is_empty())
        .collect::<Vec<_>>();
    members.sort();
    members.dedup();
    settings.teams.local_teams = vec![LocalTeamRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        member_destinations: members,
    }];
    settings.teams.local_teams_initialized = true;
    true
}

const DEFAULT_STORAGE_DIR: &str = "reticulum-mobile";
const DB_FILE_NAME: &str = "app_state.db";
const SQLITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const EAM_STATUS_FIELDS: [(&str, &str); 6] = [
    ("securityStatus", "Security"),
    ("capabilityStatus", "Capability"),
    ("preparednessStatus", "Preparedness"),
    ("medicalStatus", "Medical"),
    ("mobilityStatus", "Mobility"),
    ("commsStatus", "Comms"),
];

fn eam_status_score(status: &str) -> u32 {
    match status {
        "Green" => 100,
        "Yellow" => 50,
        "Red" => 25,
        _ => 0,
    }
}

fn clamp_score(value: f64) -> u32 {
    crate::numeric::f64_to_u32_saturating(value.clamp(0.0, 100.0))
}

fn readiness_band(score: u32) -> &'static str {
    if score >= 75 {
        "Green"
    } else if score >= 50 {
        "Yellow"
    } else if score >= 25 {
        "Orange"
    } else {
        "Red"
    }
}

fn blend_hex_color(start: &str, end: &str, ratio: f64) -> String {
    let safe_ratio = ratio.clamp(0.0, 1.0);
    let start = parse_hex_color(start).unwrap_or([0, 0, 0]);
    let end = parse_hex_color(end).unwrap_or(start);
    let mixed = [0, 1, 2].map(|index| {
        let start_value = f64::from(start[index]);
        let end_value = f64::from(end[index]);
        crate::numeric::f64_to_u8_saturating(start_value + ((end_value - start_value) * safe_ratio))
    });
    format!("#{:02x}{:02x}{:02x}", mixed[0], mixed[1], mixed[2])
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ])
}

fn readiness_ring_color(score: u32) -> String {
    let safe_score = score.min(100);
    if safe_score >= 75 {
        blend_hex_color("#16ce79", "#3df58f", f64::from(safe_score - 75) / 25.0)
    } else if safe_score >= 50 {
        blend_hex_color("#f5cc19", "#16ce79", f64::from(safe_score - 50) / 25.0)
    } else if safe_score >= 25 {
        blend_hex_color("#ff9f1c", "#f5cc19", f64::from(safe_score - 25) / 25.0)
    } else {
        blend_hex_color("#ff3648", "#ff9f1c", f64::from(safe_score) / 25.0)
    }
}

fn eam_status_value<'a>(record: &'a EamProjectionRecord, field: &str) -> &'a str {
    match field {
        "securityStatus" => record.security_status.as_str(),
        "capabilityStatus" => record.capability_status.as_str(),
        "preparednessStatus" => record.preparedness_status.as_str(),
        "medicalStatus" => record.medical_status.as_str(),
        "mobilityStatus" => record.mobility_status.as_str(),
        "commsStatus" => record.comms_status.as_str(),
        _ => "",
    }
}

fn readiness_metric(field: &str, label: &str, score: u32) -> EamReadinessStatusMetricRecord {
    EamReadinessStatusMetricRecord {
        field: field.to_string(),
        label: label.to_string(),
        score,
        band: readiness_band(score).to_string(),
        ring_color: readiness_ring_color(score),
    }
}

fn eam_overall_readiness_score(record: &EamProjectionRecord) -> u32 {
    let total: u32 = EAM_STATUS_FIELDS
        .iter()
        .map(|(field, _)| eam_status_score(eam_status_value(record, field)))
        .sum();
    clamp_score(
        f64::from(total)
            / f64::from(crate::numeric::usize_to_u32_saturating(
                EAM_STATUS_FIELDS.len(),
            )),
    )
}

fn build_eam_readiness_summary(records: Vec<EamProjectionRecord>) -> EamReadinessSummaryRecord {
    let updated_at_ms = records
        .iter()
        .map(|record| record.updated_at_ms)
        .max()
        .unwrap_or(0);
    let active_records: Vec<EamProjectionRecord> = records
        .into_iter()
        .filter(|record| record.deleted_at_ms.is_none())
        .collect();
    let active_total = crate::numeric::usize_to_u32_saturating(active_records.len());

    let status_metrics = EAM_STATUS_FIELDS
        .iter()
        .map(|(field, label)| {
            let score = if active_records.is_empty() {
                0
            } else {
                let total: u32 = active_records
                    .iter()
                    .map(|record| eam_status_score(eam_status_value(record, field)))
                    .sum();
                clamp_score(
                    f64::from(total)
                        / f64::from(crate::numeric::usize_to_u32_saturating(
                            active_records.len(),
                        )),
                )
            };
            readiness_metric(field, label, score)
        })
        .collect();

    let messages = active_records
        .iter()
        .map(|record| {
            let score = eam_overall_readiness_score(record);
            EamReadinessMessageRecord {
                callsign: record.callsign.clone(),
                overall_score: score,
                overall_band: readiness_band(score).to_string(),
                overall_ring_color: readiness_ring_color(score),
            }
        })
        .collect();

    EamReadinessSummaryRecord {
        active_total,
        updated_at_ms,
        status_metrics,
        messages,
    }
}

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

include!("app_state/storage.rs");
include!("app_state/settings.rs");
include!("app_state/plugins.rs");
include!("app_state/hub_directory.rs");
include!("app_state/peers.rs");
include!("app_state/mission.rs");
include!("app_state/checklist_lifecycle.rs");
include!("app_state/checklist_tasks.rs");
include!("app_state/messaging.rs");
include!("app_state/telemetry_sos.rs");
include!("app_state/persistence.rs");
include!("app_state/plugin_helpers.rs");
include!("app_state/conversation_helpers.rs");
include!("app_state/checklist_csv.rs");
include!("app_state/checklist_templates.rs");
include!("app_state/checklist_normalization.rs");
include!("app_state/time.rs");

#[cfg(test)]
mod tests {
    include!("app_state/tests/support.rs");
    include!("app_state/tests/checklists.rs");
    include!("app_state/tests/plugins.rs");
    include!("app_state/tests/messages.rs");
    include!("app_state/tests/peers.rs");
    include!("app_state/tests/emergency.rs");
}
