impl Node {
    /// Plans and sends replication for an event that is already stored. With
    /// `await_delivery` false the sends are only queued, so the caller returns
    /// before any delivery attempt. Returns how many sends the runtime refused.
    fn replicate_event_with_delivery(
        &self,
        record: &EventProjectionRecord,
        requested_destination_hex: Option<String>,
        traffic_class: OutboundTrafficClass,
        await_delivery: bool,
    ) -> Result<usize, NodeError> {
        let targeted_send = requested_destination_hex.is_some();
        let mut scheduled_sends = Vec::<(String, Vec<u8>, Vec<u8>, SendMode)>::new();
        let bus = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
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
                    if requested_destination_hex
                        .as_deref()
                        .is_some_and(|requested| requested != target.app_destination_hex)
                    {
                        continue;
                    }
                    match build_event_replication_payload(&status, record, &target) {
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

        let mut refused = 0;
        for (destination_hex, body, fields_bytes, send_mode) in scheduled_sends {
            let sent = if await_delivery {
                self.send_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    traffic_class,
                )
            } else {
                // Dropping the result channel leaves delivery to the runtime queue.
                self.queue_bytes_with_class(
                    destination_hex.clone(),
                    body,
                    Some(fields_bytes),
                    send_mode,
                    traffic_class,
                )
                .map(drop)
            };
            if let Err(err) = sent {
                let message = format!(
                    "event replication delivery failed destination={} uid={} reason={}",
                    destination_hex, record.uid, err
                );
                emit_replication_delivery_failure(&bus, message, &err);
                refused += 1;
                if targeted_send {
                    return Err(err);
                }
            }
        }

        Ok(refused)
    }
}
