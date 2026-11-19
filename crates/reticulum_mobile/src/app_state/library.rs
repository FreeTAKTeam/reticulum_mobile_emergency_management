// Mission Library (REM #356): the on-device catalog. The index row holds the
// shared `r3akt_library_core::LibraryItem` contract; the verified bytes live
// in `<storage>/library/<sha256>`, written to a temporary file in the same
// directory and renamed into place so a crash never leaves a partial item.

/// Largest file accepted for local import.
pub(crate) const LIBRARY_MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;
const LIBRARY_DIR: &str = "library";
const LIBRARY_PARTIAL_SUFFIX: &str = ".part";

fn library_store_error(error: impl std::fmt::Display + std::fmt::Debug) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
}

fn library_invalid(reason: impl std::fmt::Display + std::fmt::Debug) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, reason)
}

fn library_sha256_hex(digest: impl AsRef<[u8]>) -> String {
    hex::encode(digest)
}

/// A local item plus where its bytes are, when they are present.
#[derive(Debug, Clone)]
pub(crate) struct LibraryEntry {
    pub item: r3akt_library_core::LibraryItem,
    pub path: Option<PathBuf>,
}

impl AppStateStore {
    fn initialize_library(&self, connection: &Connection) -> Result<(), NodeError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS rem_library_items (
                    item_id TEXT PRIMARY KEY,
                    content_sha256 TEXT,
                    item_json TEXT NOT NULL,
                    created_ts_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_rem_library_items_sha
                    ON rem_library_items(content_sha256);",
            )
            .map_err(library_store_error)?;
        // Interrupted imports leave only `.part` files; they never became items.
        if let Ok(entries) = std::fs::read_dir(self.library_dir()) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().ends_with(LIBRARY_PARTIAL_SUFFIX) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn library_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .map_or_else(|| PathBuf::from(LIBRARY_DIR), |dir| dir.join(LIBRARY_DIR))
    }

    fn library_content_path(&self, sha256: &str) -> PathBuf {
        self.library_dir().join(sha256)
    }

    /// Copies `source` into managed storage, hashing as it streams, and
    /// indexes it. Identical content returns the existing item instead of a
    /// duplicate. `source` is left for the caller to delete.
    pub(crate) fn library_import(
        &self,
        source: &std::path::Path,
        name: &str,
        media_type: Option<&str>,
        now_ms: i64,
    ) -> Result<r3akt_library_core::LibraryItem, NodeError> {
        use sha2::Digest as _;
        use std::io::{Read as _, Write as _};

        let name = name.trim();
        r3akt_library_core::validate_name(name).map_err(library_invalid)?;
        let media_type = media_type.map(str::trim).filter(|value| !value.is_empty());
        let size = std::fs::metadata(source).map_err(library_store_error)?.len();
        if size > LIBRARY_MAX_IMPORT_BYTES {
            return Err(library_invalid(format!(
                "files are limited to {} MiB",
                LIBRARY_MAX_IMPORT_BYTES / (1024 * 1024)
            )));
        }
        let dir = self.library_dir();
        std::fs::create_dir_all(&dir).map_err(library_store_error)?;
        let mut nonce = [0u8; 8];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut nonce);
        let partial = dir.join(format!("{}{LIBRARY_PARTIAL_SUFFIX}", hex::encode(nonce)));
        let copied = (|| -> std::io::Result<(String, u64)> {
            let mut input = std::fs::File::open(source)?;
            let mut output = std::fs::File::create(&partial)?;
            let mut hasher = sha2::Sha256::new();
            let mut buffer = vec![0u8; 64 * 1024];
            let mut total = 0u64;
            loop {
                let read = input.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                total += read as u64;
                if total > LIBRARY_MAX_IMPORT_BYTES {
                    return Err(std::io::Error::other("file grew past the import limit"));
                }
                hasher.update(&buffer[..read]);
                output.write_all(&buffer[..read])?;
            }
            output.sync_all()?;
            Ok((library_sha256_hex(hasher.finalize()), total))
        })();
        let (sha256, size) = match copied {
            Ok(value) => value,
            Err(error) => {
                let _ = std::fs::remove_file(&partial);
                return Err(library_store_error(error));
            }
        };
        if let Some(existing) = self.library_item_by_sha(&sha256)? {
            let _ = std::fs::remove_file(&partial);
            return Ok(existing);
        }
        std::fs::rename(&partial, self.library_content_path(&sha256)).map_err(|error| {
            let _ = std::fs::remove_file(&partial);
            library_store_error(error)
        })?;
        let item = r3akt_library_core::LibraryItem {
            item_id: format!("local:{}", &sha256[..32]),
            legacy_file_id: None,
            kind: r3akt_library_core::ItemKind::for_media_type(media_type),
            source: r3akt_library_core::Source::Local,
            availability: r3akt_library_core::Availability::Local,
            name: name.to_string(),
            media_type: media_type.map(str::to_string),
            size,
            content_sha256: Some(sha256.clone()),
            rpp: None,
            publisher: None,
            mission_uids: Vec::new(),
            team_uid: None,
            created_ts_ms: now_ms,
            updated_ts_ms: now_ms,
        };
        item.validate().map_err(library_invalid)?;
        self.library_put(&item)?;
        Ok(item)
    }

    fn library_put(&self, item: &r3akt_library_core::LibraryItem) -> Result<(), NodeError> {
        let json = serde_json::to_string(item).map_err(library_store_error)?;
        self.connect()?
            .execute(
                "INSERT INTO rem_library_items (item_id, content_sha256, item_json, created_ts_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(item_id) DO UPDATE SET content_sha256 = excluded.content_sha256,
                    item_json = excluded.item_json",
                params![item.item_id, item.content_sha256, json, item.created_ts_ms],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    fn library_item_by_sha(
        &self,
        sha256: &str,
    ) -> Result<Option<r3akt_library_core::LibraryItem>, NodeError> {
        let json: Option<String> = self
            .connect()?
            .query_row(
                "SELECT item_json FROM rem_library_items WHERE content_sha256 = ?1 LIMIT 1",
                params![sha256],
                |row| row.get(0),
            )
            .optional()
            .map_err(library_store_error)?;
        json.map(|json| serde_json::from_str(&json).map_err(library_store_error)).transpose()
    }

    /// Every indexed item, with its availability recomputed from storage.
    pub(crate) fn library_entries(&self) -> Result<Vec<LibraryEntry>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT item_json FROM rem_library_items")
            .map_err(library_store_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(library_store_error)?;
        let mut entries = Vec::new();
        for row in rows {
            let mut item: r3akt_library_core::LibraryItem =
                serde_json::from_str(&row.map_err(library_store_error)?)
                    .map_err(library_store_error)?;
            let path = item
                .content_sha256
                .as_deref()
                .map(|sha| self.library_content_path(sha))
                .filter(|path| path.is_file());
            if item.source == r3akt_library_core::Source::Local {
                // Bytes can vanish (cleared storage): show the entry, never open it.
                item.availability = if path.is_some() {
                    r3akt_library_core::Availability::Local
                } else {
                    r3akt_library_core::Availability::Failed
                };
            }
            entries.push(LibraryEntry { item, path });
        }
        entries.extend(
            self.library_offer_items()?
                .into_iter()
                .map(|item| LibraryEntry { item, path: None }),
        );
        Ok(entries)
    }

    pub(crate) fn library_entry(&self, item_id: &str) -> Result<Option<LibraryEntry>, NodeError> {
        Ok(self.library_entries()?.into_iter().find(|entry| entry.item.item_id == item_id))
    }

    /// Removes the local copy only; remote publication is unaffected. Bytes
    /// are deleted once no item references them.
    pub(crate) fn library_delete(&self, item_id: &str) -> Result<bool, NodeError> {
        let Some(entry) = self.library_entry(item_id)? else {
            return Ok(false);
        };
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(library_store_error)?;
        transaction
            .execute("DELETE FROM rem_library_items WHERE item_id = ?1", params![item_id])
            .map_err(library_store_error)?;
        let remaining: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM rem_library_items WHERE content_sha256 = ?1",
                params![entry.item.content_sha256],
                |row| row.get(0),
            )
            .map_err(library_store_error)?;
        transaction.commit().map_err(library_store_error)?;
        if remaining == 0 {
            if let Some(path) = entry.path {
                std::fs::remove_file(path).map_err(library_store_error)?;
            }
        }
        Ok(true)
    }

    /// Replaces the mission links of a local item.
    pub(crate) fn library_set_missions(
        &self,
        item_id: &str,
        mission_uids: Vec<String>,
        now_ms: i64,
    ) -> Result<Option<r3akt_library_core::LibraryItem>, NodeError> {
        let Some(mut entry) = self.library_entry(item_id)? else {
            return Ok(None);
        };
        let mut missions: Vec<String> = mission_uids
            .into_iter()
            .map(|uid| uid.trim().to_string())
            .filter(|uid| !uid.is_empty())
            .collect();
        missions.sort();
        missions.dedup();
        entry.item.mission_uids = missions;
        entry.item.updated_ts_ms = now_ms;
        entry.item.validate().map_err(library_invalid)?;
        self.library_put(&entry.item)?;
        Ok(Some(entry.item))
    }
}
