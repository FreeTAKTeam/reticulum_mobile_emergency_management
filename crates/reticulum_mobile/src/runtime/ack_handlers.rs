async fn ack_pending_lxmf_delivery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    metadata: &MissionSyncMetadata,
) {
    if !metadata.result_present && !metadata.event_present {
        return;
    }

    let Some(source_hex) = source_hex else {
        return;
    };

    let detail = metadata.ack_detail().map(ToOwned::to_owned);
    let application_ack_state = application_ack_state_for_mission_metadata(metadata);
    let mut guard = state.pending_lxmf_deliveries.lock().await;
    let mut matched: Option<PendingLxmfDelivery> = None;

    for key in [
        metadata.correlation_id.as_deref(),
        metadata.command_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(candidate) = guard.remove(key) {
            matched = Some(candidate);
            break;
        }
    }

    drop(guard);

    let Some(pending) = matched else {
        if let Some(tracking_key) = metadata.tracking_key().map(ToOwned::to_owned) {
            state.pending_lxmf_acknowledgements.lock().await.insert(
                tracking_key.clone(),
                PendingLxmfAcknowledgement {
                    source_hex: source_hex.to_string(),
                    detail: detail.clone(),
                    application_ack_state,
                    buffered_at_ms: now_ms(),
                },
            );
            info!(
                "[lxmf][mission] buffered acknowledgement source={} command={} correlation={} detail={}",
                source_hex,
                metadata.command_type.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
                detail.as_deref().unwrap_or("-"),
            );
        }
        return;
    };
    if !peer_destinations_equivalent(state, pending.destination_hex.as_str(), source_hex).await {
        if let Some(tracking_key) = pending
            .command_id
            .as_deref()
            .or(pending.correlation_id.as_deref())
            .map(ToOwned::to_owned)
        {
            state
                .pending_lxmf_deliveries
                .lock()
                .await
                .insert(tracking_key, pending);
        }
        return;
    }

    record_peer_link_state(state, bus, source_hex, true).await;
    state.sdk.record_delivery_acknowledged(
        &pending.message_id_hex,
        &pending.destination_hex,
        Some(source_hex),
        pending.correlation_id.as_deref(),
        pending.command_id.as_deref(),
        pending.command_type.as_deref(),
        pending.event_uid.as_deref(),
        pending.mission_uid.as_deref(),
        detail.as_deref(),
    );
    emit_lxmf_delivery_with_source(
        bus,
        &pending,
        Some(source_hex.to_string()),
        LxmfDeliveryStatus::Acknowledged {},
        application_ack_state,
        detail.clone(),
    );
    info!(
        "[lxmf][mission] acknowledged message_id={} destination={} command={} correlation={} detail={}",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
        detail.as_deref().unwrap_or("-"),
    );
}

async fn send_operational_ack_if_needed(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    metadata: Option<&MissionSyncMetadata>,
    persisted: bool,
) {
    if !persisted {
        return;
    }
    let Some(ack) = operational_ack_from_metadata(source_hex, metadata) else {
        return;
    };
    let local_lxmf_hex = {
        let destination = state.lxmf_destination.lock().await;
        address_hash_to_hex(&destination.desc.address_hash)
    };
    if ack.destination_hex == local_lxmf_hex {
        return;
    }
    let fields = match build_compact_operational_ack_fields(&ack) {
        Ok(fields) => fields,
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: node_error_code(&err).to_string(),
                message: format!(
                    "operational acknowledgement build failed command={} reason={}",
                    ack.command_id, err
                ),
            });
            return;
        }
    };
    let ack_metadata = parse_mission_sync_metadata(fields.as_slice());
    match send_lxmf_with_delivery_policy(
        state,
        bus,
        ack.destination_hex.as_str(),
        &[],
        None,
        Some(fields),
        ack_metadata,
        SendMode::Auto {},
        SendTaskClass::MissionAck,
    )
    .await
    {
        Ok(report) => {
            info!(
                "[lxmf][mission] sent application result destination={} message_id={} command={} correlation={} type={}",
                report.resolved_destination_hex,
                report.message_id_hex,
                ack.command_id,
                ack.correlation_id.as_deref().unwrap_or("-"),
                ack.command_type.as_deref().unwrap_or("-"),
            );
        }
        Err(err) => {
            bus.emit(NodeEvent::Error {
                code: node_error_code(&err).to_string(),
                message: format!(
                    "application result send failed destination={} command={} reason={}",
                    ack.destination_hex, ack.command_id, err
                ),
            });
        }
    }
}

