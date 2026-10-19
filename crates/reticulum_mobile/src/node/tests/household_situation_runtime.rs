const SITUATION_NODE_IDENTITY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn situation_test_node(name: &str) -> Node {
    let storage = prepare_storage_dir(name);
    let node = Node::with_storage_dir(Some(storage.to_string_lossy().as_ref())).expect("node");
    {
        let inner = node.inner.lock().expect("inner");
        let mut status = inner.status.lock().expect("status");
        status.identity_hex = SITUATION_NODE_IDENTITY.to_string();
        status.app_destination_hex = "0123456789abcdefaaaaaaaaaaaaaaaa".to_string();
        status.name = "Alpha".to_string();
    }
    let mut settings = sample_app_settings();
    settings.community = CommunitySettingsRecord::default();
    node.set_app_settings(settings).expect("settings");
    node
}

fn situation_outcome(record: &HouseholdSituationRecord, kind: SituationMessageKind) -> SituationMessageOutcome {
    record
        .messages
        .iter()
        .find(|message| message.kind == kind)
        .map(|message| message.outcome)
        .unwrap_or_else(|| panic!("no {kind:?} outcome in {:?}", record.messages))
}

fn situation_events(node: &Node) -> Vec<EventProjectionRecord> {
    node.inner
        .lock()
        .expect("inner")
        .app_state
        .get_events()
        .expect("events")
        .into_iter()
        .filter(|event| event.uid.starts_with("rem-sit-v1:"))
        .collect()
}

fn situation_own_b04(node: &Node) -> Option<CommunityStatusProjectionRecord> {
    node.get_community_statuses()
        .expect("statuses")
        .into_iter()
        .find(|status| status.source_identity == SITUATION_NODE_IDENTITY)
}

fn situation_own_eam(node: &Node) -> EamProjectionRecord {
    node.get_eams()
        .expect("eams")
        .into_iter()
        .find(|eam| eam.callsign == "Alpha")
        .expect("own EAM")
}

#[test]
fn this_phone_reports_fans_out_b04_event_and_eam() {
    let node = situation_test_node("situation-this-phone");
    let record = node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("situation");
    assert_eq!(record.household_mode, HouseholdMode::ThisPhoneReports {});
    for kind in [SituationMessageKind::CommunityStatus {}, SituationMessageKind::Event {}, SituationMessageKind::Eam {}] {
        assert_eq!(situation_outcome(&record, kind), SituationMessageOutcome::Queued {});
    }
    assert_eq!(situation_own_b04(&node).expect("own B04").status, HouseholdStatus::OneMissing {});
    let events = situation_events(&node);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].content, "MECP/1/P07 #HH_0123456789abcdef");
    assert_eq!(events[0].uid, format!("rem-sit-v1:{SITUATION_NODE_IDENTITY}:1"));
    let eam = situation_own_eam(&node);
    assert_eq!(eam.security_status, "Red");
    assert_eq!(eam.overall_status.as_deref(), Some("Red"));
    assert_eq!(record.situation_eam_factors, vec!["security".to_string()]);

    let repeat = node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("repeat");
    assert!(repeat.messages.iter().all(|message| message.outcome == SituationMessageOutcome::SkippedUnchanged {}));
    assert_eq!(repeat.sequence, 1);
    assert_eq!(situation_events(&node).len(), 1);

    let all_ok = node.set_situation(HouseholdSituation::AllOk {}, None).expect("all ok");
    assert_eq!(all_ok.event_uid.as_deref(), Some(format!("rem-sit-v1:{SITUATION_NODE_IDENTITY}:2").as_str()));
    assert_eq!(situation_own_b04(&node).expect("own B04").status, HouseholdStatus::AllHome {});
    assert_eq!(situation_own_eam(&node).security_status, "Unknown", "restored to the value before the situation");
}

#[test]
fn household_off_sends_only_untagged_event_and_eam() {
    let node = situation_test_node("situation-off");
    node.set_household_mode(HouseholdMode::Off {}).expect("mode");
    let record = node.set_situation(HouseholdSituation::Stranded {}, None).expect("situation");
    assert_eq!(situation_outcome(&record, SituationMessageKind::CommunityStatus {}), SituationMessageOutcome::SkippedHouseholdOff {});
    assert_eq!(situation_outcome(&record, SituationMessageKind::Event {}), SituationMessageOutcome::Queued {});
    assert_eq!(record.household_id, None);
    assert_eq!(situation_events(&node)[0].content, "MECP/1/P01");
    assert_eq!(situation_own_b04(&node).expect("B04 from before").status, HouseholdStatus::AllHome {});
    assert!(node.publish_community_status().is_err(), "B04 is refused while the household is off");
    assert_eq!(situation_own_eam(&node).mobility_status, "Red");

    // Turning the household back on sends nothing until the next situation.
    let back_on = node.set_household_mode(HouseholdMode::ThisPhoneReports {}).expect("mode");
    assert!(back_on.messages.is_empty());
    assert_eq!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::AllHome {});
}

