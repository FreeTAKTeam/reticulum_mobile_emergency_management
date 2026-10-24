async fn publish_local_custody(state: &NodeRuntimeState, bus: &EventBus) -> Result<(), NodeError> {
    let Some(store) = &state.sdk.local_propagation else {
        return Ok(());
    };
    let statuses = store
        .status(crate::numeric::u64_to_i64_saturating(now_ms() / 1000))
        .map_err(crate::sdk_bridge::map_local_propagation_error)?;
    if statuses.is_empty() {
        return Ok(());
    }
    let mut changed = Vec::new();
    for status in &statuses {
        let entry = crate::sdk_bridge::local_propagation_record(status);
        if state
            .projection_journal
            .record_local_propagation(entry.clone())?
        {
            changed.push(entry);
        }
    }
    if !changed.is_empty() || statuses.iter().any(|status| status.state.terminal()) {
        state.projection_journal.persist_current_snapshot()?;
    }
    for status in &statuses {
        let entry = crate::sdk_bridge::local_propagation_record(status);
        let notify = changed
            .iter()
            .any(|changed| changed.transient_id_hex == entry.transient_id_hex);
        let (message_state, transport_state, ack_state) = match entry.state {
            crate::types::LocalPropagationState::Delivered => (
                sdkmsg::MessageState::Delivered,
                sdkmsg::TransportDeliveryState::TransportDelivered,
                None,
            ),
            crate::types::LocalPropagationState::Acknowledged => (
                sdkmsg::MessageState::Delivered,
                sdkmsg::TransportDeliveryState::TransportDelivered,
                Some(sdkmsg::ApplicationAckState::Accepted),
            ),
            crate::types::LocalPropagationState::Expired
            | crate::types::LocalPropagationState::Evicted => (
                sdkmsg::MessageState::Failed,
                sdkmsg::TransportDeliveryState::Failed,
                None,
            ),
            _ => (
                sdkmsg::MessageState::Queued,
                sdkmsg::TransportDeliveryState::Queued,
                None,
            ),
        };
        if matches!(
            status.kind,
            lxmf_runtime::propagation_store::MessageKind::Chat
        ) {
            let update = {
                let mut messages = state.messaging.lock().await;
                let persisted = state.app_state.message_for_wire(&entry.message_id_hex)?;
                let previous = persisted.map(to_sdk_message_record);
                let previous = previous.as_ref();
                let detail = local_custody_detail(entry.state);
                if previous.is_some_and(|message| {
                    message.state != message_state
                        || message.transport_state != transport_state
                        || ack_state.is_some_and(|ack| ack != message.application_ack_state)
                        || message.detail.as_deref() != Some(detail)
                }) {
                    if let Some(previous) = previous {
                        messages.upsert_message(previous.clone());
                    }
                    messages
                        .update_message_delivery_state(sdkmsg::MessageDeliveryUpdate {
                            message_id_hex: &entry.message_id_hex,
                            state: Some(message_state),
                            transport_state: Some(transport_state),
                            application_ack_state: ack_state,
                            detail: Some(detail.to_owned()),
                            last_wire_message_id_hex: None,
                            updated_at_ms: now_ms(),
                        })
                        .map(|record| from_sdk_message_record_with_persisted(state, record))
                } else {
                    None
                }
            };
            if let Some(message) = update {
                if !upsert_message_record(state, bus, message, false).await {
                    return Err(NodeError::IoError {});
                }
            }
        }
        if matches!(
            status.kind,
            lxmf_runtime::propagation_store::MessageKind::Telemetry { .. }
        ) {
            if let Some(command) = status.application_key.as_deref() {
                if let Some(invalidation) = state
                    .app_state
                    .record_local_telemetry_custody(command, &entry)?
                {
                    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
                }
            }
        }
        if notify {
            bus.emit(NodeEvent::LocalPropagationUpdated { entry });
        }
    }
    for status in &statuses {
        if status.state.terminal() {
            store
                .acknowledge_status(&status.transient_id)
                .map_err(crate::sdk_bridge::map_local_propagation_error)?;
        }
    }
    Ok(())
}

