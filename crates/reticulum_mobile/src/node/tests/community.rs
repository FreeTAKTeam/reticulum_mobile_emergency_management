#[test]
fn community_b04_round_trip_is_canonical_and_bounded() {
    let settings = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "North Block".to_string(),
        adults: 2,
        children: 1,
        pets: 3,
        role_badges: vec!["Medic".to_string(), "Radio".to_string()],
        status: HouseholdStatus::NeedsHelp {},
        preferred_map_layer: PreferredMapLayer::Satellite {},
    };
    let body = encode_community_status_body(&settings, true, 1_700_000_000_000)
        .expect("encode community status");
    assert!(body.starts_with("MECP/2/B04 #HH_0123456789abcdef REMCS1:"));
    let parsed = parse_community_status_body(&body, 1_700_000_000_001)
        .expect("parse community status");
    assert_eq!(parsed.n, "North Block");
    assert_eq!(parsed.s, HouseholdStatus::NeedsHelp {});
    assert!(parsed.b);
}

#[test]
fn community_projection_rejects_malformed_future_stale_and_replay() {
    let settings = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "North Block".to_string(),
        ..CommunitySettingsRecord::default()
    };
    let now = 1_700_000_000_000;
    assert!(parse_community_status_body("MECP/2/B04 broken", now).is_err());
    let future = encode_community_status_body(
        &settings,
        false,
        now + COMMUNITY_MAX_FUTURE_MS + 1,
    )
    .expect("future body");
    assert!(parse_community_status_body(&future, now).is_err());
    let stale = encode_community_status_body(&settings, false, now - COMMUNITY_MAX_AGE_MS - 1)
        .expect("stale body");
    assert!(parse_community_status_body(&stale, now).is_err());

    let existing = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now - 1,
    )
    .expect("existing event");
    let replay = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now - 1,
    )
    .expect("replay event");
    assert!(!community_event_is_newer(Some(&existing), &replay, now).expect("merge"));

    let fresh = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now - 1,
    )
    .expect("fresh event");
    let stale_existing = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now - COMMUNITY_MAX_AGE_MS - 1,
    )
    .expect("stale existing event");
    assert!(community_event_is_newer(Some(&stale_existing), &fresh, now)
        .expect("fresh replaces stale"));
}

#[test]
fn community_projection_rejects_noncanonical_and_unknown_payload_fields() {
    let now = 1_700_000_000_000;
    let noncanonical = r#"{"v":1,"u":1700000000000,"s":"all_home","r":[],"p":0,"n":"North Block","h":"0123456789abcdef","c":0,"b":false,"a":1}"#;
    let unknown = r#"{"a":1,"b":false,"c":0,"gps":"44,-63","h":"0123456789abcdef","n":"North Block","p":0,"r":[],"s":"all_home","u":1700000000000,"v":1}"#;
    for payload in [noncanonical, unknown] {
        let body = format!(
            "{COMMUNITY_PREFIX} #HH_0123456789abcdef {COMMUNITY_PAYLOAD_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(payload)
        );
        assert!(parse_community_status_body(&body, now).is_err());
    }
}

#[test]
fn community_publish_coalesces_unchanged_profile_and_advances_changed_profile_time() {
    let now = 1_700_000_000_000;
    let settings = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "North Block".to_string(),
        ..CommunitySettingsRecord::default()
    };
    let existing = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now,
    )
    .expect("community event");
    assert!(community_payload_matches(&existing, &settings, false, now + 1)
        .expect("matching payload"));

    let mut changed = settings;
    changed.status = HouseholdStatus::Evacuated {};
    assert!(!community_payload_matches(&existing, &changed, false, now + 1)
        .expect("changed payload"));
    let replacement = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &changed,
        false,
        now + 1,
    )
    .expect("replacement event");
    assert!(community_event_is_newer(Some(&existing), &replacement, now + 1)
        .expect("newer replacement"));
}

#[test]
fn community_projection_uses_verified_runtime_identity_and_replays_from_storage() {
    let now = now_ms();
    let settings = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "North Block".to_string(),
        ..CommunitySettingsRecord::default()
    };
    let sender_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let verified_identity = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let event = community_event_for(sender_identity, None, &settings, false, now)
        .expect("community event");
    let target = MissionReplicationTarget {
        app_destination_hex: "cccccccccccccccccccccccccccccccc".to_string(),
        send_mode: SendMode::Auto {},
    };
    let status = Node::new().expect("node").get_status();
    let (body, fields) =
        build_event_replication_payload(&status, &event, &target).expect("wire payload");
    let received = crate::runtime::event_projection_from_fields(
        &fields,
        Some(&body),
        Some("cccccccccccccccccccccccccccccccc"),
        Some(verified_identity),
        Some("Verified neighbor"),
        now + 1,
    )
    .expect("received community projection");
    assert_eq!(received.source_identity, verified_identity);
    assert_eq!(
        received.uid,
        format!("rem-community-status-v1:{verified_identity}")
    );
    assert_eq!(received.command_type, "event.create");

    let storage = prepare_storage_dir("community-replay");
    let store = AppStateStore::new(storage.to_str()).expect("store");
    store.upsert_event(&received).expect("persist projection");
    let replayed = AppStateStore::new(storage.to_str())
        .expect("reopen store")
        .get_events()
        .expect("replay events");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].uid, received.uid);
    assert_eq!(replayed[0].content, received.content);
}

#[test]
fn typed_community_projection_never_repromotes_demoted_generic_events() {
    let storage = prepare_storage_dir("community-demoted-generic");
    let node = Node::with_storage_dir(Some(storage.to_string_lossy().as_ref())).expect("node");
    let settings = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "North Block".to_string(),
        ..CommunitySettingsRecord::default()
    };
    let mut invalid = community_event_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        &settings,
        false,
        now_ms(),
    )
    .expect("community event");
    invalid.mission_uid = "wrong-mission".to_string();
    node.upsert_event(invalid).expect("generic event remains visible");
    assert!(node
        .get_community_statuses()
        .expect("typed projection")
        .is_empty());
}
