fn log_send_task(class: SendTaskClass, message: String) {
    match class {
        SendTaskClass::Mission
        | SendTaskClass::MissionAck
        | SendTaskClass::MissionPropagation
        | SendTaskClass::MissionRecovery => {
            info!("{message}")
        }
        SendTaskClass::General => debug!("{message}"),
    }
}

impl ReceiptHandler for RuntimeReceiptBridge {
    fn on_receipt(&self, receipt: &DeliveryReceipt) {
        let packet_hash_hex = hex::encode(receipt.message_id);
        let Some(message_id_hex) = self
            .receipt_message_ids
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&packet_hash_hex))
            .map(|tracking| tracking.message_id_hex)
        else {
            return;
        };
        let _ = self.tx.send(message_id_hex);
    }
}

fn transport_state_for_lxmf_status(status: LxmfDeliveryStatus) -> TransportDeliveryState {
    match status {
        LxmfDeliveryStatus::Sent {} => TransportDeliveryState::SentDirect {},
        LxmfDeliveryStatus::SentToPropagation {} => TransportDeliveryState::SentToPropagation {},
        LxmfDeliveryStatus::Acknowledged {} => TransportDeliveryState::TransportDelivered {},
        LxmfDeliveryStatus::Failed {} => TransportDeliveryState::Failed {},
        LxmfDeliveryStatus::TimedOut {} => TransportDeliveryState::TimedOut {},
    }
}

fn application_ack_state_for_lxmf_status(status: LxmfDeliveryStatus) -> ApplicationAckState {
    match status {
        LxmfDeliveryStatus::Acknowledged {} => ApplicationAckState::Accepted {},
        LxmfDeliveryStatus::Failed {} | LxmfDeliveryStatus::TimedOut {} => {
            ApplicationAckState::Failed {}
        }
        LxmfDeliveryStatus::Sent {} | LxmfDeliveryStatus::SentToPropagation {} => {
            ApplicationAckState::Waiting {}
        }
    }
}

fn application_ack_state_for_mission_metadata(
    metadata: &MissionSyncMetadata,
) -> ApplicationAckState {
    if metadata.result_present {
        return match metadata
            .result_status
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("completed" | "complete" | "done" | "success" | "succeeded" | "ok") => {
                ApplicationAckState::Completed {}
            }
            Some("rejected" | "reject" | "denied" | "declined") => ApplicationAckState::Rejected {},
            Some("failed" | "failure" | "error" | "timeout" | "timed_out" | "cancelled") => {
                ApplicationAckState::Failed {}
            }
            _ => ApplicationAckState::Accepted {},
        };
    }

    if metadata.event_present {
        ApplicationAckState::Accepted {}
    } else {
        ApplicationAckState::Waiting {}
    }
}

fn transport_state_for_message_state(state: MessageState) -> TransportDeliveryState {
    match state {
        MessageState::Queued {} | MessageState::PathRequested {} => {
            TransportDeliveryState::Queued {}
        }
        MessageState::LinkEstablishing {} | MessageState::Sending {} => {
            TransportDeliveryState::Sending {}
        }
        MessageState::SentDirect {} => TransportDeliveryState::SentDirect {},
        MessageState::SentToPropagation {} => TransportDeliveryState::SentToPropagation {},
        MessageState::Delivered {} | MessageState::Received {} => {
            TransportDeliveryState::TransportDelivered {}
        }
        MessageState::Failed {} => TransportDeliveryState::Failed {},
        MessageState::TimedOut {} => TransportDeliveryState::TimedOut {},
        MessageState::Cancelled {} => TransportDeliveryState::Cancelled {},
    }
}

fn emit_lxmf_delivery(
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    status: LxmfDeliveryStatus,
    detail: Option<String>,
) {
    let now = now_ms();
    bus.emit(NodeEvent::LxmfDelivery {
        update: LxmfDeliveryUpdate {
            message_id_hex: pending.message_id_hex.clone(),
            destination_hex: pending.destination_hex.clone(),
            source_hex: None,
            correlation_id: pending.correlation_id.clone(),
            command_id: pending.command_id.clone(),
            command_type: pending.command_type.clone(),
            event_uid: pending.event_uid.clone(),
            mission_uid: pending.mission_uid.clone(),
            status,
            transport_state: transport_state_for_lxmf_status(status),
            application_ack_state: application_ack_state_for_lxmf_status(status),
            method: pending.method,
            representation: pending.representation,
            relay_destination_hex: pending.relay_destination_hex.clone(),
            fallback_stage: pending.fallback_stage,
            detail,
            sent_at_ms: pending.sent_at_ms,
            updated_at_ms: now,
        },
    });
}

fn emit_lxmf_delivery_with_source(
    bus: &EventBus,
    pending: &PendingLxmfDelivery,
    source_hex: Option<String>,
    status: LxmfDeliveryStatus,
    application_ack_state: ApplicationAckState,
    detail: Option<String>,
) {
    let now = now_ms();
    bus.emit(NodeEvent::LxmfDelivery {
        update: LxmfDeliveryUpdate {
            message_id_hex: pending.message_id_hex.clone(),
            destination_hex: pending.destination_hex.clone(),
            source_hex,
            correlation_id: pending.correlation_id.clone(),
            command_id: pending.command_id.clone(),
            command_type: pending.command_type.clone(),
            event_uid: pending.event_uid.clone(),
            mission_uid: pending.mission_uid.clone(),
            status,
            transport_state: transport_state_for_lxmf_status(status),
            application_ack_state,
            method: pending.method,
            representation: pending.representation,
            relay_destination_hex: pending.relay_destination_hex.clone(),
            fallback_stage: pending.fallback_stage,
            detail,
            sent_at_ms: pending.sent_at_ms,
            updated_at_ms: now,
        },
    });
}

