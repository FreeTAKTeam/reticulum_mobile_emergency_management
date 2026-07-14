#[test]
fn create_online_checklist_rejects_invalid_payload_before_local_persist() {
    let storage_dir = prepare_storage_dir("checklist-create-prevalidate");
    let node = Node::with_storage_dir(Some(storage_dir.to_string_lossy().as_ref()))
        .expect("node storage");
    {
        let inner = node.inner.lock().expect("node inner");
        let mut status = inner.status.lock().expect("status");
        status.identity_hex = "joiner-identity".to_string();
    }

    let result = node.create_online_checklist(ChecklistCreateOnlineRequest {
        checklist_uid: Some("chk-invalid".to_string()),
        mission_uid: None,
        template_uid: "tmpl-evac-001".to_string(),
        name: "Mission Alpha Evac".to_string(),
        description: "Shared run for Alpha".to_string(),
        start_time: "2026-04-22T12:00:00Z".to_string(),
        created_by_team_member_rns_identity: None,
        created_by_team_member_display_name: None,
    });

    assert!(matches!(result, Err(NodeError::InvalidConfig {})));
    assert!(node
        .get_checklist("chk-invalid".to_string())
        .expect("query checklist")
        .is_none());
}
