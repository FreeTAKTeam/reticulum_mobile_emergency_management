impl Node {
    pub fn get_events(&self) -> Result<Vec<EventProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_events()
    }

    pub fn upsert_event(&self, record: EventProjectionRecord) -> Result<(), NodeError> {
        self.upsert_event_with_class(record, OutboundTrafficClass::Event {})
    }

    fn upsert_event_with_class(
        &self,
        record: EventProjectionRecord,
        traffic_class: OutboundTrafficClass,
    ) -> Result<(), NodeError> {
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
                let mut saved_peers =
                    saved_peers_for_replication(&inner.app_state, &inner.bus, "event-upsert");
                if matches!(traffic_class, OutboundTrafficClass::CommunityStatus {}) {
                    for peer in &mut saved_peers {
                        peer.circle_tier = CircleTier::Inner {};
                    }
                }
                let route_hops =
                    route_hops_for_replication(&inner.app_state, &inner.bus, "event-upsert");
                let sync_status = inner
                    .sync_status_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let target_result = if matches!(traffic_class, OutboundTrafficClass::CommunityStatus {}) {
                    Ok(build_event_replication_targets(
                        &status,
                        peers.as_slice(),
                        saved_peers.as_slice(),
                        sync_status.active_propagation_node_hex.as_deref(),
                    ))
                } else {
                    build_runtime_event_replication_targets(
                        &status,
                        peers.as_slice(),
                        saved_peers.as_slice(),
                        sync_status.active_propagation_node_hex.as_deref(),
                        inner.active_config.as_ref(),
                        hub_directory_snapshot.as_ref(),
                    )
                };
                let mut replication_targets = match target_result {
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

            inner.bus.clone()
        };

        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            if let Err(err) =
                self.send_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    traffic_class,
                )
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "event replication enqueue failed destination={} uid={} reason={}",
                        destination_hex, record.uid, err
                    ),
                });
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
                self.send_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    OutboundTrafficClass::Event {},
                )
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "event delete replication enqueue failed destination={destination_hex} uid={uid} reason={err}"
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn get_telemetry_positions(&self) -> Result<Vec<TelemetryPositionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_telemetry_positions()
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
        self.send_bytes_with_class(
            request.destination_hex,
            request.body_utf8.into_bytes(),
            Some(fields_bytes),
            request.send_mode,
            OutboundTrafficClass::Plugin {},
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
            let mut inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
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

            let now = now_ms();
            let telemetry_due = inner
                .next_telemetry_publish_at_ms
                .is_none_or(|deadline| now >= deadline);
            if inner.cmd_tx.is_some() && telemetry_due {
                let normal_cadence = inner
                    .app_state
                    .get_app_settings()?
                    .map(|settings| settings.telemetry.publish_interval_seconds)
                    .unwrap_or(60);
                let cadence = effective_power_cadence_seconds(
                    normal_cadence,
                    inner.power_state.saver_active,
                );
                inner.next_telemetry_publish_at_ms =
                    Some(now.saturating_add(u64::from(cadence).saturating_mul(1_000)));
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
                let mut telemetry_destinations = build_runtime_telemetry_destinations(
                    &status,
                    peers.as_slice(),
                    sync_status.active_propagation_node_hex.as_deref(),
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                )?;
                let saved_peers = inner.app_state.get_saved_peers()?;
                let connected_mode = inner.active_config.as_ref().is_some_and(|config| {
                    matches!(
                        effective_hub_mode(config.hub_mode, hub_directory_snapshot.as_ref()),
                        HubMode::Connected {}
                    )
                });
                telemetry_destinations.retain(|target| {
                    exact_telemetry_target_is_allowed(target, &saved_peers, connected_mode)
                });
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
                self.send_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    OutboundTrafficClass::Telemetry {},
                )
            {
                bus.emit(NodeEvent::Error {
                    code: "NotRunning".to_string(),
                    message: format!(
                        "telemetry replication enqueue failed destination={} callsign={} reason={}",
                        destination_hex, position.callsign, err
                    ),
                });
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
