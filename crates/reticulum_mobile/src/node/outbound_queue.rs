impl Node {
    /// Admits, routes and queues one outbound send, returning the runtime's
    /// result channel without waiting on it. Best-effort replication drops the
    /// receiver so a local edit never waits for delivery attempts.
    fn queue_bytes_with_class(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
        traffic_class: OutboundTrafficClass,
    ) -> Result<cb::Receiver<Result<(), NodeError>>, NodeError> {
        let (tx, active_config, hub_directory_snapshot, local_target, hub_target, sensor_hub) = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            ensure_outbound_admitted(inner.power_state.saver_active, traffic_class)?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
                })?
                .clone();
            // shortcut: reclassifies each send; carry planner classification when the dispatch contract is unified.
            let saved_peers = inner.app_state.get_saved_peers()?;
            let peers = inner.peers_snapshot.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let local_target = destination_is_active_local_member(
                &destination_hex,
                hub_directory_snapshot.as_ref(),
                &saved_peers,
                &peers,
            );
            let hub_target = destination_is_configured_hub(
                &destination_hex,
                inner.active_config.as_ref(),
                &peers,
                &saved_peers,
            );
            let sensor_hub = hub_target
                && matches!(traffic_class, OutboundTrafficClass::Telemetry {})
                && sensor_hub_queue_authorized(
                    &inner,
                    &destination_hex,
                    fields_bytes.as_deref(),
                    &peers,
                    hub_directory_snapshot.as_ref(),
                )?;
            if !sensor_hub {
                ensure_telemetry_destination_authorized(
                    &inner,
                    traffic_class,
                    &destination_hex,
                    &peers,
                    &saved_peers,
                    hub_directory_snapshot.as_ref(),
                )?;
            }
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
                local_target,
                hub_target,
                sensor_hub,
            )
        };
        let destination_hex = if sensor_hub {
            destination_hex
        } else {
            routed_outbound_destination_hex(
                destination_hex,
                traffic_class,
                active_config.as_ref(),
                hub_directory_snapshot.as_ref(),
                local_target,
                hub_target,
            )?
        };
        let fields_bytes = fields_bytes
            .map(|fields| {
                fields_with_active_team(
                    Some(fields),
                    active_team_uid(hub_directory_snapshot.as_ref()),
                )
            })
            .transpose()?;

        let (resp_tx, resp_rx) = cb::bounded(1);
        // Queue admission must not block platform callbacks when the runtime is busy.
        try_dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                traffic_class,
                resp: resp_tx,
            },
        )?;
        Ok(resp_rx)
    }
}
