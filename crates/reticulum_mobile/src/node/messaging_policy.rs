impl Node {
    pub fn list_telemetry_destinations(&self) -> Result<Vec<String>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let status = inner.status.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let peers = inner.peers_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let hub_directory_snapshot = inner.hub_directory_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let sync_status = inner.sync_status_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let saved_peers = inner.app_state.get_saved_peers()?;
        let connected_mode = inner.active_config.as_ref().is_some_and(|config| {
            matches!(effective_hub_mode(config.hub_mode, hub_directory_snapshot.as_ref()), HubMode::Connected {})
        });
        Ok(build_runtime_telemetry_destinations(
            &status,
            peers.as_slice(),
            sync_status.active_propagation_node_hex.as_deref(),
            inner.active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?
        .into_iter()
        .filter(|target| exact_telemetry_target_is_allowed(target, &saved_peers, connected_mode))
        .map(|target| target.app_destination_hex)
        .collect())
    }

    pub fn set_announce_capabilities(&self, capability_string: String) -> Result<(), NodeError> {
        let tx = {
            let mut inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if inner.power_state.saver_active {
                inner.deferred_announce_capabilities = Some(capability_string);
                return Ok(());
            }
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };
        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(&tx, Command::SetAnnounceCapabilities { capability_string, resp: resp_tx })?;
        resp_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn get_saved_peers(&self) -> Result<Vec<SavedPeerRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_saved_peers()
    }

    pub fn set_saved_peers(&self, peers: Vec<SavedPeerRecord>) -> Result<(), NodeError> {
        let cmd_tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let invalidation = inner.app_state.set_saved_peers(&peers)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {}, None, Some("saved-peers-updated".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);
            inner.cmd_tx.clone()
        };
        if let Some(tx) = cmd_tx {
            let (resp_tx, resp_rx) = cb::bounded(1);
            dispatch_command(&tx, Command::SetSavedPeers { peers, resp: resp_tx })?;
            resp_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(Err(NodeError::Timeout {}))?;
        }
        Ok(())
    }
}
