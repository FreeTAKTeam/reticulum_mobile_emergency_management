#[test]
fn checklist_participants_cannot_bypass_the_active_team_roster() {
    let peer = build_peer_record(
        "abababababababababababababababab",
        "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
        true,
        true,
        true,
    );
    for snapshot in [None, Some(HubDirectorySnapshot::yellow_only(1))] {
        let targets = build_runtime_checklist_replication_targets(
            &build_status_for_tests(),
            std::slice::from_ref(&peer),
            &[],
            None,
            None,
            snapshot.as_ref(),
            None,
        )
        .expect("checklist targets");
        assert!(targets.is_empty());
    }
}
