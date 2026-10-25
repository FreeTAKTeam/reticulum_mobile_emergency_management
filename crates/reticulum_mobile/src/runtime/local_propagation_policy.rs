fn local_permission_check(
    state: &NodeRuntimeState,
    destination: &str,
    class: OutboundTrafficClass,
    metadata: Option<&MissionSyncMetadata>,
) -> bool {
    let result = (|| {
        let status = state
            .status
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let peers = state
            .peers_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let snapshot = state
            .hub_directory_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .clone();
        let relay = state
            .sync_status_snapshot
            .lock()
            .map_err(|_| NodeError::InternalError {})?
            .active_propagation_node_hex
            .clone();
        crate::node::local_custody_recipient_allowed(
            &state.app_state,
            &status,
            &state.config,
            &peers,
            snapshot.as_ref(),
            destination,
            class,
            metadata,
            relay.as_deref(),
        )
    })();
    result.unwrap_or(false)
}

fn local_wire_metadata(wire: &[u8]) -> Option<MissionSyncMetadata> {
    let wire = lxmf::WireMessage::unpack(wire).ok()?;
    let fields = wire.payload.fields?;
    parse_mission_sync_metadata(&rmp_serde::to_vec(&fields).ok()?)
}

fn local_custody_kind(
    kind: &lxmf_runtime::propagation_store::MessageKind,
) -> r3akt_mesh_delivery::CustodyKind {
    use lxmf_runtime::propagation_store::MessageKind;
    use r3akt_mesh_delivery::CustodyKind;
    match kind {
        MessageKind::Chat => CustodyKind::Chat,
        MessageKind::Application => CustodyKind::Mission,
        MessageKind::Telemetry { key } => CustodyKind::Telemetry { key: key.clone() },
    }
}

fn local_custody_storage_kind(
    kind: r3akt_mesh_delivery::CustodyKind,
) -> lxmf_runtime::propagation_store::MessageKind {
    use lxmf_runtime::propagation_store::MessageKind;
    use r3akt_mesh_delivery::CustodyKind;
    match kind {
        CustodyKind::Chat => MessageKind::Chat,
        CustodyKind::Mission => MessageKind::Application,
        CustodyKind::Telemetry { key } => MessageKind::Telemetry { key },
    }
}

#[test]
fn local_custody_kind_preserves_mission_class_and_telemetry_key() {
    use lxmf_runtime::propagation_store::MessageKind;
    use r3akt_mesh_delivery::CustodyKind;
    assert_eq!(local_custody_storage_kind(CustodyKind::Mission), MessageKind::Application);
    for kind in [
        CustodyKind::Chat,
        CustodyKind::Mission,
        CustodyKind::Telemetry { key: "recipient-fix".to_owned() },
    ] {
        assert_eq!(local_custody_kind(&local_custody_storage_kind(kind.clone())), kind);
    }
}

fn local_recovery_allowed(
    state: &NodeRuntimeState,
    status: &lxmf_runtime::propagation_store::Status,
    wire: &[u8],
    event_direct: bool,
) -> bool {
    if crate::numeric::u64_to_i64_saturating(now_ms() / 1000) >= status.expires_at {
        return false;
    }
    if *state.power_saver_rx.borrow() && !event_direct {
        return false;
    }
    let metadata = local_wire_metadata(wire);
    let class = match local_custody_kind(&status.kind) {
        r3akt_mesh_delivery::CustodyKind::Chat => OutboundTrafficClass::Chat {},
        r3akt_mesh_delivery::CustodyKind::Telemetry { .. } => {
            OutboundTrafficClass::Telemetry {}
        }
        r3akt_mesh_delivery::CustodyKind::Mission => {
            let command = metadata
                .as_ref()
                .and_then(|metadata| metadata.command_type.as_deref())
                .unwrap_or_default();
            if command.starts_with("mission.marker.")
                || metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata.event_present)
            {
                OutboundTrafficClass::Event {}
            } else if command.starts_with("mission.registry.eam.") {
                OutboundTrafficClass::Eam {}
            } else {
                OutboundTrafficClass::Checklist {}
            }
        }
    };
    local_permission_check(state, &status.destination, class, metadata.as_ref())
}

fn local_custody_detail(state: crate::types::LocalPropagationState) -> &'static str {
    use crate::types::LocalPropagationState;
    match state {
        LocalPropagationState::StoredLocally => "Stored locally",
        LocalPropagationState::Uploading => "Uploading",
        LocalPropagationState::UploadedUnverified => "Uploaded (unverified)",
        LocalPropagationState::RelayHolds => "Relay holds message",
        LocalPropagationState::Delivered => "Delivered",
        LocalPropagationState::Acknowledged => "Acknowledged",
        LocalPropagationState::Expired => "Expired",
        LocalPropagationState::Evicted => "Evicted from local storage",
    }
}

