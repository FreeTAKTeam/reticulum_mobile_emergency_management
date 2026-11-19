// Mission Library (REM #356): local import, listing, removal and mission
// links. Integrity (hashing) and storage policy stay in Rust; the UI only
// sees metadata and a path it can hand to the platform viewer.

/// One page of the on-device catalog, filtered with the shared contract.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryListing {
    pub page: r3akt_library_core::LibraryPage,
    /// Every mission UID present on device, for the mission filter.
    pub missions: Vec<String>,
}

impl Node {
    fn library_changed(&self, bus: &EventBus, app_state: &AppStateStore, key: Option<String>) {
        match app_state.bump_projection_revision(
            ProjectionScope::Library {},
            key,
            Some("library".to_string()),
        ) {
            Ok(invalidation) => bus.emit(NodeEvent::ProjectionInvalidated { invalidation }),
            Err(error) => log::warn!("library projection revision failed: {error:?}"),
        }
    }

    /// Imports a file the platform already copied to `source`. Works offline;
    /// nothing is sent anywhere.
    pub(crate) fn library_import(
        &self,
        source: &std::path::Path,
        name: &str,
        media_type: Option<&str>,
    ) -> Result<r3akt_library_core::LibraryItem, NodeError> {
        self.with_app_state(|app_state, bus| {
            let now = crate::numeric::u64_to_i64_saturating(now_ms());
            let item = app_state.library_import(source, name, media_type, now)?;
            self.library_changed(bus, app_state, Some(item.item_id.clone()));
            Ok(item)
        })
    }

    pub(crate) fn library_list(
        &self,
        query: &r3akt_library_core::LibraryQuery,
    ) -> Result<LibraryListing, NodeError> {
        self.with_app_state(|app_state, _| {
            let items: Vec<_> =
                app_state.library_entries()?.into_iter().map(|entry| entry.item).collect();
            let mut missions: Vec<String> =
                items.iter().flat_map(|item| item.mission_uids.clone()).collect();
            missions.sort();
            missions.dedup();
            // Local items belong to this operator; every right is granted.
            let page = query.apply(items, |_, _| true);
            Ok(LibraryListing { page, missions })
        })
    }

    pub(crate) fn library_entry(&self, item_id: &str) -> Result<Option<LibraryEntry>, NodeError> {
        self.with_app_state(|app_state, _| app_state.library_entry(item_id))
    }

    pub(crate) fn library_delete(&self, item_id: &str) -> Result<bool, NodeError> {
        self.with_app_state(|app_state, bus| {
            let removed = app_state.library_delete(item_id)?;
            if removed {
                self.library_changed(bus, app_state, Some(item_id.to_string()));
            }
            Ok(removed)
        })
    }

    pub(crate) fn library_set_missions(
        &self,
        item_id: &str,
        mission_uids: Vec<String>,
    ) -> Result<Option<r3akt_library_core::LibraryItem>, NodeError> {
        self.with_app_state(|app_state, bus| {
            let now = crate::numeric::u64_to_i64_saturating(now_ms());
            let item = app_state.library_set_missions(item_id, mission_uids, now)?;
            if item.is_some() {
                self.library_changed(bus, app_state, Some(item_id.to_string()));
            }
            Ok(item)
        })
    }
}
