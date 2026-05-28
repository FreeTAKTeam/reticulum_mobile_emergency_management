use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

use crate::types::{
    LocalPropagationDeliveryStatus, LocalPropagationMessageRecord,
    LocalPropagationReplicationStatus,
};

pub(crate) fn encode_payload(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

pub(crate) fn decode_payload(value: &str) -> Option<Vec<u8>> {
    BASE64_STANDARD.decode(value).ok()
}

pub(crate) fn build_local_message_id(
    sender_id_hex: &str,
    target_hex: &str,
    body: &[u8],
    title: Option<&str>,
    fields_bytes: Option<&[u8]>,
    accepted_at_ms: u64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rem-local-propagation-v1");
    digest.update(sender_id_hex.trim().to_ascii_lowercase().as_bytes());
    digest.update(target_hex.trim().to_ascii_lowercase().as_bytes());
    digest.update(accepted_at_ms.to_be_bytes());
    digest.update((body.len() as u64).to_be_bytes());
    digest.update(body);
    if let Some(title) = title {
        digest.update((title.len() as u64).to_be_bytes());
        digest.update(title.as_bytes());
    }
    if let Some(fields_bytes) = fields_bytes {
        digest.update((fields_bytes.len() as u64).to_be_bytes());
        digest.update(fields_bytes);
    }
    hex::encode(digest.finalize())
}

#[expect(
    clippy::too_many_arguments,
    reason = "local propagation records preserve the durable send envelope fields"
)]
pub(crate) fn build_local_record(
    message_id_hex: String,
    sender_id_hex: String,
    target_hex: String,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    accepted_at_ms: u64,
    detail: Option<String>,
) -> LocalPropagationMessageRecord {
    LocalPropagationMessageRecord {
        message_id_hex,
        sender_id_hex,
        target_hex,
        created_at_ms: accepted_at_ms,
        updated_at_ms: accepted_at_ms,
        payload_base64: encode_payload(body),
        title,
        message_type: "lxmf".to_string(),
        delivery_status: LocalPropagationDeliveryStatus::Pending {},
        replication_status: LocalPropagationReplicationStatus::Pending {},
        retry_count: 0,
        last_attempted_delivery_at_ms: None,
        last_attempted_replication_at_ms: None,
        next_retry_at_ms: None,
        acknowledgement_state: None,
        signature_metadata: Some("local-node-accepted".to_string()),
        causal_metadata: Some(format!("accepted_at_ms={accepted_at_ms}")),
        fields_base64: fields_bytes.as_deref().map(encode_payload),
        last_error: detail,
        replicated_node_hex: None,
    }
}
