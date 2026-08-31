impl Node {
    pub fn set_log_level(&self, level: LogLevel) {
        NodeLogger::global().set_level(level);
        if let Ok(inner) = self.inner.lock() {
            if let Some(tx) = inner.cmd_tx.clone() {
                let _ = tx.try_send(Command::SetLogLevel { level });
            }
        }
    }

    pub fn legacy_import_completed(&self) -> Result<bool, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.legacy_import_completed()
    }

    pub fn import_legacy_state(&self, payload: LegacyImportPayload) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidations = inner.app_state.import_legacy_state(&payload)?;
        for invalidation in invalidations {
            emit_projection_invalidation(&inner.bus, invalidation);
        }
        Ok(())
    }
}
