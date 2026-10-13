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
        let (result, record) = self.with_app_state(|app_state, bus| {
            let (result, invalidation) = app_state.create_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            let record = app_state.map_marker_core_record(&result.marker.object_destination_hash)?;
            Ok((result, record))
        })?;
        // A replayed idempotent create was already replicated the first time.
        if let Some(record) = record.filter(|_| result.created) {
            self.replicate_map_marker_change(&MapMarkerChange::created(record));
        }
        Ok(result)
    }

    pub fn rename_map_marker(
        &self,
        request: MapMarkerRenameRequest,
    ) -> Result<MapMarkerRecord, NodeError> {
        let (marker, record) = self.with_app_state(|app_state, bus| {
            let (marker, invalidation) = app_state.rename_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            let record = app_state.map_marker_core_record(&marker.object_destination_hash)?;
            Ok((marker, record))
        })?;
        if let Some(record) = record {
            self.replicate_map_marker_change(&MapMarkerChange::renamed(record));
        }
        Ok(marker)
    }

    pub fn move_map_marker(&self, request: MapMarkerMoveRequest) -> Result<MapMarkerRecord, NodeError> {
        let (marker, record) = self.with_app_state(|app_state, bus| {
            let (marker, invalidation) = app_state.move_map_marker(&request)?;
            emit_projection_invalidation(bus, invalidation);
            let record = app_state.map_marker_core_record(&marker.object_destination_hash)?;
            Ok((marker, record))
        })?;
        if let Some(record) = record {
            self.replicate_map_marker_change(&MapMarkerChange::moved(record));
        }
        Ok(marker)
    }

    /// Returns `false` when the marker did not exist.
    pub fn delete_map_marker(&self, object_destination_hash: String) -> Result<bool, NodeError> {
        let removed = self.with_app_state(|app_state, bus| {
            let record = app_state.map_marker_core_record(&object_destination_hash)?;
            let invalidation = app_state.delete_map_marker(&object_destination_hash)?;
            Ok(invalidation.map(|invalidation| {
                emit_projection_invalidation(bus, invalidation);
                record
            }))
        })?;
        let Some(record) = removed else {
            return Ok(false);
        };
        if let Some(record) = record {
            self.replicate_map_marker_change(&MapMarkerChange::deleted(record));
        }
        Ok(true)
    }
}
