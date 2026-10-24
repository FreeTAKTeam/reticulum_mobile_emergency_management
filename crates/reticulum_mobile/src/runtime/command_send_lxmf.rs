#[expect(
    clippy::too_many_arguments,
    reason = "chat command boundary keeps the existing request and response explicit"
)]
fn spawn_send_lxmf_command(
    executor: &RuntimeCommandExecutor,
    state: &NodeRuntimeState,
    bus: &EventBus,
    receipt_tracker: &ReceiptTracker,
    request: SendLxmfRequest,
    requested_destination_hex: String,
    fields_bytes: Vec<u8>,
    resp: cb::Sender<Result<String, NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    let receipt_tracker = receipt_tracker.clone();
    log_send_task(
        SendTaskClass::General,
        format!(
            "[lxmf][queue] enqueued general send destination={} mode={:?} has_fields=false",
            request.destination_hex, request.send_mode,
        ),
    );
    executor.spawn_send(resp, async move {
        let result = async {
            let body_bytes = request.body_utf8.as_bytes().to_vec();
            let report = send_lxmf_with_delivery_policy(
                &state,
                &bus,
                request.destination_hex.as_str(),
                body_bytes.as_slice(),
                request.title.clone(),
                Some(fields_bytes),
                None,
                request.send_mode,
                SendTaskClass::General,
                true,
                OutboundTrafficClass::Chat {},
            )
            .await?;
            if report.local_custody.is_some() {
                // History already exists; only Core may advance a custody-owned message.
                state
                    .messaging
                    .lock()
                    .await
                    .store_outbound(sdkmsg::StoredOutboundMessage {
                        request: to_sdk_send_request(&request),
                        message_id_hex: report.message_id_hex.clone(),
                    });
                return Ok::<String, NodeError>(report.message_id_hex);
            }
            let method = match (report.method, report.representation) {
                (LxmfDeliveryMethod::Propagated {}, _) => MessageMethod::Propagated {},
                (LxmfDeliveryMethod::Opportunistic {}, _) => MessageMethod::Opportunistic {},
                (_, LxmfDeliveryRepresentation::Resource {}) => MessageMethod::Resource {},
                _ => MessageMethod::Direct {},
            };
            let state_value = if let Some(custody) = &report.local_custody {
                match custody.state {
                    crate::types::LocalPropagationState::Delivered
                    | crate::types::LocalPropagationState::Acknowledged => {
                        MessageState::Delivered {}
                    }
                    crate::types::LocalPropagationState::Expired
                    | crate::types::LocalPropagationState::Evicted => MessageState::Failed {},
                    _ => MessageState::Queued {},
                }
            } else if report.used_propagation_node
                && matches!(
                    report.outcome,
                    RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                )
            {
                MessageState::SentToPropagation {}
            } else if matches!(
                report.representation,
                LxmfDeliveryRepresentation::Resource {}
            ) && matches!(
                report.outcome,
                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
            ) {
                MessageState::Delivered {}
            } else if matches!(
                report.outcome,
                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
            ) {
                MessageState::SentDirect {}
            } else {
                MessageState::Failed {}
            };
            let detail = if let Some(custody) = &report.local_custody {
                Some(local_custody_detail(custody.state).to_owned())
            } else if matches!(state_value, MessageState::Failed {}) {
                Some(format!("{:?}", send_outcome_to_udl(report.outcome)))
            } else {
                None
            };
            let conversation_id = conversation_id_for(report.resolved_destination_hex.as_str());
            let record = MessageRecord {
                message_id_hex: report.message_id_hex.clone(),
                conversation_id,
                direction: MessageDirection::Outbound {},
                destination_hex: report.resolved_destination_hex.clone(),
                source_hex: Some(address_hash_to_hex(
                    &state.lxmf_destination.lock().await.desc.address_hash,
                )),
                requested_destination_hex: Some(requested_destination_hex),
                delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                recipient_identity_hex: None,
                last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                title: request.title.clone(),
                body_utf8: request.body_utf8.clone(),
                traffic_class: OutboundTrafficClass::Chat {},
                method,
                state: state_value,
                transport_state: transport_state_for_message_state(state_value),
                application_ack_state: if matches!(state_value, MessageState::Failed {}) {
                    ApplicationAckState::Failed {}
                } else {
                    ApplicationAckState::NotRequired {}
                },
                detail: detail.clone(),
                sent_at_ms: report.local_custody.is_none().then(now_ms),
                received_at_ms: None,
                updated_at_ms: now_ms(),
            };
            upsert_message_record(&state, &bus, record, false).await;
            state
                .messaging
                .lock()
                .await
                .store_outbound(sdkmsg::StoredOutboundMessage {
                    request: to_sdk_send_request(&request),
                    message_id_hex: report.message_id_hex.clone(),
                });
            if !matches!(report.method, LxmfDeliveryMethod::Propagated {}) {
                register_receipt_tracking(
                    &receipt_tracker,
                    report.receipt_hash_hex.as_deref(),
                    report.message_id_hex.as_str(),
                );
            }
            Ok::<String, NodeError>(report.message_id_hex)
        }
        .await;
        if let Err(err) = &result {
            bus.emit(NodeEvent::Error {
                code: node_error_code(err).to_string(),
                message: format!(
                    "send_lxmf failed destination={} reason={}",
                    request.destination_hex, err
                ),
            });
        }
        result
    });
}
