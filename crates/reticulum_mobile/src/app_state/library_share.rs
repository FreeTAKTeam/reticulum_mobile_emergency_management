// Mission Library peer sharing (REM #358): published Packages to republish
// after restart, and Offers received from peers ("Shared With Me").

/// A Package this device published, with who may pull it.
#[derive(Debug, Clone)]
pub(crate) struct LibraryShareRecord {
    pub item_id: String,
    pub shared: rpp_core::rns_share::SharedPackage,
    pub recipients: Vec<String>,
}

const OFFER_ITEM_PREFIX: &str = "peer:";

pub(crate) fn library_offer_item_id(package_destination_hex: &str) -> String {
    format!("{OFFER_ITEM_PREFIX}{package_destination_hex}")
}

pub(crate) fn library_offer_package(item_id: &str) -> Option<&str> {
    item_id.strip_prefix(OFFER_ITEM_PREFIX)
}

impl AppStateStore {
    fn initialize_library_share(connection: &Connection) -> Result<(), NodeError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS rem_library_shares (
                    item_id TEXT PRIMARY KEY,
                    package_token BLOB NOT NULL,
                    manifest BLOB NOT NULL,
                    publication BLOB NOT NULL,
                    recipients_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS rem_library_offers (
                    package_destination TEXT PRIMARY KEY,
                    sender TEXT NOT NULL,
                    manifest_hash TEXT NOT NULL,
                    state TEXT NOT NULL,
                    received_ts_ms INTEGER NOT NULL
                );",
            )
            .map_err(library_store_error)?;
        // No RPP pull survives a restart, so a download left over can never finish.
        connection
            .execute(
                "UPDATE rem_library_offers SET state = 'failed' WHERE state = 'downloading'",
                [],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    pub(crate) fn library_share_record(
        &self,
        item_id: &str,
    ) -> Result<Option<LibraryShareRecord>, NodeError> {
        Ok(self.library_share_records()?.into_iter().find(|record| record.item_id == item_id))
    }

    pub(crate) fn library_share_records(&self) -> Result<Vec<LibraryShareRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT item_id, package_token, manifest, publication, recipients_json
                 FROM rem_library_shares",
            )
            .map_err(library_store_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(library_store_error)?;
        let mut records = Vec::new();
        for row in rows {
            let (item_id, token, manifest, publication, recipients) =
                row.map_err(library_store_error)?;
            let package_token: [u8; 16] = token
                .try_into()
                .map_err(|_| library_store_error("stored package token has the wrong length"))?;
            records.push(LibraryShareRecord {
                item_id,
                shared: rpp_core::rns_share::SharedPackage { package_token, manifest, publication },
                recipients: serde_json::from_str(&recipients).map_err(library_store_error)?,
            });
        }
        Ok(records)
    }

    pub(crate) fn library_save_share(&self, record: &LibraryShareRecord) -> Result<(), NodeError> {
        let recipients = serde_json::to_string(&record.recipients).map_err(library_store_error)?;
        self.connect()?
            .execute(
                "INSERT INTO rem_library_shares
                    (item_id, package_token, manifest, publication, recipients_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(item_id) DO UPDATE SET recipients_json = excluded.recipients_json",
                params![
                    record.item_id,
                    record.shared.package_token.to_vec(),
                    record.shared.manifest,
                    record.shared.publication,
                    recipients
                ],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    /// Records a verified Offer once; a repeat keeps the existing state.
    pub(crate) fn library_record_offer(
        &self,
        package_destination: &str,
        sender: &str,
        manifest_hash: &str,
        now_ms: i64,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT OR IGNORE INTO rem_library_offers
                    (package_destination, sender, manifest_hash, state, received_ts_ms)
                 VALUES (?1, ?2, ?3, 'remote', ?4)",
                params![package_destination, sender, manifest_hash, now_ms],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    pub(crate) fn library_set_offer_state(
        &self,
        package_destination: &str,
        state: &str,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "UPDATE rem_library_offers SET state = ?2 WHERE package_destination = ?1",
                params![package_destination, state],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    pub(crate) fn library_remove_offer(&self, package_destination: &str) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "DELETE FROM rem_library_offers WHERE package_destination = ?1",
                params![package_destination],
            )
            .map(|_| ())
            .map_err(library_store_error)
    }

    /// Pending Offers as remote "Shared With Me" items: metadata only, the
    /// name and size are learned from the signed manifest on download.
    pub(crate) fn library_offer_items(
        &self,
    ) -> Result<Vec<r3akt_library_core::LibraryItem>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT package_destination, sender, state, received_ts_ms FROM rem_library_offers",
            )
            .map_err(library_store_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(library_store_error)?;
        let mut items = Vec::new();
        for row in rows {
            let (package, sender, state, received) = row.map_err(library_store_error)?;
            let availability = match state.as_str() {
                "downloading" => r3akt_library_core::Availability::Downloading,
                "failed" => r3akt_library_core::Availability::Failed,
                _ => r3akt_library_core::Availability::Remote,
            };
            items.push(r3akt_library_core::LibraryItem {
                item_id: library_offer_item_id(&package),
                legacy_file_id: None,
                kind: r3akt_library_core::ItemKind::File,
                source: r3akt_library_core::Source::Peer,
                availability,
                name: format!("Shared by {}", &sender[..sender.len().min(8)]),
                media_type: None,
                size: 0,
                content_sha256: None,
                rpp: None,
                publisher: Some(sender),
                mission_uids: Vec::new(),
                team_uid: None,
                created_ts_ms: received,
                updated_ts_ms: received,
            });
        }
        Ok(items)
    }
}

impl AppStateStore {
    /// Adds a verified fetched file as a "Shared With Me" item, records its
    /// RPP identities and publisher, and retires the Offer.
    pub(crate) fn library_receive(
        &self,
        package_destination: &str,
        fetched: &rpp_core::rns_share::FetchedFile,
    ) -> Result<r3akt_library_core::LibraryItem, NodeError> {
        let sender: Option<String> = self
            .connect()?
            .query_row(
                "SELECT sender FROM rem_library_offers WHERE package_destination = ?1",
                params![package_destination],
                |row| row.get(0),
            )
            .optional()
            .map_err(library_store_error)?;
        let now = crate::numeric::u64_to_i64_saturating(now_ms());
        let mut item =
            self.library_import(&fetched.path, &fetched.name, fetched.media_type.as_deref(), now)?;
        let file = fetched.manifest.manifest.files.first();
        item.source = r3akt_library_core::Source::Peer;
        item.publisher = sender;
        item.rpp = file.map(|file| r3akt_library_core::RppReference {
            package_destination: package_destination.to_string(),
            file_destination: hex::encode(file.destination),
            manifest_hash: hex::encode(fetched.manifest.hash),
        });
        item.validate().map_err(library_invalid)?;
        self.library_put(&item)?;
        self.library_remove_offer(package_destination)?;
        // The verified copy now lives in the Library; drop the transfer copy.
        if let Some(dir) = fetched.path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
        Ok(item)
    }
}
