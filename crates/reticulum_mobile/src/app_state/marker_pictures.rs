// Picture markers (r3akt-uas #19). The picture metadata lives on the shared
// marker record; this REM table only keeps the bytes the record cannot hold,
// keyed by the picture ID (the marker message's LXMF message ID): the
// relayed thumbnail and the reduced image fetched from RCH on demand.

/// What the map needs to draw and inspect a picture marker.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapMarkerPictureView {
    pub message_id: Option<String>,
    pub captured_at: Option<String>,
    pub crop: Option<String>,
    pub camera: Option<serde_json::Value>,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    /// `data:` URL of the hash-checked thumbnail; `None` draws the fallback.
    pub thumbnail_data_url: Option<String>,
    /// `requested`, or the last RCH answer: `ok`, `pending`, `unavailable`,
    /// `unauthorized`, `invalid` (image hash mismatch).
    pub image_status: Option<String>,
}

fn marker_picture_store_error(error: rusqlite::Error) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
}

pub(crate) fn jpeg_data_url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

impl AppStateStore {
    fn initialize_marker_pictures(connection: &Connection) -> Result<(), NodeError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS rem_marker_pictures (
                    message_id TEXT PRIMARY KEY,
                    thumbnail BLOB,
                    image BLOB,
                    image_status TEXT
                );",
            )
            .map_err(marker_picture_store_error)
    }

    pub(crate) fn store_marker_picture_thumbnail(
        &self,
        message_id: &str,
        thumbnail: &[u8],
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT INTO rem_marker_pictures (message_id, thumbnail) VALUES (?1, ?2)
                 ON CONFLICT(message_id) DO UPDATE SET thumbnail = excluded.thumbnail",
                params![message_id, thumbnail],
            )
            .map(|_| ())
            .map_err(marker_picture_store_error)
    }

    /// Records a fetch state; an image is only replaced by a newer image.
    pub(crate) fn set_marker_picture_image(
        &self,
        message_id: &str,
        status: &str,
        image: Option<&[u8]>,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT INTO rem_marker_pictures (message_id, image, image_status) VALUES (?1, ?2, ?3)
                 ON CONFLICT(message_id) DO UPDATE SET
                    image = COALESCE(excluded.image, image), image_status = excluded.image_status",
                params![message_id, image, status],
            )
            .map(|_| ())
            .map_err(marker_picture_store_error)
    }

    fn marker_picture_row(
        &self,
        message_id: &str,
    ) -> Result<(Option<Vec<u8>>, Option<String>), NodeError> {
        self.connect()?
            .query_row(
                "SELECT thumbnail, image_status FROM rem_marker_pictures WHERE message_id = ?1",
                params![message_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map(Option::unwrap_or_default)
            .map_err(marker_picture_store_error)
    }

    /// The fetched reduced image of a picture marker, if RCH sent it.
    pub fn marker_picture_image(&self, message_id: &str) -> Result<Option<Vec<u8>>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT image FROM rem_marker_pictures WHERE message_id = ?1",
                params![message_id],
                |row| row.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()
            .map(Option::flatten)
            .map_err(marker_picture_store_error)
    }

    /// Picture views of every stored picture marker, by marker hash.
    pub fn list_map_marker_pictures(
        &self,
    ) -> Result<HashMap<String, MapMarkerPictureView>, NodeError> {
        let markers =
            r3akt_map_core::sqlite::load_all(&self.connect()?).map_err(map_marker_store_error)?;
        markers
            .into_iter()
            .filter_map(|marker| Some((marker.object_destination_hash, marker.picture?)))
            .map(|(hash, picture)| {
                let (thumbnail, image_status) = match picture.message_id.as_deref() {
                    Some(id) => self.marker_picture_row(id)?,
                    None => (None, None),
                };
                let view = MapMarkerPictureView {
                    message_id: picture.message_id,
                    captured_at: picture.captured_at,
                    crop: picture
                        .crop
                        .and_then(|crop| serde_json::to_value(crop).ok())
                        .and_then(|crop| crop.as_str().map(str::to_string)),
                    camera: picture
                        .camera
                        .and_then(|camera| serde_json::to_value(camera).ok()),
                    thumbnail_width: picture.thumbnail.width,
                    thumbnail_height: picture.thumbnail.height,
                    thumbnail_data_url: thumbnail.as_deref().map(jpeg_data_url),
                    image_status,
                };
                Ok((hash, view))
            })
            .collect()
    }

    /// The marker that carries picture `message_id`, for invalidations.
    pub(crate) fn map_marker_for_picture(
        &self,
        message_id: &str,
    ) -> Result<Option<r3akt_map_core::MarkerRecord>, NodeError> {
        let markers =
            r3akt_map_core::sqlite::load_all(&self.connect()?).map_err(map_marker_store_error)?;
        Ok(markers.into_iter().find(|marker| {
            marker
                .picture
                .as_ref()
                .and_then(|picture| picture.message_id.as_deref())
                == Some(message_id)
        }))
    }

    /// Bumps the marker projection for the marker carrying `message_id`.
    pub(crate) fn invalidate_marker_picture(
        &self,
        message_id: &str,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let Some(marker) = self.map_marker_for_picture(message_id)? else {
            return Ok(None);
        };
        self.bump_projection_revision(
            ProjectionScope::MapMarkers {},
            Some(marker.object_destination_hash),
            Some("marker-picture".to_string()),
        )
        .map(Some)
    }
}
