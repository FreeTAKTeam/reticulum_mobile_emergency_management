impl Node {
    pub fn set_checklist_task_status(
        &self,
        request: ChecklistTaskStatusSetRequest,
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
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.set_checklist_task_status(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
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
                let checklist = inner
                    .app_state
                    .get_checklist_any(request.checklist_uid.as_str())?;
                let replication_targets = build_runtime_checklist_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    checklist.as_ref(),
                )?;
                let mut args = checklist_task_status_args_json(&request);
                if let Some(number) = checklist
                    .as_ref()
                    .and_then(|record| {
                        record
                            .tasks
                            .iter()
                            .find(|task| task.task_uid == request.task_uid && task.number > 0)
                    })
                    .map(|task| task.number)
                {
                    args.insert("number".to_string(), JsonValue::from(number));
                }
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.status.set",
                        &args,
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
                                target.app_destination_hex, "checklist.task.status.set", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "checklist replication enqueue failed destination={} command={} reason={}",
                        destination_hex, "checklist.task.status.set", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn add_checklist_task_row(
        &self,
        request: ChecklistTaskRowAddRequest,
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
                .task_uid
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.task_uid = Some(format!(
                    "{}-task-{}-{}",
                    request.checklist_uid.trim(),
                    request.number,
                    now_ms()
                ));
            }
            if request
                .changed_by_team_member_rns_identity
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                request.changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            }
            let invalidations = inner.app_state.add_checklist_task_row(&request)?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
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
                let checklist = inner
                    .app_state
                    .get_checklist_any(request.checklist_uid.as_str())?;
                let replication_targets = build_runtime_checklist_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    checklist.as_ref(),
                )?;
                let args = checklist_task_row_add_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.task.row.add",
                        &args,
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
                                target.app_destination_hex, "checklist.task.row.add", err
                            ),
                        }),
                    }
                }
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
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

        Ok(())
    }

}