fn emit_local_mission_custody(state: &NodeRuntimeState, bus: &EventBus, report: &LxmfSendReport) {
    let Some(entry) = &report.local_custody else {
        return;
    };
    let Some(metadata) = &report.metadata else {
        return;
    };
    let status = match entry.state {
        crate::types::LocalPropagationState::StoredLocally => LxmfDeliveryStatus::StoredLocally {},
        crate::types::LocalPropagationState::Uploading => LxmfDeliveryStatus::Uploading {},
        crate::types::LocalPropagationState::UploadedUnverified => {
            LxmfDeliveryStatus::UploadedUnverified {}
        }
        crate::types::LocalPropagationState::RelayHolds => LxmfDeliveryStatus::RelayHolds {},
        crate::types::LocalPropagationState::Delivered => LxmfDeliveryStatus::Delivered {},
        crate::types::LocalPropagationState::Acknowledged => LxmfDeliveryStatus::Acknowledged {},
        crate::types::LocalPropagationState::Expired => LxmfDeliveryStatus::Expired {},
        crate::types::LocalPropagationState::Evicted => LxmfDeliveryStatus::Evicted {},
    };
    let pending = PendingLxmfDelivery {
        message_id_hex: report.message_id_hex.clone(),
        destination_hex: report.resolved_destination_hex.clone(),
        correlation_id: metadata.correlation_id.clone(),
        command_id: metadata.command_id.clone(),
        command_type: metadata.command_type.clone(),
        event_uid: metadata.event_uid.clone(),
        mission_uid: metadata.mission_uid.clone(),
        method: report.method,
        representation: report.representation,
        relay_destination_hex: report.relay_destination_hex.clone(),
        fallback_stage: None,
        resend: None,
        sent_at_ms: 0,
    };
    emit_lxmf_delivery_with_source(
        &state.app_state,
        bus,
        &pending,
        None,
        status,
        application_ack_state_for_lxmf_status(status),
        Some(local_custody_detail(entry.state).to_owned()),
    );
}

async fn local_message_in_custody(
    state: &NodeRuntimeState,
    message_id: &str,
) -> Result<bool, NodeError> {
    let Some(store) = &state.sdk.local_propagation else {
        return Ok(false);
    };
    let wire_id = state
        .messaging
        .lock()
        .await
        .list_messages(None)
        .into_iter()
        .find(|message| message.message_id_hex == message_id)
        .and_then(|message| message.last_wire_message_id_hex)
        .unwrap_or_else(|| message_id.to_owned());
    Ok(store
        .message(&wire_id)
        .map_err(crate::sdk_bridge::map_local_propagation_error)?
        .is_some_and(|record| !record.state.terminal()))
}

async fn known_recipient_has_no_usable_route(
    state: &NodeRuntimeState,
    requested: &str,
) -> Result<bool, NodeError> {
    let resolved = resolve_lxmf_destination_hex(state, requested).await;
    let destination = parse_address_hash(&resolved)?;
    if state.transport.has_path(&destination).await
        || state.transport.delivery_link_available(&destination).await
    {
        return Ok(false);
    }
    let saved = saved_peer_matches_destination(state, requested, &resolved).await;
    let team_member = state.app_state.get_app_settings()?.is_some_and(|settings| {
        settings.teams.local_teams.iter().any(|team| {
            crate::team_membership::destinations_include_peer(
                &team.member_destinations,
                requested,
                Some(&resolved),
                None,
            )
        })
    });
    if !saved && !team_member {
        return Ok(false);
    }
    // A live key and usable normal relay still belong to the baseline send path.
    if state
        .transport
        .destination_identity(&destination)
        .await
        .is_some()
        && has_active_relay_transport_interface(state).await
    {
        let active = state.active_propagation_node_hex.lock().await.clone();
        let announces = state.messaging.lock().await.list_announces();
        let candidates = propagation_sync_candidate_relays(
            &announces,
            active.as_deref().unwrap_or(""),
            state.preferred_propagation_node_hex.as_deref(),
        );
        for relay in candidates
            .iter()
            .filter_map(|relay| parse_address_hash(relay).ok())
        {
            if state.transport.has_path(&relay).await
                || state.transport.delivery_link_available(&relay).await
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