fn create_transport_data_packet(destination: AddressHash, bytes: &[u8]) -> Packet {
    let mut packet = Packet::default();
    packet.header.propagation_type = PropagationType::Transport;
    packet.destination = destination;
    packet.data = PacketDataBuffer::new_from_slice(bytes);
    packet
}

async fn send_transport_packet_with_path_retry(
    transport: &Arc<Transport>,
    destination: AddressHash,
    bytes: &[u8],
) -> RnsSendOutcome {
    const MAX_ATTEMPTS: usize = 6;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    let mut last_outcome = RnsSendOutcome::DroppedNoRoute;

    for _ in 0..MAX_ATTEMPTS {
        let packet = create_transport_data_packet(destination, bytes);
        let outcome = transport.send_packet_with_outcome(packet).await;
        if matches!(
            outcome,
            RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
        ) {
            return outcome;
        }

        last_outcome = outcome;
        if matches!(
            outcome,
            RnsSendOutcome::DroppedNoRoute | RnsSendOutcome::DroppedMissingDestinationIdentity
        ) {
            transport.request_path(&destination, None, None).await;
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }
        break;
    }

    last_outcome
}

fn conversation_id_for(destination_hex: &str) -> String {
    sdkmsg::MessagingStore::conversation_id_for(destination_hex)
}

fn app_data_from_hub_directory_capabilities(capabilities: &[String]) -> Option<String> {
    (!capabilities.is_empty()).then(|| capabilities.join(","))
}

fn merge_hub_directory_peer_records(
    peers: &mut Vec<PeerRecord>,
    snapshot: Option<&HubDirectorySnapshot>,
    local_app_destination_hex: &str,
) {
    let Some(snapshot) = snapshot else {
        return;
    };

    let local_app_destination_hex = normalize_hex_32(local_app_destination_hex);
    let mut existing_by_destination = peers
        .iter()
        .enumerate()
        .filter_map(|(index, peer)| {
            normalize_hex_32(peer.destination_hex.as_str()).map(|destination| (destination, index))
        })
        .collect::<HashMap<_, _>>();

    for item in &snapshot.items {
        let Some(destination_hex) = normalize_hex_32(item.destination_hash.as_str()) else {
            continue;
        };
        if local_app_destination_hex.as_deref() == Some(destination_hex.as_str()) {
            continue;
        }

        let item_identity_hex = normalize_hex_32(item.identity.as_str());
        let item_app_data = app_data_from_hub_directory_capabilities(&item.announce_capabilities);

        if let Some(index) = existing_by_destination
            .get(destination_hex.as_str())
            .copied()
        {
            let peer = &mut peers[index];
            peer.hub_derived = true;
            if peer.identity_hex.is_none() {
                peer.identity_hex = item_identity_hex.clone();
            }
            if peer.display_name.is_none() {
                peer.display_name = item.display_name.clone();
            }
            if peer.app_data.as_deref().is_none_or(str::is_empty) {
                peer.app_data = item_app_data.clone();
            }
            continue;
        }

        peers.push(PeerRecord {
            destination_hex: destination_hex.clone(),
            identity_hex: item_identity_hex,
            lxmf_destination_hex: None,
            display_name: item.display_name.clone(),
            app_data: item_app_data,
            state: PeerState::Disconnected {},
            saved: false,
            stale: false,
            active_link: false,
            hub_derived: true,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: None,
            last_seen_at_ms: snapshot.received_at_ms,
            announce_last_seen_at_ms: None,
            lxmf_last_seen_at_ms: None,
        });
        existing_by_destination.insert(destination_hex, peers.len().saturating_sub(1));
    }
}

async fn snapshot_peer_records(state: &NodeRuntimeState) -> Vec<PeerRecord> {
    let mut peers = state
        .messaging
        .lock()
        .await
        .list_peers()
        .into_iter()
        .map(from_sdk_peer_record)
        .collect::<Vec<_>>();
    let hub_directory_snapshot = state
        .hub_directory_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    merge_hub_directory_peer_records(
        &mut peers,
        hub_directory_snapshot.as_ref(),
        state.app_destination_hex.as_str(),
    );
    peers
}

async fn refresh_peer_snapshot(state: &NodeRuntimeState) -> bool {
    let peers = snapshot_peer_records(state).await;
    let changed = state
        .projection_journal
        .record_peers(peers.clone(), Some("peer-snapshot-refresh"));
    if let Ok(mut guard) = state.peers_snapshot.lock() {
        *guard = peers;
    }
    changed
}

fn refresh_sync_status_snapshot(state: &NodeRuntimeState, status: &SyncStatus) -> bool {
    let changed = state
        .projection_journal
        .record_sync_status(status.clone(), Some("sync-status-refresh"));
    if let Ok(mut guard) = state.sync_status_snapshot.lock() {
        *guard = status.clone();
    }
    changed
}

async fn emit_sync_status_update(
    state: &NodeRuntimeState,
    bus: &EventBus,
    phase: sdkmsg::SyncPhase,
    requested_at_ms: u64,
    messages_received: u32,
    detail: Option<String>,
    completed: bool,
) -> SyncStatus {
    let status_update =
        from_sdk_sync_status(state.messaging.lock().await.update_sync_status(|status| {
            status.phase = phase;
            status.requested_at_ms = Some(requested_at_ms);
            status.completed_at_ms = completed.then(now_ms);
            status.messages_received = messages_received;
            status.detail = detail;
        }));
    if refresh_sync_status_snapshot(state, &status_update) {
        bus.emit(NodeEvent::SyncUpdated {
            status: status_update.clone(),
        });
    }
    status_update
}
