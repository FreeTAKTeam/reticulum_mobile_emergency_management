impl RuntimeProjectionJournal {
    /// A read model of custody status only; it contains neither payloads nor retries.
    pub(crate) fn local_propagation(
        &self,
    ) -> Result<Vec<crate::types::LocalPropagationRecord>, crate::types::NodeError> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.local_propagation.clone())
            .map_err(|_| crate::types::NodeError::InternalError {})
    }

    pub(crate) fn record_local_propagation(
        &self,
        entry: crate::types::LocalPropagationRecord,
    ) -> Result<bool, crate::types::NodeError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| crate::types::NodeError::InternalError {})?;
        if let Some(previous) = snapshot
            .local_propagation
            .iter_mut()
            .find(|old| old.transient_id_hex == entry.transient_id_hex)
        {
            if previous == &entry {
                return Ok(false);
            }
            *previous = entry;
        } else {
            snapshot.local_propagation.push(entry);
        }
        // The native store admits at most this many notices/tombstones per retention window.
        snapshot
            .local_propagation
            .sort_by_key(|entry| entry.admitted_at_ms);
        if snapshot.local_propagation.len() > lxmf_runtime::propagation_store::MAX_STATUS {
            snapshot.local_propagation.remove(0);
        }
        snapshot.updated_at_ms = now_ms();
        Ok(true)
    }

    /// Called before acknowledging a terminal notice to the custody owner.
    /// All projection writes share this lock so an older snapshot cannot overwrite it.
    pub(crate) fn persist_current_snapshot(&self) -> Result<(), crate::types::NodeError> {
        use crate::types::NodeError;
        use std::io::Write;
        let _writer = self
            .persist_lock
            .lock()
            .map_err(|_| NodeError::InternalError {})?;
        let snapshot = self
            .snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let path = self.path.as_ref().ok_or(NodeError::IoError {})?;
        let parent = path.parent().ok_or(NodeError::IoError {})?;
        fs::create_dir_all(parent).map_err(|_| NodeError::IoError {})?;
        let temporary = path.with_extension("json.tmp");
        let raw = serde_json::to_vec_pretty(&snapshot).map_err(|_| NodeError::InternalError {})?;
        let mut file = std::fs::File::create(&temporary).map_err(|_| NodeError::IoError {})?;
        file.write_all(&raw)
            .and_then(|()| file.sync_all())
            .map_err(|_| NodeError::IoError {})?;
        std::fs::rename(&temporary, path).map_err(|_| NodeError::IoError {})?;
        std::fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }
}
