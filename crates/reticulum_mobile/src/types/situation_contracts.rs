// Household situation contract: the operator picks one situation and the runtime
// derives the B04 beacon, the MECP event and the EAM update from it.

community_string_enum!(HouseholdSituation, AllOk, {
    AllOk => "all_ok",
    ShelteringInPlace => "sheltering_in_place",
    Evacuating => "evacuating",
    SomeoneMissing => "someone_missing",
    Lost => "lost",
    Stranded => "stranded",
    ThreatNearby => "threat_nearby",
    NeedSupplies => "need_supplies",
    MedicalHelpNeeded => "medical_help_needed",
    Sos => "sos"
});

community_string_enum!(HouseholdMode, ThisPhoneReports, {
    Off => "off",
    ThisPhoneReports => "this_phone_reports",
    AnotherPhoneReports => "another_phone_reports"
});

community_string_enum!(SituationMessageKind, Event, {
    CommunityStatus => "community_status",
    Event => "event",
    Eam => "eam",
    Sos => "sos"
});

community_string_enum!(SituationMessageOutcome, Queued, {
    Queued => "queued",
    HeldByPowerSaver => "held_by_power_saver",
    SkippedUnchanged => "skipped_unchanged",
    SkippedHouseholdOff => "skipped_household_off",
    SkippedAnotherPhoneReports => "skipped_another_phone_reports",
    Failed => "failed",
    WaitingForSosCountdown => "waiting_for_sos_countdown"
});

/// What happened to one of the messages a situation fans out to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SituationMessageRecord {
    pub kind: SituationMessageKind,
    pub outcome: SituationMessageOutcome,
    /// Event UID or MECP code the outcome refers to, when there is one.
    pub reference: Option<String>,
}

/// Another identity publishing B04 for this phone's household ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseholdReporterConflictRecord {
    pub source_identity: String,
    pub household_name: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseholdSituationRecord {
    /// `None` until the operator chooses a situation for the first time.
    pub situation: Option<HouseholdSituation>,
    pub detail: Option<String>,
    pub mecp_code: Option<String>,
    pub event_uid: Option<String>,
    pub sequence: u64,
    pub updated_at_ms: u64,
    pub household_mode: HouseholdMode,
    pub household_id: Option<String>,
    /// EAM factors whose current value came from the situation (for a "from situation" tag).
    pub situation_eam_factors: Vec<String>,
    /// The MECP event and EAM are stored and wait for power saver to end.
    pub held_by_power_saver: bool,
    pub duplicate_reporter: Option<HouseholdReporterConflictRecord>,
    pub messages: Vec<SituationMessageRecord>,
}
