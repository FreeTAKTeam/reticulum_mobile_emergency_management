#[expect(
    clippy::too_many_arguments,
    reason = "preserve the release delivery boundary"
)]
async fn send_lxmf_with_delivery_policy(
    state: &NodeRuntimeState,
    bus: &EventBus,
    requested_destination: &str,
    body: &[u8],
    title: Option<String>,
    fields: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    mode: SendMode,
    task_class: SendTaskClass,
    allow_inbound_correspondent: bool,
    class: OutboundTrafficClass,
) -> Result<LxmfSendReport, NodeError> {
    let mut failed_attempt = None;
    let result = send_lxmf_with_legacy_delivery_policy(
        state,
        bus,
        requested_destination,
        body,
        title.clone(),
        fields.clone(),
        metadata.clone(),
        mode,
        task_class,
        allow_inbound_correspondent,
        class,
        &mut failed_attempt,
    )
    .await;
    // Successful submissions keep the release's report and retry ownership.
    let error = match result {
        Ok(report) => return Ok(report),
        Err(error) => error,
    };
    if state.sdk.local_propagation.is_none() && !state.sdk.local_propagation_init_failed
        || !matches!(mode, SendMode::Auto {} | SendMode::PropagationOnly {})
        || is_accepted_result_metadata(metadata.as_ref())
        || !matches!(
            class,
            OutboundTrafficClass::Chat {}
                | OutboundTrafficClass::Event {}
                | OutboundTrafficClass::Eam {}
                | OutboundTrafficClass::Checklist {}
                | OutboundTrafficClass::Telemetry {}
        )
        || !matches!(error, NodeError::NetworkError {} | NodeError::Timeout {})
    {
        return Err(error);
    }
    if failed_attempt
        .as_ref()
        .is_some_and(|attempt| !attempt.unreachable)
    {
        return Err(error);
    }
    let resolved = resolve_lxmf_destination_hex(state, requested_destination).await;
    let destination = parse_address_hash(&resolved)?;
    let interfaces_active = has_active_reticulum_interface(state).await;
    if interfaces_active && state.transport.delivery_link_available(&destination).await {
        return Err(error);
    }
    if failed_attempt.is_none() && interfaces_active && state.transport.has_path(&destination).await
    {
        return Err(error);
    }
    if state.sdk.local_propagation_init_failed {
        return Err(NodeError::IoError {});
    }
    // Without a native attempt, the baseline could only reject a missing current route.
    // A busy admission or a failed live transfer never classifies the peer as offline.
    deposit_offline_message(
        state,
        bus,
        requested_destination,
        body,
        title,
        fields,
        metadata,
        class,
        failed_attempt.and_then(|attempt| attempt.outgoing),
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "offline admission retains original message data"
)]
async fn deposit_offline_message(
    state: &NodeRuntimeState,
    bus: &EventBus,
    requested_destination: &str,
    body: &[u8],
    title: Option<String>,
    fields: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    class: OutboundTrafficClass,
    captured: Option<lxmf_runtime::CapturedOutgoing>,
) -> Result<LxmfSendReport, NodeError> {
    let store = state
        .sdk
        .local_propagation
        .as_ref()
        .ok_or(NodeError::IoError {})?;
    let destination =
        resolve_lxmf_destination_for_send(state, requested_destination, false).await?;
    if !local_permission_check(state, &destination, class, metadata.as_ref()) {
        return Err(NodeError::InvalidConfig {});
    }
    let prepared = if let Some(captured) = captured {
        store
            .prepare_captured(
                captured,
                metadata
                    .as_ref()
                    .and_then(MissionSyncMetadata::tracking_key),
            )
            .map_err(crate::sdk_bridge::map_sdk_error_to_node_error)?
    } else {
        // This branch runs only when the release never constructed an outgoing wire.
        state
            .sdk
            .prepare_local_outgoing(
                parse_address_hash(&destination)?,
                body,
                title.as_deref(),
                fields.as_deref(),
                metadata.as_ref(),
            )
            .await?
            .0
    };
    ensure_current_telemetry_delivery(state, class, requested_destination, metadata.as_ref())?;
    if !local_permission_check(state, &prepared.destination(), class, metadata.as_ref()) {
        return Err(NodeError::InvalidConfig {});
    }
    let now = crate::numeric::u64_to_i64_saturating(now_ms() / 1000);
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
            crate::numeric::u64_to_i64_saturating(fix.updated_at_ms / 1000)
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
    // This store is the only retry owner from this commit onward.
    state
        .pending_lxmf_deliveries
        .lock()
        .await
        .retain(|key, pending| {
            pending.message_id_hex != status.message_id
                && metadata
                    .as_ref()
                    .and_then(MissionSyncMetadata::tracking_key)
                    != Some(key.as_str())
        });
    if let Some(key) = metadata
        .as_ref()
        .and_then(MissionSyncMetadata::tracking_key)
    {
        state.pending_lxmf_acknowledgements.lock().await.remove(key);
    }
    if let Err(error) = publish_local_custody(state, bus).await {
        warn!("[local-propagation] durable admission awaits projection retry: {error}");
    }
    let entry = crate::sdk_bridge::local_propagation_record(&status);
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

fn local_propagation_route_limits(lora: bool) -> lxmf_runtime::PropagationRouteLimits {
    lxmf_runtime::PropagationRouteLimits {
        max_packet_bytes: lora.then_some(RNODE_BLE_DIRECT_PACKET_MAX_WIRE_BYTES),
        link_connect_timeout: lora.then_some(RNODE_BLE_LINK_CONNECT_TIMEOUT),
    }
}
