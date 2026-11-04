async fn import_relay_chat(
    state: &NodeRuntimeState,
    bus: &EventBus,
    plugin_id: &str,
    record: MessageRecord,
) -> Result<(), NodeError> {
    if !state
        .app_state
        .get_plugin(plugin_id)?
        .as_ref()
        .is_some_and(crate::app_state::relay_allowed)
    {
        return Err(NodeError::InvalidConfig {});
    }
    let local_hash = state
        .status
        .lock()
        .map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?
        .lxmf_destination_hex
        .clone();
    if matches!(record.direction, MessageDirection::Inbound {})
        && record.destination_hex != local_hash
        && record.destination_hex != state.app_destination_hex
    {
        return Err(NodeError::InvalidConfig {});
    }
    if let Some(existing) = state.app_state.message_for_wire(&record.message_id_hex)? {
        if existing.body_utf8 != record.body_utf8 {
            return Err(NodeError::InvalidConfig {});
        }
        let upgrading_receipt = matches!(record.direction, MessageDirection::Outbound {})
            && existing
                .detail
                .as_deref()
                .is_some_and(|detail| detail.ends_with(":radio-attempt-unconfirmed"))
            && record
                .detail
                .as_deref()
                .is_some_and(|detail| detail.ends_with(":radio-accepted-unconfirmed"));
        if !upgrading_receipt {
            return Ok(());
        }
    }
    let emit_received = matches!(record.direction, MessageDirection::Inbound {});
    if upsert_message_record(state, bus, record, emit_received).await {
        Ok(())
    } else {
        Err(NodeError::IoError {})
    }
}