struct PropagationFlagGuard(Arc<AtomicBool>);
impl Drop for PropagationFlagGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

async fn recover_local_upload(
    state: &NodeRuntimeState,
    bus: &EventBus,
    id: &str,
    relay: AddressHash,
    limit: usize,
) -> Result<(), NodeError> {
    let Some(store) = &state.sdk.local_propagation else {
        return Ok(());
    };
    if state
        .propagation_sync_inflight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }
    let _owner = PropagationFlagGuard(state.propagation_sync_inflight.clone());
    let now = crate::numeric::u64_to_i64_saturating(now_ms() / 1000);
    let policy_state = state.clone();
    let lora = destination_uses_rnode_ble_route(state, &relay).await;
    let limit = if lora {
        limit.min(crate::sdk_bridge::LOCAL_PROPAGATION_LORA_TRANSFER_BYTES as usize)
    } else {
        limit
    };
    let result = store
        .recover_upload(
            id,
            relay,
            now,
            now.saturating_add(30),
            lxmf_runtime::PropagationUploadLimits {
                max_transfer_bytes: limit,
                route: local_propagation_route_limits(lora),
            },
            move |status, wire| local_recovery_allowed(&policy_state, status, wire, false),
        )
        .await;
    publish_local_custody(state, bus).await?;
    result
        .map(|_| ())
        .map_err(crate::sdk_bridge::map_sdk_error_to_node_error)
}

fn spawn_local_custody_recovery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    recipient_hint: Option<String>,
) {
    let Some(store) = &state.sdk.local_propagation else {
        return;
    };
    let now = crate::numeric::u64_to_i64_saturating(now_ms() / 1000);
    if !store.status(now).is_ok_and(|rows| !rows.is_empty()) {
        return;
    }
    if state
        .sdk
        .local_recovery_inflight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let state = state.clone();
    let bus = bus.clone();
    tokio::spawn(async move {
        let _owner = PropagationFlagGuard(state.sdk.local_recovery_inflight.clone());
        if let Err(error) = recover_local_custody_once(&state, &bus, recipient_hint).await {
            debug!("[local-propagation] recovery deferred: {error}");
        }
    });
}

async fn recover_local_custody_once(
    state: &NodeRuntimeState,
    bus: &EventBus,
    recipient_hint: Option<String>,
) -> Result<(), NodeError> {
    publish_local_custody(state, bus).await?;
    if *state.power_saver_rx.borrow() && recipient_hint.is_none() {
        return Ok(());
    }
    let Some(store) = &state.sdk.local_propagation else {
        return Ok(());
    };
    let now = crate::numeric::u64_to_i64_saturating(now_ms() / 1000);
    let mut statuses = store
        .status(now)
        .map_err(crate::sdk_bridge::map_local_propagation_error)?;
    statuses.sort_by_key(|status| (status.next_attempt_at, status.admitted_at));
    for status in statuses.iter().filter(|status| {
        !status.state.terminal()
            && status.next_attempt_at <= now
            && recipient_hint
                .as_deref()
                .is_none_or(|recipient| recipient == status.destination)
    }) {
        let destination = parse_address_hash(&status.destination)?;
        let relay = state
            .active_propagation_node_hex
            .lock()
            .await
            .clone()
            .and_then(|relay| parse_address_hash(&relay).ok());
        let event_direct = recipient_hint.as_deref() == Some(status.destination.as_str());
        let direct_route = event_direct
            || state.transport.has_path(&destination).await
            || state.transport.delivery_link_available(&destination).await;
        let relay_route = if let Some(relay) = relay.as_ref() {
            state.transport.has_path(relay).await
                || state.transport.delivery_link_available(relay).await
        } else {
            false
        };
        // After an unconfirmed attempt, a cached path must not starve a visible relay.
        if local_custody_should_try_direct(status, direct_route, event_direct, relay_route) {
            let lora = destination_uses_rnode_ble_route(state, &destination).await;
            let policy_state = state.clone();
            let result = store
                .recover_direct(
                    &status.transient_id,
                    now,
                    now.saturating_add(30),
                    local_propagation_route_limits(lora),
                    move |record, wire| {
                        local_recovery_allowed(&policy_state, record, wire, event_direct)
                    },
                )
                .await;
            if let Ok(Some(report)) = result.as_ref() {
                register_receipt_tracking(
                    &state.receipt_tracker,
                    report.receipt_hash.as_deref(),
                    &report.message_id.0,
                );
            }
            publish_local_custody(state, bus).await?;
            return result
                .map(|_| ())
                .map_err(crate::sdk_bridge::map_sdk_error_to_node_error);
        }
        if *state.power_saver_rx.borrow() {
            continue;
        }
        if !status.uploaded {
            if let Some(relay) = relay {
                let lora = active_interfaces_are_rnode_ble_only(
                    &*state.active_interface_registry.lock().await,
                );
                let limit = if lora {
                    crate::sdk_bridge::LOCAL_PROPAGATION_LORA_TRANSFER_BYTES as usize
                } else {
                    25 * 1024 * 1024
                };
                return recover_local_upload(state, bus, &status.transient_id, relay, limit).await;
            }
        }
    }
    Ok(())
}

