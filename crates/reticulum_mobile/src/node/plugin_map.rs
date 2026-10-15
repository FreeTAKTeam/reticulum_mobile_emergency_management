impl Node {
    pub(crate) fn record_plugin_map_marker(
        &self,
        plugin_id: &str,
        object_destination_hash: &str,
    ) -> Result<(), NodeError> {
        self.with_app_state(|app_state, _| {
            app_state.record_plugin_map_marker(plugin_id, object_destination_hash)
        })
    }

    pub(crate) fn plugin_owns_map_marker(
        &self,
        plugin_id: &str,
        object_destination_hash: &str,
    ) -> Result<bool, NodeError> {
        self.with_app_state(|app_state, _| {
            app_state.plugin_owns_map_marker(plugin_id, object_destination_hash)
        })
    }

    pub(crate) fn plugin_map_markers(
        &self,
        plugin_id: &str,
        limit: usize,
    ) -> Result<Vec<r3akt_map_core::MarkerRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.plugin_map_markers(plugin_id, limit))
    }

    pub(crate) fn plugin_map_marker_record(
        &self,
        object_destination_hash: &str,
    ) -> Result<Option<r3akt_map_core::MarkerRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.map_marker_core_record(object_destination_hash))
    }

    pub(crate) fn local_observer_fix(&self) -> Result<Option<TelemetryPositionRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.local_observer_fix())
    }
}

impl Node {
    pub(crate) fn start_plugin_map_session(
        &self,
        plugin_id: &str,
        object_destination_hashes: &[String],
    ) -> Result<PluginMapSessionRecord, NodeError> {
        self.with_app_state(|app_state, bus| {
            let (session, invalidation) =
                app_state.start_plugin_map_session(plugin_id, object_destination_hashes)?;
            emit_projection_invalidation(bus, invalidation);
            Ok(session)
        })
    }

    /// Returns whether any session stopped.
    pub(crate) fn stop_plugin_map_sessions(
        &self,
        plugin_id: Option<&str>,
        session_id: Option<&str>,
        reason: &str,
        notify: bool,
    ) -> Result<bool, NodeError> {
        self.with_app_state(|app_state, bus| {
            let invalidation =
                app_state.stop_plugin_map_sessions(plugin_id, session_id, reason, notify)?;
            Ok(invalidation
                .map(|invalidation| emit_projection_invalidation(bus, invalidation))
                .is_some())
        })
    }

    pub(crate) fn list_plugin_map_sessions(
        &self,
        plugin_id: Option<&str>,
    ) -> Result<Vec<PluginMapSessionRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.list_plugin_map_sessions(plugin_id))
    }

    pub(crate) fn plugin_map_session_objects(
        &self,
        plugin_id: &str,
        session_id: &str,
    ) -> Result<Option<(PluginMapSessionRecord, Vec<serde_json::Value>)>, NodeError> {
        self.with_app_state(|app_state, _| {
            let Some(session) = app_state.plugin_map_session(plugin_id, session_id)? else {
                return Ok(None);
            };
            let objects = app_state.plugin_map_session_objects(&session)?;
            Ok(Some((session, objects)))
        })
    }

    pub(crate) fn drain_plugin_map_session_events(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, serde_json::Value)>, NodeError> {
        self.with_app_state(|app_state, _| app_state.drain_plugin_map_session_events(limit))
    }
}
