#[test]
fn local_and_rch_members_with_the_same_color_merge_without_leaking_other_local_teams() {
    const RED_TEAM_UID: &str = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9";
    const LOCAL_RED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const LOCAL_YELLOW: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const RCH_RED: &str = "cccccccccccccccccccccccccccccccc";
    let status = build_status_for_tests();
    let config = build_config_fingerprint_for_tests(
        HubMode::SemiAutonomous {},
        Some("56565656565656565656565656565656"),
    );
    let peers = vec![
        build_peer_record(LOCAL_RED, "11111111111111111111111111111111", true, true, true),
        build_peer_record(LOCAL_YELLOW, "22222222222222222222222222222222", true, true, true),
        build_peer_record(RCH_RED, "33333333333333333333333333333333", false, true, true),
    ];
    let snapshot = HubDirectorySnapshot {
        schema_version: HUB_DIRECTORY_SCHEMA_VERSION,
        active_team_uid: RED_TEAM_UID.to_string(),
        caller_memberships: vec![crate::types::HubCallerMembershipRecord {
            team_uid: RED_TEAM_UID.to_string(),
            team_member_uid: "caller-red".to_string(),
        }],
        members: vec![crate::types::HubTeamMemberRecord {
            team_uid: RED_TEAM_UID.to_string(),
            team_member_uid: "rch-red".to_string(),
            identity: "44444444444444444444444444444444".to_string(),
            destination_hash: RCH_RED.to_string(),
            display_name: Some("SAR".to_string()),
            announce_capabilities: vec!["r3akt".to_string()],
            client_type: Some("rem".to_string()),
            registered_mode: Some("semi_autonomous".to_string()),
            last_seen: None,
            status: Some("active".to_string()),
        }],
        local_teams: vec![
            crate::types::LocalTeamRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                member_destinations: vec![LOCAL_YELLOW.to_string()],
            },
            crate::types::LocalTeamRecord {
                team_uid: RED_TEAM_UID.to_string(),
                member_destinations: vec![LOCAL_RED.to_string()],
            },
        ],
        ..HubDirectorySnapshot::yellow_only(123)
    };
    let targets = build_runtime_mission_replication_targets(
        &status,
        &peers,
        &[build_saved_peer_for(LOCAL_RED), build_saved_peer_for(LOCAL_YELLOW)],
        None,
        Some(&config),
        Some(&snapshot),
    )
    .expect("merged red targets");
    let destinations = targets
        .into_iter()
        .map(|target| target.app_destination_hex)
        .collect::<HashSet<_>>();

    assert_eq!(destinations.len(), 2);
    assert!(destinations.contains(LOCAL_RED));
    assert!(destinations.contains(RCH_RED));
    assert!(!destinations.contains(LOCAL_YELLOW));
}
