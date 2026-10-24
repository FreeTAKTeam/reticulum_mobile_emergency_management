pub(crate) const LOCAL_PROPAGATION_LORA_INTERVAL_SECONDS: u32 = 300;
// Phase 0 measurement was waived; keep each LoRa transfer conservatively bounded.
pub(crate) const LOCAL_PROPAGATION_LORA_TRANSFER_BYTES: u32 = 2048;

pub(crate) fn map_local_propagation_error(
    error: lxmf_runtime::propagation_store::Error,
) -> NodeError {
    use lxmf_runtime::propagation_store::Error;
    match error {
        Error::UnresolvedRecipient => NodeError::UnresolvedRecipient {},
        Error::PayloadTooLarge => NodeError::PayloadTooLarge {},
        Error::StoreFull => NodeError::StoreFull {},
        Error::Expired => NodeError::Expired {},
        Error::Storage(_) => NodeError::IoError {},
        _ => NodeError::InternalError {},
    }
}

pub(crate) fn local_propagation_record(
    status: &lxmf_runtime::propagation_store::Status,
) -> crate::types::LocalPropagationRecord {
    use crate::types::{LocalPropagationRecord, LocalPropagationState as ViewState};
    use lxmf_runtime::propagation_store::State;
    let state = match status.state {
        State::StoredLocally => ViewState::StoredLocally {},
        State::Uploading => ViewState::Uploading {},
        State::UploadedUnverified => ViewState::UploadedUnverified {},
        State::RelayHolds => ViewState::RelayHolds {},
        State::Delivered => ViewState::Delivered {},
        State::Acknowledged => ViewState::Acknowledged {},
        State::Expired => ViewState::Expired {},
        State::Evicted => ViewState::Evicted {},
    };
    LocalPropagationRecord {
        transient_id_hex: status.transient_id.clone(),
        message_id_hex: status.message_id.clone(),
        destination_hex: status.destination.clone(),
        state,
        encrypted_bytes: status.encrypted_bytes,
        admitted_at_ms: crate::numeric::i64_to_u64_saturating(status.admitted_at)
            .saturating_mul(1000),
        expires_at_ms: crate::numeric::i64_to_u64_saturating(status.expires_at)
            .saturating_mul(1000),
        may_exist_on_relay: status.uploaded,
    }
}

impl RuntimeLxmfSdk {
    pub(crate) async fn prepare_local_outgoing(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<&str>,
        fields: Option<&[u8]>,
        metadata: Option<&MissionSyncMetadata>,
    ) -> Result<(lxmf_runtime::PreparedOutgoing, SendRequest), NodeError> {
        let store = self
            .local_propagation
            .as_ref()
            .ok_or(NodeError::IoError {})?;
        let source = self
            .transport
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .to_hex_string();
        let mut request = SendRequest::new(
            source,
            destination.to_hex_string(),
            json!({"title":title.unwrap_or_default(),"content_base64":BASE64_STANDARD.encode(content)}),
        );
        if let Some(fields) = fields {
            request =
                request.with_extension(EXT_FIELDS_BASE64, json!(BASE64_STANDARD.encode(fields)));
        }
        if let Some(key) = metadata
            .filter(|metadata| !metadata_uses_compact_eam_tracking_marker(metadata))
            .and_then(MissionSyncMetadata::tracking_key)
        {
            request = request.with_correlation_id(key.to_owned());
        }
        let prepared = store
            .prepare(&request)
            .await
            .map_err(map_sdk_error_to_node_error)?;
        Ok((prepared, request))
    }
}
