#[test]
fn initialized_settings_reads_do_not_require_the_sqlite_writer_lock() {
    let dir = prepare_storage_dir("team-settings-reader-lock");
    let store = AppStateStore::new(dir.to_str()).unwrap();
    store.set_app_settings(&sample_app_settings()).unwrap();
    let expected = store.get_app_settings().unwrap().unwrap();
    let mut writer = rusqlite::Connection::open(dir.join("app_state.db")).unwrap();
    let transaction = writer
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    transaction
        .execute("UPDATE app_settings SET updated_at_ms = 42", [])
        .unwrap();
    assert_eq!(
        serde_json::to_value(store.get_app_settings().unwrap().unwrap()).unwrap(),
        serde_json::to_value(expected).unwrap(),
    );
}

#[test]
fn legacy_import_reinitializes_teams_after_circle_migration_completed() {
    let dir = prepare_storage_dir("team-late-legacy-import");
    let store = AppStateStore::new(dir.to_str()).unwrap();
    store.set_app_settings(&sample_app_settings()).unwrap();
    assert!(
        store
            .get_app_settings()
            .unwrap()
            .unwrap()
            .teams
            .local_teams_initialized
    );
    let mut settings = sample_app_settings();
    settings.teams.local_teams_initialized = false;
    settings.teams.local_teams.clear();
    let destination = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    store
        .import_legacy_state(&crate::types::LegacyImportPayload {
            settings: Some(settings),
            saved_peers: vec![build_saved_peer_for(destination)],
            eams: vec![],
            events: vec![],
            messages: vec![],
            telemetry_positions: vec![],
        })
        .unwrap();
    let migrated = store.get_app_settings().unwrap().unwrap();
    assert!(migrated.teams.local_teams_initialized);
    assert_eq!(
        migrated
            .teams
            .local_teams
            .iter()
            .find(|team| team.team_uid == YELLOW_TEAM_UID)
            .unwrap()
            .member_destinations,
        vec![destination.to_string()]
    );
    assert_eq!(
        serde_json::to_value(&migrated).unwrap(),
        serde_json::to_value(store.get_app_settings().unwrap().unwrap()).unwrap()
    );
}

#[test]
fn chat_correspondent_query_filters_and_deduplicates_without_loading_message_bodies() {
    let dir = prepare_storage_dir("team-chat-correspondents");
    let store = AppStateStore::new(dir.to_str()).unwrap();
    let outbound = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let inbound = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let fallback = "cccccccccccccccccccccccccccccccc";
    let hub = "dddddddddddddddddddddddddddddddd";
    let mut message = policy_chat_message(hub);
    message.requested_destination_hex = Some(outbound.to_string());
    store.upsert_message(&message).unwrap();
    message.message_id_hex = "duplicate".to_string();
    store.upsert_message(&message).unwrap();
    message.message_id_hex = "inbound".to_string();
    message.direction = MessageDirection::Inbound {};
    message.source_hex = Some(inbound.to_string());
    store.upsert_message(&message).unwrap();
    message.message_id_hex = "fallback".to_string();
    message.direction = MessageDirection::Outbound {};
    message.requested_destination_hex = None;
    message.destination_hex = fallback.to_string();
    store.upsert_message(&message).unwrap();
    message.message_id_hex = "telemetry".to_string();
    message.traffic_class = OutboundTrafficClass::Telemetry {};
    message.destination_hex = hub.to_string();
    store.upsert_message(&message).unwrap();
    let connection = rusqlite::Connection::open(dir.join("app_state.db")).unwrap();
    // Legacy messages omit traffic_class. No message deserialization/repair is needed here.
    connection.execute(
        "UPDATE messages SET json = json_remove(json, '$.traffic_class', '$.body_utf8') WHERE message_id_hex = 'fallback'",
        [],
    ).unwrap();
    let destinations = store.chat_correspondent_destinations().unwrap();
    assert_eq!(destinations.len(), 3);
    assert_eq!(
        destinations.into_iter().collect::<HashSet<_>>(),
        HashSet::from([
            outbound.to_string(),
            inbound.to_string(),
            fallback.to_string()
        ])
    );
}

