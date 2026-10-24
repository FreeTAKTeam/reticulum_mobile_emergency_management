// Persist history before custody can expire, evict or deliver its encrypted payload.
async fn persist_chat_before_local_admission(
    state: &NodeRuntimeState,
    prepared: &lxmf_runtime::PreparedOutgoing,
    requested_destination: &str,
    body: &[u8],
    title: Option<String>,
) -> Result<(), NodeError> {
    let message_id = prepared
        .message_id()
        .map_err(crate::sdk_bridge::map_sdk_error_to_node_error)?;
    let destination = prepared.destination();
    if state.app_state.message_for_wire(&message_id)?.is_some() {
        return Ok(());
    }
    let record = MessageRecord {
        message_id_hex: message_id.clone(),
        conversation_id: conversation_id_for(&destination),
        direction: MessageDirection::Outbound {},
        destination_hex: destination.clone(),
        source_hex: Some(address_hash_to_hex(
            &state.lxmf_destination.lock().await.desc.address_hash,
        )),
        requested_destination_hex: Some(requested_destination.to_owned()),
        delivery_destination_hex: Some(destination),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some(message_id),
        title,
        body_utf8: String::from_utf8_lossy(body).into_owned(),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        // A crash before the subsequent custody commit must not claim successful admission.
        state: MessageState::Failed {},
        transport_state: TransportDeliveryState::Failed {},
        application_ack_state: ApplicationAckState::NotRequired {},
        detail: Some("Local storage admission interrupted".to_owned()),
        sent_at_ms: None,
        received_at_ms: None,
        updated_at_ms: now_ms(),
    };
    state.app_state.upsert_message(&record)?;
    state
        .messaging
        .lock()
        .await
        .upsert_message(to_sdk_message_record(record));
    Ok(())
}
