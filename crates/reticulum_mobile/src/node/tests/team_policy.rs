fn policy_peer(destination: &str) -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: destination.to_string(),
        circle_tier: CircleTier::Outer {},
        ..sample_saved_peer()
    }
}

fn set_test_team(node: &Node, destinations: Vec<String>) {
    let mut settings = node
        .get_app_settings()
        .expect("settings")
        .unwrap_or_else(sample_app_settings);
    settings.teams.local_teams_initialized = true;
    settings.teams.local_teams = vec![crate::types::LocalTeamRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        member_destinations: destinations,
    }];
    node.set_app_settings(settings).expect("set team");
}

#[test]
fn derived_circle_tier_reflects_local_team_membership_and_ignores_writes() {
    let dir = prepare_storage_dir("derived-team-tier");
    let node = Node::with_storage_dir(dir.to_str()).expect("node");
    let mut peer = policy_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    peer.identity_hex = Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());
    peer.lxmf_destination_hex = Some("cccccccccccccccccccccccccccccccc".to_string());
    node.set_saved_peers(vec![peer.clone()])
        .expect("save contact");
    for alias in [
        &peer.destination_hex,
        peer.identity_hex.as_ref().unwrap(),
        peer.lxmf_destination_hex.as_ref().unwrap(),
    ] {
        set_test_team(&node, vec![format!(" {} ", alias.to_uppercase())]);
        assert_eq!(
            node.get_saved_peers().unwrap()[0].circle_tier,
            CircleTier::Inner {}
        );
        node.set_saved_peers(vec![peer.clone()])
            .expect("ignored outer write");
        assert_eq!(
            node.get_saved_peers().unwrap()[0].circle_tier,
            CircleTier::Inner {}
        );
    }
    set_test_team(&node, vec![]);
    peer.circle_tier = CircleTier::Inner {};
    node.set_saved_peers(vec![peer])
        .expect("ignored inner write");
    assert_eq!(
        node.get_saved_peers().unwrap()[0].circle_tier,
        CircleTier::Outer {}
    );
}

#[test]
fn inner_peers_outside_teams_migrate_to_yellow_once() {
    for initialized in [false, true] {
        let dir = prepare_storage_dir(if initialized {
            "circle-migration-new"
        } else {
            "circle-migration-old"
        });
        let store = AppStateStore::new(dir.to_str()).expect("store");
        let mut settings = sample_app_settings();
        settings.teams.local_teams_initialized = initialized;
        settings.teams.local_teams = vec![crate::types::LocalTeamRecord {
            team_uid: "team-red".to_string(),
            member_destinations: vec!["cccccccccccccccccccccccccccccccc".to_string()],
        }];
        store.set_app_settings(&settings).unwrap();
        let mut inner = policy_peer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        inner.circle_tier = CircleTier::Inner {};
        let outer = policy_peer("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let mut member = policy_peer("dddddddddddddddddddddddddddddddd");
        member.circle_tier = CircleTier::Inner {};
        member.identity_hex = Some("cccccccccccccccccccccccccccccccc".to_string());
        store.set_saved_peers(&[inner, outer, member]).unwrap();
        let migrated = store.get_app_settings().unwrap().unwrap();
        let yellow = migrated
            .teams
            .local_teams
            .iter()
            .find(|team| team.team_uid == YELLOW_TEAM_UID)
            .unwrap();
        assert_eq!(
            yellow.member_destinations,
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
        );
        assert_eq!(
            serde_json::to_value(&migrated).unwrap(),
            serde_json::to_value(store.get_app_settings().unwrap()).unwrap()
        );
        let mut removed = migrated;
        removed
            .teams
            .local_teams
            .iter_mut()
            .for_each(|team| team.member_destinations.clear());
        store.set_app_settings(&removed).unwrap();
        drop(store);
        let reopened = AppStateStore::new(dir.to_str()).unwrap();
        assert!(reopened
            .get_app_settings()
            .unwrap()
            .unwrap()
            .teams
            .local_teams
            .iter()
            .all(|team| team.member_destinations.is_empty()));
        assert!(reopened
            .get_saved_peers()
            .unwrap()
            .iter()
            .all(|peer| peer.circle_tier == CircleTier::Outer {}));
    }
}

#[test]
fn event_and_telemetry_targets_follow_active_team_without_tier_gate() {
    let destination = "abababababababababababababababab";
    let peer = build_peer_record(
        destination,
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        true,
        true,
        true,
    );
    let saved = policy_peer(destination);
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    for members in [vec![], vec![destination.to_string()]] {
        snapshot.local_teams = vec![crate::types::LocalTeamRecord {
            team_uid: YELLOW_TEAM_UID.to_string(),
            member_destinations: members.clone(),
        }];
        let targets = build_runtime_event_replication_targets(
            &build_status_for_tests(),
            &[peer.clone()],
            &[saved.clone()],
            None,
            None,
            Some(&snapshot),
        )
        .unwrap();
        assert_eq!(targets.len(), members.len());
        let telemetry = build_runtime_telemetry_destinations(
            &build_status_for_tests(),
            &[peer.clone()],
            None,
            None,
            Some(&snapshot),
        )
        .unwrap();
        assert_eq!(telemetry.len(), members.len());
    }
    assert!(build_runtime_event_replication_targets(
        &build_status_for_tests(),
        &[peer],
        &[saved],
        None,
        None,
        None
    )
    .unwrap()
    .is_empty());
}

#[test]
fn telemetry_never_sends_exact_position_through_hub() {
    let hub = "56565656565656565656565656565656";
    let contact = "abababababababababababababababab";
    let config = build_config_fingerprint_for_tests(HubMode::Connected {}, Some(hub));
    let peers = vec![
        build_peer_record(contact, contact, true, true, true),
        build_peer_record(hub, hub, true, true, true),
    ];
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    snapshot.local_teams = vec![crate::types::LocalTeamRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        member_destinations: vec![contact.to_string(), hub.to_string()],
    }];
    let targets = build_runtime_telemetry_destinations(
        &build_status_for_tests(),
        &peers,
        None,
        Some(&config),
        Some(&snapshot),
    )
    .unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].app_destination_hex, contact);
}

fn test_team_snapshot(destinations: &[&str]) -> HubDirectorySnapshot {
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    snapshot.local_teams = vec![crate::types::LocalTeamRecord {
        team_uid: YELLOW_TEAM_UID.to_string(),
        member_destinations: destinations.iter().map(|value| value.to_string()).collect(),
    }];
    snapshot
}
