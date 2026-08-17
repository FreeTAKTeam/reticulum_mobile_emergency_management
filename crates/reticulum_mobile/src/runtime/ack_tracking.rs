async fn has_active_reticulum_interface(state: &NodeRuntimeState) -> bool {
    !state.active_interface_registry.lock().await.is_empty()
}

fn active_interfaces_include_relay_transport(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
) -> bool {
    active_interfaces
        .values()
        .any(|interface| !interface_label_is_rnode_ble(&interface.label))
}

fn active_interfaces_are_rnode_ble_only(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
) -> bool {
    !active_interfaces.is_empty()
        && active_interfaces
            .values()
            .all(|interface| interface_label_is_rnode_ble(&interface.label))
}

fn interface_label_is_rnode_ble(interface: &str) -> bool {
    interface.starts_with("rnode-ble:") || interface.starts_with("rnode-bluetooth-classic:")
}

fn active_interface_is_rnode_ble(
    active_interfaces: &HashMap<AddressHash, InterfaceStatusRecord>,
    interface: &AddressHash,
) -> bool {
    active_interfaces
        .get(interface)
        .is_some_and(|status| interface_label_is_rnode_ble(&status.label))
}

fn link_connect_timeout(rnode_route: bool) -> Duration {
    if rnode_route {
        RNODE_BLE_LINK_CONNECT_TIMEOUT
    } else {
        DEFAULT_LINK_CONNECT_TIMEOUT
    }
}

async fn destination_uses_rnode_ble_route(
    state: &NodeRuntimeState,
    destination: &AddressHash,
) -> bool {
    let destination_hex = address_hash_to_hex(destination);
    let active_interfaces = state.active_interface_registry.lock().await.clone();
    if active_interfaces_are_rnode_ble_only(&active_interfaces) {
        return true;
    }

    state
        .app_state
        .list_announces()
        .ok()
        .and_then(|announces| {
            announces.into_iter().find(|announce| {
                normalize_hex_32(announce.destination_hex.as_str()).as_deref()
                    == Some(destination_hex.as_str())
            })
        })
        .and_then(|announce| parse_address_hash(announce.interface_hex.as_str()).ok())
        .is_some_and(|interface| active_interface_is_rnode_ble(&active_interfaces, &interface))
}

async fn has_active_relay_transport_interface(state: &NodeRuntimeState) -> bool {
    let active_interfaces = state.active_interface_registry.lock().await;
    active_interfaces_include_relay_transport(&active_interfaces)
}

#[expect(
    clippy::too_many_arguments,
    reason = "resend construction mirrors the persisted pending delivery fields"
)]
fn build_pending_lxmf_resend(
    report: &LxmfSendReport,
    requested_destination_hex: &str,
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    send_mode: SendMode,
    send_task_class: SendTaskClass,
) -> Option<PendingLxmfResend> {
    if !report.track_delivery_timeout
        || !matches!(send_mode, SendMode::Auto {})
        || (report.used_propagation_node
            && !matches!(
                report.fallback_stage,
                Some(LxmfFallbackStage::AfterDirectRetryBudget {})
            ))
    {
        return None;
    }
    let metadata = metadata?;
    if !metadata.command_present || metadata.tracking_key().is_none() {
        return None;
    }
    Some(PendingLxmfResend {
        requested_destination_hex: requested_destination_hex.to_string(),
        body: body.to_vec(),
        title,
        fields_bytes,
        metadata,
        send_task_class,
        original_send_mode: send_mode,
        direct_ack_retry_attempted: matches!(
            report.fallback_stage,
            Some(LxmfFallbackStage::AfterDirectRetryBudget {})
        ),
        propagation_fallback_attempted: matches!(
            report.fallback_stage,
            Some(LxmfFallbackStage::AfterDirectRetryBudget {})
        ),
    })
}

fn pending_tracking_key(pending: &PendingLxmfDelivery) -> Option<String> {
    pending
        .command_id
        .as_deref()
        .or(pending.correlation_id.as_deref())
        .map(ToOwned::to_owned)
}

fn chat_delivery_ack_body(message_id_hex: &str) -> String {
    format!("{CHAT_DELIVERY_ACK_PREFIX}{message_id_hex}")
}

fn parse_chat_delivery_ack_body(body: &str) -> Option<String> {
    let message_id_hex = body.trim().strip_prefix(CHAT_DELIVERY_ACK_PREFIX)?.trim();
    let valid_message_id =
        message_id_hex.len() == 64 && message_id_hex.chars().all(|ch| ch.is_ascii_hexdigit());
    valid_message_id.then(|| message_id_hex.to_ascii_lowercase())
}

