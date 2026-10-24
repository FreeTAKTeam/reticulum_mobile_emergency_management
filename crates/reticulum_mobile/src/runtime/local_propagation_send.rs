#[expect(
    clippy::too_many_arguments,
    reason = "bounded routing reuses the existing send contract"
)]
async fn send_with_local_custody(
    state: &NodeRuntimeState,
    bus: &EventBus,
    requested_destination: &str,
    body: &[u8],
    title: Option<String>,
    fields: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    mode: SendMode,
    class: OutboundTrafficClass,
    task_class: SendTaskClass,
    direct_attempts: usize,
) -> Result<LxmfSendReport, NodeError> {
    let store = state
        .sdk
        .local_propagation
        .as_ref()
        .ok_or(NodeError::IoError {})?;
    let started = tokio::time::Instant::now();
    let lora = active_interfaces_are_rnode_ble_only(&*state.active_interface_registry.lock().await);
    let destination = resolve_lxmf_destination_for_send(state, requested_destination, false)
        .await
        .map_err(|_| NodeError::UnresolvedRecipient {})?;
    let destination = parse_address_hash(&destination)?;
    let lora = lora || destination_uses_rnode_ble_route(state, &destination).await;
    let bound = Duration::from_secs(if lora { 180 } else { 60 });
    if !local_permission_check(
        state,
        &destination.to_hex_string(),
        class,
        metadata.as_ref(),
    ) {
        return Err(NodeError::InvalidConfig {});
    }
    let remaining = bound
        .checked_sub(started.elapsed())
        .ok_or(NodeError::Timeout {})?;
    let (prepared, mut request) = tokio::time::timeout(
        remaining,
        state.sdk.prepare_local_outgoing(
            destination,
            body,
            title.as_deref(),
            fields.as_deref(),
            metadata.as_ref(),
        ),
    )
    .await
    .map_err(|_| NodeError::UnresolvedRecipient {})??;
    if lora {
        request = request.with_extension(
            "reticulum.link_connect_timeout_ms",
            serde_json::json!(RNODE_BLE_LINK_CONNECT_TIMEOUT.as_millis() as u64),
        );
        request = request.with_extension(
            lxmf_runtime::EXT_DIRECT_PACKET_MAX_WIRE_BYTES,
            serde_json::json!(RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES),
        );
    }
    let interfaces_active = !state.active_interface_registry.lock().await.is_empty();
    if interfaces_active && matches!(mode, SendMode::Auto {}) {
        for attempt in 0..direct_attempts.max(1) {
            let Some(remaining) = bound.checked_sub(started.elapsed()) else {
                break;
            };
            if *state.power_saver_rx.borrow()
                || !local_permission_check(state, &prepared.destination(), class, metadata.as_ref())
            {
                break;
            }
            ensure_current_telemetry_delivery(
                state,
                class,
                requested_destination,
                metadata.as_ref(),
            )?;
            let _permit = match tokio::time::timeout(
                remaining,
                acquire_send_task_permit(&state.send_task_permits, task_class),
            )
            .await
            {
                Ok(result) => result?,
                Err(_) => break,
            };
            let Some(remaining) = bound.checked_sub(started.elapsed()) else {
                break;
            };
            if !local_permission_check(state, &prepared.destination(), class, metadata.as_ref()) {
                return Err(NodeError::InvalidConfig {});
            }
            let policy_state = state.clone();
            let policy_destination = prepared.destination();
            let policy_metadata = metadata.clone();
            let result = tokio::time::timeout(
                remaining,
                store.send_direct(&prepared, &request, remaining, move || {
                    !*policy_state.power_saver_rx.borrow()
                        && local_permission_check(
                            &policy_state,
                            &policy_destination,
                            class,
                            policy_metadata.as_ref(),
                        )
                }),
            )
            .await;
            if let Ok(Ok(report)) = result {
                return Ok(local_runtime_send_report(report, metadata));
            }
            if attempt + 1 < direct_attempts {
                let Some(remaining) = bound.checked_sub(started.elapsed()) else {
                    break;
                };
                tokio::time::sleep(remaining.min(Duration::from_secs(10))).await;
            }
        }
    }
    let now = (now_ms() / 1000).min(i64::MAX as u64) as i64;
    let kind = match class {
        OutboundTrafficClass::Chat {} => lxmf_runtime::propagation_store::MessageKind::Chat,
        OutboundTrafficClass::Telemetry {} => {
            lxmf_runtime::propagation_store::MessageKind::Telemetry {
                key: "latest-fix".to_owned(),
            }
        }
        _ => lxmf_runtime::propagation_store::MessageKind::Mission,
    };
    let expiry = if matches!(class, OutboundTrafficClass::Telemetry {}) {
        let settings = state
            .app_state
            .get_app_settings()?
            .ok_or(NodeError::InvalidConfig {})?;
        let publication = state.app_state.telemetry_publication()?;
        publication.fix.map(|fix| {
            ((fix.updated_at_ms / 1000).min(i64::MAX as u64) as i64)
                .saturating_add(i64::from(settings.telemetry.expire_after_minutes) * 60)
        })
    } else {
        None
    };
    if matches!(class, OutboundTrafficClass::Chat {}) {
        persist_chat_before_local_admission(state, &prepared, requested_destination, body, title)
            .await?;
    }
    let status = store
        .deposit_own(prepared, kind, now, expiry)
        .map_err(crate::sdk_bridge::map_local_propagation_error)?;
    publish_local_custody(state, bus).await?;
    // This store is the only retry owner from this commit onward.
    state
        .pending_lxmf_deliveries
        .lock()
        .await
        .retain(|_, pending| pending.message_id_hex != status.message_id);
    if !*state.power_saver_rx.borrow() {
        if let Some(remaining) = bound.checked_sub(started.elapsed()) {
            if !remaining.is_zero() {
                let relay = state.active_propagation_node_hex.lock().await.clone();
                if let Some(relay) = relay.and_then(|value| parse_address_hash(&value).ok()) {
                    let transfer_limit = if lora {
                        crate::sdk_bridge::LOCAL_PROPAGATION_LORA_TRANSFER_BYTES as usize
                    } else {
                        25 * 1024 * 1024
                    };
                    let _ = tokio::time::timeout(
                        remaining,
                        recover_local_upload(
                            state,
                            bus,
                            &status.transient_id,
                            relay,
                            transfer_limit,
                        ),
                    )
                    .await;
                }
            }
        }
    }
    let entry = state
        .projection_journal
        .local_propagation()?
        .into_iter()
        .find(|entry| entry.transient_id_hex == status.transient_id)
        .unwrap_or_else(|| crate::sdk_bridge::local_propagation_record(&status));
    Ok(LxmfSendReport {
        outcome: RnsSendOutcome::DroppedNoRoute,
        message_id_hex: status.message_id,
        resolved_destination_hex: status.destination,
        metadata,
        track_delivery_timeout: false,
        used_propagation_node: entry.may_exist_on_relay,
        method: LxmfDeliveryMethod::Direct {},
        representation: LxmfDeliveryRepresentation::Packet {},
        relay_destination_hex: status.relay,
        fallback_stage: None,
        receipt_hash_hex: None,
        local_custody: Some(entry),
    })
}

