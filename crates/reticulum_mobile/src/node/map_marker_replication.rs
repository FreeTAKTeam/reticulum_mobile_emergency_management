/// A committed local marker change to replicate. RCH applies `command_type`
/// with `args`; REM peers apply the `event_type` replica built from `marker`.
struct MapMarkerChange {
    command_type: &'static str,
    event_type: &'static str,
    args: JsonValue,
    marker: r3akt_map_core::MarkerRecord,
}

impl MapMarkerChange {
    fn created(marker: r3akt_map_core::MarkerRecord) -> Self {
        let args = json!({
            "object_destination_hash": marker.object_destination_hash,
            "marker_type": marker.marker_type,
            "symbol": marker.symbol,
            "name": marker.name,
            "category": marker.category,
            "lat": marker.lat,
            "lon": marker.lon,
            "notes": marker.notes,
        });
        Self::new("mission.marker.create", "mission.marker.created", args, marker)
    }

    fn renamed(marker: r3akt_map_core::MarkerRecord) -> Self {
        let args = json!({ "object_destination_hash": marker.object_destination_hash, "name": marker.name });
        Self::new("mission.marker.patch", "mission.marker.updated", args, marker)
    }

    fn moved(marker: r3akt_map_core::MarkerRecord) -> Self {
        let args = json!({
            "object_destination_hash": marker.object_destination_hash,
            "lat": marker.lat,
            "lon": marker.lon,
        });
        Self::new("mission.marker.position.patch", "mission.marker.position.updated", args, marker)
    }

    fn deleted(marker: r3akt_map_core::MarkerRecord) -> Self {
        let args = json!({ "object_destination_hash": marker.object_destination_hash });
        Self::new("mission.marker.delete", "mission.marker.deleted", args, marker)
    }

    fn new(
        command_type: &'static str,
        event_type: &'static str,
        mut args: JsonValue,
        marker: r3akt_map_core::MarkerRecord,
    ) -> Self {
        if let Some(object) = args.as_object_mut() {
            object.retain(|_, value| !value.is_null());
        }
        Self { command_type, event_type, args, marker }
    }
}

fn map_marker_command_id() -> String {
    let mut bytes = [0_u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    format!("marker-{}", hex::encode(bytes))
}

/// One LXMF message for both audiences: the mission command for RCH in
/// `FIELD_COMMANDS` and the marker replica event for REM in `FIELD_EVENT`.
fn map_marker_replication_payload(
    status: &NodeStatus,
    change: &MapMarkerChange,
) -> Result<(Vec<u8>, Vec<u8>), NodeError> {
    let source = json!({ "rns_identity": status.identity_hex });
    let command = json!({
        "command_id": map_marker_command_id(),
        "command_type": change.command_type,
        "timestamp": r3akt_map_core::time::iso8601_from_unix_ms(
            crate::numeric::u64_to_i64_saturating(now_ms()),
        ),
        "source": source,
        "args": change.args,
    });
    let event = json!({
        "event_type": change.event_type,
        "payload": change.marker.api_value(),
        "source": source,
    });
    let encode = |value: &JsonValue| {
        rmpv::ext::to_value(value).map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })
    };
    let fields = MsgPackValue::Map(vec![
        (MsgPackValue::from(FIELD_COMMANDS), MsgPackValue::Array(vec![encode(&command)?])),
        (MsgPackValue::from(crate::lxmf_fields::FIELD_EVENT), encode(&event)?),
    ]);
    let fields = rmp_serde::to_vec(&fields).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
    })?;
    let action = change.event_type.trim_start_matches("mission.marker.");
    let body = format!("Map marker {action}: {}", change.marker.name).into_bytes();
    Ok((body, fields))
}

impl Node {
    /// Plans sends with the same targets as mission events: the hub in
    /// Connected mode (RCH relays to the TEAM), saved and TEAM peers otherwise.
    fn map_marker_replication_sends(
        &self,
        change: &MapMarkerChange,
    ) -> Result<(EventBus, Vec<ScheduledMissionSend>), NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let bus = inner.bus.clone();
        if inner.cmd_tx.is_none() {
            return Ok((bus, Vec::new()));
        }
        let status = inner.status.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let peers = inner.peers_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
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
        let saved_peers = saved_peers_for_replication(&inner.app_state, &bus, "map-marker");
        let route_hops = route_hops_for_replication(&inner.app_state, &bus, "map-marker");
        let mut targets = build_runtime_event_replication_targets(
            &status,
            peers.as_slice(),
            saved_peers.as_slice(),
            sync_status.active_propagation_node_hex.as_deref(),
            inner.active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )
        .unwrap_or_else(|err| {
            emit_replication_planning_error(&bus, "map-marker", "target-selection", err);
            Vec::new()
        });
        prioritize_replication_targets_by_route_hops(targets.as_mut_slice(), peers.as_slice(), &route_hops);
        let (body, fields) = map_marker_replication_payload(&status, change)?;
        let sends = targets
            .into_iter()
            .map(|target| (target.app_destination_hex, body.clone(), fields.clone(), target.send_mode))
            .collect();
        Ok((bus, sends))
    }

    /// Best effort: the local change is already committed, so delivery
    /// failures are reported as events rather than failing the edit.
    fn replicate_map_marker_change(&self, change: &MapMarkerChange) {
        let (bus, sends) = match self.map_marker_replication_sends(change) {
            Ok(planned) => planned,
            Err(error) => {
                log::warn!("[marker] replication planning failed: {error}");
                return;
            }
        };
        for (destination_hex, body, fields, send_mode) in sends {
            if let Err(err) = self.send_bytes_with_class(
                destination_hex.clone(),
                body,
                Some(fields),
                send_mode,
                OutboundTrafficClass::Event {},
            ) {
                let message = format!(
                    "map marker replication failed destination={destination_hex} marker={} reason={err}",
                    change.marker.object_destination_hash
                );
                emit_replication_delivery_failure(&bus, message, &err);
            }
        }
    }
}