#[test]
fn connected_chat_requires_caller_membership_and_the_active_roster_for_hub_forwarding() {
    let destination = "abababababababababababababababab";
    let hub = "56565656565656565656565656565656";
    let config = build_config_fingerprint_for_tests(HubMode::Connected {}, Some(hub));
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    snapshot.schema_version = HUB_DIRECTORY_SCHEMA_VERSION;
    snapshot.active_team_uid = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9".to_string();
    snapshot.members = vec![crate::types::HubTeamMemberRecord {
        team_uid: "65ce79a3a3e4b51ec0ec52d1d3d2b0b9".to_string(),
        team_member_uid: "peer".to_string(),
        identity: destination.to_string(),
        destination_hash: destination.to_string(),
        display_name: None,
        announce_capabilities: vec![],
        client_type: None,
        registered_mode: None,
        last_seen: None,
        status: None,
    }];
    for (caller_team, expected) in [
        (None, destination),
        (Some("43341e5c822d99857fa6e8641f2ca9c0"), destination),
        (Some("65ce79a3a3e4b51ec0ec52d1d3d2b0b9"), hub),
    ] {
        snapshot.caller_memberships = caller_team
            .into_iter()
            .map(|team| crate::types::HubCallerMembershipRecord {
                team_uid: team.to_string(),
                team_member_uid: "self".to_string(),
            })
            .collect();
        assert_eq!(
            routed_chat_destination_hex(
                destination.to_string(),
                Some(&config),
                Some(&snapshot),
                || Ok(vec![])
            )
            .unwrap(),
            expected
        );
    }
    snapshot.members[0].team_uid = "43341e5c822d99857fa6e8641f2ca9c0".to_string();
    assert_eq!(
        routed_chat_destination_hex(
            destination.to_string(),
            Some(&config),
            Some(&snapshot),
            || panic!("other team's roster must not be consulted")
        )
        .unwrap(),
        destination
    );
}

#[test]
fn modern_yellow_chat_does_not_treat_unscoped_directory_items_as_team_members() {
    let destination = "abababababababababababababababab";
    let config = build_config_fingerprint_for_tests(
        HubMode::Connected {},
        Some("56565656565656565656565656565656"),
    );
    let mut snapshot = HubDirectorySnapshot::yellow_only(1);
    snapshot.schema_version = HUB_DIRECTORY_SCHEMA_VERSION;
    snapshot
        .caller_memberships
        .push(crate::types::HubCallerMembershipRecord {
            team_uid: YELLOW_TEAM_UID.to_string(),
            team_member_uid: "self".to_string(),
        });
    snapshot.items.push(crate::types::HubDirectoryPeerRecord {
        identity: destination.to_string(),
        destination_hash: destination.to_string(),
        display_name: None,
        announce_capabilities: vec![],
        client_type: None,
        registered_mode: None,
        last_seen: None,
        status: None,
    });
    assert_eq!(
        routed_chat_destination_hex(
            destination.to_string(),
            Some(&config),
            Some(&snapshot),
            || panic!("unscoped item is not a team member")
        )
        .unwrap(),
        destination
    );
}

#[test]
fn block_inner_selection_uses_active_team_or_creates_yellow_fallback() {
    for active_exists in [false, true] {
        let dir = prepare_storage_dir(&format!("block-active-team-{active_exists}"));
        let store = AppStateStore::new(dir.to_str()).unwrap();
        store.set_app_settings(&sample_app_settings()).unwrap();
        let mut settings = store.get_app_settings().unwrap().unwrap();
        settings.teams.active_team_uid = "65ce79a3a3e4b51ec0ec52d1d3d2b0b9".to_string();
        settings.teams.local_teams = if active_exists {
            vec![crate::types::LocalTeamRecord {
                team_uid: "65ce79a3a3e4b51ec0ec52d1d3d2b0b9".to_string(),
                member_destinations: vec![],
            }]
        } else {
            vec![]
        };
        store.set_app_settings(&settings).unwrap();
        let now = 1_700_000_000_000;
        let identity =
            reticulum::transport::identity::PrivateIdentity::new_from_name("block-active-team");
        let encoded = signed_block_fixture(&identity, now, now + 60_000);
        let inspection = decode_and_verify_block_envelope(&encoded, now + 1).unwrap();
        let request = BlockOnboardingImportRequest {
            encoded_text: encoded,
            confirmed_signer_fingerprint: inspection.signer_fingerprint.clone(),
            community: CommunitySettingsRecord::default(),
            peer_tiers: vec![
                BlockPeerTierRecord {
                    destination_hex: inspection.issuer_app_destination_hex.clone(),
                    circle_tier: CircleTier::Inner {},
                },
                BlockPeerTierRecord {
                    destination_hex: "dddddddddddddddddddddddddddddddd".to_string(),
                    circle_tier: CircleTier::Outer {},
                },
            ],
        };
        store
            .import_block_onboarding_atomic(&inspection, &request)
            .unwrap();
        store
            .import_block_onboarding_atomic(&inspection, &request)
            .unwrap();
        let imported = store.get_app_settings().unwrap().unwrap();
        assert_eq!(
            imported.teams.active_team_uid,
            if active_exists {
                "65ce79a3a3e4b51ec0ec52d1d3d2b0b9"
            } else {
                YELLOW_TEAM_UID
            }
        );
        assert_eq!(imported.teams.local_teams.len(), 1);
        assert_eq!(
            imported.teams.local_teams[0].member_destinations,
            vec![inspection.issuer_app_destination_hex]
        );
    }
}