fn local_runtime_send_report(
    report: lxmf_runtime::InProcessSendReport,
    metadata: Option<MissionSyncMetadata>,
) -> LxmfSendReport {
    LxmfSendReport {
        outcome: match report.outcome {
            lxmf_runtime::DeliveryOutcome::SentDirect => RnsSendOutcome::SentDirect,
            lxmf_runtime::DeliveryOutcome::SentBroadcast => RnsSendOutcome::SentBroadcast,
        },
        message_id_hex: report.message_id.0,
        resolved_destination_hex: report.resolved_destination,
        track_delivery_timeout: metadata
            .as_ref()
            .is_some_and(|metadata| metadata.command_present && metadata.tracking_key().is_some()),
        metadata,
        used_propagation_node: matches!(report.method, lxmf_runtime::DeliveryMethod::Propagated),
        method: match report.method {
            lxmf_runtime::DeliveryMethod::Direct => LxmfDeliveryMethod::Direct {},
            lxmf_runtime::DeliveryMethod::Opportunistic => LxmfDeliveryMethod::Opportunistic {},
            lxmf_runtime::DeliveryMethod::Propagated => LxmfDeliveryMethod::Propagated {},
        },
        representation: match report.representation {
            lxmf_runtime::DeliveryRepresentation::Packet => LxmfDeliveryRepresentation::Packet {},
            lxmf_runtime::DeliveryRepresentation::Resource => {
                LxmfDeliveryRepresentation::Resource {}
            }
        },
        relay_destination_hex: report.relay_destination,
        fallback_stage: None,
        receipt_hash_hex: report.receipt_hash,
        local_custody: None,
    }
}

fn local_propagation_route_limits(lora: bool) -> lxmf_runtime::PropagationRouteLimits {
    lxmf_runtime::PropagationRouteLimits {
        max_packet_bytes: lora.then_some(RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES as usize),
        link_connect_timeout: lora.then_some(RNODE_BLE_LINK_CONNECT_TIMEOUT),
    }
}
