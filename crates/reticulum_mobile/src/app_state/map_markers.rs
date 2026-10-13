// Map markers are owned by the shared RCH crate `r3akt-map-core`: validation,
// symbol normalization, idempotency and SQL all live there. This adapter only
// maps REM requests onto it and publishes projection invalidations.

/// Carries a shared-crate validation message as the error cause.
struct MapMarkerReason(String);

impl std::fmt::Debug for MapMarkerReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[track_caller]
fn map_marker_invalid(reason: impl Into<String>) -> NodeError {
    crate::error_context::contextual_node_error(
        NodeError::InvalidConfig {},
        MapMarkerReason(reason.into()),
    )
}

#[track_caller]
fn map_marker_store_error(error: r3akt_map_core::sqlite::StoreError) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
}

pub(crate) fn map_marker_record(marker: &r3akt_map_core::MarkerRecord) -> crate::types::MapMarkerRecord {
    crate::types::MapMarkerRecord {
        object_destination_hash: marker.object_destination_hash.clone(),
        origin_rch: marker.origin_rch.clone(),
        marker_type: marker.marker_type.clone(),
        symbol: marker.symbol.clone(),
        name: marker.name.clone(),
        category: marker.category.clone(),
        lat: marker.lat,
        lon: marker.lon,
        notes: marker.notes.clone(),
        created_at_ms: crate::numeric::i64_to_u64_saturating(marker.created_ts_ms),
        updated_at_ms: crate::numeric::i64_to_u64_saturating(marker.updated_ts_ms),
    }
}

pub(crate) fn map_marker_symbols() -> Vec<crate::types::MapMarkerSymbolRecord> {
    r3akt_map_core::symbols::CATALOG
        .iter()
        .map(|symbol| crate::types::MapMarkerSymbolRecord {
            id: symbol.id.to_string(),
            set: serde_json::to_value(symbol.set)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_default(),
            mdi: symbol.mdi.map(str::to_string),
            description: symbol.description.to_string(),
            tak: symbol.tak.map(str::to_string),
            category: symbol.category.to_string(),
            aliases: r3akt_map_core::symbols::ALIASES
                .iter()
                .filter(|(_, canonical)| *canonical == symbol.id)
                .map(|(alias, _)| (*alias).to_string())
                .collect(),
        })
        .collect()
}

fn optional_marker_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Builds a marker the way the RCH HTTP route does: type and symbol are
/// required catalog keys, the category defaults to the symbol and the name
/// to "Marker". Locally created markers have no originating RCH.
fn new_map_marker(
    request: &crate::types::MapMarkerCreateRequest,
) -> Result<r3akt_map_core::MarkerRecord, NodeError> {
    use r3akt_map_core::marker;
    let invalid = |error: r3akt_map_core::MarkerError| map_marker_invalid(error.to_string());
    let position = r3akt_map_core::Position::new(request.lat, request.lon).map_err(invalid)?;
    let marker_type = marker::marker_type(&request.marker_type).map_err(invalid)?;
    let symbol = marker::marker_symbol(&request.symbol).map_err(invalid)?;
    let category =
        optional_marker_text(request.category.as_deref()).unwrap_or_else(|| symbol.clone());
    let draft = r3akt_map_core::MarkerDraft {
        marker_type,
        symbol,
        name: optional_marker_text(request.name.as_deref()).unwrap_or_else(|| "Marker".to_string()),
        category,
        position,
        notes: optional_marker_text(request.notes.as_deref()),
        origin_rch: String::new(),
    };
    Ok(r3akt_map_core::MarkerRecord::create(
        draft,
        uuid_local_id(),
        crate::numeric::u64_to_i64_saturating(now_ms()),
    ))
}