fn should_retry_pending_ack_timeout_via_direct(pending: &PendingLxmfDelivery) -> bool {
    pending.resend.as_ref().is_some_and(|resend| {
        matches!(resend.original_send_mode, SendMode::Auto {})
            && !resend.direct_ack_retry_attempted
            && !resend.propagation_fallback_attempted
            && !matches!(pending.method, LxmfDeliveryMethod::Propagated {})
            && pending.relay_destination_hex.is_none()
    })
}

fn should_retry_pending_ack_timeout_via_propagation(
    pending: &PendingLxmfDelivery,
    has_active_relay: bool,
) -> bool {
    has_active_relay
        && pending.resend.as_ref().is_some_and(|resend| {
            matches!(resend.original_send_mode, SendMode::Auto {})
                && !resend.propagation_fallback_attempted
        })
}

fn pending_ack_timeout_elapsed(pending: &PendingLxmfDelivery, now: u64) -> bool {
    let timeout = if matches!(pending.method, LxmfDeliveryMethod::Propagated {})
        || pending.relay_destination_hex.is_some()
    {
        PROPAGATED_LXMF_ACK_TIMEOUT
    } else {
        DEFAULT_LXMF_ACK_TIMEOUT
    };
    now.saturating_sub(pending.sent_at_ms)
        >= crate::numeric::u128_to_u64_saturating(timeout.as_millis())
}

fn record_pending_delivery_timed_out(
    sdk: &RuntimeLxmfSdk,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    detail: &str,
) {
    sdk.record_delivery_timed_out(
        &pending.message_id_hex,
        &pending.destination_hex,
        pending.correlation_id.as_deref(),
        pending.command_id.as_deref(),
        pending.command_type.as_deref(),
        pending.event_uid.as_deref(),
        pending.mission_uid.as_deref(),
        Some(detail),
    );
    emit_lxmf_delivery(
        bus,
        pending,
        LxmfDeliveryStatus::TimedOut {},
        Some(detail.to_string()),
    );
    bus.emit(NodeEvent::Error {
        code: "NetworkError".to_string(),
        message: format!(
            "lxmf delivery acknowledgement timeout destination={} command={} correlation={} detail={detail}",
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
        ),
    });
    info!(
        "[lxmf][mission] timed out message_id={} destination={} command={} correlation={} detail={}",
        pending.message_id_hex,
        pending.destination_hex,
        pending.command_type.as_deref().unwrap_or("-"),
        pending.correlation_id.as_deref().unwrap_or("-"),
        detail,
    );
}

async fn acknowledge_pending_with_buffered_ack(
    state: &NodeRuntimeState,
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    buffered_ack: PendingLxmfAcknowledgement,
) -> bool {
    let tracking_key = pending_tracking_key(pending);
    if peer_destinations_equivalent(
        state,
        pending.destination_hex.as_str(),
        buffered_ack.source_hex.as_str(),
    )
    .await
    {
        if let Some(tracking_key) = tracking_key.as_deref() {
            state
                .pending_lxmf_deliveries
                .lock()
                .await
                .remove(tracking_key);
        }
        state.sdk.record_delivery_acknowledged(
            &pending.message_id_hex,
            &pending.destination_hex,
            Some(buffered_ack.source_hex.as_str()),
            pending.correlation_id.as_deref(),
            pending.command_id.as_deref(),
            pending.command_type.as_deref(),
            pending.event_uid.as_deref(),
            pending.mission_uid.as_deref(),
            buffered_ack.detail.as_deref(),
        );
        emit_lxmf_delivery_with_source(
            bus,
            pending,
            Some(buffered_ack.source_hex.clone()),
            LxmfDeliveryStatus::Acknowledged {},
            buffered_ack.application_ack_state,
            buffered_ack.detail.clone(),
        );
        info!(
            "[lxmf][mission] acknowledged buffered message_id={} destination={} command={} correlation={} detail={}",
            pending.message_id_hex,
            pending.destination_hex,
            pending.command_type.as_deref().unwrap_or("-"),
            pending.correlation_id.as_deref().unwrap_or("-"),
            buffered_ack.detail.as_deref().unwrap_or("-"),
        );
        true
    } else {
        if let Some(tracking_key) = tracking_key {
            state
                .pending_lxmf_acknowledgements
                .lock()
                .await
                .insert(tracking_key, buffered_ack.clone());
        }
        info!(
            "[lxmf][mission] buffered acknowledgement source mismatch message_id={} destination={} source={}",
            pending.message_id_hex,
            pending.destination_hex,
            buffered_ack.source_hex,
        );
        false
    }
}
