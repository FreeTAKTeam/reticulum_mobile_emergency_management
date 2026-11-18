// Picture markers (r3akt-uas #19): the relayed thumbnail in field 6 and RCH's
// field-10 answers to `mission.marker.picture.get`. Decoding and hash checks
// come from `r3akt_map_core::marker_picture`.

/// Stores the hash-checked thumbnail of a relayed picture marker. A missing
/// or mismatching thumbnail is dropped; the marker itself is applied anyway
/// and the map draws the picture fallback.
pub(crate) fn store_received_marker_picture_thumbnail(app_state: &AppStateStore, fields_bytes: &[u8]) {
    let Some(fields) = r3akt_map_core::msgpack::to_json(fields_bytes) else {
        return;
    };
    let received = match r3akt_map_core::marker_picture::decode_relay(&fields, String::new()) {
        Ok(Some(received)) => received,
        Ok(None) => return,
        Err(error) => {
            warn!("[lxmf][marker] picture marker rejected error={error}");
            return;
        }
    };
    let message_id = received
        .marker
        .picture
        .as_ref()
        .and_then(|picture| picture.message_id.as_deref());
    match (message_id, received.thumbnail) {
        (Some(message_id), Some(thumbnail)) => {
            if let Err(error) = app_state.store_marker_picture_thumbnail(message_id, &thumbnail) {
                warn!("[lxmf][marker] picture thumbnail not stored: {error}");
            }
        }
        (_, None) => warn!(
            "[lxmf][marker] picture thumbnail missing or hash mismatch marker={}",
            received.marker.object_destination_hash
        ),
        (None, Some(_)) => warn!(
            "[lxmf][marker] picture without message_id marker={}",
            received.marker.object_destination_hash
        ),
    }
}

/// One RCH answer to REM's picture request: `(message_id, status, image)`.
/// `status` is `ok`, `pending`, `unavailable`, `unauthorized`, or `invalid`
/// when the image does not match its announced SHA-256.
pub(crate) fn marker_picture_answers(
    fields_bytes: &[u8],
) -> Vec<(String, String, Option<Vec<u8>>)> {
    use r3akt_map_core::marker_picture::{image_field, message_id, sha256_hex, FIELD_IMAGE_KEY};
    let Ok(results) = r3akt_profile_rch::decode_results(fields_bytes) else {
        return Vec::new();
    };
    let fields = r3akt_map_core::msgpack::to_json(fields_bytes);
    results
        .into_iter()
        .filter(|result| {
            result
                .command_id
                .starts_with(crate::node::MARKER_PICTURE_GET_PREFIX)
                && result.status != r3akt_profile_rch::CommandResultStatus::Accepted
        })
        .filter_map(|result| {
            let id = message_id(result.correlation_id.as_deref()?).ok()?;
            let text = |key: &str| result.result.get(key).and_then(serde_json::Value::as_str);
            let status = text("status")
                .or(result.reason_code.as_deref())
                .unwrap_or("unavailable")
                .to_ascii_lowercase();
            if status != "ok" {
                return Some((id, status, None));
            }
            let image = fields
                .as_ref()
                .and_then(|fields| image_field(fields.get(FIELD_IMAGE_KEY)?))
                .map(|(_, data)| data)
                .filter(|data| {
                    text("sha256").is_some_and(|sha| sha.eq_ignore_ascii_case(&sha256_hex(data)))
                });
            Some(match image {
                Some(image) => (id, status, Some(image)),
                None => (id, "invalid".to_string(), None),
            })
        })
        .collect()
}

/// Applies RCH's answers to picture requests; only the configured hub is
/// trusted to answer. Returns whether the message was such an answer.
fn persist_received_marker_picture_answer(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source: Option<&str>,
    fields_bytes: Option<&[u8]>,
) -> bool {
    let hub = state
        .config
        .hub_identity_hash
        .as_deref()
        .and_then(crate::delivery_policy::normalize_hex_32);
    let (Some(source), Some(fields_bytes)) = (source, fields_bytes) else {
        return false;
    };
    if hub.as_deref() != Some(source) {
        return false;
    }
    let answers = marker_picture_answers(fields_bytes);
    for (message_id, status, image) in &answers {
        info!("[lxmf][marker] picture answer message_id={message_id} status={status}");
        let stored = state
            .app_state
            .set_marker_picture_image(message_id, status, image.as_deref())
            .and_then(|()| state.app_state.invalidate_marker_picture(message_id));
        match stored {
            Ok(Some(invalidation)) => bus.emit(NodeEvent::ProjectionInvalidated { invalidation }),
            Ok(None) => {}
            Err(error) => warn!("[lxmf][marker] picture answer not stored: {error}"),
        }
    }
    !answers.is_empty()
}