fn local_custody_should_try_direct(
    status: &lxmf_runtime::propagation_store::Status,
    direct_route: bool,
    recipient_event: bool,
    relay_route: bool,
) -> bool {
    direct_route && (recipient_event || status.attempts % 2 == 0 || status.uploaded || !relay_route)
}

async fn acknowledge_local_application_message(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source: &str,
    metadata: &MissionSyncMetadata,
    application_ack_state: ApplicationAckState,
) -> bool {
    let Some(store) = &state.sdk.local_propagation else {
        return false;
    };
    let Some(key) = metadata.tracking_key() else {
        return false;
    };
    let Ok(candidates) = store.application_messages(key) else {
        return false;
    };
    let mut matched = Vec::new();
    for candidate in candidates {
        if peer_destinations_equivalent(state, &candidate.destination, source).await {
            matched.push(candidate);
        }
    }
    // Reused application keys cannot identify which immutable wire was acknowledged.
    if matched.len() != 1 {
        return false;
    }
    let candidate = &matched[0];
    state
        .pending_lxmf_deliveries
        .lock()
        .await
        .retain(|_, pending| pending.message_id_hex != candidate.message_id);
    if let Some(key) = candidate.application_key.as_deref() {
        state.pending_lxmf_acknowledgements.lock().await.remove(key);
    }
    let accepted = matches!(
        application_ack_state,
        ApplicationAckState::Accepted {} | ApplicationAckState::Completed {}
    );
    if let Ok(true) = store.delivered(
        &candidate.transient_id,
        &candidate.message_id,
        accepted,
        crate::numeric::u64_to_i64_saturating(now_ms() / 1000),
    ) {
        let _ = publish_local_custody(state, bus).await;
        if !accepted {
            let pending = PendingLxmfDelivery {
                message_id_hex: candidate.message_id.clone(),
                destination_hex: candidate.destination.clone(),
                correlation_id: metadata.correlation_id.clone(),
                command_id: candidate.application_key.clone(),
                command_type: if matches!(
                    candidate.kind,
                    lxmf_runtime::propagation_store::MessageKind::Telemetry { .. }
                ) {
                    Some("mission.registry.telemetry.upsert".to_owned())
                } else {
                    metadata.command_type.clone()
                },
                event_uid: metadata.event_uid.clone(),
                mission_uid: metadata.mission_uid.clone(),
                method: LxmfDeliveryMethod::Direct {},
                representation: LxmfDeliveryRepresentation::Packet {},
                relay_destination_hex: candidate.relay.clone(),
                fallback_stage: None,
                resend: None,
                sent_at_ms: 0,
            };
            emit_lxmf_delivery_with_source(
                &state.app_state,
                bus,
                &pending,
                Some(source.to_owned()),
                LxmfDeliveryStatus::Acknowledged {},
                application_ack_state,
                metadata.ack_detail().map(ToOwned::to_owned),
            );
        }
    }
    true
}
