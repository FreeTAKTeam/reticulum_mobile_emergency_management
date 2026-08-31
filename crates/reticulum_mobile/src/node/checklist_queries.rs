impl Node {
    pub fn list_active_checklists(
        &self,
        request: Option<ChecklistListActiveRequest>,
    ) -> Result<Vec<ChecklistRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let mut items = inner.app_state.get_active_checklists()?;
        if let Some(request) = request {
            if let Some(search) = request
                .search
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let needle = search.to_ascii_lowercase();
                items.retain(|item| {
                    [
                        Some(item.uid.as_str()),
                        Some(item.name.as_str()),
                        Some(item.description.as_str()),
                        item.mission_uid.as_deref(),
                        item.template_uid.as_deref(),
                        item.template_name.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|value| value.to_ascii_lowercase().contains(needle.as_str()))
                });
            }
            match request.sort_by.as_deref().map(str::trim) {
                Some("name_asc") => items.sort_by(|left, right| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                        .then_with(|| left.uid.cmp(&right.uid))
                }),
                Some("name_desc") => items.sort_by(|left, right| {
                    right
                        .name
                        .to_ascii_lowercase()
                        .cmp(&left.name.to_ascii_lowercase())
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
                Some("updated_at_asc") | Some("created_at_asc") => items.sort_by(|left, right| {
                    left.updated_at
                        .cmp(&right.updated_at)
                        .then_with(|| left.created_at.cmp(&right.created_at))
                        .then_with(|| left.uid.cmp(&right.uid))
                }),
                Some("created_at_desc") => items.sort_by(|left, right| {
                    right
                        .created_at
                        .cmp(&left.created_at)
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
                _ => items.sort_by(|left, right| {
                    right
                        .updated_at
                        .cmp(&left.updated_at)
                        .then_with(|| right.created_at.cmp(&left.created_at))
                        .then_with(|| right.uid.cmp(&left.uid))
                }),
            }
        }
        Ok(items)
    }

    pub fn get_checklist(
        &self,
        checklist_uid: String,
    ) -> Result<Option<ChecklistRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_checklist(checklist_uid.trim())
    }

    pub fn list_checklist_templates(
        &self,
        request: Option<ChecklistTemplateListRequest>,
    ) -> Result<Vec<ChecklistTemplateRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let mut items = inner.app_state.list_checklist_templates()?;
        if let Some(request) = request {
            if let Some(search) = request
                .search
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let needle = search.to_ascii_lowercase();
                items.retain(|record| {
                    record.uid.to_ascii_lowercase().contains(needle.as_str())
                        || record.name.to_ascii_lowercase().contains(needle.as_str())
                        || record
                            .description
                            .to_ascii_lowercase()
                            .contains(needle.as_str())
                });
            }
        }
        Ok(items)
    }

    pub fn get_checklist_template(
        &self,
        template_uid: String,
    ) -> Result<Option<ChecklistTemplateRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_checklist_template(template_uid.trim())
    }

    pub fn import_checklist_template_csv(
        &self,
        request: ChecklistTemplateImportCsvRequest,
    ) -> Result<ChecklistTemplateRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.import_checklist_template_csv(&request)
    }

    pub fn create_checklist_from_template(
        &self,
        request: ChecklistCreateFromTemplateRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<ScheduledMissionSend>::new();
        let mut delayed_sends = Vec::<ScheduledMissionSend>::new();
        let mut cmd_tx = None;
        let (bus, power_saver_rx) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(
                inner.power_state.saver_active,
                OutboundTrafficClass::Checklist {},
            )?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let mut request = request;
            if request
                .checklist_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.checklist_uid = Some(format!("chk-{}", now_ms()));
            }
            if request
                .created_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.created_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            if request
                .created_by_team_member_display_name
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                let display_name = status.name.trim();
                if !display_name.is_empty() {
                    request.created_by_team_member_display_name = Some(display_name.to_string());
                }
            }
            let checklist_uid = request
                .checklist_uid
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(NodeError::InvalidConfig {})?
                .to_string();
            let create_command_id = format!("cmd-{checklist_uid}");
            let invalidations = inner.app_state.create_checklist_from_template(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if let Some(tx) = inner.cmd_tx.clone() {
                cmd_tx = Some(tx);
                let create_request = ChecklistCreateOnlineRequest {
                    checklist_uid: Some(checklist_uid.clone()),
                    mission_uid: request.mission_uid.clone(),
                    template_uid: request.template_uid.clone(),
                    name: request.name.clone(),
                    description: request.description.clone(),
                    start_time: request.start_time.clone(),
                    created_by_team_member_rns_identity: request
                        .created_by_team_member_rns_identity
                        .clone(),
                    created_by_team_member_display_name: request
                        .created_by_team_member_display_name
                        .clone(),
                };
                let mut snapshot = inner
                    .app_state
                    .get_checklist_any(checklist_uid.as_str())?
                    .ok_or(NodeError::InternalError {})?;
                snapshot.uploaded_at = Some(current_timestamp_rfc3339());
                snapshot.last_changed_by_team_member_rns_identity =
                    request.created_by_team_member_rns_identity.clone();
                snapshot.sync_state = crate::types::ChecklistSyncState::Synced {};
                let invalidations = inner
                    .app_state
                    .upsert_checklist(&snapshot, "checklist-uploaded")?;
                for invalidation in invalidations {
                    emit_projection_invalidation(&inner.bus, invalidation);
                }
                let create_args = compact_checklist_create_online_args_json(
                    &create_request,
                    snapshot.expected_task_count,
                )?;
                let create_replicates_template_tasks =
                    create_template_replicates_tasks_from_template(&create_args);
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let replication_targets = build_runtime_checklist_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    Some(&snapshot),
                )?;
                for target in replication_targets {
                    match build_checklist_replication_payload_with_command_id(
                        &status,
                        &target,
                        "checklist.create.online",
                        &create_args,
                        Some(create_command_id.as_str()),
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.create.online", err
                            ),
                        }),
                    }
                    if !create_replicates_template_tasks {
                        delayed_sends.extend(build_initial_checklist_task_payloads(
                            &status,
                            &target,
                            checklist_uid.as_str(),
                            snapshot.tasks.as_slice(),
                            request.created_by_team_member_rns_identity.as_deref(),
                        ));
                    }
                }
            }

            (inner.bus.clone(), inner.power_saver_tx.subscribe())
        };

        let Some(tx) = cmd_tx else {
            return Ok(());
        };

        for send in scheduled_sends {
            let destination_hex = send.0.clone();
            if let Err(err) =
                dispatch_scheduled_mission_send(&tx, send, *power_saver_rx.borrow())
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.create.online", err
                    ),
                });
            }
        }

        if !delayed_sends.is_empty() {
            let bus = bus.clone();
            std::thread::spawn(move || {
                for send in delayed_sends {
                    std::thread::sleep(CHECKLIST_INITIAL_TASK_SEND_INTERVAL);
                    let destination_hex = send.0.clone();
                    if let Err(err) =
                        dispatch_scheduled_mission_send(&tx, send, *power_saver_rx.borrow())
                    {
                        bus.emit(NodeEvent::Error {
                            code: "NotRunning".to_string(),
                            message: format!(
                                "checklist replication enqueue failed destination={} command={} reason={}",
                                destination_hex, "checklist.task.row.add", err
                            ),
                        });
                    }
                }
            });
        }

        Ok(())
    }

    pub fn create_online_checklist(
        &self,
        request: ChecklistCreateOnlineRequest,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let mut request = request;
            if request
                .checklist_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.checklist_uid = Some(format!("chk-{}", now_ms()));
            }
            if request
                .created_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.created_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            if request
                .created_by_team_member_display_name
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                let display_name = status.name.trim();
                if !display_name.is_empty() {
                    request.created_by_team_member_display_name = Some(display_name.to_string());
                }
            }
            let checklist_uid = request
                .checklist_uid
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(NodeError::InvalidConfig {})?
                .to_string();
            let create_args = compact_checklist_create_online_args_json(&request, None)?;
            let command_id = format!("cmd-{checklist_uid}");
            let invalidations = inner.app_state.create_online_checklist(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }
            let checklist = inner
                .app_state
                .get_checklist_any(checklist_uid.as_str())?
                .ok_or(NodeError::InternalError {})?;
            let mut args = create_args;
            if let Some(total_tasks) = checklist.expected_task_count {
                args.insert("total_tasks".to_string(), JsonValue::from(total_tasks));
            }

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let saved_peers = inner.app_state.get_saved_peers()?;
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let replication_targets = build_runtime_checklist_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    Some(&checklist),
                )?;
                for target in replication_targets {
                    match build_checklist_replication_payload_with_command_id(
                        &status,
                        &target,
                        "checklist.create.online",
                        &args,
                        Some(command_id.as_str()),
                    ) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "checklist replication skipped destination={} command={} reason={}",
                                target.app_destination_hex, "checklist.create.online", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    OutboundTrafficClass::Checklist {},
                )
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.create.online", err
                    ),
                });
            }
        }

        Ok(())
    }

}
