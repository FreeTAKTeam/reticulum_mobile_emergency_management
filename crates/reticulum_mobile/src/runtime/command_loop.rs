    loop {
        let (cmd, command_lane) = tokio::select! {
            biased;
            Some(cmd) = priority_cmd_rx.recv() => (cmd, RuntimeCommandLane::Priority),
            Some(cmd) = cmd_rx.recv() => (cmd, RuntimeCommandLane::Normal),
            else => break,
        };
        match cmd {
            Command::Stop { resp } => {
                if let Ok(mut guard) = status.lock() {
                    guard.running = false;
                    guard.refresh_readiness();
                    bus.emit(NodeEvent::StatusChanged {
                        status: guard.clone(),
                    });
                }
                let _ = resp.send(Ok(()));
                break;
            }
            Command::AnnounceNow {} => {
                spawn_manual_announce(
                    &command_executor,
                    command_lane,
                    &transport,
                    &app_destination,
                    &lxmf_destination,
                    &announce_capabilities,
                );
            }
            Command::SetLogLevel { level } => {
                crate::logger::NodeLogger::global().set_level(level);
            }
            Command::RequestPeerIdentity {
                destination_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                command_executor.spawn(command_lane, RuntimeCommandClass::Work, resp, async move {
                    let result = resolve_peer_route(&state, &bus, destination_hex.as_str()).await;
                    if let Err(err) = &result {
                        state.messaging.lock().await.record_resolution_error(
                            destination_hex.as_str(),
                            Some(err.to_string()),
                        );
                        emit_peer_changed(&state, &bus, destination_hex.as_str()).await;
                    }
                    result
                });
            }
            Command::SetAnnounceCapabilities {
                capability_string,
                resp,
            } => {
                spawn_announce_capability_update(
                    &command_executor,
                    command_lane,
                    &transport,
                    (&app_destination, &lxmf_destination),
                    &announce_capabilities,
                    capability_string,
                    resp,
                );
            }
            Command::ConnectPeer {
                destination_hex,
                resp,
            } => {
                spawn_connect_peer_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    destination_hex,
                    resp,
                );
            }
            Command::DisconnectPeer {
                destination_hex,
                resp,
            } => {
                spawn_disconnect_peer_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    destination_hex,
                    resp,
                );
            }
            Command::SetSavedPeers { peers, resp } => {
                spawn_saved_peer_projection_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    peers,
                    resp,
                );
            }
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                resp,
            } => {
                spawn_send_bytes_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    &transport,
                    SendBytesCommand {
                        destination_hex,
                        bytes,
                        fields_bytes,
                        send_mode,
                        resp,
                    },
                );
            }
            Command::SendLxmf { request, resp } => {
                let state = state.clone();
                let bus = bus.clone();
                let receipt_message_ids = receipt_message_ids.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general send destination={} mode={:?} has_fields=false",
                        request.destination_hex,
                        request.send_mode,
                    ),
                );
                command_executor.spawn(command_lane, RuntimeCommandClass::Work, resp, async move {
                    let result = async {
                        let body_bytes = request.body_utf8.as_bytes().to_vec();
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            request.destination_hex.as_str(),
                            body_bytes.as_slice(),
                            request.title.clone(),
                            None,
                            None,
                            request.send_mode,
                            SendTaskClass::General,
                        )
                        .await?;
                        let method = match (report.method, report.representation) {
                            (LxmfDeliveryMethod::Propagated {}, _) => MessageMethod::Propagated {},
                            (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                MessageMethod::Opportunistic {}
                            }
                            (_, LxmfDeliveryRepresentation::Resource {}) => {
                                MessageMethod::Resource {}
                            }
                            _ => MessageMethod::Direct {},
                        };
                        let state_value = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else if matches!(
                            report.outcome,
                            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                        ) {
                            MessageState::SentDirect {}
                        } else {
                            MessageState::Failed {}
                        };
                        let detail = if matches!(state_value, MessageState::Failed {}) {
                            Some(format!("{:?}", send_outcome_to_udl(report.outcome)))
                        } else {
                            None
                        };
                        let conversation_id =
                            conversation_id_for(report.resolved_destination_hex.as_str());
                        let record = MessageRecord {
                            message_id_hex: report.message_id_hex.clone(),
                            conversation_id,
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(request.destination_hex.clone()),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: request.title.clone(),
                            body_utf8: request.body_utf8.clone(),
                            method,
                            state: state_value,
                            transport_state: transport_state_for_message_state(state_value),
                            application_ack_state: if matches!(state_value, MessageState::Failed {})
                            {
                                ApplicationAckState::Failed {}
                            } else {
                                ApplicationAckState::Waiting {}
                            },
                            detail: detail.clone(),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, record, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: to_sdk_send_request(&request),
                                message_id_hex: report.message_id_hex.clone(),
                            },
                        );
                        if let Some(receipt_hash_hex) = report.receipt_hash_hex.as_ref() {
                            if let Ok(mut guard) = receipt_message_ids.lock() {
                                guard.insert(
                                    receipt_hash_hex.clone(),
                                    ReceiptMessageTracking {
                                        message_id_hex: report.message_id_hex.clone(),
                                        recorded_at_ms: now_ms(),
                                    },
                                );
                            }
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
            Command::RetryLxmf {
                message_id_hex,
                resp,
            } => {
                let state = state.clone();
                let bus = bus.clone();
                log_send_task(
                    SendTaskClass::General,
                    format!(
                        "[lxmf][queue] enqueued general retry message_id={}",
                        message_id_hex,
                    ),
                );
                command_executor.spawn(command_lane, RuntimeCommandClass::Work, resp, async move {
                    let result = async {
                        let outbound = state
                            .messaging
                            .lock()
                            .await
                            .outbound(message_id_hex.as_str())
                            .ok_or(NodeError::InvalidConfig {})?;
                        let report = send_lxmf_with_delivery_policy(
                            &state,
                            &bus,
                            outbound.request.destination_hex.as_str(),
                            outbound.request.body_utf8.as_bytes(),
                            outbound.request.title.clone(),
                            None,
                            None,
                            match outbound.request.effective_send_mode() {
                                sdkmsg::SendMode::Auto => SendMode::Auto {},
                                sdkmsg::SendMode::DirectOnly => SendMode::DirectOnly {},
                                sdkmsg::SendMode::PropagationOnly => SendMode::PropagationOnly {},
                            },
                            SendTaskClass::General,
                        )
                        .await?;
                        let retried_state = if report.used_propagation_node
                            && matches!(
                                report.outcome,
                                RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                            ) {
                            MessageState::SentToPropagation {}
                        } else {
                            MessageState::SentDirect {}
                        };
                        let retried = MessageRecord {
                            message_id_hex: outbound.message_id_hex.clone(),
                            conversation_id: conversation_id_for(
                                report.resolved_destination_hex.as_str(),
                            ),
                            direction: MessageDirection::Outbound {},
                            destination_hex: report.resolved_destination_hex.clone(),
                            source_hex: Some(address_hash_to_hex(
                                &state.lxmf_destination.lock().await.desc.address_hash,
                            )),
                            requested_destination_hex: Some(
                                outbound.request.destination_hex.clone(),
                            ),
                            delivery_destination_hex: Some(report.resolved_destination_hex.clone()),
                            recipient_identity_hex: None,
                            last_wire_message_id_hex: Some(report.message_id_hex.clone()),
                            title: outbound.request.title.clone(),
                            body_utf8: outbound.request.body_utf8.clone(),
                            method: match (report.method, report.representation) {
                                (LxmfDeliveryMethod::Propagated {}, _) => {
                                    MessageMethod::Propagated {}
                                }
                                (LxmfDeliveryMethod::Opportunistic {}, _) => {
                                    MessageMethod::Opportunistic {}
                                }
                                (_, LxmfDeliveryRepresentation::Resource {}) => {
                                    MessageMethod::Resource {}
                                }
                                _ => MessageMethod::Direct {},
                            },
                            state: retried_state,
                            transport_state: transport_state_for_message_state(retried_state),
                            application_ack_state: ApplicationAckState::Waiting {},
                            detail: Some(format!("retry of {}", outbound.message_id_hex)),
                            sent_at_ms: Some(now_ms()),
                            received_at_ms: None,
                            updated_at_ms: now_ms(),
                        };
                        upsert_message_record(&state, &bus, retried, false).await;
                        state.messaging.lock().await.store_outbound(
                            sdkmsg::StoredOutboundMessage {
                                request: outbound.request,
                                message_id_hex: outbound.message_id_hex.clone(),
                            },
                        );
                        Ok::<(), NodeError>(())
                    }
                    .await;
                    if let Err(err) = &result {
                        bus.emit(NodeEvent::Error {
                            code: node_error_code(err).to_string(),
                            message: format!(
                                "retry_lxmf failed message_id={} reason={}",
                                message_id_hex, err
                            ),
                        });
                    }
                    result
                });
            }
            Command::CancelLxmf {
                message_id_hex,
                resp,
            } => {
                spawn_cancel_lxmf_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    message_id_hex,
                    resp,
                );
            }
            Command::SetActivePropagationNode {
                destination_hex,
                resp,
            } => {
                spawn_active_propagation_node_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    destination_hex,
                    resp,
                );
            }
            Command::RequestLxmfSync { limit, resp } => {
                spawn_sync_request_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    limit,
                    resp,
                );
            }
            Command::ListAnnounces { resp } => {
                spawn_list_announces_command(&command_executor, command_lane, &state, resp);
            }
            Command::ListPeers { resp } => {
                spawn_list_peers_command(&command_executor, command_lane, &state, resp);
            }
            Command::ListConversations { resp } => {
                spawn_list_conversations_command(&command_executor, command_lane, &state, resp);
            }
            Command::ListMessages {
                conversation_id,
                resp,
            } => {
                spawn_list_messages_command(
                    &command_executor,
                    command_lane,
                    &state,
                    conversation_id,
                    resp,
                );
            }
            Command::DeleteConversation {
                conversation_id,
                resp,
            } => {
                spawn_delete_conversation_command(
                    &command_executor,
                    command_lane,
                    &state,
                    &bus,
                    conversation_id,
                    resp,
                );
            }
            Command::GetLxmfSyncStatus { resp } => {
                spawn_sync_status_command(&command_executor, command_lane, &state, resp);
            }
            Command::BroadcastBytes { bytes, resp } => {
                spawn_broadcast_command(&command_executor, command_lane, &state, &bus, bytes, resp);
            }
            Command::RefreshHubDirectory { resp } => {
                spawn_hub_refresh_command(
                    &command_executor,
                    command_lane,
                    &config,
                    &state,
                    &bus,
                    resp,
                );
            }
        }
    }
