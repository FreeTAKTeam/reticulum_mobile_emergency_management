async fn retry_pending_ack_timeout_via_propagation(
    state: &NodeRuntimeState,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
) -> Result<bool, String> {
    let has_active_relay = has_active_propagation_relay(state).await;
    let Some(mut resend) = pending.resend.clone() else {
        return Ok(false);
    };
    if should_retry_pending_ack_timeout_via_direct(pending) {
        resend.direct_ack_retry_attempted = true;
        info!(
            "[lxmf][mission] ack timeout message_id={} destination={} command={} correlation={}; retrying direct delivery",
            pending.message_id_hex,
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
        );
        match send_lxmf_with_delivery_policy(
            state,
            bus,
            resend.requested_destination_hex.as_str(),
            resend.body.as_slice(),
            resend.title.clone(),
            resend.fields_bytes.clone(),
            Some(resend.metadata.clone()),
            SendMode::DirectOnly {},
            resend.send_task_class.direct_recovery_equivalent(),
        )
        .await
        {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                let Some(registered) = register_pending_lxmf_delivery(
                    state,
                    &report,
                    Some(resend),
                    Some(pending.message_id_hex.clone()),
                )
                .await
                else {
                    return Err("direct retry did not register pending delivery".to_string());
                };
                let retry_pending = &registered.pending;
                state.sdk.record_delivery_sent(
                    &retry_pending.message_id_hex,
                    &retry_pending.destination_hex,
                    retry_pending.correlation_id.as_deref(),
                    retry_pending.command_id.as_deref(),
                    retry_pending.command_type.as_deref(),
                    retry_pending.event_uid.as_deref(),
                    retry_pending.mission_uid.as_deref(),
                );
                emit_lxmf_delivery(
                    bus,
                    retry_pending,
                    LxmfDeliveryStatus::Sent {},
                    Some("ack timeout; retrying direct delivery".to_string()),
                );
                info!(
                    "[lxmf][mission] resent direct after ack timeout original_message_id={} retry_message_id={} destination={} command={} correlation={}",
                    retry_pending.message_id_hex,
                    report.message_id_hex,
                    retry_pending.destination_hex,
                    retry_pending.command_type.as_deref().unwrap_or("-"),
                    retry_pending.correlation_id.as_deref().unwrap_or("-"),
                );
                if let Some(buffered_ack) = registered.buffered_ack {
                    acknowledge_pending_with_buffered_ack(state, bus, retry_pending, buffered_ack)
                        .await;
                }
                return Ok(true);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] direct retry after ack timeout failed destination={} command={} correlation={} outcome={:?}",
                    pending.destination_hex,
                    pending.command_type.as_deref().unwrap_or("-"),
                    pending.correlation_id.as_deref().unwrap_or("-"),
                    send_outcome_to_udl(report.outcome),
                );
            }
            Err(err) => {
                info!(
                    "[lxmf][mission] direct retry after ack timeout errored destination={} command={} correlation={} err={}",
                    pending.destination_hex,
                    pending.command_type.as_deref().unwrap_or("-"),
                    pending.correlation_id.as_deref().unwrap_or("-"),
                    err,
                );
            }
        }
    }
    if !should_retry_pending_ack_timeout_via_propagation(pending, has_active_relay) {
        return Ok(false);
    }
    resend.propagation_fallback_attempted = true;
    info!(
        "[lxmf][mission] ack timeout message_id={} destination={} command={} correlation={}; retrying via propagation relay",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
    );
    let report = send_lxmf_with_delivery_policy(
        state,
        bus,
        resend.requested_destination_hex.as_str(),
        resend.body.as_slice(),
        resend.title.clone(),
        resend.fields_bytes.clone(),
        Some(resend.metadata.clone()),
        SendMode::PropagationOnly {},
        resend.send_task_class.direct_recovery_equivalent(),
    )
    .await
    .map_err(|err| err.to_string())?;

    if !lxmf_send_succeeded(report.outcome) {
        return Err(format!("{:?}", send_outcome_to_udl(report.outcome)));
    }

    let Some(registered) = register_pending_lxmf_delivery(
        state,
        &report,
        Some(resend),
        Some(pending.message_id_hex.clone()),
    )
    .await
    else {
        return Err("propagation retry did not register pending delivery".to_string());
    };
    let retry_pending = &registered.pending;
    state.sdk.record_delivery_sent(
        &retry_pending.message_id_hex,
        &retry_pending.destination_hex,
        retry_pending.correlation_id.as_deref(),
        retry_pending.command_id.as_deref(),
        retry_pending.command_type.as_deref(),
        retry_pending.event_uid.as_deref(),
        retry_pending.mission_uid.as_deref(),
    );
    emit_lxmf_delivery(
        bus,
        retry_pending,
        LxmfDeliveryStatus::SentToPropagation {},
        Some("ack timeout; retrying via propagation".to_string()),
    );
    info!(
        "[lxmf][mission] resent after ack timeout original_message_id={} retry_message_id={} destination={} command={} correlation={}",
        retry_pending.message_id_hex,
        report.message_id_hex,
        retry_pending.destination_hex,
        retry_pending.command_type.as_deref().unwrap_or("-"),
        retry_pending.correlation_id.as_deref().unwrap_or("-"),
    );
    if let Some(buffered_ack) = registered.buffered_ack {
        acknowledge_pending_with_buffered_ack(state, bus, retry_pending, buffered_ack).await;
    }
    Ok(true)
}

