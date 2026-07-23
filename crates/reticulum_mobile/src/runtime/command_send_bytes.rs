struct SendBytesCommand {
    destination_hex: String,
    bytes: Vec<u8>,
    fields_bytes: Option<Vec<u8>>,
    send_mode: SendMode,
    resp: cb::Sender<Result<(), NodeError>>,
}

fn spawn_send_bytes_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    transport: &Arc<Transport>,
    receipt_tracker: &ReceiptTracker,
    command: SendBytesCommand,
) {
    let SendBytesCommand {
        destination_hex,
        bytes,
        fields_bytes,
        send_mode,
        resp,
    } = command;
    let state = state.clone();
    let bus = bus.clone();
    let transport = transport.clone();
    let receipt_tracker = receipt_tracker.clone();
    let metadata = fields_bytes
        .as_deref()
        .and_then(parse_mission_sync_metadata);
    let send_task_class = SendTaskClass::from_lxmf_request(
        fields_bytes.is_some(),
        metadata.as_ref(),
        &send_mode,
    );
    log_send_task(
        send_task_class,
        format!(
            "[lxmf][queue] enqueued {} send destination={} mode={:?} has_fields={}",
            send_task_class.label(),
            destination_hex,
            send_mode,
            fields_bytes.is_some(),
        ),
    );
    executor.spawn(lane, RuntimeCommandClass::Work, resp, async move {
        let result = async {
            let lxmf_report = if fields_bytes.is_some() {
                Some(
                    send_lxmf_with_delivery_policy(
                        &state,
                        &bus,
                        &destination_hex,
                        &bytes,
                        None,
                        fields_bytes.clone(),
                        metadata.clone(),
                        send_mode,
                        send_task_class,
                        false,
                    )
                    .await?,
                )
            } else {
                None
            };
            let outcome = if let Some(report) = lxmf_report.as_ref() {
                report.outcome
            } else {
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] waiting for general send slot destination={destination_hex} mode=transport-bytes",
                    ),
                );
                let _permit =
                    acquire_send_task_permit(&state.send_task_permits, SendTaskClass::General)
                        .await?;
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] acquired general send slot destination={destination_hex} mode=transport-bytes",
                    ),
                );
                let dest = parse_address_hash(&destination_hex)?;
                send_transport_packet_with_path_retry(&transport, dest, &bytes).await
            };
            let mapped = send_outcome_to_udl(outcome);
            bus.emit(NodeEvent::PacketSent {
                destination_hex: destination_hex.clone(),
                bytes: bytes.clone(),
                outcome: mapped,
            });

            if let Some(report) = lxmf_report.as_ref() {
                if let Some(metadata) = report.metadata.as_ref() {
                    if metadata.is_mission_related() {
                        info!(
                            "[lxmf][mission] outbound kind={} name={} destination={} message_id={} event_uid={} mission_uid={} correlation={}",
                            metadata.primary_kind(),
                            metadata.primary_name().unwrap_or("-"),
                            report.resolved_destination_hex.as_str(),
                            report.message_id_hex,
                            metadata.event_uid.as_deref().unwrap_or("-"),
                            metadata.mission_uid.as_deref().unwrap_or("-"),
                            metadata.correlation_id.as_deref().unwrap_or("-"),
                        );
                    }
                }

                let resend = build_pending_lxmf_resend(
                    report,
                    destination_hex.as_str(),
                    bytes.as_slice(),
                    None,
                    fields_bytes.clone(),
                    metadata.clone(),
                    send_mode,
                    send_task_class,
                );
                if let Some(registered) =
                    register_pending_lxmf_delivery(&state, report, resend, None).await
                {
                    let pending = &registered.pending;
                    if !matches!(report.method, LxmfDeliveryMethod::Propagated {}) {
                        register_receipt_tracking(
                            &receipt_tracker,
                            report.receipt_hash_hex.as_deref(),
                            pending.message_id_hex.as_str(),
                        );
                    }
                    if matches!(
                        report.outcome,
                        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                    ) {
                        state.sdk.record_delivery_sent(
                            &pending.message_id_hex,
                            &pending.destination_hex,
                            pending.correlation_id.as_deref(),
                            pending.command_id.as_deref(),
                            pending.command_type.as_deref(),
                            pending.event_uid.as_deref(),
                            pending.mission_uid.as_deref(),
                        );
                        let delivery_status = lxmf_delivery_status_for(report);
                        if matches!(delivery_status, LxmfDeliveryStatus::Delivered {}) {
                            state.sdk.record_delivery_acknowledged(
                                &pending.message_id_hex,
                                &pending.destination_hex,
                                None,
                                pending.correlation_id.as_deref(),
                                pending.command_id.as_deref(),
                                pending.command_type.as_deref(),
                                pending.event_uid.as_deref(),
                                pending.mission_uid.as_deref(),
                                Some("resource transfer completed"),
                            );
                        }
                        emit_lxmf_delivery(&bus, pending, delivery_status, None);
                        info!(
                            "[lxmf][mission] sent message_id={} destination={} command={} correlation={}",
                            pending.message_id_hex,
                            pending.destination_hex,
                            pending.command_type.as_deref().unwrap_or("-"),
                            pending.correlation_id.as_deref().unwrap_or("-"),
                        );
                        if let Some(buffered_ack) = registered.buffered_ack {
                            acknowledge_pending_with_buffered_ack(
                                &state,
                                &bus,
                                pending,
                                buffered_ack,
                            )
                            .await;
                        }
                    } else {
                        let failure_detail = format!("{mapped:?}");
                        if let Some(tracking_key) = pending_tracking_key(pending) {
                            state
                                .pending_lxmf_deliveries
                                .lock()
                                .await
                                .remove(&tracking_key);
                        }
                        state.sdk.record_delivery_failed(
                            &pending.message_id_hex,
                            &pending.destination_hex,
                            pending.correlation_id.as_deref(),
                            pending.command_id.as_deref(),
                            pending.command_type.as_deref(),
                            pending.event_uid.as_deref(),
                            pending.mission_uid.as_deref(),
                            Some(failure_detail.as_str()),
                        );
                        emit_lxmf_delivery(
                            &bus,
                            pending,
                            LxmfDeliveryStatus::Failed {},
                            Some(failure_detail),
                        );
                        info!(
                            "[lxmf][mission] failed message_id={} destination={} command={} correlation={} outcome={:?}",
                            pending.message_id_hex,
                            pending.destination_hex,
                            pending.command_type.as_deref().unwrap_or("-"),
                            pending.correlation_id.as_deref().unwrap_or("-"),
                            mapped,
                        );
                    }
                }
            }

            if matches!(
                outcome,
                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
            ) {
                Ok(())
            } else {
                Err(NodeError::NetworkError {})
            }
        }
        .await;
        if let Err(err) = &result {
            if !should_emit_global_send_bytes_error(send_task_class) {
                info!(
                    "[lxmf][mission] propagation send exhausted destination={destination_hex} reason={err}"
                );
            } else {
                bus.emit(NodeEvent::Error {
                    code: node_error_code(err).to_string(),
                    message: format!(
                        "send_bytes failed destination={destination_hex} reason={err}"
                    ),
                });
            }
        }
        result
    });
}