fn uuid_local_id() -> String {
    let mut bytes = [0_u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    hex::encode(bytes)
}

impl AppStateStore {
    pub fn list_map_markers(&self) -> Result<Vec<crate::types::MapMarkerRecord>, NodeError> {
        let markers = r3akt_map_core::sqlite::load_all(&self.connect()?)
            .map_err(map_marker_store_error)?;
        Ok(markers.iter().map(map_marker_record).collect())
    }

    pub fn get_map_marker(
        &self,
        object_destination_hash: &str,
    ) -> Result<Option<crate::types::MapMarkerRecord>, NodeError> {
        let marker = r3akt_map_core::sqlite::get(&self.connect()?, object_destination_hash.trim())
            .map_err(map_marker_store_error)?;
        Ok(marker.as_ref().map(map_marker_record))
    }

    pub fn create_map_marker(
        &self,
        request: &crate::types::MapMarkerCreateRequest,
    ) -> Result<(crate::types::MapMarkerCreateResult, ProjectionInvalidation), NodeError> {
        let marker = new_map_marker(request)?;
        let mut connection = self.connect()?;
        let (stored, created) = match optional_marker_text(request.idempotency_key.as_deref()) {
            Some(key) => {
                r3akt_map_core::idempotency::validate_key(&key)
                    .map_err(|error| map_marker_invalid(error.to_string()))?;
                let creation = r3akt_map_core::sqlite::create_once(
                    &mut connection,
                    &marker,
                    &r3akt_map_core::idempotency::command_id(&key),
                    &r3akt_map_core::idempotency::creation_fingerprint(&marker),
                )
                .map_err(map_marker_store_error)?;
                match creation {
                    r3akt_map_core::sqlite::MarkerCreation::Created(marker) => (marker, true),
                    r3akt_map_core::sqlite::MarkerCreation::Replayed(marker) => (marker, false),
                    r3akt_map_core::sqlite::MarkerCreation::KeyConflict => {
                        return Err(map_marker_invalid(
                            "Idempotency-Key was already used with different marker fields",
                        ));
                    }
                }
            }
            None => {
                r3akt_map_core::sqlite::insert(&connection, &marker)
                    .map_err(map_marker_store_error)?;
                (marker, true)
            }
        };
        let invalidation = self.bump_projection_revision(
            ProjectionScope::MapMarkers {},
            Some(stored.object_destination_hash.clone()),
            Some(if created { "marker-created" } else { "marker-replayed" }.to_string()),
        )?;
        let result = crate::types::MapMarkerCreateResult {
            marker: map_marker_record(&stored),
            created,
        };
        Ok((result, invalidation))
    }

    pub fn rename_map_marker(
        &self,
        request: &crate::types::MapMarkerRenameRequest,
    ) -> Result<(crate::types::MapMarkerRecord, ProjectionInvalidation), NodeError> {
        let name = r3akt_map_core::marker::normalize_name(&request.name)
            .ok_or_else(|| map_marker_invalid("Marker name is required"))?;
        self.update_map_marker(&request.object_destination_hash, "marker-renamed", |marker, now| {
            marker.rename(name, now);
        })
    }

    pub fn move_map_marker(
        &self,
        request: &crate::types::MapMarkerMoveRequest,
    ) -> Result<(crate::types::MapMarkerRecord, ProjectionInvalidation), NodeError> {
        let position = r3akt_map_core::Position::new(request.lat, request.lon)
            .map_err(|error| map_marker_invalid(error.to_string()))?;
        self.update_map_marker(&request.object_destination_hash, "marker-moved", |marker, now| {
            marker.move_to(position, now);
        })
    }

    /// Deletes a marker; returns `None` when it does not exist.
    pub fn delete_map_marker(
        &self,
        object_destination_hash: &str,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let key = object_destination_hash.trim();
        let connection = self.connect()?;
        if r3akt_map_core::sqlite::get(&connection, key)
            .map_err(map_marker_store_error)?
            .is_none()
        {
            return Ok(None);
        }
        r3akt_map_core::sqlite::delete(&connection, key).map_err(map_marker_store_error)?;
        self.bump_projection_revision(
            ProjectionScope::MapMarkers {},
            Some(key.to_string()),
            Some("marker-deleted".to_string()),
        )
        .map(Some)
    }

    fn update_map_marker(
        &self,
        object_destination_hash: &str,
        reason: &str,
        apply: impl FnOnce(&mut r3akt_map_core::MarkerRecord, i64),
    ) -> Result<(crate::types::MapMarkerRecord, ProjectionInvalidation), NodeError> {
        let key = object_destination_hash.trim();
        let now = crate::numeric::u64_to_i64_saturating(now_ms());
        let updated = r3akt_map_core::sqlite::update(&mut self.connect()?, key, |marker| {
            apply(marker, now);
        })
        .map_err(map_marker_store_error)?
        .ok_or_else(|| map_marker_invalid("Marker not found"))?;
        let invalidation = self.bump_projection_revision(
            ProjectionScope::MapMarkers {},
            Some(key.to_string()),
            Some(reason.to_string()),
        )?;
        Ok((map_marker_record(&updated), invalidation))
    }
}
