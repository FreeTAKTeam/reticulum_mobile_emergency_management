impl RuntimeProjectionJournal {
    pub(crate) fn load_snapshot(&self) -> Option<RuntimeProjectionSnapshot> {
        let path = self.path.as_ref()?;
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                warn!(
                    "[projection] failed to read runtime snapshot {}: {error}",
                    path.display()
                );
                return None;
            }
        };
        match serde_json::from_str(&raw) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                warn!(
                    "[projection] failed to decode runtime snapshot {}: {error}",
                    path.display()
                );
                None
            }
        }
    }

    pub(crate) fn seed_snapshot(&self, snapshot: RuntimeProjectionSnapshot) {
        match self.snapshot.lock() {
            Ok(mut guard) => *guard = snapshot,
            Err(error) => {
                warn!("[projection] failed to seed runtime snapshot: {error}");
            }
        }
    }
}
