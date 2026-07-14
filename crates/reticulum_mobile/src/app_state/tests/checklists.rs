#[test]
fn checklist_lifecycle_persists_and_invalidates_list_and_detail_scopes() {
    let storage_dir = test_storage_dir("checklist-lifecycle");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let checklist = checklist("chk-1");

    let invalidations = store
        .upsert_checklist(&checklist, "checklist-upserted")
        .expect("upsert checklist");
    assert_eq!(invalidations.len(), 2);
    assert!(matches!(
        invalidations[0].scope,
        ProjectionScope::Checklists {}
    ));
    assert!(matches!(
        invalidations[1].scope,
        ProjectionScope::ChecklistDetail {}
    ));
    assert_eq!(invalidations[1].key.as_deref(), Some("chk-1"));

    let list = store
        .get_active_checklists()
        .expect("get active checklists");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].uid, "chk-1");

    let fetched = store
        .get_checklist("chk-1")
        .expect("get checklist")
        .expect("checklist exists");
    assert_eq!(fetched.name, "Alpha Checklist");

    let updated = store
        .update_checklist(&ChecklistUpdateRequest {
            checklist_uid: "chk-1".to_string(),
            patch: ChecklistUpdatePatch {
                mission_uid: Some("mission-bravo".to_string()),
                template_uid: None,
                name: Some("Bravo Checklist".to_string()),
                description: Some("Updated after briefing".to_string()),
                start_time: None,
            },
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("update checklist");
    assert_eq!(updated.len(), 2);
    let fetched = store
        .get_checklist("chk-1")
        .expect("get updated checklist")
        .expect("updated checklist exists");
    assert_eq!(fetched.mission_uid.as_deref(), Some("mission-bravo"));
    assert_eq!(fetched.name, "Bravo Checklist");
    assert_eq!(
        fetched.last_changed_by_team_member_rns_identity.as_deref(),
        Some("abcd1234")
    );

    let deleted = store.delete_checklist("chk-1").expect("delete checklist");
    assert_eq!(deleted.len(), 2);
    assert!(store
        .get_checklist("chk-1")
        .expect("query deleted checklist")
        .is_none());
}

#[test]
fn checklist_task_mutations_update_counts_and_cells() {
    let storage_dir = test_storage_dir("checklist-task-mutations");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    store
        .upsert_checklist(&checklist("chk-2"), "seed-checklist")
        .expect("seed checklist");

    store
        .add_checklist_task_row(&ChecklistTaskRowAddRequest {
            checklist_uid: "chk-2".to_string(),
            task_uid: Some("task-2".to_string()),
            number: 2,
            due_relative_minutes: Some(30),
            legacy_value: Some("Confirm rally point".to_string()),
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("add task row");
    store
        .set_checklist_task_row_style(&ChecklistTaskRowStyleSetRequest {
            checklist_uid: "chk-2".to_string(),
            task_uid: "task-2".to_string(),
            row_background_color: Some("#402020".to_string()),
            line_break_enabled: Some(true),
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("set row style");
    store
        .set_checklist_task_cell(&ChecklistTaskCellSetRequest {
            checklist_uid: "chk-2".to_string(),
            task_uid: "task-2".to_string(),
            column_uid: "col-task".to_string(),
            value: "Move to alternate pickup".to_string(),
            updated_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("set task cell");
    store
        .set_checklist_task_status(&ChecklistTaskStatusSetRequest {
            checklist_uid: "chk-2".to_string(),
            task_uid: "task-2".to_string(),
            user_status: ChecklistUserTaskStatus::Complete {},
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("set task status");

    let checklist = store
        .get_checklist("chk-2")
        .expect("get checklist")
        .expect("checklist exists");
    assert_eq!(checklist.tasks.len(), 2);
    assert_eq!(checklist.counts.pending_count, 1);
    assert_eq!(checklist.counts.complete_count, 1);
    assert_eq!(checklist.progress_percent, 50.0);
    assert_eq!(
        checklist
            .last_changed_by_team_member_rns_identity
            .as_deref(),
        Some("abcd1234")
    );
    let second_task = checklist
        .tasks
        .iter()
        .find(|task| task.task_uid == "task-2")
        .expect("second task exists");
    assert!(matches!(
        second_task.task_status,
        ChecklistTaskStatus::Complete {}
    ));
    assert_eq!(second_task.row_background_color.as_deref(), Some("#402020"));
    assert!(second_task.line_break_enabled);
    assert_eq!(
        second_task
            .cells
            .iter()
            .find(|cell| cell.column_uid == "col-task")
            .and_then(|cell| cell.value.as_deref()),
        Some("Move to alternate pickup")
    );

    store
        .delete_checklist_task_row(&ChecklistTaskRowDeleteRequest {
            checklist_uid: "chk-2".to_string(),
            task_uid: "task-2".to_string(),
            changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
        })
        .expect("delete task row");
    let checklist = store
        .get_checklist("chk-2")
        .expect("get checklist after delete")
        .expect("checklist still exists");
    assert_eq!(checklist.tasks.len(), 1);
    assert_eq!(checklist.counts.pending_count, 1);
    assert_eq!(checklist.counts.complete_count, 0);
}

#[test]
fn deleted_checklists_reject_local_mutations() {
    let storage_dir = test_storage_dir("checklist-deleted-mutations");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    store
        .upsert_checklist(&checklist("chk-3"), "seed-checklist")
        .expect("seed checklist");
    store.delete_checklist("chk-3").expect("delete checklist");

    let update = store.update_checklist(&ChecklistUpdateRequest {
        checklist_uid: "chk-3".to_string(),
        patch: ChecklistUpdatePatch {
            mission_uid: Some("mission-bravo".to_string()),
            template_uid: None,
            name: Some("Bravo Checklist".to_string()),
            description: None,
            start_time: None,
        },
        changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
    });
    assert!(matches!(update, Err(NodeError::InvalidConfig {})));

    let add_row = store.add_checklist_task_row(&ChecklistTaskRowAddRequest {
        checklist_uid: "chk-3".to_string(),
        task_uid: Some("task-2".to_string()),
        number: 2,
        due_relative_minutes: Some(30),
        legacy_value: Some("Confirm rally point".to_string()),
        changed_by_team_member_rns_identity: Some("abcd1234".to_string()),
    });
    assert!(matches!(add_row, Err(NodeError::InvalidConfig {})));
}

#[test]
fn default_checklist_templates_are_seeded() {
    let storage_dir = test_storage_dir("checklist-template-seed");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

    let templates = store
        .list_checklist_templates()
        .expect("list checklist templates");
    assert_eq!(templates.len(), 3);
    assert!(templates
        .iter()
        .any(|template| template.uid == "tmpl-24-hour-survival-pack"));
    assert!(templates.iter().all(|template| {
        template.columns.iter().any(|column| {
            column.system_key == Some(ChecklistSystemColumnKey::DueRelativeDtg {})
                && column.column_type == ChecklistColumnType::RelativeTime {}
        }) && template
            .tasks
            .iter()
            .all(|task| task.due_relative_minutes.is_some())
    }));
}

#[test]
fn checklist_late_status_is_calculated_from_due_dtg() {
    let storage_dir = test_storage_dir("checklist-due-dtg-late");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    let mut pending_late = checklist("chk-pending-late");
    pending_late.start_time = Some("1970-01-01T00:00:00Z".to_string());
    pending_late.tasks[0].due_relative_minutes = Some(15);
    store
        .upsert_checklist(&pending_late, "seed-late-checklist")
        .expect("seed late checklist");

    let pending_late = store
        .get_checklist("chk-pending-late")
        .expect("get checklist")
        .expect("checklist exists");
    assert_eq!(
        pending_late.tasks[0].due_dtg.as_deref(),
        Some("1970-01-01T00:15:00Z")
    );
    assert!(pending_late.tasks[0].is_late);
    assert_eq!(
        pending_late.tasks[0].task_status,
        ChecklistTaskStatus::Late {}
    );
    assert_eq!(pending_late.counts.late_count, 1);

    let mut completed_on_time = checklist("chk-completed-on-time");
    completed_on_time.start_time = Some("1970-01-01T00:00:00Z".to_string());
    completed_on_time.tasks[0].due_relative_minutes = Some(15);
    completed_on_time.tasks[0].user_status = ChecklistUserTaskStatus::Complete {};
    completed_on_time.tasks[0].completed_at = Some("1970-01-01T00:10:00Z".to_string());
    store
        .upsert_checklist(&completed_on_time, "seed-complete-checklist")
        .expect("seed complete checklist");

    let completed_on_time = store
        .get_checklist("chk-completed-on-time")
        .expect("get checklist")
        .expect("checklist exists");
    assert!(!completed_on_time.tasks[0].is_late);
    assert_eq!(
        completed_on_time.tasks[0].task_status,
        ChecklistTaskStatus::Complete {}
    );
}

#[test]
fn csv_template_import_can_spawn_checklist_from_template() {
    let storage_dir = test_storage_dir("checklist-template-import");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

    let template = store
        .import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
            template_uid: Some("tmpl-imported".to_string()),
            name: "Imported Preparedness".to_string(),
            description: Some("CSV import".to_string()),
            csv_text: "Task,CompletedDTG,Description,Owner,Radio Channel\nTorch,+1 hour,Portable light,Logistics,1\nRadio,+01:30,Receive alerts,Comms,7\n".to_string(),
            source_filename: Some("preparedness.csv".to_string()),
        })
        .expect("import csv template");
    assert_eq!(template.origin_type.as_str(), "CSV_IMPORT");
    assert_eq!(template.tasks.len(), 2);
    assert!(template.columns.iter().any(|column| {
        column.system_key == Some(ChecklistSystemColumnKey::DueRelativeDtg {})
            && column.column_type == ChecklistColumnType::RelativeTime {}
    }));
    assert_eq!(template.tasks[0].due_relative_minutes, Some(60));
    assert_eq!(template.tasks[1].due_relative_minutes, Some(90));
    assert_eq!(template.tasks[0].cells.len(), 4);

    store
        .create_checklist_from_template(&ChecklistCreateFromTemplateRequest {
            checklist_uid: Some("chk-imported".to_string()),
            mission_uid: Some("mission-ready".to_string()),
            template_uid: template.uid.clone(),
            name: "Imported Checklist".to_string(),
            description: "Generated from imported template".to_string(),
            start_time: "2099-04-23T12:00:00Z".to_string(),
            created_by_team_member_rns_identity: Some("alpha".to_string()),
            created_by_team_member_display_name: Some("Alpha Operator".to_string()),
        })
        .expect("create from template");

    let checklist = store
        .get_checklist("chk-imported")
        .expect("get checklist")
        .expect("checklist exists");
    assert_eq!(checklist.template_uid.as_deref(), Some("tmpl-imported"));
    assert_eq!(checklist.tasks.len(), 2);
    assert_eq!(checklist.counts.pending_count, 2);
    assert_eq!(checklist.tasks[0].due_relative_minutes, Some(60));
    assert_eq!(checklist.tasks[1].cells.len(), 4);
}

#[test]
fn csv_template_import_generates_default_completed_dtg_when_missing() {
    let storage_dir = test_storage_dir("checklist-template-import-default-due");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");
    store
        .set_app_settings(&app_settings_with_due_step(45))
        .expect("set app settings");

    let template = store
        .import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
            template_uid: Some("tmpl-import-default-due".to_string()),
            name: "Imported Default Due".to_string(),
            description: None,
            csv_text: "Title,Details,Owner\nStage kits,Prepare rescue kits,Alpha\nOpen channel,Start radio watch,Bravo\n".to_string(),
            source_filename: Some("default-due.csv".to_string()),
        })
        .expect("import csv template");

    assert_eq!(template.tasks[0].due_relative_minutes, Some(45));
    assert_eq!(template.tasks[1].due_relative_minutes, Some(90));
    assert_eq!(template.columns[0].column_name, "CompletedDTG");
    assert_eq!(
        template.columns[0].system_key,
        Some(ChecklistSystemColumnKey::DueRelativeDtg {})
    );
}

#[test]
fn csv_template_import_rejects_invalid_completed_dtg() {
    let storage_dir = test_storage_dir("checklist-template-import-invalid-due");
    let store =
        AppStateStore::new(Some(storage_dir.to_string_lossy().as_ref())).expect("create store");

    let result = store.import_checklist_template_csv(&ChecklistTemplateImportCsvRequest {
        template_uid: Some("tmpl-import-invalid-due".to_string()),
        name: "Invalid Due".to_string(),
        description: None,
        csv_text: "Task,CompletedDTG\nOpen channel,tomorrow\n".to_string(),
        source_filename: Some("invalid-due.csv".to_string()),
    });

    assert!(matches!(result, Err(NodeError::InvalidConfig {})));
}
