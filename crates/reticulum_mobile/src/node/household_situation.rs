// "Choose one situation, send everything" (docs/plans/household-situation.md).
// This module is the single owner of the situation table: which B04 status, MECP
// event and EAM factors each situation implies. Wire formats stay unchanged; the
// runtime sends three honest messages through their existing contracts.

const SITUATION_EVENT_TYPE_PREFIX: &str = "r3akt:event-type:";
const SITUATION_KEYWORD_PREFIX: &str = "rem-situation:";
const SITUATION_DETAIL_MAX_CHARS: usize = 80;
const EAM_RED: &str = "Red";
const EAM_YELLOW: &str = "Yellow";
const EAM_UNKNOWN: &str = "Unknown";

const SUPPLY_CODES: [(&str, &str); 6] = [
    ("water", "S01"),
    ("food", "S02"),
    ("medication", "S03"),
    ("power", "S04"),
    ("fuel", "S05"),
    ("tools", "S06"),
];
const MEDICAL_CODES: [&str; 9] = ["M01", "M02", "M03", "M04", "M05", "M06", "M07", "M08", "M09"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EamFactor {
    Security,
    Capability,
    Preparedness,
    Medical,
    Mobility,
    Comms,
}

impl EamFactor {
    fn name(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Capability => "capability",
            Self::Preparedness => "preparedness",
            Self::Medical => "medical",
            Self::Mobility => "mobility",
            Self::Comms => "comms",
        }
    }

    fn slot(self, eam: &mut EamProjectionRecord) -> &mut String {
        match self {
            Self::Security => &mut eam.security_status,
            Self::Capability => &mut eam.capability_status,
            Self::Preparedness => &mut eam.preparedness_status,
            Self::Medical => &mut eam.medical_status,
            Self::Mobility => &mut eam.mobility_status,
            Self::Comms => &mut eam.comms_status,
        }
    }
}

/// The complete fan-out of one situation (plan section 3).
#[derive(Debug, Clone, PartialEq, Eq)]
struct SituationSpec {
    b04: HouseholdStatus,
    severity: u8,
    mecp_code: &'static str,
    /// False for SOS: the SOS runtime publishes its own B01 event.
    publishes_event: bool,
    factors: &'static [(EamFactor, &'static str)],
    /// Detail kept with the situation (supply kind, medical code, destination).
    detail: Option<String>,
    /// Free text appended to the MECP event (the evacuation destination).
    content_detail: Option<String>,
}