#[test]
fn another_phone_reports_tags_event_and_never_sends_b04() {
    let node = situation_test_node("situation-another-phone");
    node.set_household_mode(HouseholdMode::AnotherPhoneReports {}).expect("mode");
    let record = node.set_situation(HouseholdSituation::ThreatNearby {}, None).expect("situation");
    assert_eq!(
        situation_outcome(&record, SituationMessageKind::CommunityStatus {}),
        SituationMessageOutcome::SkippedAnotherPhoneReports {}
    );
    assert_eq!(record.household_id.as_deref(), Some("0123456789abcdef"));
    assert_eq!(situation_events(&node)[0].content, "MECP/1/X01 #HH_0123456789abcdef");
    assert_eq!(situation_own_b04(&node).expect("B04 from before").status, HouseholdStatus::AllHome {});
    assert!(node.publish_community_status().is_err());
}

#[test]
fn duplicate_reporter_warning_only_while_this_phone_reports() {
    let node = situation_test_node("situation-duplicate-reporter");
    let other = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "Harbour House".to_string(),
        ..CommunitySettingsRecord::default()
    };
    let foreign = community_event_for("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", None, &other, false, now_ms())
        .expect("foreign B04");
    node.upsert_event(foreign).expect("store foreign B04");

    let warned = node.get_household_situation().expect("situation");
    let conflict = warned.duplicate_reporter.expect("duplicate reporter");
    assert_eq!(conflict.source_identity, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(conflict.household_name, "Harbour House");

    let switched = node.set_household_mode(HouseholdMode::AnotherPhoneReports {}).expect("mode");
    assert!(switched.duplicate_reporter.is_none());
}

#[test]
fn power_saver_sends_b04_and_holds_then_releases_the_rest() {
    let node = situation_test_node("situation-saver");
    node.inner.lock().expect("inner").power_state.saver_active = true;
    let held = node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("held");
    assert_eq!(situation_outcome(&held, SituationMessageKind::CommunityStatus {}), SituationMessageOutcome::Queued {});
    assert_eq!(situation_outcome(&held, SituationMessageKind::Event {}), SituationMessageOutcome::HeldByPowerSaver {});
    assert_eq!(situation_outcome(&held, SituationMessageKind::Eam {}), SituationMessageOutcome::HeldByPowerSaver {});
    assert!(held.held_by_power_saver);
    assert_eq!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::OneMissing {});

    let replaced = node.set_situation(HouseholdSituation::Evacuating {}, Some("school".to_string())).expect("replaced");
    assert_eq!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::Evacuated {});
    assert_eq!(situation_events(&node).len(), 2, "both stay in the local timeline");

    let inner = node.inner.lock().expect("inner");
    let state = load_household_situation(&inner.app_state).expect("state");
    assert_eq!(state.held_event_uid, replaced.event_uid, "only the newest held event goes out");
    drop(inner);
    node.inner.lock().expect("inner").power_state.saver_active = false;
    let (_, sends) = held_situation_sends(&node.inner.lock().expect("inner"))
        .expect("held")
        .expect("held sends");
    assert_eq!(sends.event.expect("held event").content, "MECP/2/P02 #HH_0123456789abcdef school");
    assert_eq!(sends.eam.expect("held EAM").security_status, "Yellow");
    assert!(sends.b04.is_none());
    assert!(
        node.get_household_situation().expect("situation").held_by_power_saver,
        "reading the holds does not clear them; only an accepted send does"
    );
}

#[test]
fn hand_edited_eam_factor_survives_the_next_situation() {
    let node = situation_test_node("situation-hand-edit");
    node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("missing");
    let mut eam = situation_own_eam(&node);
    eam.security_status = "Yellow".to_string();
    eam.medical_status = "Red".to_string();
    eam.updated_at_ms += 1;
    node.upsert_eam(eam).expect("hand edit");

    let medical = node.set_situation(HouseholdSituation::MedicalHelpNeeded {}, Some("M04".to_string())).expect("medical");
    let eam = situation_own_eam(&node);
    assert_eq!(eam.security_status, "Yellow", "hand value wins over the restore");
    assert_eq!(eam.medical_status, "Red");
    let medical_event = situation_events(&node)
        .into_iter()
        .find(|event| Some(&event.uid) == medical.event_uid.as_ref())
        .expect("medical event");
    assert_eq!(medical_event.content, "MECP/0/M04 #HH_0123456789abcdef");

    node.set_situation(HouseholdSituation::AllOk {}, None).expect("all ok");
    let eam = situation_own_eam(&node);
    assert_eq!(eam.security_status, "Yellow");
    assert_eq!(eam.medical_status, "Red", "the situation did not set it, so it is not restored");
}