fn lxmf_send_succeeded(outcome: RnsSendOutcome) -> bool {
    matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    )
}

fn lxmf_delivery_status_for(report: &LxmfSendReport) -> LxmfDeliveryStatus {
    if report.used_propagation_node && lxmf_send_succeeded(report.outcome) {
        LxmfDeliveryStatus::SentToPropagation {}
    } else if matches!(report.representation, LxmfDeliveryRepresentation::Resource {})
        && lxmf_send_succeeded(report.outcome)
    {
        LxmfDeliveryStatus::Delivered {}
    } else {
        LxmfDeliveryStatus::Sent {}
    }
}

fn node_error_code(err: &NodeError) -> &'static str {
    match err {
        NodeError::InvalidConfig {} => "InvalidConfig",
        NodeError::IoError {} => "IoError",
        NodeError::NetworkError {} => "NetworkError",
        NodeError::ReticulumError {} => "ReticulumError",
        NodeError::AlreadyRunning {} => "AlreadyRunning",
        NodeError::NotRunning {} => "NotRunning",
        NodeError::Timeout {} => "Timeout",
        NodeError::LxmfWireEncodeError {} => "LxmfWireEncodeError",
        NodeError::LxmfMessageIdParseError {} => "LxmfMessageIdParseError",
        NodeError::LxmfPacketTooLarge {} => "LxmfPacketTooLarge",
        NodeError::LxmfPacketBuildError {} => "LxmfPacketBuildError",
        NodeError::EventStreamClosed {} => "EventStreamClosed",
        NodeError::InternalError {} => "InternalError",
    }
}

fn is_retriable_lxmf_error(err: &NodeError) -> bool {
    matches!(
        err,
        NodeError::NetworkError {}
            | NodeError::Timeout {}
            | NodeError::ReticulumError {}
            | NodeError::InternalError {}
    )
}

fn is_accepted_result_metadata(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.result_present && metadata.result_status.as_deref() == Some("accepted")
    })
}

fn is_sos_status_metadata(metadata: Option<&MissionSyncMetadata>) -> bool {
    metadata.is_some_and(|metadata| {
        metadata.command_present && metadata.command_type.as_deref() == Some("sos.status")
    })
}

fn should_serialize_lxmf_destination_send(is_accepted_result: bool, is_sos_status: bool) -> bool {
    !is_accepted_result && !is_sos_status
}
