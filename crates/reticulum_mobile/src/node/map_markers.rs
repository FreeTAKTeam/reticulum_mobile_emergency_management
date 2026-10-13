impl Node {
    fn with_app_state<T>(
        &self,
        operation: impl FnOnce(&AppStateStore, &EventBus) -> Result<T, NodeError>,
    ) -> Result<T, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        operation(&inner.app_state, &inner.bus)
    }

    pub fn list_map_marker_symbols(&self) -> Vec<MapMarkerSymbolRecord> {
        crate::app_state::map_marker_symbols()
    }

    pub fn list_map_markers(&self) -> Result<Vec<MapMarkerRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.list_map_markers())
    }

    pub fn get_map_marker(
        &self,
        object_destination_hash: String,
    ) -> Result<Option<MapMarkerRecord>, NodeError> {
        self.with_app_state(|app_state, _| app_state.get_map_marker(&object_destination_hash))
    }

    pub fn create_map_marker(
        &self,
        request: MapMarkerCreateRequest,
    ) -> Result<MapMarkerCreateResult, NodeError> {
        self.with_app_state(|app_state, bus| {
            let (result, invalidation) = app_state.create_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            Ok(result)
        })
    }

    pub fn rename_map_marker(
        &self,
        request: MapMarkerRenameRequest,
    ) -> Result<MapMarkerRecord, NodeError> {
        self.with_app_state(|app_state, bus| {
            let (marker, invalidation) = app_state.rename_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            Ok(marker)
        })
    }

    pub fn move_map_marker(&self, request: MapMarkerMoveRequest) -> Result<MapMarkerRecord, NodeError> {
        self.with_app_state(|app_state, bus| {
            let (marker, invalidation) = app_state.move_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            Ok(marker)
        })
    }

    /// Returns `false` when the marker did not exist.
    pub fn delete_map_marker(&self, object_destination_hash: String) -> Result<bool, NodeError> {
        self.with_app_state(|app_state, bus| {
            let invalidation = app_state.delete_map_marker(&object_destination_hash)?;
            let deleted = invalidation.is_some();
            if let Some(invalidation) = invalidation {
                emit_projection_invalidation(bus, invalidation);
            }
            Ok(deleted)
        })
    }
}
