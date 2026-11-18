/// Command ID prefix of REM's `mission.marker.picture.get` requests; the
/// receiver only treats results with this prefix as picture answers.
pub(crate) const MARKER_PICTURE_GET_PREFIX: &str = "marker-picture-get-";

/// The `mission.marker.picture.get` command for picture `message_id`. The
/// picture ID also goes in `correlation_id`, which RCH echoes on every
/// answer, including rejections that carry no `message_id`.
fn marker_picture_get_fields(identity_hex: &str, message_id: &str) -> Result<Vec<u8>, NodeError> {
    let command = r3akt_profile_rch::MissionCommandEnvelope {
        command_id: format!("{MARKER_PICTURE_GET_PREFIX}{}", map_marker_command_id()),
        source: r3akt_profile_rch::RchSource::new(identity_hex),
        timestamp: r3akt_map_core::time::iso8601_from_unix_ms(crate::numeric::u64_to_i64_saturating(
            now_ms(),
        )),
        command_type: r3akt_map_core::marker_picture::GET_COMMAND.to_string(),
        args: json!({ "message_id": message_id }),
        correlation_id: Some(message_id.to_string()),
        topics: Vec::new(),
    };
    r3akt_profile_rch::encode_commands(&[command]).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
    })
}

impl Node {
    /// Picture views of the stored picture markers, keyed by marker hash.
    pub(crate) fn list_map_marker_pictures(
        &self,
    ) -> Result<HashMap<String, crate::app_state::MapMarkerPictureView>, NodeError> {
        self.with_app_state(|app_state, _| app_state.list_map_marker_pictures())
    }

    /// The fetched reduced image of a picture marker, if RCH sent it.
    pub(crate) fn map_marker_picture_image(
        &self,
        object_destination_hash: String,
    ) -> Result<Option<Vec<u8>>, NodeError> {
        self.with_app_state(|app_state, _| {
            let Some(message_id) = picture_message_id(app_state, &object_destination_hash)? else {
                return Ok(None);
            };
            app_state.marker_picture_image(&message_id)
        })
    }

    /// Queues `mission.marker.picture.get` to the configured RCH hub. The
    /// answer arrives later in field 10 and updates the marker projection.
    pub(crate) fn request_map_marker_picture(&self, object_destination_hash: String) -> Result<(), NodeError> {
        let (identity_hex, message_id, hub, send_mode) = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let message_id = picture_message_id(&inner.app_state, &object_destination_hash)?
                .ok_or(NodeError::InvalidConfig {})?;
            let config = inner.active_config.as_ref().ok_or(NodeError::NotRunning {})?;
            let peers = inner
                .peers_snapshot
                .lock()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
                })?
                .clone();
            let propagation = inner
                .sync_status_snapshot
                .lock()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
                })?
                .active_propagation_node_hex
                .clone();
            let target = connected_hub_replication_target(&peers, propagation.as_deref(), config)?;
            let identity_hex = inner
                .status
                .lock()
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
                })?
                .identity_hex
                .clone();
            (identity_hex, message_id, target.app_destination_hex, target.send_mode)
        };
        let fields = marker_picture_get_fields(&identity_hex, &message_id)?;
        self.queue_bytes_with_class(
            hub,
            b"Picture request".to_vec(),
            Some(fields),
            send_mode,
            OutboundTrafficClass::Event {},
        )?;
        self.with_app_state(|app_state, bus| {
            app_state.set_marker_picture_image(&message_id, "requested", None)?;
            if let Some(invalidation) = app_state.invalidate_marker_picture(&message_id)? {
                emit_projection_invalidation(bus, invalidation);
            }
            Ok(())
        })
    }
}

fn picture_message_id(
    app_state: &AppStateStore,
    object_destination_hash: &str,
) -> Result<Option<String>, NodeError> {
    Ok(app_state
        .map_marker_core_record(object_destination_hash)?
        .and_then(|marker| marker.picture?.message_id))
}
