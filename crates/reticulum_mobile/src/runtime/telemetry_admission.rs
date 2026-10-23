fn ensure_current_telemetry_delivery(
    state: &NodeRuntimeState,
    traffic_class: OutboundTrafficClass,
    destination: &str,
    metadata: Option<&MissionSyncMetadata>,
) -> Result<(), NodeError> {
    if !matches!(traffic_class, OutboundTrafficClass::Telemetry {}) {
        return Ok(());
    }
    let command = metadata
        .and_then(|metadata| metadata.command_id.as_deref())
        .ok_or(NodeError::InvalidConfig {})?;
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
    if crate::node::telemetry_delivery_is_current(
        &state.app_state,
        &status,
        &state.config,
        &peers,
        snapshot.as_ref(),
        (destination, command),
    )? {
        Ok(())
    } else {
        Err(NodeError::InvalidConfig {})
    }
}
