impl Node {
    pub fn upload_checklist(&self, checklist_uid: String) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let normalized_uid = checklist_uid.trim();
        if normalized_uid.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let mut checklist = inner
                .app_state
                .get_checklist_any(normalized_uid)?
                .ok_or(NodeError::InvalidConfig {})?;
            if checklist.deleted_at.is_some() {
                return Err(NodeError::InvalidConfig {});
            }
            checklist.uploaded_at = Some(current_timestamp_rfc3339());
            checklist.last_changed_by_team_member_rns_identity = Some(status.identity_hex.clone());
            checklist.sync_state = crate::types::ChecklistSyncState::Synced {};
            let invalidations = inner
                .app_state
                .upsert_checklist(&checklist, "checklist-uploaded")?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() {
                let args = checklist_uid_args_json(normalized_uid);
                let snapshot_json =
                    serde_json::to_string(&checklist).map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
                let upload_command_id = format!("cmd-{normalized_uid}-upload");
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
                    match build_checklist_replication_payload_with_snapshot(
                        &status,
                        &target,
                        "checklist.upload",
                        &args,
                        Some(upload_command_id.as_str()),
                        snapshot_json.as_str(),
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
                                target.app_destination_hex, "checklist.upload", err
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
                        destination_hex, "checklist.upload", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn update_checklist(&self, request: ChecklistUpdateRequest) -> Result<(), NodeError> {
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
            let invalidations = inner.app_state.update_checklist(&request)?;
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
                let args = checklist_update_args_json(&request);
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.update",
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
                                target.app_destination_hex, "checklist.update", err
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
                        destination_hex, "checklist.update", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_checklist(&self, request: ChecklistDeleteRequest) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<ScheduledMissionSend>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let normalized_uid = request.checklist_uid.trim().to_string();
            let existing_checklist = inner.app_state.get_checklist_any(normalized_uid.as_str())?;
            let invalidations = inner.app_state.delete_checklist_with_actor(
                normalized_uid.as_str(),
                Some(status.identity_hex.as_str()),
            )?;
            for invalidation in invalidations {
                emit_projection_invalidation(&inner.bus, invalidation);
            }

            if inner.cmd_tx.is_some() && request.delete_remote {
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
                match build_checklist_delete_replication_sends(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    existing_checklist.as_ref(),
                    normalized_uid.as_str(),
                    request.delete_remote,
                ) {
                    Ok(sends) => scheduled_sends = sends,
                    Err(err) => {
                        inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!("checklist delete replication skipped reason={err}"),
                        });
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
                        destination_hex, "checklist.delete", err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn join_checklist(&self, checklist_uid: String) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let normalized_uid = checklist_uid.trim().to_string();
            let mut checklist = inner
                .app_state
                .get_checklist_any(normalized_uid.as_str())?
                .ok_or(NodeError::InvalidConfig {})?;
            if checklist.deleted_at.is_some() {
                return Err(NodeError::InvalidConfig {});
            }
            if !status.identity_hex.trim().is_empty()
                && !checklist
                    .participant_rns_identities
                    .iter()
                    .any(|value| value == &status.identity_hex)
            {
                checklist
                    .participant_rns_identities
                    .push(status.identity_hex.clone());
                checklist.updated_at = Some(current_timestamp_rfc3339());
                checklist.last_changed_by_team_member_rns_identity =
                    Some(status.identity_hex.clone());
                let invalidations = inner
                    .app_state
                    .upsert_checklist(&checklist, "checklist-joined")?;
                for invalidation in invalidations {
                    emit_projection_invalidation(&inner.bus, invalidation);
                }
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
                let args = checklist_uid_args_json(normalized_uid.as_str());
                for target in replication_targets {
                    match build_checklist_replication_payload(
                        &status,
                        &target,
                        "checklist.join",
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
                                target.app_destination_hex, "checklist.join", err
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
                        destination_hex, "checklist.join", err
                    ),
                });
            }
        }

        Ok(())
    }

}
