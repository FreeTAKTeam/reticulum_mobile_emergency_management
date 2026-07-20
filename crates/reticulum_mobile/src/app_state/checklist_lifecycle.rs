impl AppStateStore {
    pub fn get_active_checklists(&self) -> Result<Vec<ChecklistRecord>, NodeError> {
        Ok(query_json_records(
            &self.connect()?,
            "SELECT json FROM checklists ORDER BY updated_at_ms DESC, uid ASC",
        )?
        .into_iter()
        .filter_map(sanitize_active_checklist)
        .collect())
    }

    pub fn get_checklist(&self, checklist_uid: &str) -> Result<Option<ChecklistRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklists WHERE uid = ?1",
                params![checklist_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        raw.map(|value| deserialize_json(&value))
            .transpose()
            .map(|record| record.and_then(sanitize_active_checklist))
    }

    pub fn list_checklist_templates(&self) -> Result<Vec<ChecklistTemplateRecord>, NodeError> {
        let mut items = query_json_records(
            &self.connect()?,
            "SELECT json FROM checklist_templates ORDER BY updated_at_ms DESC, uid ASC",
        )?;
        for item in &mut items {
            normalize_checklist_template(item);
        }
        Ok(items)
    }

    pub fn get_checklist_template(
        &self,
        template_uid: &str,
    ) -> Result<Option<ChecklistTemplateRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklist_templates WHERE uid = ?1",
                params![template_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        raw.map(|value| {
            let mut record: ChecklistTemplateRecord = deserialize_json(&value)?;
            normalize_checklist_template(&mut record);
            Ok(record)
        })
        .transpose()
    }

    pub(crate) fn get_checklist_any(
        &self,
        checklist_uid: &str,
    ) -> Result<Option<ChecklistRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM checklists WHERE uid = ?1",
                params![checklist_uid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        raw.map(|value| deserialize_json(&value)).transpose()
    }

    pub fn upsert_checklist(
        &self,
        checklist: &ChecklistRecord,
        reason: &str,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut normalized = checklist.clone();
        normalize_checklist(&mut normalized);
        self.write_checklist_tx(&transaction, &normalized)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            normalized.uid.as_str(),
            reason,
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }

    pub fn create_online_checklist(
        &self,
        request: &ChecklistCreateOnlineRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let timestamp = current_timestamp_rfc3339();
        let checklist_uid = request
            .checklist_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("chk-{}", now_ms()));
        let changed_by =
            normalize_optional_string(request.created_by_team_member_rns_identity.as_deref());
        let checklist = ChecklistRecord {
            uid: checklist_uid,
            mission_uid: normalize_optional_string(request.mission_uid.as_deref()),
            template_uid: Some(request.template_uid.trim().to_string()),
            template_version: None,
            template_name: None,
            name: request.name.trim().to_string(),
            description: request.description.trim().to_string(),
            start_time: Some(request.start_time.trim().to_string()),
            mode: ChecklistMode::Online {},
            sync_state: ChecklistSyncState::Synced {},
            origin_type: ChecklistOriginType::RchTemplate {},
            checklist_status: ChecklistTaskStatus::Pending {},
            created_at: Some(timestamp.clone()),
            created_by_team_member_rns_identity: request
                .created_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string(),
            created_by_team_member_display_name: normalize_optional_string(
                request.created_by_team_member_display_name.as_deref(),
            ),
            updated_at: Some(timestamp),
            last_changed_by_team_member_rns_identity: changed_by,
            deleted_at: None,
            uploaded_at: None,
            participant_rns_identities: request
                .created_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| vec![value.to_string()])
                .unwrap_or_default(),
            expected_task_count: Some(0),
            progress_percent: 0.0,
            counts: crate::types::ChecklistStatusCounts {
                pending_count: 0,
                late_count: 0,
                complete_count: 0,
            },
            columns: Vec::new(),
            tasks: Vec::new(),
            feed_publications: Vec::new(),
        };
        self.upsert_checklist(&checklist, "checklist-created")
    }

    pub fn create_checklist_from_template(
        &self,
        request: &ChecklistCreateFromTemplateRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let template = self
            .get_checklist_template(request.template_uid.trim())?
            .ok_or(NodeError::InvalidConfig {})?;
        let timestamp = current_timestamp_rfc3339();
        let checklist_uid = request
            .checklist_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("chk-{}", now_ms()));
        let created_by = request
            .created_by_team_member_rns_identity
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        let mut checklist = ChecklistRecord {
            uid: checklist_uid,
            mission_uid: normalize_optional_string(request.mission_uid.as_deref()),
            template_uid: Some(template.uid.clone()),
            template_version: Some(template.version),
            template_name: Some(template.name.clone()),
            name: request.name.trim().to_string(),
            description: request.description.trim().to_string(),
            start_time: Some(request.start_time.trim().to_string()),
            mode: ChecklistMode::Online {},
            sync_state: ChecklistSyncState::Synced {},
            origin_type: template.origin_type,
            checklist_status: ChecklistTaskStatus::Pending {},
            created_at: Some(timestamp.clone()),
            created_by_team_member_rns_identity: created_by.clone(),
            created_by_team_member_display_name: normalize_optional_string(
                request.created_by_team_member_display_name.as_deref(),
            ),
            updated_at: Some(timestamp),
            last_changed_by_team_member_rns_identity: normalize_optional_string(Some(
                created_by.as_str(),
            )),
            deleted_at: None,
            uploaded_at: None,
            participant_rns_identities: normalize_optional_string(Some(created_by.as_str()))
                .map(|value| vec![value])
                .unwrap_or_default(),
            expected_task_count: Some(
                crate::numeric::usize_to_u32_saturating(
                    template
                        .tasks
                        .iter()
                        .filter(|task| task.deleted_at.is_none())
                        .count(),
                ),
            ),
            progress_percent: 0.0,
            counts: crate::types::ChecklistStatusCounts {
                pending_count: 0,
                late_count: 0,
                complete_count: 0,
            },
            columns: template.columns.clone(),
            tasks: template.tasks.clone(),
            feed_publications: Vec::new(),
        };
        normalize_checklist(&mut checklist);
        self.upsert_checklist(&checklist, "checklist-created-from-template")
    }

    pub fn import_checklist_template_csv(
        &self,
        request: &ChecklistTemplateImportCsvRequest,
    ) -> Result<ChecklistTemplateRecord, NodeError> {
        let due_step_minutes = self
            .get_app_settings()?
            .map(|settings| settings.checklists.default_task_due_step_minutes.max(1))
            .unwrap_or(DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES);
        let mut template = parse_checklist_template_csv(request, due_step_minutes)?;
        let timestamp = current_timestamp_rfc3339();
        if template.created_at.is_none() {
            template.created_at = Some(timestamp.clone());
        }
        template.updated_at = Some(timestamp);
        self.upsert_checklist_template(&template)?;
        Ok(template)
    }

    pub fn update_checklist(
        &self,
        request: &ChecklistUpdateRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut checklist = self.load_checklist_tx(&transaction, request.checklist_uid.as_str())?;
        if checklist.deleted_at.is_some() {
            return Err(NodeError::InvalidConfig {});
        }
        checklist.updated_at = Some(current_timestamp_rfc3339());
        set_checklist_last_changed_by(
            &mut checklist,
            request.changed_by_team_member_rns_identity.as_deref(),
        );
        if let Some(mission_uid) = request.patch.mission_uid.as_deref() {
            checklist.mission_uid = normalize_optional_string(Some(mission_uid));
        }
        if let Some(template_uid) = request.patch.template_uid.as_deref() {
            checklist.template_uid = normalize_optional_string(Some(template_uid));
        }
        if let Some(name) = request.patch.name.as_deref() {
            checklist.name = name.trim().to_string();
        }
        if let Some(description) = request.patch.description.as_deref() {
            checklist.description = description.trim().to_string();
        }
        if let Some(start_time) = request.patch.start_time.as_deref() {
            checklist.start_time = normalize_optional_string(Some(start_time));
        }
        normalize_checklist(&mut checklist);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist.uid.as_str(),
            "checklist-updated",
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }

    #[cfg(test)]
    pub fn delete_checklist(
        &self,
        checklist_uid: &str,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        self.delete_checklist_with_actor(checklist_uid, None)
    }

    pub fn delete_checklist_with_actor(
        &self,
        checklist_uid: &str,
        changed_by_team_member_rns_identity: Option<&str>,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        let mut checklist = self.load_checklist_tx(&transaction, checklist_uid)?;
        let timestamp = current_timestamp_rfc3339();
        checklist.deleted_at = Some(timestamp.clone());
        checklist.updated_at = Some(timestamp);
        set_checklist_last_changed_by(&mut checklist, changed_by_team_member_rns_identity);
        self.write_checklist_tx(&transaction, &checklist)?;
        let invalidations = self.bump_checklist_projection_revisions_tx(
            &transaction,
            checklist_uid,
            "checklist-deleted",
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }

    pub fn set_checklist_task_status(
        &self,
        request: &ChecklistTaskStatusSetRequest,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
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
        task.user_status = request.user_status;
        task.task_status = checklist_task_status_for(task.user_status, task.is_late);
        if task.task_status.is_complete() {
            task.completed_at = Some(timestamp.clone());
            task.completed_by_team_member_rns_identity = request
                .changed_by_team_member_rns_identity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        } else {
            task.completed_at = None;
            task.completed_by_team_member_rns_identity = None;
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
            "checklist-task-status-set",
        )?;
        transaction.commit().map_err(|error| crate::error_context::contextual_node_error(NodeError::IoError {}, error))?;
        Ok(invalidations)
    }

}