fn normalized_situation_detail(detail: Option<&str>) -> Option<String> {
    let text = detail?.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.chars().take(SITUATION_DETAIL_MAX_CHARS).collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn supply_code(detail: Option<&str>) -> Result<(&'static str, Option<String>), NodeError> {
    let Some(detail) = normalized_situation_detail(detail) else {
        return Ok(("S01", None));
    };
    let wanted = detail.to_ascii_lowercase();
    SUPPLY_CODES
        .iter()
        .find(|(name, code)| *name == wanted || code.eq_ignore_ascii_case(&wanted))
        .map(|(name, code)| (*code, Some((*name).to_string())))
        .ok_or_else(|| {
            crate::error_context::invalid_config(
                "supplies must be one of water, food, medication, power, fuel or tools",
            )
        })
}

fn medical_code(detail: Option<&str>) -> Result<(&'static str, Option<String>), NodeError> {
    let Some(detail) = normalized_situation_detail(detail) else {
        return Ok(("M01", None));
    };
    MEDICAL_CODES
        .iter()
        .find(|code| code.eq_ignore_ascii_case(&detail))
        .map(|code| (*code, Some((*code).to_string())))
        .ok_or_else(|| crate::error_context::invalid_config("medical detail must be an MECP code M01 to M09"))
}

/// The situation table. `detail` is the optional follow-up: supply kind, medical
/// code or evacuation destination; for *All OK* it is the SOS PIN and never stored.
fn situation_spec(
    situation: HouseholdSituation,
    detail: Option<&str>,
) -> Result<SituationSpec, NodeError> {
    use EamFactor::{Medical, Mobility, Preparedness, Security};
    let spec = |b04, severity, mecp_code, factors| SituationSpec {
        b04,
        severity,
        mecp_code,
        publishes_event: true,
        factors,
        detail: None,
        content_detail: None,
    };
    Ok(match situation {
        HouseholdSituation::AllOk {} => spec(HouseholdStatus::AllHome {}, 2, "R07", &[]),
        HouseholdSituation::ShelteringInPlace {} => {
            spec(HouseholdStatus::AllHome {}, 2, "P03", &[(Security, EAM_YELLOW)])
        }
        HouseholdSituation::Evacuating {} => {
            let destination = normalized_situation_detail(detail);
            SituationSpec {
                detail: destination.clone(),
                content_detail: destination,
                ..spec(HouseholdStatus::Evacuated {}, 2, "P02", &[(Security, EAM_YELLOW)])
            }
        }
        HouseholdSituation::SomeoneMissing {} => {
            spec(HouseholdStatus::OneMissing {}, 1, "P07", &[(Security, EAM_RED)])
        }
        HouseholdSituation::Lost {} => {
            spec(HouseholdStatus::NeedsHelp {}, 1, "P06", &[(Security, EAM_YELLOW)])
        }
        HouseholdSituation::Stranded {} => {
            spec(HouseholdStatus::NeedsHelp {}, 1, "P01", &[(Mobility, EAM_RED)])
        }
        HouseholdSituation::ThreatNearby {} => {
            spec(HouseholdStatus::NeedsHelp {}, 1, "X01", &[(Security, EAM_RED)])
        }
        HouseholdSituation::NeedSupplies {} => {
            let (code, kind) = supply_code(detail)?;
            SituationSpec {
                detail: kind,
                ..spec(HouseholdStatus::NeedsHelp {}, 1, code, &[(Preparedness, EAM_RED)])
            }
        }
        HouseholdSituation::MedicalHelpNeeded {} => {
            let (code, chosen) = medical_code(detail)?;
            SituationSpec {
                detail: chosen,
                ..spec(HouseholdStatus::NeedsHelp {}, 0, code, &[(Medical, EAM_RED)])
            }
        }
        HouseholdSituation::Sos {} => SituationSpec {
            publishes_event: false,
            ..spec(
                HouseholdStatus::NeedsHelp {},
                0,
                "B01",
                &[(Medical, EAM_RED), (Security, EAM_RED)],
            )
        },
    })
}

fn situation_event_for(
    source_identity: &str,
    source_name: Option<String>,
    situation: HouseholdSituation,
    spec: &SituationSpec,
    household_id: Option<&str>,
    sequence: u64,
    at_ms: u64,
) -> Result<EventProjectionRecord, NodeError> {
    let source_identity = normalize_hex_32(source_identity).ok_or(NodeError::InvalidConfig {})?;
    let code = spec.mecp_code;
    let mut content = format!("MECP/{}/{code}", spec.severity);
    if let Some(household_id) = household_id {
        content.push_str(&format!(" #HH_{household_id}"));
    }
    if let Some(text) = spec.content_detail.as_deref() {
        content.push(' ');
        content.push_str(text);
    }
    Ok(EventProjectionRecord {
        uid: format!("rem-sit-v1:{source_identity}:{sequence}"),
        command_id: format!("situation-{sequence}-{at_ms}"),
        source_identity,
        callsign: source_name.clone().unwrap_or_default(),
        source_display_name: source_name,
        timestamp: at_ms.to_string(),
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: DEFAULT_R3AKT_MISSION_UID.to_string(),
        content,
        server_time: None,
        client_time: None,
        keywords: vec![
            format!("{SITUATION_EVENT_TYPE_PREFIX}{}", &code[..1]),
            code.to_string(),
            format!("{SITUATION_KEYWORD_PREFIX}{}", situation.as_str()),
        ],
        content_hashes: Vec::new(),
        updated_at_ms: at_ms,
        deleted_at_ms: None,
        correlation_id: household_id.map(str::to_string),
        topics: vec![DEFAULT_R3AKT_MISSION_UID.to_string()],
    })
}

/// One EAM factor the current situation set, and the value it replaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AppliedEamFactor {
    factor: EamFactor,
    previous: String,
    applied: String,
}

/// Restores what the previous situation set (unless edited by hand since), then
/// applies the new situation's factors without ever lowering one.
/// Returns the factors the new situation now owns.
fn apply_situation_eam_factors(
    eam: &mut EamProjectionRecord,
    previously_applied: &[AppliedEamFactor],
    factors: &[(EamFactor, &str)],
) -> Vec<AppliedEamFactor> {
    for applied in previously_applied {
        let slot = applied.factor.slot(eam);
        // A different value means the operator edited it by hand: the hand value wins.
        if *slot == applied.applied {
            slot.clone_from(&applied.previous);
        }
    }
    let mut owned = Vec::new();
    for (factor, value) in factors {
        let slot = factor.slot(eam);
        if eam_status_rank(slot) >= eam_status_rank(value) {
            continue;
        }
        owned.push(AppliedEamFactor {
            factor: *factor,
            previous: slot.clone(),
            applied: (*value).to_string(),
        });
        *slot = (*value).to_string();
    }
    owned
}

