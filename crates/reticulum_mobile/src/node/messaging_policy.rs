impl Node {
    pub fn list_telemetry_destinations(&self) -> Result<Vec<String>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let status = inner.status.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let peers = inner.peers_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let hub_directory_snapshot = inner.hub_directory_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        let sync_status = inner.sync_status_snapshot.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?.clone();
        Ok(build_runtime_telemetry_destinations(
            &status,
            peers.as_slice(),
            sync_status.active_propagation_node_hex.as_deref(),
            inner.active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?
        .into_iter()
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
        self.set_saved_peers_from_bridge(peers, false)
    }

    pub(crate) fn set_saved_peers_from_bridge(&self, mut peers: Vec<SavedPeerRecord>, tier_supplied: bool) -> Result<(), NodeError> {
        let cmd_tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let teams = inner.app_state.get_app_settings()?.map(|settings| settings.teams).unwrap_or_default();
            let requested_tiers = peers.iter().map(|peer| peer.circle_tier).collect::<Vec<_>>();
            crate::team_membership::apply_derived_circle_tiers(&mut peers, &teams.local_teams);
            if tier_supplied || peers.iter().zip(requested_tiers).any(|(peer, tier)| peer.circle_tier != tier) {
                log::warn!("circle_tier is derived from team membership and was ignored");
            }
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
