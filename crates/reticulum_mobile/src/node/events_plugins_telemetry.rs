impl Node {
    fn upsert_event_with_destination(
        &self,
        record: EventProjectionRecord,
        requested_destination_hex: Option<String>,
    ) -> Result<(), NodeError> {
        let targeted_send = requested_destination_hex.is_some();
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let invalidation = inner.app_state.upsert_event(&record)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("event-upserted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

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
                let saved_peers =
                    saved_peers_for_replication(&inner.app_state, &inner.bus, "event-upsert");
                let route_hops =
                    route_hops_for_replication(&inner.app_state, &inner.bus, "event-upsert");
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let mut replication_targets = match build_runtime_event_replication_targets(
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
                            "event-upsert",
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
                    if requested_destination_hex
                        .as_deref()
                        .is_some_and(|requested| requested != target.app_destination_hex)
                    {
                        continue;
                    }
                    match build_event_replication_payload(&status, &record, &target) {
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
                                    "event replication skipped destination={} uid={} reason={}",
                                    target.app_destination_hex, record.uid, err
                                ),
                            });
                        }
                    }
                }
            }

            if requested_destination_hex.is_some() && scheduled_sends.is_empty() {
                return Err(NodeError::InvalidConfig {});
            }

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes(destination_hex.clone(), body, Some(fields_bytes), send_mode)
            {
                let message = format!(
                    "event replication delivery failed destination={} uid={} reason={}",
                    destination_hex, record.uid, err
                );
                emit_replication_delivery_failure(&bus, message, &err);
                if targeted_send {
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    pub fn delete_event(&self, uid: String, deleted_at_ms: u64) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let existing_record = inner
                .app_state
                .get_events()?
                .into_iter()
                .find(|event| event.uid == uid);
            let invalidation = inner.app_state.delete_event(&uid, deleted_at_ms)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("event-deleted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

            if inner.cmd_tx.is_some() {
                if let Some(existing_record) = existing_record.as_ref() {
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
                    let saved_peers =
                        saved_peers_for_replication(&inner.app_state, &inner.bus, "event-delete");
                    let route_hops =
                        route_hops_for_replication(&inner.app_state, &inner.bus, "event-delete");
                    let sync_status = inner
                        .sync_status_snapshot
                        .lock()
                        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                        .clone();
                    let mut replication_targets = match build_runtime_event_replication_targets(
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
                                "event-delete",
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
                        match build_event_delete_replication_payload(
                            &status,
                            existing_record,
                            deleted_at_ms,
                            &target,
                        ) {
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
                                        "event delete replication skipped destination={} uid={} reason={}",
                                        target.app_destination_hex, uid, err
                                    ),
                                });
                            }
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
                let message = format!(
                    "event delete replication delivery failed destination={destination_hex} uid={uid} reason={err}"
                );
                emit_replication_delivery_failure(&bus, message, &err);
            }
        }

        Ok(())
    }

    pub fn get_telemetry_positions(&self) -> Result<Vec<TelemetryPositionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_telemetry_positions()
    }

    pub fn list_plugins(&self) -> Result<Vec<InstalledPluginRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_plugins()
    }

    pub fn sync_discovered_plugins(
        &self,
        plugins: Vec<DiscoveredPluginRecord>,
    ) -> Result<Vec<InstalledPluginRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .sync_discovered_plugins(plugins.as_slice())?;
        emit_projection_invalidation(&inner.bus, invalidation);
        inner.app_state.list_plugins()
    }

    pub fn approve_plugin_publisher(
        &self,
        plugin_id: &str,
        display_name: Option<&str>,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .approve_plugin_publisher(plugin_id, display_name)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn revoke_plugin_publisher(&self, fingerprint: &str) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.revoke_plugin_publisher(fingerprint)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.set_plugin_enabled(plugin_id, enabled)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn grant_plugin_capabilities(
        &self,
        plugin_id: &str,
        capabilities: PluginCapabilityRecord,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .grant_plugin_capabilities(plugin_id, capabilities)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_plugin_runtime_state(
        &self,
        plugin_id: &str,
        state: &str,
        diagnostic: Option<String>,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .set_plugin_runtime_state(plugin_id, state, diagnostic)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn list_plugin_sensors(&self) -> Result<Vec<PluginSensorRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_plugin_sensors()
    }

    pub fn record_plugin_sensor(
        &self,
        plugin_id: &str,
        sample: PluginSensorSampleRequest,
    ) -> Result<PluginSensorRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let (record, invalidation) = inner.app_state.record_plugin_sensor(plugin_id, sample)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(record)
    }

    pub fn publish_plugin_event(&self, plugin_id: &str, event: JsonValue) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let plugin = inner
            .app_state
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin.trusted
            || !plugin.enabled
            || !plugin.discovered.declared_capabilities.events_publish
            || !plugin.granted_capabilities.events_publish
            || !event.is_object()
        {
            return Err(NodeError::InvalidConfig {});
        }
        inner.bus.emit(NodeEvent::PluginEventPublished {
            event: PluginEventRecord {
                plugin_id: plugin_id.to_string(),
                event_json: serde_json::to_string(&event)
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, error))?,
            },
        });
        Ok(())
    }

    pub fn send_plugin_lxmf(&self, request: PluginLxmfSendRequest) -> Result<(), NodeError> {
        let fields_bytes = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let plugin = inner
                .app_state
                .get_plugin(request.plugin_id.as_str())?
                .ok_or(NodeError::InvalidConfig {})?;
            if !plugin.trusted
                || !plugin.enabled
                || !plugin.discovered.declared_capabilities.lxmf_send
                || !plugin.granted_capabilities.lxmf_send
            {
                return Err(NodeError::InvalidConfig {});
            }
            crate::plugin_runtime::encode_plugin_fields(
                &plugin,
                request.message_name.as_str(),
                request.payload.clone(),
            )?
        };
        self.send_bytes(
            request.destination_hex,
            request.body_utf8.into_bytes(),
            Some(fields_bytes),
            request.send_mode,
        )
    }

    pub fn decode_plugin_lxmf_fields(
        &self,
        fields_bytes: &[u8],
    ) -> Result<Option<crate::plugin_runtime::PluginLxmfEnvelope>, NodeError> {
        let Some(plugin_id) = crate::plugin_runtime::plugin_id_from_fields(fields_bytes)? else {
            return Ok(None);
        };
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let plugin = inner
            .app_state
            .get_plugin(plugin_id.as_str())?
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin.trusted
            || !plugin.enabled
            || !plugin.discovered.declared_capabilities.lxmf_receive
            || !plugin.granted_capabilities.lxmf_receive
        {
            return Err(NodeError::InvalidConfig {});
        }
        crate::plugin_runtime::decode_plugin_fields(fields_bytes, &plugin)
    }

    pub fn record_local_telemetry_fix(
        &self,
        position: TelemetryPositionRecord,
    ) -> Result<(), NodeError> {
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let invalidation = inner.app_state.record_local_telemetry_fix(&position)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("telemetry-upserted".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);

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
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let telemetry_destinations = build_runtime_telemetry_destinations(
                    &status,
                    peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                for target in telemetry_destinations {
                    match build_telemetry_replication_payload(&position, &target) {
                        Ok((body, fields)) => scheduled_sends.push((
                            target.app_destination_hex.clone(),
                            body,
                            fields,
                            target.send_mode,
                        )),
                        Err(err) => inner.bus.emit(NodeEvent::Error {
                            code: "InvalidConfig".to_string(),
                            message: format!(
                                "telemetry replication skipped destination={} callsign={} reason={}",
                                target.app_destination_hex, position.callsign, err
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
                let message = format!(
                    "telemetry replication delivery failed destination={} callsign={} reason={}",
                    destination_hex, position.callsign, err
                );
                emit_replication_delivery_failure(&bus, message, &err);
            }
        }

        Ok(())
    }

    pub fn delete_local_telemetry(&self, callsign: String) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.delete_local_telemetry(&callsign)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("telemetry-deleted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

}
