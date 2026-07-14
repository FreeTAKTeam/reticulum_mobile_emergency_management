impl Node {
    pub fn get_eams(&self) -> Result<Vec<EamProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eams()
    }

    pub fn upsert_eam(&self, record: EamProjectionRecord) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let normalized_record = populate_eam_defaults(&status, &record);
            let invalidation = inner.app_state.upsert_eam(&normalized_record)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("eam-upserted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers =
                    saved_peers_for_replication(&inner.app_state, &inner.bus, "eam-upsert");
                let route_hops =
                    route_hops_for_replication(&inner.app_state, &inner.bus, "eam-upsert");
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let mut replication_targets = match build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                ) {
                    Ok(targets) => targets,
                    Err(err) => {
                        emit_replication_planning_error(
                            &inner.bus,
                            "eam-upsert",
                            "target-selection",
                            err,
                        );
                        Vec::new()
                    }
                };
                prioritize_replication_targets_by_route_hops(
                    replication_targets.as_mut_slice(),
                    peers.as_slice(),
                    &route_hops,
                );
                for target in replication_targets {
                    match build_eam_replication_payload(&status, &normalized_record, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((
                                target.app_destination_hex.clone(),
                                body,
                                fields,
                                target.send_mode,
                            ));
                        }
                        Err(err) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "InvalidConfig".to_string(),
                                message: format!(
                                    "eam replication skipped destination={} callsign={} reason={}",
                                    target.app_destination_hex, normalized_record.callsign, err
                                ),
                            });
                        }
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
                        "eam replication enqueue failed destination={} callsign={} reason={}",
                        destination_hex, record.callsign, err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_eam(&self, callsign: String, deleted_at_ms: u64) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let invalidation = inner.app_state.delete_eam(&callsign, deleted_at_ms)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("eam-deleted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let saved_peers =
                    saved_peers_for_replication(&inner.app_state, &inner.bus, "eam-delete");
                let route_hops =
                    route_hops_for_replication(&inner.app_state, &inner.bus, "eam-delete");
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|_| NodeError::InternalError {})?
                    .clone();
                let mut replication_targets = match build_runtime_mission_replication_targets(
                    &status,
                    peers.as_slice(),
                    saved_peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                ) {
                    Ok(targets) => targets,
                    Err(err) => {
                        emit_replication_planning_error(
                            &inner.bus,
                            "eam-delete",
                            "target-selection",
                            err,
                        );
                        Vec::new()
                    }
                };
                prioritize_replication_targets_by_route_hops(
                    replication_targets.as_mut_slice(),
                    peers.as_slice(),
                    &route_hops,
                );
                for target in replication_targets {
                    match build_eam_delete_replication_payload(&callsign, deleted_at_ms, &target) {
                        Ok((body, fields)) => {
                            scheduled_sends.push((
                                target.app_destination_hex.clone(),
                                body,
                                fields,
                                target.send_mode,
                            ));
                        }
                        Err(err) => {
                            inner.bus.emit(NodeEvent::Error {
                                code: "InvalidConfig".to_string(),
                                message: format!(
                                    "eam delete replication skipped destination={} callsign={} reason={}",
                                    target.app_destination_hex, callsign, err
                                ),
                            });
                        }
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
                        "eam delete replication enqueue failed destination={} callsign={} reason={}",
                        destination_hex, callsign, err
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn delete_local_eam(&self, callsign: String, deleted_at_ms: u64) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.delete_eam(&callsign, deleted_at_ms)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("eam-deleted-local".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn get_eam_team_summary(
        &self,
        team_uid: String,
    ) -> Result<Option<EamTeamSummaryRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eam_team_summary(&team_uid)
    }

    pub fn get_eam_readiness_summary(&self) -> Result<EamReadinessSummaryRecord, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner.app_state.get_eam_readiness_summary()
    }

}
