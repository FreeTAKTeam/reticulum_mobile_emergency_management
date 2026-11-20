// Mission Library RCH source (REM #357): the hub's metadata-only catalog,
// cached for offline browsing. Bytes arrive only through an explicit RPP
// download and then live as normal Library items.

const LIBRARY_HUB_STATUS_KEY: &str = "library_hub_status";

impl AppStateStore {
    fn initialize_library_hub(connection: &Connection) -> Result<(), NodeError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS rem_library_hub_items (
                    item_id TEXT PRIMARY KEY,
                    item_json TEXT NOT NULL,
                    state TEXT NOT NULL
                );",
            )
            .map_err(library_store_error)
    }

    /// Caches one page of hub metadata. Items already downloaded stay local;
    /// cached entries are kept (not deleted) when rights change, so evidence
    /// already on device is never silently removed.
    pub(crate) fn library_cache_hub_items(
        &self,
        items: &[r3akt_library_core::LibraryItem],
    ) -> Result<(), NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(library_store_error)?;
        for item in items {
            let mut item = item.clone();
            item.source = r3akt_library_core::Source::Rch;
            item.availability = r3akt_library_core::Availability::Remote;
            if item.validate().is_err() {
                continue;
            }
            let json = serde_json::to_string(&item).map_err(library_store_error)?;
            transaction
                .execute(
                    "INSERT INTO rem_library_hub_items (item_id, item_json, state)
                     VALUES (?1, ?2, 'remote')
                     ON CONFLICT(item_id) DO UPDATE SET item_json = excluded.item_json",
                    params![item.item_id, json],
                )
                .map_err(library_store_error)?;
        }
        transaction.commit().map_err(library_store_error)
    }

    pub(crate) fn library_set_hub_item_state(&self, item_id: &str, state: &str) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "UPDATE rem_library_hub_items SET state = ?2 WHERE item_id = ?1",
                params![item_id, state],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    pub(crate) fn library_hub_items(&self) -> Result<Vec<r3akt_library_core::LibraryItem>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT item_json, state FROM rem_library_hub_items")
            .map_err(library_store_error)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(library_store_error)?;
        let mut items = Vec::new();
        for row in rows {
            let (json, state) = row.map_err(library_store_error)?;
            let mut item: r3akt_library_core::LibraryItem =
                serde_json::from_str(&json).map_err(library_store_error)?;
            item.availability = match state.as_str() {
                "downloading" => r3akt_library_core::Availability::Downloading,
                "failed" => r3akt_library_core::Availability::Failed,
                _ => r3akt_library_core::Availability::Remote,
            };
            items.push(item);
        }
        Ok(items)
    }

    pub(crate) fn library_hub_item(
        &self,
        item_id: &str,
    ) -> Result<Option<r3akt_library_core::LibraryItem>, NodeError> {
        Ok(self.library_hub_items()?.into_iter().find(|item| item.item_id == item_id))
    }

    /// Imports a verified hub download and retires the remote entry.
    pub(crate) fn library_receive_from_hub(
        &self,
        hub_item_id: &str,
        fetched: &rpp_core::rns_share::FetchedFile,
    ) -> Result<r3akt_library_core::LibraryItem, NodeError> {
        let remote = self.library_hub_item(hub_item_id)?;
        let now = crate::numeric::u64_to_i64_saturating(now_ms());
        let mut item =
            self.library_import(&fetched.path, &fetched.name, fetched.media_type.as_deref(), now)?;
        item.source = r3akt_library_core::Source::Rch;
        item.legacy_file_id = remote.as_ref().and_then(|remote| remote.legacy_file_id);
        item.mission_uids = remote.map(|remote| remote.mission_uids).unwrap_or_default();
        item.rpp = fetched.manifest.manifest.files.first().map(|file| r3akt_library_core::RppReference {
            package_destination: hex::encode(fetched.manifest.manifest.package_destination),
            file_destination: hex::encode(file.destination),
            manifest_hash: hex::encode(fetched.manifest.hash),
        });
        item.validate().map_err(library_invalid)?;
        self.library_put(&item)?;
        self.connect()?
            .execute("DELETE FROM rem_library_hub_items WHERE item_id = ?1", params![hub_item_id])
            .map_err(library_store_error)?;
        if let Some(dir) = fetched.path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
        Ok(item)
    }

    /// The last hub Library answer: `ok`, or why the hub refused or lacks it.
    pub(crate) fn library_hub_status(&self) -> Result<Option<String>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![LIBRARY_HUB_STATUS_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(library_store_error)
    }

    pub(crate) fn library_set_hub_status(&self, status: &str) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT INTO metadata (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![LIBRARY_HUB_STATUS_KEY, status],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }
}
