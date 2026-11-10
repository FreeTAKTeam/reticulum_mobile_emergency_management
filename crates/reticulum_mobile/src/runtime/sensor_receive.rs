fn sensor_receive_policy(
    state: &NodeRuntimeState,
    source: &str,
) -> Result<(bool, bool), NodeError> {
    let status = state
        .status
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    let peers = state
        .peers_snapshot
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    let snapshot = state
        .hub_directory_snapshot
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    crate::node::sensor_ingress_policy(
        &state.app_state,
        &status,
        &state.config,
        &peers,
        snapshot.as_ref(),
        source,
    )
}

fn persist_received_sensors(
    state: &NodeRuntimeState,
    bus: &EventBus,
    fields: Option<&[u8]>,
    source: Option<&str>,
) -> bool {
    let (Some(fields), Some(source)) = (fields, source) else {
        return false;
    };
    let Ok((true, relay)) = sensor_receive_policy(state, source) else {
        return false;
    };
    match r3akt_map_core::sensor_frame::decode(fields, source) {
        Ok(frame) => {
            for diagnostic in &frame.diagnostics {
                warn!("[lxmf][sensor] rejected optional geometry: {diagnostic}");
            }
            match state.app_state.receive_sensor_frame(source, relay, &frame) {
                Ok(ids) => {
                    for id in &ids {
                        if let Ok(invalidation) = state.app_state.sensor_geometry_invalidation(id) {
                            bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                        }
                    }
                    !ids.is_empty()
                }
                Err(error) => {
                    warn!("[lxmf][sensor] rejected sensor frame: {error}");
                    false
                }
            }
        }
        Err(reason) => {
            warn!("[lxmf][sensor] rejected frame: {reason}");
            false
        }
    }
}