#[test]
fn situation_state_survives_a_restart() {
    let storage = prepare_storage_dir("situation-restart");
    let node = Node::with_storage_dir(Some(storage.to_string_lossy().as_ref())).expect("node");
    {
        let inner = node.inner.lock().expect("inner");
        inner.status.lock().expect("status").identity_hex = SITUATION_NODE_IDENTITY.to_string();
    }
    node.set_app_settings(sample_app_settings()).expect("settings");
    node.set_household_mode(HouseholdMode::AnotherPhoneReports {}).expect("mode");
    node.set_situation(HouseholdSituation::NeedSupplies {}, Some("fuel".to_string())).expect("supplies");
    drop(node);
    let reopened = Node::with_storage_dir(Some(storage.to_string_lossy().as_ref())).expect("reopen");
    let record = reopened.get_household_situation().expect("situation");
    assert_eq!(record.situation, Some(HouseholdSituation::NeedSupplies {}));
    assert_eq!(record.detail.as_deref(), Some("fuel"));
    assert_eq!(record.mecp_code.as_deref(), Some("S05"));
    assert_eq!(record.household_mode, HouseholdMode::AnotherPhoneReports {});
}

#[test]
fn sos_situation_starts_sos_and_all_ok_needs_the_cancel_pin() {
    let node = situation_test_node("situation-sos");
    let mut settings = default_sos_settings();
    settings.enabled = true;
    settings.countdown_seconds = 0;
    settings.include_location = false;
    node.set_sos_settings(settings).expect("sos settings");
    node.set_sos_pin(Some("1234".to_string())).expect("pin");
    // The SOS runtime needs a command queue; no peers are known, so nothing is sent on it.
    let (tx, _rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    node.inner.lock().expect("inner").cmd_tx = Some(tx);

    let sos = node.set_situation(HouseholdSituation::Sos {}, None).expect("sos");
    assert_eq!(situation_outcome(&sos, SituationMessageKind::Sos {}), SituationMessageOutcome::Queued {});
    assert!(sos.messages.iter().all(|message| message.kind != SituationMessageKind::Event {}), "B01 comes from the SOS runtime");
    assert!(sos_is_engaged(&node.get_sos_status().expect("sos status")));
    assert_eq!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::NeedsHelp {});
    let eam = situation_own_eam(&node);
    assert_eq!((eam.medical_status.as_str(), eam.security_status.as_str()), ("Red", "Red"));

    let again = node.set_situation(HouseholdSituation::Sos {}, None).expect("sos again");
    assert_eq!(situation_outcome(&again, SituationMessageKind::Sos {}), SituationMessageOutcome::Queued {});
    assert_eq!(again.sequence, sos.sequence);

    assert!(node.set_situation(HouseholdSituation::AllOk {}, Some("0000".to_string())).is_err());
    let unchanged = node.get_household_situation().expect("situation");
    assert_eq!(unchanged.situation, Some(HouseholdSituation::Sos {}), "a wrong PIN changes nothing");

    let all_ok = node.set_situation(HouseholdSituation::AllOk {}, Some("1234".to_string())).expect("all ok");
    assert_eq!(all_ok.detail, None, "the PIN is not stored");
    assert!(all_ok.messages.iter().any(|message| message.kind == SituationMessageKind::Sos {} && message.reference.as_deref() == Some("B03")));
    assert!(!sos_is_engaged(&node.get_sos_status().expect("sos status")));
    let eam = situation_own_eam(&node);
    assert_eq!((eam.medical_status.as_str(), eam.security_status.as_str()), ("Unknown", "Unknown"));
}

#[test]
fn set_situation_queues_sends_without_waiting_for_delivery() {
    let node = situation_test_node("situation-queued");
    let (tx, mut rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    {
        let mut inner = node.inner.lock().expect("inner");
        inner.app_state.set_saved_peers(&[build_saved_peer()]).expect("saved peer");
        *inner.peers_snapshot.lock().expect("peers") = vec![build_peer_record(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            true,
            true,
            true,
        )];
        inner.cmd_tx = Some(tx);
    }
    // Nobody answers the send commands: an awaiting path would block here.
    let started = Instant::now();
    let record = node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("situation");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(situation_outcome(&record, SituationMessageKind::CommunityStatus {}), SituationMessageOutcome::Queued {});
    let mut sent = 0;
    while let Ok(command) = rx.try_recv() {
        if matches!(command, Command::SendBytes { .. }) {
            sent += 1;
        }
    }
    assert!(sent >= 1, "at least the B04 to the saved peer is queued");
}
