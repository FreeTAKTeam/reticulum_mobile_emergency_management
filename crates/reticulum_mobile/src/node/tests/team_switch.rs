#[test]
fn active_team_switch_moves_local_eam_while_transport_is_offline() {
    const BLUE_TEAM_UID: &str = "43341e5c822d99857fa6e8641f2ca9c0";
    const LOCAL_IDENTITY: &str = "11111111111111111111111111111111";
    const LOCAL_DESTINATION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let storage_dir = prepare_storage_dir("offline-team-switch");
    let storage_path = storage_dir.to_string_lossy().to_string();
    let node = Node::with_storage_dir(Some(&storage_path)).expect("node storage");

    {
        let inner = node.inner.lock().expect("node inner");
        let mut settings = sample_app_settings();
        settings.teams.active_team_uid = YELLOW_TEAM_UID.to_string();
        inner
            .app_state
            .set_app_settings(&settings)
            .expect("persist settings");
        let mut status = inner.status.lock().expect("status");
        status.identity_hex = LOCAL_IDENTITY.to_string();
        status.app_destination_hex = LOCAL_DESTINATION.to_string();
        drop(status);

        let mut snapshot = HubDirectorySnapshot::yellow_only(123);
        snapshot.schema_version = HUB_DIRECTORY_SCHEMA_VERSION;
        snapshot.teams.push(crate::types::HubTeamRecord {
            uid: BLUE_TEAM_UID.to_string(),
            color: "BLUE".to_string(),
            team_name: "Blue".to_string(),
        });
        snapshot.caller_memberships = vec![
            crate::types::HubCallerMembershipRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                team_member_uid: "caller-yellow".to_string(),
            },
            crate::types::HubCallerMembershipRecord {
                team_uid: BLUE_TEAM_UID.to_string(),
                team_member_uid: "caller-blue".to_string(),
            },
        ];
        *inner.hub_directory_snapshot.lock().expect("directory") = Some(snapshot);

        let mut eam = build_eam();
        eam.source = Some(EamSourceRecord {
            rns_identity: LOCAL_IDENTITY.to_string(),
            display_name: Some("Atlas-1".to_string()),
        });
        eam.group_name = "YELLOW".to_string();
        eam.team_uid = Some(YELLOW_TEAM_UID.to_string());
        eam.team_member_uid = Some("caller-yellow".to_string());
        inner.app_state.upsert_eam(&eam).expect("persist EAM");
    }

    node.set_active_team(BLUE_TEAM_UID.to_string())
        .expect("offline team switch");

    let settings = node
        .get_app_settings()
        .expect("settings")
        .expect("persisted settings");
    assert_eq!(settings.teams.active_team_uid, BLUE_TEAM_UID);
    let moved = node
        .get_eams()
        .expect("EAMs")
        .into_iter()
        .find(|eam| eam.callsign == "POCO" && eam.deleted_at_ms.is_none())
        .expect("active moved EAM");
    assert_eq!(moved.team_uid.as_deref(), Some(BLUE_TEAM_UID));
    assert_eq!(moved.team_member_uid.as_deref(), Some("caller-blue"));
    assert_eq!(moved.security_status, "Green");
    assert_eq!(moved.capability_status, "Yellow");

    drop(node);
    std::fs::remove_dir_all(storage_dir).expect("remove storage dir");
}
