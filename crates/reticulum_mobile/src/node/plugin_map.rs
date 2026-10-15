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
