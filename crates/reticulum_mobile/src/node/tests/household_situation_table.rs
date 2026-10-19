const SITUATION_TEST_IDENTITY: &str = "e6fcf8f02e290ed46f88a729460dfffc";

fn situation_test_eam(security: &str, mobility: &str) -> EamProjectionRecord {
    let mut eam = blank_eam("Pixel", 10);
    eam.security_status = security.to_string();
    eam.mobility_status = mobility.to_string();
    eam
}

fn situation_test_context(eam: &EamProjectionRecord, saver_active: bool, at_ms: u64) -> SituationContext<'_> {
    SituationContext {
        identity_hex: SITUATION_TEST_IDENTITY,
        node_name: "Pixel",
        household_tag: Some("0123456789abcdef"),
        current_eam: eam,
        saver_active,
        at_ms,
    }
}

#[test]
fn situation_table_covers_every_row() {
    use EamFactor::{Medical, Mobility, Preparedness, Security};
    type Row = (HouseholdSituation, HouseholdStatus, &'static str, Vec<(EamFactor, &'static str)>);
    let rows: Vec<Row> = vec![
        (HouseholdSituation::AllOk {}, HouseholdStatus::AllHome {}, "MECP/2/R07", vec![]),
        (HouseholdSituation::ShelteringInPlace {}, HouseholdStatus::AllHome {}, "MECP/2/P03", vec![(Security, "Yellow")]),
        (HouseholdSituation::Evacuating {}, HouseholdStatus::Evacuated {}, "MECP/2/P02", vec![(Security, "Yellow")]),
        (HouseholdSituation::SomeoneMissing {}, HouseholdStatus::OneMissing {}, "MECP/1/P07", vec![(Security, "Red")]),
        (HouseholdSituation::Lost {}, HouseholdStatus::NeedsHelp {}, "MECP/1/P06", vec![(Security, "Yellow")]),
        (HouseholdSituation::Stranded {}, HouseholdStatus::NeedsHelp {}, "MECP/1/P01", vec![(Mobility, "Red")]),
        (HouseholdSituation::ThreatNearby {}, HouseholdStatus::NeedsHelp {}, "MECP/1/X01", vec![(Security, "Red")]),
        (HouseholdSituation::NeedSupplies {}, HouseholdStatus::NeedsHelp {}, "MECP/1/S01", vec![(Preparedness, "Red")]),
        (HouseholdSituation::MedicalHelpNeeded {}, HouseholdStatus::NeedsHelp {}, "MECP/0/M01", vec![(Medical, "Red")]),
        (HouseholdSituation::Sos {}, HouseholdStatus::NeedsHelp {}, "MECP/0/B01", vec![(Medical, "Red"), (Security, "Red")]),
    ];
    for (situation, b04, prefix, factors) in rows {
        let spec = situation_spec(situation, None).expect("table row");
        assert_eq!(spec.b04, b04, "{situation:?}");
        assert_eq!(format!("MECP/{}/{}", spec.severity, spec.mecp_code), prefix, "{situation:?}");
        assert_eq!(spec.factors.to_vec(), factors, "{situation:?}");
        // Only SOS leaves the B01 event to the SOS runtime.
        assert_eq!(spec.publishes_event, situation != (HouseholdSituation::Sos {}));
        if spec.publishes_event {
            let event = situation_event_for(SITUATION_TEST_IDENTITY, Some("Pixel".to_string()), situation, &spec, Some("0123456789abcdef"), 4, 99)
                .expect("event");
            assert_eq!(event.content, format!("{prefix} #HH_0123456789abcdef"));
            assert_eq!(event.uid, format!("rem-sit-v1:{SITUATION_TEST_IDENTITY}:4"));
            assert_eq!(event.command_type, "mission.registry.log_entry.upsert");
            assert!(event.keywords.contains(&spec.mecp_code.to_string()));
            assert!(event.keywords.contains(&format!("rem-situation:{}", situation.as_str())));
        }
    }
}

