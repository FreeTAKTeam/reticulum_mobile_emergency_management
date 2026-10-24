impl Node {
    pub fn list_local_propagation(
        &self,
    ) -> Result<Vec<crate::types::LocalPropagationRecord>, NodeError> {
        let tx = self
            .inner
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .cmd_tx
            .clone();
        let Some(tx) = tx else { return Ok(Vec::new()) };
        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::ListLocalPropagation { resp: resp_tx })?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }
    pub fn get_local_propagation_config(&self) -> crate::types::LocalPropagationConfig {
        crate::types::LocalPropagationConfig {
            capacity_bytes: lxmf_runtime::propagation_store::CAPACITY_BYTES,
            retention_seconds: lxmf_runtime::propagation_store::RETENTION_SECS as u32,
            lora_poll_interval_seconds: crate::sdk_bridge::LOCAL_PROPAGATION_LORA_INTERVAL_SECONDS,
            lora_transfer_limit_bytes: crate::sdk_bridge::LOCAL_PROPAGATION_LORA_TRANSFER_BYTES,
        }
    }
}

/// Reuse the current native routing/sharing owners for immutable stored destinations.
pub(crate) fn local_custody_recipient_allowed(
    app_state: &AppStateStore,
    status: &NodeStatus,
    config: &NodeConfig,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
    destination: &str,
    class: OutboundTrafficClass,
    metadata: Option<&crate::mission_sync::MissionSyncMetadata>,
    relay: Option<&str>,
) -> Result<bool, NodeError> {
    if !status.running {
        return Ok(false);
    }
    let saved = app_state.get_saved_peers()?;
    let ignored = app_state.get_ignored_peer_destinations()?;
    if destination_matches_peer_set(destination, &ignored, &saved, peers) {
        return Ok(false);
    }
    if matches!(class, OutboundTrafficClass::Telemetry {}) {
        let Some(command) = metadata.and_then(|value| value.command_id.as_deref()) else {
            return Ok(false);
        };
        return telemetry_delivery_is_current(
            app_state,
            status,
            config,
            peers,
            snapshot,
            (destination, command),
        );
    }
    let fingerprint = NodeConfigFingerprint::from_config(config)?;
    if matches!(class, OutboundTrafficClass::Chat {}) {
        return routed_chat_destination_hex(
            destination.to_owned(),
            Some(&fingerprint),
            snapshot,
            || Ok(peers.to_vec()),
        )
        .map(|route| route.eq_ignore_ascii_case(destination));
    }
    let targets = if matches!(
        class,
        OutboundTrafficClass::Event {} | OutboundTrafficClass::CommunityStatus {}
    ) {
        build_runtime_event_replication_targets(
            status,
            peers,
            &saved,
            relay,
            Some(&fingerprint),
            snapshot,
        )?
    } else {
        build_runtime_mission_replication_targets(
            status,
            peers,
            &saved,
            relay,
            Some(&fingerprint),
            snapshot,
        )?
    };
    Ok(targets.iter().any(|target| {
        destination_matches_peer_set(
            destination,
            &[target.app_destination_hex.clone()],
            &saved,
            peers,
        )
    }))
}