fn eam_factors_equal(left: &EamProjectionRecord, right: &EamProjectionRecord) -> bool {
    left.security_status == right.security_status
        && left.capability_status == right.capability_status
        && left.preparedness_status == right.preparedness_status
        && left.medical_status == right.medical_status
        && left.mobility_status == right.mobility_status
        && left.comms_status == right.comms_status
}

fn blank_eam(callsign: &str, at_ms: u64) -> EamProjectionRecord {
    let unknown = || EAM_UNKNOWN.to_string();
    EamProjectionRecord {
        callsign: callsign.to_string(),
        group_name: String::new(),
        security_status: unknown(),
        capability_status: unknown(),
        preparedness_status: unknown(),
        medical_status: unknown(),
        mobility_status: unknown(),
        comms_status: unknown(),
        notes: None,
        updated_at_ms: at_ms,
        deleted_at_ms: None,
        eam_uid: None,
        team_member_uid: None,
        team_uid: None,
        reported_at: None,
        reported_by: None,
        overall_status: None,
        confidence: None,
        ttl_seconds: None,
        source: None,
        sync_state: None,
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: None,
    }
}

/// Persisted situation state (app-state metadata, JSON).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct StoredHouseholdSituation {
    situation: Option<HouseholdSituation>,
    detail: Option<String>,
    mecp_code: Option<String>,
    event_uid: Option<String>,
    sequence: u64,
    updated_at_ms: u64,
    /// `None` until the operator picks a mode; resolves to *This phone reports*.
    household_mode: Option<HouseholdMode>,
    applied_eam_factors: Vec<AppliedEamFactor>,
    eam_callsign: Option<String>,
    /// Newest situation event waiting for power saver to end. A newer situation
    /// replaces it, so the team never receives an outdated sequence.
    held_event_uid: Option<String>,
    held_eam: bool,
}

impl StoredHouseholdSituation {
    fn mode(&self) -> HouseholdMode {
        self.household_mode.unwrap_or_default()
    }

    fn is_held(&self) -> bool {
        self.held_event_uid.is_some() || self.held_eam
    }
}

fn load_household_situation(store: &AppStateStore) -> Result<StoredHouseholdSituation, NodeError> {
    store
        .household_situation_json()?
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn household_situation_json(state: &StoredHouseholdSituation) -> Result<String, NodeError> {
    serde_json::to_string(state).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
    })
}

/// Only one phone per household sends B04: the one set to *This phone reports*.
fn ensure_this_phone_reports(store: &AppStateStore) -> Result<(), NodeError> {
    match load_household_situation(store)?.mode() {
        HouseholdMode::ThisPhoneReports {} => Ok(()),
        HouseholdMode::Off {} | HouseholdMode::AnotherPhoneReports {} => Err(
            crate::error_context::invalid_config(
                "household B04 is sent only when this phone reports for the household",
            ),
        ),
    }
}

fn situation_message(
    kind: SituationMessageKind,
    outcome: SituationMessageOutcome,
    reference: Option<String>,
) -> SituationMessageRecord {
    SituationMessageRecord { kind, outcome, reference }
}

/// Newest B04 from another identity that claims this phone's household ID.
fn duplicate_household_reporter(
    statuses: &[CommunityStatusProjectionRecord],
    household_id: &str,
    own_identity: &str,
) -> Option<HouseholdReporterConflictRecord> {
    statuses
        .iter()
        .filter(|status| {
            status.household_id.eq_ignore_ascii_case(household_id)
                && !status.source_identity.eq_ignore_ascii_case(own_identity)
        })
        .max_by_key(|status| status.updated_at_ms)
        .map(|status| HouseholdReporterConflictRecord {
            source_identity: status.source_identity.clone(),
            household_name: status.household_name.clone(),
            updated_at_ms: status.updated_at_ms,
        })
}

fn household_situation_record(
    state: &StoredHouseholdSituation,
    household_id: Option<String>,
    duplicate_reporter: Option<HouseholdReporterConflictRecord>,
    messages: Vec<SituationMessageRecord>,
) -> HouseholdSituationRecord {
    HouseholdSituationRecord {
        situation: state.situation,
        detail: state.detail.clone(),
        mecp_code: state.mecp_code.clone(),
        event_uid: state.event_uid.clone(),
        sequence: state.sequence,
        updated_at_ms: state.updated_at_ms,
        household_mode: state.mode(),
        household_id,
        situation_eam_factors: state
            .applied_eam_factors
            .iter()
            .map(|applied| applied.factor.name().to_string())
            .collect(),
        held_by_power_saver: state.is_held(),
        duplicate_reporter,
        messages,
    }
}
