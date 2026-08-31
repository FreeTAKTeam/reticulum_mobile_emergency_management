impl Node {
    pub fn list_plugins(&self) -> Result<Vec<InstalledPluginRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_plugins()
    }

    pub fn sync_discovered_plugins(
        &self,
        plugins: Vec<DiscoveredPluginRecord>,
    ) -> Result<Vec<InstalledPluginRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .sync_discovered_plugins(plugins.as_slice())?;
        emit_projection_invalidation(&inner.bus, invalidation);
        inner.app_state.list_plugins()
    }

    pub fn approve_plugin_publisher(
        &self,
        plugin_id: &str,
        display_name: Option<&str>,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .approve_plugin_publisher(plugin_id, display_name)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn revoke_plugin_publisher(&self, fingerprint: &str) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.revoke_plugin_publisher(fingerprint)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.set_plugin_enabled(plugin_id, enabled)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn grant_plugin_capabilities(
        &self,
        plugin_id: &str,
        capabilities: PluginCapabilityRecord,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .grant_plugin_capabilities(plugin_id, capabilities)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_plugin_runtime_state(
        &self,
        plugin_id: &str,
        state: &str,
        diagnostic: Option<String>,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner
            .app_state
            .set_plugin_runtime_state(plugin_id, state, diagnostic)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn list_plugin_sensors(&self) -> Result<Vec<PluginSensorRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_plugin_sensors()
    }

    pub fn record_plugin_sensor(
        &self,
        plugin_id: &str,
        sample: PluginSensorSampleRequest,
    ) -> Result<PluginSensorRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let (record, invalidation) = inner.app_state.record_plugin_sensor(plugin_id, sample)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(record)
    }
}
