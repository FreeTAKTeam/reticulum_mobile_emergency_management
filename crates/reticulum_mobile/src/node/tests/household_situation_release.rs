fn situation_release_node(storage: &Path) -> Node {
    let node = Node::with_storage_dir(Some(storage.to_string_lossy().as_ref())).expect("node");
    let inner = node.inner.lock().expect("inner");
    {
        let mut status = inner.status.lock().expect("status");
        status.identity_hex = SITUATION_NODE_IDENTITY.to_string();
        status.app_destination_hex = "0123456789abcdefaaaaaaaaaaaaaaaa".to_string();
        status.name = "Alpha".to_string();
    }
    *inner.peers_snapshot.lock().expect("peers") = vec![build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        true,
        true,
        true,
    )];
    // Event and EAM targets are the active local team's members.
    *inner.hub_directory_snapshot.lock().expect("directory") = Some(HubDirectorySnapshot {
        local_teams: vec![crate::types::LocalTeamRecord {
            team_uid: YELLOW_TEAM_UID.to_string(),
            member_destinations: vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()],
        }],
        ..HubDirectorySnapshot::yellow_only(123)
    });
    drop(inner);
    node
}

fn situation_send_count(rx: &mut mpsc::Receiver<Command>) -> usize {
    let mut sent = 0;
    while let Ok(command) = rx.try_recv() {
        if matches!(command, Command::SendBytes { .. }) {
            sent += 1;
        }
    }
    sent
}

#[test]
fn held_situation_survives_restart_and_is_released_after_an_accepted_send() {
    let storage = prepare_storage_dir("situation-release-restart");
    {
        let node = situation_release_node(&storage);
        let mut settings = sample_app_settings();
        settings.community = CommunitySettingsRecord::default();
        node.set_app_settings(settings).expect("settings");
        node.inner.lock().expect("inner").app_state.set_saved_peers(&[build_saved_peer()]).expect("saved peer");
        node.inner.lock().expect("inner").power_state.saver_active = true;
        let held = node.set_situation(HouseholdSituation::SomeoneMissing {}, None).expect("held");
        assert!(held.held_by_power_saver);
    }

    // Restart outside saver: no saver -> normal transition ever happens in memory.
    let node = situation_release_node(&storage);
    assert!(node.get_household_situation().expect("situation").held_by_power_saver);

    // A refused enqueue keeps the holds for the next update.
    let (closed_tx, closed_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    drop(closed_rx);
    node.inner.lock().expect("inner").cmd_tx = Some(closed_tx);
    node.update_battery_state(80, false).expect("battery");
    let still_held = node.get_household_situation().expect("situation");
    assert!(still_held.held_by_power_saver, "nothing was accepted, so nothing is cleared");

    // The next battery update outside saver releases them through the queue.
    let (tx, mut rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    node.inner.lock().expect("inner").cmd_tx = Some(tx);
    node.update_battery_state(79, false).expect("battery");
    assert!(situation_send_count(&mut rx) >= 1, "held event and EAM were queued");
    assert!(!node.get_household_situation().expect("situation").held_by_power_saver);
}

#[test]
fn sos_countdown_defers_the_situation_fan_out() {
    let node = situation_test_node("situation-sos-countdown");
    let mut settings = default_sos_settings();
    settings.enabled = true;
    settings.countdown_seconds = 30;
    settings.include_location = false;
    node.set_sos_settings(settings).expect("sos settings");
    node.set_sos_pin(Some("1234".to_string())).expect("pin");
    let (tx, _rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    node.inner.lock().expect("inner").cmd_tx = Some(tx);

    let waiting = node.set_situation(HouseholdSituation::Sos {}, None).expect("sos countdown");
    assert!(matches!(node.get_sos_status().expect("sos").state, SosState::Countdown {}));
    assert!(waiting
        .messages
        .iter()
        .all(|message| message.outcome == SituationMessageOutcome::WaitingForSosCountdown {}));
    assert_eq!(waiting.situation, None, "nothing is stored during the countdown");
    assert_eq!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::AllHome {});
    assert!(node.get_eams().expect("eams").is_empty(), "no red EAM was written");

    // Cancelling the countdown with the PIN leaves nothing to roll back.
    node.set_situation(HouseholdSituation::AllOk {}, Some("1234".to_string())).expect("cancel");
    assert!(!sos_is_engaged(&node.get_sos_status().expect("sos")));
    assert_ne!(situation_own_b04(&node).expect("B04").status, HouseholdStatus::NeedsHelp {});
    let eams = node.get_eams().expect("eams");
    assert!(eams.iter().all(|eam| eam.medical_status != "Red" && eam.security_status != "Red"));
}