async fn acknowledge_chat_delivery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    body_utf8: &str,
) -> bool {
    let Some(message_id_hex) = parse_chat_delivery_ack_body(body_utf8) else {
        return false;
    };
    let Some(source_hex) = source_hex else {
        return true;
    };
    let Some(source_peer) = peer_for_any_destination_hex(state, source_hex).await else {
        return true;
    };
    if !requires_legacy_rem_chat_ack(source_peer.app_data.as_deref()) {
        return true;
    }
    let Some(outbound) = state
        .messaging
        .lock()
        .await
        .outbound_for_message_or_wire_id(message_id_hex.as_str())
    else {
        return true;
    };
    let Some(requested_destination) = normalize_hex_32(&outbound.request.destination_hex) else {
        return true;
    };
    if !peer_matches_hex(&source_peer, requested_destination.as_str()) {
        return true;
    }
    let maybe_record = state
        .messaging
        .lock()
        .await
        .update_message_delivery_state(sdkmsg::MessageDeliveryUpdate {
            message_id_hex: message_id_hex.as_str(),
            state: Some(sdkmsg::MessageState::Delivered),
            transport_state: Some(sdkmsg::TransportDeliveryState::TransportDelivered),
            application_ack_state: Some(sdkmsg::ApplicationAckState::NotRequired),
            detail: Some("legacy REM delivery ack".to_string()),
            last_wire_message_id_hex: None,
            updated_at_ms: now_ms(),
        })
        .map(from_sdk_message_record);

    if let Some(record) = maybe_record {
        record_peer_link_state(state, bus, source_hex, true).await;
        state.sdk.record_delivery_acknowledged(
            &record.message_id_hex,
            &record.destination_hex,
            Some(source_hex),
            None,
            None,
            None,
            None,
            None,
            record.detail.as_deref(),
        );
        bus.emit(NodeEvent::MessageUpdated {
            message: record.clone(),
        });
        info!(
            "[lxmf][chat] acknowledged message_id={} source={}",
            record.message_id_hex,
            source_hex,
        );
    }
    true
}

async fn send_chat_delivery_ack_if_needed(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source_hex: Option<&str>,
    message_id_hex: &str,
    body_utf8: &str,
) {
    if parse_chat_delivery_ack_body(body_utf8).is_some() {
        return;
    }
    let Some(source_hex) = source_hex else {
        return;
    };
    let legacy_rem_peer = peer_for_any_destination_hex(state, source_hex)
        .await
        .is_some_and(|peer| requires_legacy_rem_chat_ack(peer.app_data.as_deref()));
    if !legacy_rem_peer {
        return;
    }
    let body = chat_delivery_ack_body(message_id_hex);
    match send_lxmf_with_delivery_policy(
        state,
        bus,
        source_hex,
        body.as_bytes(),
        Some(CHAT_DELIVERY_ACK_TITLE.to_string()),
        None,
        None,
        SendMode::Auto {},
        SendTaskClass::General,
    )
    .await
    {
        Ok(report) => {
            info!(
                "[lxmf][chat] sent legacy REM delivery acknowledgement destination={} message_id={} acked_message_id={}",
                report.resolved_destination_hex, report.message_id_hex, message_id_hex,
            );
        }
        Err(err) => {
            warn!(
                "[lxmf][chat] legacy REM delivery acknowledgement send failed destination={source_hex} acked_message_id={message_id_hex} reason={err}",
            );
        }
    }
}

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
    timeout: Duration,
) -> Result<(), NodeError> {
    if link.lock().await.status() == LinkStatus::Active {
        return Ok(());
    }

    let link_id = *link.lock().await.id();
    let mut events = transport.out_link_events();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if link.lock().await.status() == LinkStatus::Active {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        match tokio::time::timeout(Duration::from_millis(250), events.recv()).await {
            Ok(Ok(event)) => {
                if event.id == link_id && matches!(event.event, LinkEvent::Activated) {
                    return Ok(());
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        }
    }
}