#[test]
fn situation_follow_up_details_pick_codes_and_never_store_the_pin() {
    let supplies = situation_spec(HouseholdSituation::NeedSupplies {}, Some(" Medication ")).expect("supplies");
    assert_eq!((supplies.mecp_code, supplies.detail.as_deref()), ("S03", Some("medication")));
    let by_code = situation_spec(HouseholdSituation::NeedSupplies {}, Some("s06")).expect("supply code");
    assert_eq!((by_code.mecp_code, by_code.detail.as_deref()), ("S06", Some("tools")));
    assert!(situation_spec(HouseholdSituation::NeedSupplies {}, Some("pizza")).is_err());

    let medical = situation_spec(HouseholdSituation::MedicalHelpNeeded {}, Some("m06")).expect("medical");
    assert_eq!((medical.mecp_code, medical.detail.as_deref()), ("M06", Some("M06")));
    assert!(situation_spec(HouseholdSituation::MedicalHelpNeeded {}, Some("M10")).is_err());

    let evacuating = situation_spec(HouseholdSituation::Evacuating {}, Some("  north   school ")).expect("evacuating");
    let event = situation_event_for(SITUATION_TEST_IDENTITY, None, HouseholdSituation::Evacuating {}, &evacuating, None, 1, 1)
        .expect("event");
    assert_eq!(event.content, "MECP/2/P02 north school", "no household tag without a household");

    let all_ok = situation_spec(HouseholdSituation::AllOk {}, Some("1234")).expect("all ok");
    assert_eq!(all_ok.detail, None, "the SOS PIN is never stored");
    assert_eq!(all_ok.content_detail, None);
}

#[test]
fn situation_factors_restore_on_change_and_respect_hand_edits() {
    let missing = situation_spec(HouseholdSituation::SomeoneMissing {}, None).expect("missing");
    let sheltering = situation_spec(HouseholdSituation::ShelteringInPlace {}, None).expect("sheltering");
    let all_ok = situation_spec(HouseholdSituation::AllOk {}, None).expect("all ok");

    // Missing -> Sheltering -> All OK walks security Red -> Yellow -> back to Green.
    let mut eam = situation_test_eam("Green", "Green");
    let owned = apply_situation_eam_factors(&mut eam, &[], missing.factors);
    assert_eq!(eam.security_status, "Red");
    let owned = apply_situation_eam_factors(&mut eam, &owned, sheltering.factors);
    assert_eq!(eam.security_status, "Yellow");
    assert_eq!(owned[0].previous, "Green");
    let owned = apply_situation_eam_factors(&mut eam, &owned, all_ok.factors);
    assert_eq!(eam.security_status, "Green");
    assert!(owned.is_empty());

    // A hand edit after the situation set the factor wins and is not restored.
    let mut eam = situation_test_eam("Green", "Green");
    let owned = apply_situation_eam_factors(&mut eam, &[], missing.factors);
    eam.security_status = "Yellow".to_string();
    apply_situation_eam_factors(&mut eam, &owned, all_ok.factors);
    assert_eq!(eam.security_status, "Yellow");

    // A situation never lowers a factor the operator set Red by hand.
    let mut eam = situation_test_eam("Red", "Green");
    let owned = apply_situation_eam_factors(&mut eam, &[], sheltering.factors);
    assert_eq!(eam.security_status, "Red");
    assert!(owned.is_empty(), "nothing to restore later");
    apply_situation_eam_factors(&mut eam, &owned, all_ok.factors);
    assert_eq!(eam.security_status, "Red");

    // Mobility is only touched by Stranded; other factors stay as they were.
    let stranded = situation_spec(HouseholdSituation::Stranded {}, None).expect("stranded");
    let mut eam = situation_test_eam("Unknown", "Green");
    apply_situation_eam_factors(&mut eam, &[], stranded.factors);
    assert_eq!((eam.security_status.as_str(), eam.mobility_status.as_str()), ("Unknown", "Red"));
}

