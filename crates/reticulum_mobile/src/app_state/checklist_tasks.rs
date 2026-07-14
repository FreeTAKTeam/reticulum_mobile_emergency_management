impl AppStateStore {
    pub fn add_checklist_task_row(
        &self,
        request: &ChecklistTaskRowAddRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        let task_uid = request
            .task_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}-task-{}-{}", checklist.uid, request.number, now_ms()));
        if checklist.tasks.iter().any(|task| task.task_uid == task_uid) {
            let task = checklist
                .tasks
                .iter_mut()
                .find(|task| task.task_uid == task_uid)
                .ok_or(NodeError::InvalidConfig {})?;
            if task.deleted_at.is_some() {
                task.deleted_at = None;
                task.updated_at = Some(timestamp.clone());
                task.number = request.number;
                task.user_status = ChecklistUserTaskStatus::Pending {};
                task.task_status = ChecklistTaskStatus::Pending {};
                task.is_late = false;
                task.custom_status = None;
                task.due_relative_minutes = request.due_relative_minutes;
                task.due_dtg = None;
                task.notes = None;
                task.row_background_color = None;
                task.line_break_enabled = false;
                task.completed_at = None;
                task.completed_by_team_member_rns_identity = None;
                task.legacy_value = request.legacy_value.clone();
                task.cells = checklist
                    .columns
                    .iter()
                    .map(|column| ChecklistCellRecord {
                        cell_uid: format!("{}:{}", task.task_uid, column.column_uid),
                        task_uid: task.task_uid.clone(),
                        column_uid: column.column_uid.clone(),
                        value: None,
                        updated_at: None,
                        updated_by_team_member_rns_identity: None,
                    })
                    .collect();
                checklist.updated_at = Some(timestamp);
                set_checklist_last_changed_by(
                    &mut checklist,
                    request.changed_by_team_member_rns_identity.as_deref(),
                );
                normalize_checklist(&mut checklist);
                self.write_checklist_tx(&transaction, &checklist)?;
                let invalidations = self.bump_checklist_projection_revisions_tx(
                    &transaction,
                    checklist.uid.as_str(),
                    "checklist-task-row-added",
                )?;
                transaction.commit().map_err(|_| NodeError::IoError {})?;
                return Ok(invalidations);
            }
            return Err(NodeError::InvalidConfig {});
        }
        let cells = checklist
            .columns
            .iter()
            .map(|column| ChecklistCellRecord {
                cell_uid: format!("{task_uid}:{}", column.column_uid),
                task_uid: task_uid.clone(),
                column_uid: column.column_uid.clone(),
                value: None,
                updated_at: None,
                updated_by_team_member_rns_identity: None,
            })
            .collect::<Vec<_>>();
        checklist.tasks.push(ChecklistTaskRecord {
            task_uid,
            number: request.number,
            user_status: ChecklistUserTaskStatus::Pending {},
            task_status: ChecklistTaskStatus::Pending {},
            is_late: false,
            updated_at: Some(timestamp.clone()),
            deleted_at: None,
            custom_status: None,
            due_relative_minutes: request.due_relative_minutes,
            due_dtg: None,
            notes: None,
            row_background_color: None,
            line_break_enabled: false,
            completed_at: None,
            completed_by_team_member_rns_identity: None,
            legacy_value: request.legacy_value.clone(),
            cells,
        });
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-added",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn delete_checklist_task_row(
        &self,
        request: &ChecklistTaskRowDeleteRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        let task = checklist
            .tasks
            .iter_mut()
            .find(|task| task.task_uid == request.task_uid)
            .ok_or(NodeError::InvalidConfig {})?;
        task.deleted_at = Some(timestamp.clone());
        task.updated_at = Some(timestamp.clone());
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-deleted",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_row_style(
        &self,
        request: &ChecklistTaskRowStyleSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let task = find_checklist_task_mut(&mut checklist, request.task_uid.as_str())?;
        if task.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        let timestamp = current_timestamp_rfc3339();
        task.updated_at = Some(timestamp.clone());
        if let Some(row_background_color) = request.row_background_color.as_deref() {
            task.row_background_color = normalize_optional_string(Some(row_background_color));
        }
        if let Some(line_break_enabled) = request.line_break_enabled {
            task.line_break_enabled = line_break_enabled;
        }
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-row-style-set",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_cell(
        &self,
        request: &ChecklistTaskCellSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        if !checklist
            .columns
            .iter()
            .any(|column| column.column_uid == request.column_uid)
        {
            let display_order = checklist.columns.len() as u32;
            checklist.columns.push(ChecklistColumnRecord {
                column_uid: request.column_uid.clone(),
                column_name: request.column_uid.clone(),
                display_order,
                column_type: ChecklistColumnType::ShortString {},
                column_editable: true,
                background_color: None,
                text_color: None,
                is_removable: true,
                system_key: None,
            });
        }
        let timestamp = current_timestamp_rfc3339();
        let task = find_checklist_task_mut(&mut checklist, request.task_uid.as_str())?;
        if task.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        task.updated_at = Some(timestamp.clone());
        if let Some(cell) = task
            .cells
            .iter_mut()
            .find(|cell| cell.column_uid == request.column_uid)
        {
            cell.value = Some(request.value.clone());
            cell.updated_at = Some(timestamp.clone());
            cell.updated_by_team_member_rns_identity = request
                .updated_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        } else {
            task.cells.push(ChecklistCellRecord {
                cell_uid: format!("{}:{}", task.task_uid, request.column_uid),
                task_uid: task.task_uid.clone(),
                column_uid: request.column_uid.clone(),
                value: Some(request.value.clone()),
                updated_at: Some(timestamp.clone()),
                updated_by_team_member_rns_identity: request
                    .updated_by_team_member_rns_identity
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            });
        }
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(
            &mut checklist,
            request.updated_by_team_member_rns_identity.as_deref(),
        );
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-task-cell-set",
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(invalidations)
    }

}
