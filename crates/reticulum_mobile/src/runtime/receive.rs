async fn emit_received_payload(
    state: &NodeRuntimeState,
    bus: &EventBus,
    sdk: &RuntimeLxmfSdk,
    destination_hex: String,
    payload: Vec<u8>,
    fallback_fields_bytes: Option<Vec<u8>>,
    expected_lxmf: bool,
) {
    match LxmfMessage::from_wire(payload.as_slice()) {
        Ok(message) => {
            let wire_message_id_hex = LxmfWireMessage::unpack(payload.as_slice())
                .map(|wire| hex::encode(wire.message_id()))
                .ok();
            let source_hex = message.source_hash.map(hex::encode);
            let body_utf8 = String::from_utf8_lossy(message.content.as_slice()).to_string();
            let title = if message.title.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(message.title.as_slice()).to_string())
            };
            let fields_bytes = message
                .fields
                .and_then(|value| rmp_serde::to_vec(&value).ok());
            let sos_fields = fields_bytes.as_deref().and_then(parse_sos_fields);
            let mut sos_telemetry = sos_fields
                .as_ref()
                .and_then(|fields| fields.telemetry.clone());
            if sos_telemetry.is_none() {
                if let Some((lat, lon)) = extract_text_coordinates(body_utf8.as_str()) {
                    sos_telemetry = Some(SosDeviceTelemetryRecord {
                        lat: Some(lat),
                        lon: Some(lon),
                        alt: None,
                        speed: None,
                        course: None,
                        accuracy: None,
                        battery_percent: None,
                        battery_charging: None,
                        updated_at_ms: now_ms(),
                    });
                }
            }
            let sos_command = sos_fields
                .as_ref()
                .and_then(|fields| fields.command.clone());
            let text_sos_kind = sos_kind_from_text(body_utf8.as_str());
            let is_sos_message = sos_command.is_some() || text_sos_kind.is_some();
            let metadata = fields_bytes
                .as_deref()
                .and_then(parse_mission_sync_metadata);
            if let Some(metadata) = metadata.as_ref().filter(|_| !is_sos_message) {
                if metadata.is_mission_related() {
                    info!(
                    "[lxmf][mission] received kind={} name={} source={} destination={} event_uid={} mission_uid={} correlation={}",
                    metadata.primary_kind(),
                    metadata.primary_name().unwrap_or("-"),
                    source_hex.as_deref().unwrap_or("-"),
                    destination_hex,
                    metadata.event_uid.as_deref().unwrap_or("-"),
                    metadata.mission_uid.as_deref().unwrap_or("-"),
                    metadata.correlation_id.as_deref().unwrap_or("-"),
                );
                }
                ack_pending_lxmf_delivery(state, bus, source_hex.as_deref(), metadata).await;
                let persisted_eam = persist_received_eam_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    body_utf8.as_str(),
                    source_hex.as_deref(),
                )
                .await;
                let persisted_event = persist_received_event_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    Some(message.content.as_slice()),
                    source_hex.as_deref(),
                )
                .await;
                let persisted_telemetry = persist_received_telemetry_if_present(
                    state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                )
                .await;
                let persisted_checklist = persist_received_checklist_if_present(
                    &state.app_state,
                    bus,
                    Some(metadata),
                    fields_bytes.as_deref(),
                    Some(message.content.as_slice()),
                );
                send_operational_ack_if_needed(
                    state,
                    bus,
                    source_hex.as_deref(),
                    Some(metadata),
                    persisted_eam || persisted_event || persisted_telemetry || persisted_checklist,
                )
                .await;
            }
            if is_sos_message {
                let peer_hex = source_hex
                    .clone()
                    .unwrap_or_else(|| destination_hex.clone());
                let message_id_hex = wire_message_id_hex
                    .clone()
                    .unwrap_or_else(|| format!("sos-{}-{}", peer_hex, now_ms()));
                let state_kind = sos_command
                    .as_ref()
                    .map(|command| command.state)
                    .or(text_sos_kind)
                    .unwrap_or(SosMessageKind::Active {});
                let incident_id = sos_command
                    .as_ref()
                    .map(|command| command.incident_id.clone())
                    .or_else(|| {
                        matches!(state_kind, SosMessageKind::Cancelled {}).then(|| {
                            state
                                .app_state
                                .latest_active_sos_alert_for_source(peer_hex.as_str())
                                .ok()
                                .flatten()
                                .map(|alert| alert.incident_id)
                        })?
                    })
                    .unwrap_or_else(|| format!("legacy-sos-{}-{}", peer_hex, now_ms()));
                let received_at_ms = now_ms();
                let record = MessageRecord {
                    message_id_hex: message_id_hex.clone(),
                    conversation_id: conversation_id_for(peer_hex.as_str()),
                    direction: MessageDirection::Inbound {},
                    destination_hex: peer_hex.clone(),
                    source_hex: source_hex.clone(),
                    requested_destination_hex: Some(peer_hex.clone()),
                    delivery_destination_hex: Some(peer_hex.clone()),
                    recipient_identity_hex: None,
                    last_wire_message_id_hex: Some(message_id_hex.clone()),
                    title: title.clone(),
                    body_utf8: body_utf8.clone(),
                    traffic_class: OutboundTrafficClass::Sos {},
                    method: MessageMethod::Direct {},
                    state: MessageState::Received {},
                    transport_state: TransportDeliveryState::TransportDelivered {},
                    application_ack_state: ApplicationAckState::NotRequired {},
                    detail: Some("sos".to_string()),
                    sent_at_ms: None,
                    received_at_ms: Some(received_at_ms),
                    updated_at_ms: received_at_ms,
                };
                upsert_message_record(state, bus, record, true).await;
                let alert = received_alert_from_sos(
                    incident_id,
                    peer_hex.clone(),
                    conversation_id_for(peer_hex.as_str()),
                    state_kind,
                    body_utf8.clone(),
                    sos_telemetry.as_ref(),
                    sos_command
                        .as_ref()
                        .and_then(|command| command.audio_id.clone()),
                    Some(message_id_hex),
                    received_at_ms,
                );
                if let Ok(invalidation) = state.app_state.upsert_sos_alert(&alert) {
                    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                }
                if let Some(location) = location_from_alert(&alert) {
                    if let Ok(invalidation) = state.app_state.upsert_sos_location(&location) {
                        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                    }
                }
                if let Some(position) = telemetry_position_from_sos(
                    peer_hex.as_str(),
                    sos_telemetry.as_ref(),
                    received_at_ms,
                ) {
                    if let Ok(invalidation) = state.app_state.record_local_telemetry_fix(&position)
                    {
                        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                    }
                }
                bus.emit(NodeEvent::SosAlertChanged { alert });
                send_operational_ack_if_needed(
                    state,
                    bus,
                    source_hex.as_deref(),
                    metadata.as_ref(),
                    true,
                )
                .await;
            } else if !metadata
                .as_ref()
                .is_some_and(MissionSyncMetadata::is_mission_related)
            {
                let peer_hex = source_hex
                    .clone()
                    .unwrap_or_else(|| destination_hex.clone());
                let message_id_hex = wire_message_id_hex
                    .clone()
                    .unwrap_or_else(|| hex::encode(destination_hex.as_bytes()));
                if !acknowledge_chat_delivery(state, bus, source_hex.as_deref(), body_utf8.as_str())
                    .await
                {
                    let record = MessageRecord {
                        message_id_hex: message_id_hex.clone(),
                        conversation_id: conversation_id_for(peer_hex.as_str()),
                        direction: MessageDirection::Inbound {},
                        destination_hex: peer_hex.clone(),
                        source_hex: source_hex.clone(),
                        requested_destination_hex: Some(peer_hex.clone()),
                        delivery_destination_hex: Some(peer_hex.clone()),
                        recipient_identity_hex: None,
                        last_wire_message_id_hex: Some(message_id_hex.clone()),
                        title,
                        body_utf8: body_utf8.clone(),
                        traffic_class: OutboundTrafficClass::Chat {},
                        method: MessageMethod::Direct {},
                        state: MessageState::Received {},
                        transport_state: TransportDeliveryState::TransportDelivered {},
                        application_ack_state: ApplicationAckState::NotRequired {},
                        detail: None,
                        sent_at_ms: None,
                        received_at_ms: Some(now_ms()),
                        updated_at_ms: now_ms(),
                    };
                    upsert_message_record(state, bus, record, true).await;
                    send_chat_delivery_ack_if_needed(
                        state,
                        bus,
                        source_hex.as_deref(),
                        message_id_hex.as_str(),
                        body_utf8.as_str(),
                    )
                    .await;
                }
            }
            sdk.record_packet_received(
                &destination_hex,
                source_hex.as_deref(),
                message.content.as_slice(),
                fields_bytes.as_deref(),
            );
            bus.emit(NodeEvent::PacketReceived {
                destination_hex,
                source_hex,
                bytes: message.content,
                fields_bytes,
            });
            return;
        }
        Err(err) if expected_lxmf => {
            let prefix = hex::encode(payload.iter().take(16).copied().collect::<Vec<_>>());
            warn!(
                "[lxmf][rx] decode_failed destination={} bytes={} prefix={} reason={}",
                destination_hex,
                payload.len(),
                prefix,
                err,
            );
            bus.emit(NodeEvent::Error {
                code: "LxmfDecodeError".to_string(),
                message: format!(
                    "Failed to decode LXMF payload for destination {destination_hex}: {err}"
                ),
            });
            return;
        }
        Err(_) => {}
    }

    info!(
        "[lxmf][rx] non_lxmf_payload destination={} bytes={} prefix={}",
        destination_hex,
        payload.len(),
        hex::encode(payload.iter().take(16).copied().collect::<Vec<_>>()),
    );
    sdk.record_packet_received(
        &destination_hex,
        None,
        payload.as_slice(),
        fallback_fields_bytes.as_deref(),
    );
    bus.emit(NodeEvent::PacketReceived {
        destination_hex,
        source_hex: None,
        bytes: payload,
        fields_bytes: fallback_fields_bytes,
    });
}