#[test]
fn situation_plan_skips_repeats_except_sos() {
    let eam = situation_test_eam("Green", "Green");
    let missing = situation_spec(HouseholdSituation::SomeoneMissing {}, None).expect("missing");
    let first = plan_situation(&StoredHouseholdSituation::default(), HouseholdSituation::SomeoneMissing {}, &missing, &situation_test_context(&eam, false, 100))
        .expect("plan")
        .expect("changed");
    assert_eq!(first.state.sequence, 1);
    assert!(first.send_event && first.send_eam);
    let repeat = plan_situation(&first.state, HouseholdSituation::SomeoneMissing {}, &missing, &situation_test_context(first.eam.as_ref().expect("eam"), false, 200))
        .expect("plan");
    assert!(repeat.is_none(), "choosing the current situation again sends nothing new");

    let sos = situation_spec(HouseholdSituation::Sos {}, None).expect("sos");
    let sos_plan = plan_situation(&first.state, HouseholdSituation::Sos {}, &sos, &situation_test_context(&eam, false, 300))
        .expect("plan")
        .expect("sos plan");
    assert!(sos_plan.event.is_none(), "SOS keeps its own B01 event");
    let again = plan_situation(&sos_plan.state, HouseholdSituation::Sos {}, &sos, &situation_test_context(sos_plan.eam.as_ref().expect("eam"), false, 400))
        .expect("plan")
        .expect("SOS re-broadcasts");
    assert_eq!(again.state.sequence, sos_plan.state.sequence);
    assert!(again.eam.is_none());
}

#[test]
fn situation_saver_holds_event_and_eam_and_newer_situation_replaces_them() {
    let eam = situation_test_eam("Green", "Green");
    let missing = situation_spec(HouseholdSituation::SomeoneMissing {}, None).expect("missing");
    let held = plan_situation(&StoredHouseholdSituation::default(), HouseholdSituation::SomeoneMissing {}, &missing, &situation_test_context(&eam, true, 100))
        .expect("plan")
        .expect("changed");
    assert!(!held.send_event && !held.send_eam);
    assert_eq!(held.state.held_event_uid.as_deref(), Some(format!("rem-sit-v1:{SITUATION_TEST_IDENTITY}:1").as_str()));
    assert!(held.state.held_eam);
    assert!(held.messages.iter().all(|message| message.outcome == SituationMessageOutcome::HeldByPowerSaver {}));

    let lost = situation_spec(HouseholdSituation::Lost {}, None).expect("lost");
    let replaced = plan_situation(&held.state, HouseholdSituation::Lost {}, &lost, &situation_test_context(held.eam.as_ref().expect("eam"), true, 200))
        .expect("plan")
        .expect("changed");
    assert_eq!(replaced.state.held_event_uid.as_deref(), Some(format!("rem-sit-v1:{SITUATION_TEST_IDENTITY}:2").as_str()));
    assert!(replaced.state.held_eam);

    // Saver off: the next change sends now and clears anything still held.
    let all_ok = situation_spec(HouseholdSituation::AllOk {}, None).expect("all ok");
    let sent = plan_situation(&replaced.state, HouseholdSituation::AllOk {}, &all_ok, &situation_test_context(replaced.eam.as_ref().expect("eam"), false, 300))
        .expect("plan")
        .expect("changed");
    assert!(sent.send_event && sent.send_eam);
    assert!(!sent.state.is_held());
}

#[test]
fn duplicate_reporter_ignores_own_identity_and_other_households() {
    let status = |household_id: &str, source_identity: &str, updated_at_ms: u64| CommunityStatusProjectionRecord {
        household_id: household_id.to_string(),
        household_name: "Harbour House".to_string(),
        adults: 1,
        children: 0,
        pets: 0,
        role_badges: Vec::new(),
        status: HouseholdStatus::AllHome {},
        saver_active: false,
        updated_at_ms,
        source_identity: source_identity.to_string(),
    };
    let statuses = vec![
        status("0123456789abcdef", SITUATION_TEST_IDENTITY, 9),
        status("fedcba9876543210", "bb", 8),
        status("0123456789abcdef", "cc", 5),
        status("0123456789abcdef", "dd", 7),
    ];
    let conflict = duplicate_household_reporter(&statuses, "0123456789abcdef", SITUATION_TEST_IDENTITY).expect("conflict");
    assert_eq!(conflict.source_identity, "dd");
    assert!(duplicate_household_reporter(&statuses[..2], "0123456789abcdef", SITUATION_TEST_IDENTITY).is_none());
}
