#[test]
fn hub_directory_keeps_membership_and_does_not_trust_remote_activity() {
    let value = MsgPackValue::Map(vec![
        (
            MsgPackValue::from("identity"),
            MsgPackValue::from("11111111111111111111111111111111"),
        ),
        (
            MsgPackValue::from("destination_hash"),
            MsgPackValue::from("22222222222222222222222222222222"),
        ),
        (
            MsgPackValue::from("announce_capabilities"),
            MsgPackValue::Array(vec![
                MsgPackValue::from("R3AKT"),
                MsgPackValue::from("EMergencyMessages"),
            ]),
        ),
        (MsgPackValue::from("client_type"), MsgPackValue::from("REM")),
        (MsgPackValue::from("status"), MsgPackValue::from("offline")),
    ]);

    let peer = parse_hub_directory_peer_record(&value).expect("valid directory member");

    assert_eq!(peer.destination_hash, "22222222222222222222222222222222");
    assert_eq!(peer.status.as_deref(), Some("offline"));
}

fn team_directory_peer(
    team_uid: Option<&str>,
    team_member_uid: Option<&str>,
    identity: &str,
    destination: &str,
) -> MsgPackValue {
    let mut entries = vec![
        (MsgPackValue::from("identity"), MsgPackValue::from(identity)),
        (
            MsgPackValue::from("destination_hash"),
            MsgPackValue::from(destination),
        ),
        (
            MsgPackValue::from("announce_capabilities"),
            MsgPackValue::Array(vec![
                MsgPackValue::from("r3akt"),
                MsgPackValue::from("emergencymessages"),
            ]),
        ),
        (MsgPackValue::from("client_type"), MsgPackValue::from("rem")),
        (MsgPackValue::from("status"), MsgPackValue::from("offline")),
    ];
    if let Some(team_uid) = team_uid {
        entries.push((MsgPackValue::from("team_uid"), MsgPackValue::from(team_uid)));
    }
    if let Some(team_member_uid) = team_member_uid {
        entries.push((
            MsgPackValue::from("team_member_uid"),
            MsgPackValue::from(team_member_uid),
        ));
    }
    MsgPackValue::Map(entries)
}

#[test]
fn v2_hub_directory_preserves_overlapping_canonical_memberships() {
    const BLUE_TEAM_UID: &str = "43341e5c822d99857fa6e8641f2ca9c0";
    let peer_identity = "11111111111111111111111111111111";
    let peer_destination = "22222222222222222222222222222222";
    let value = MsgPackValue::Map(vec![
        (
            MsgPackValue::from("scope"),
            MsgPackValue::from(HUB_TEAM_DIRECTORY_SCOPE),
        ),
        (
            MsgPackValue::from("schema_version"),
            MsgPackValue::from(HUB_DIRECTORY_SCHEMA_VERSION),
        ),
        (
            MsgPackValue::from("effective_connected_mode"),
            MsgPackValue::from(false),
        ),
        (
            MsgPackValue::from("teams"),
            MsgPackValue::Array(vec![
                MsgPackValue::Map(vec![
                    (MsgPackValue::from("uid"), MsgPackValue::from(BLUE_TEAM_UID)),
                    (MsgPackValue::from("color"), MsgPackValue::from("BLUE")),
                    (MsgPackValue::from("team_name"), MsgPackValue::from("Blue")),
                ]),
                MsgPackValue::Map(vec![
                    (MsgPackValue::from("uid"), MsgPackValue::from("custom-team")),
                    (MsgPackValue::from("color"), MsgPackValue::from("BLACK")),
                ]),
            ]),
        ),
        (
            MsgPackValue::from("caller_memberships"),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("team_uid"),
                    MsgPackValue::from(BLUE_TEAM_UID),
                ),
                (
                    MsgPackValue::from("team_member_uid"),
                    MsgPackValue::from("caller-blue"),
                ),
            ])]),
        ),
        (
            MsgPackValue::from("members"),
            MsgPackValue::Array(vec![
                team_directory_peer(
                    Some(YELLOW_TEAM_UID),
                    Some("peer-yellow"),
                    peer_identity,
                    peer_destination,
                ),
                team_directory_peer(
                    Some(BLUE_TEAM_UID),
                    Some("peer-blue"),
                    peer_identity,
                    peer_destination,
                ),
                team_directory_peer(
                    Some("custom-team"),
                    Some("peer-custom"),
                    peer_identity,
                    peer_destination,
                ),
            ]),
        ),
        (MsgPackValue::from("items"), MsgPackValue::Array(Vec::new())),
    ]);

    let snapshot = parse_hub_directory_snapshot_value(&value, 456).expect("v2 snapshot");

    assert_eq!(snapshot.schema_version, HUB_DIRECTORY_SCHEMA_VERSION);
    assert_eq!(snapshot.teams[0].uid, YELLOW_TEAM_UID);
    assert!(snapshot.teams.iter().any(|team| team.uid == BLUE_TEAM_UID));
    assert!(snapshot.teams.iter().all(|team| team.uid != "custom-team"));
    assert_eq!(snapshot.caller_memberships.len(), 1);
    assert_eq!(snapshot.members.len(), 2);
    assert_eq!(
        snapshot
            .members
            .iter()
            .filter(|member| member.destination_hash == peer_destination)
            .count(),
        2
    );
}

#[test]
fn legacy_hub_directory_maps_flat_items_to_yellow() {
    let identity = "33333333333333333333333333333333";
    let destination = "44444444444444444444444444444444";
    let value = MsgPackValue::Map(vec![
        (
            MsgPackValue::from("scope"),
            MsgPackValue::from(HUB_TEAM_DIRECTORY_SCOPE),
        ),
        (
            MsgPackValue::from("effective_connected_mode"),
            MsgPackValue::from(false),
        ),
        (
            MsgPackValue::from("items"),
            MsgPackValue::Array(vec![team_directory_peer(None, None, identity, destination)]),
        ),
    ]);

    let snapshot = parse_hub_directory_snapshot_value(&value, 789).expect("legacy snapshot");

    assert_eq!(snapshot.schema_version, 0);
    assert_eq!(snapshot.active_team_uid, YELLOW_TEAM_UID);
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.members.len(), 1);
    assert_eq!(snapshot.members[0].team_uid, YELLOW_TEAM_UID);
    assert_eq!(snapshot.members[0].destination_hash, destination);
}

#[test]
fn authoritative_refresh_rejects_a_selected_team_after_membership_disappears() {
    const BLUE_TEAM_UID: &str = "43341e5c822d99857fa6e8641f2ca9c0";
    let snapshot = HubDirectorySnapshot {
        schema_version: HUB_DIRECTORY_SCHEMA_VERSION,
        caller_memberships: vec![HubCallerMembershipRecord {
            team_uid: YELLOW_TEAM_UID.to_string(),
            team_member_uid: "caller-yellow".to_string(),
        }],
        ..HubDirectorySnapshot::yellow_only(123)
    };

    assert!(hub_directory_contains_active_team(
        &snapshot,
        YELLOW_TEAM_UID
    ));
    assert!(!hub_directory_contains_active_team(
        &snapshot,
        BLUE_TEAM_UID
    ));
}
