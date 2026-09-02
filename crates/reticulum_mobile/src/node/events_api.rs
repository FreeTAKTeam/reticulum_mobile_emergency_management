impl Node {
    pub fn get_events(&self) -> Result<Vec<EventProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        inner.app_state.get_events()
    }

    pub fn upsert_event(&self, record: EventProjectionRecord) -> Result<(), NodeError> {
        self.upsert_event_with_destination(record, None)
    }

    pub(crate) fn upsert_event_to_destination(
        &self,
        record: EventProjectionRecord,
        destination_hex: String,
    ) -> Result<(), NodeError> {
        let destination_hex = normalize_hex_32(destination_hex.as_str())
            .ok_or(NodeError::InvalidConfig {})?;
        self.upsert_event_with_destination(record, Some(destination_hex))
    }
}
