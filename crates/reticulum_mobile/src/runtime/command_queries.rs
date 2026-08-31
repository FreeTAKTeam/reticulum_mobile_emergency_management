fn spawn_cancel_lxmf_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    message_id_hex: String,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        let updated = state
            .messaging
            .lock()
            .await
            .update_message_delivery_state(sdkmsg::MessageDeliveryUpdate {
                message_id_hex: message_id_hex.as_str(),
                state: Some(sdkmsg::MessageState::Cancelled),
                transport_state: Some(sdkmsg::TransportDeliveryState::Cancelled),
                application_ack_state: Some(sdkmsg::ApplicationAckState::Failed),
                detail: Some("cancelled locally".to_string()),
                last_wire_message_id_hex: None,
                updated_at_ms: now_ms(),
            })
            .map(|record| from_sdk_message_record_with_persisted(&state, record))
            .ok_or(NodeError::InvalidConfig {})?;
        upsert_message_record(&state, &bus, updated, false).await;
        Ok(())
    });
}

fn spawn_active_propagation_node_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    destination_hex: Option<String>,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        *state.active_propagation_node_hex.lock().await = destination_hex.clone();
        let status_update = from_sdk_sync_status(
            state
                .messaging
                .lock()
                .await
                .set_active_propagation_node(destination_hex),
        );
        if refresh_sync_status_snapshot(&state, &status_update) {
            bus.emit(NodeEvent::SyncUpdated {
                status: status_update,
            });
        }
        Ok(())
    });
}

fn spawn_list_announces_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    resp: cb::Sender<Result<Vec<AnnounceRecord>, NodeError>>,
) {
    let state = state.clone();
    executor.spawn(lane, RuntimeCommandClass::Local, resp, async move {
        Ok(state
            .messaging
            .lock()
            .await
            .list_announces()
            .into_iter()
            .map(from_sdk_announce_record)
            .collect())
    });
}

fn spawn_list_peers_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    resp: cb::Sender<Result<Vec<PeerRecord>, NodeError>>,
) {
    let state = state.clone();
    executor.spawn(lane, RuntimeCommandClass::Local, resp, async move {
        Ok(snapshot_peer_records(&state).await)
    });
}

fn spawn_list_conversations_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    resp: cb::Sender<Result<Vec<ConversationRecord>, NodeError>>,
) {
    let state = state.clone();
    executor.spawn(lane, RuntimeCommandClass::Local, resp, async move {
        Ok(conversation_records_snapshot(&state).await)
    });
}

fn spawn_list_messages_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    conversation_id: Option<String>,
    resp: cb::Sender<Result<Vec<MessageRecord>, NodeError>>,
) {
    let state = state.clone();
    executor.spawn(lane, RuntimeCommandClass::Local, resp, async move {
        message_records_snapshot(&state, conversation_id.as_deref()).await
    });
}

fn spawn_delete_conversation_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    bus: &EventBus,
    conversation_id: String,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Control, resp, async move {
        delete_conversation_records(&state, &bus, conversation_id.as_str()).await
    });
}

fn spawn_sync_status_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    state: &NodeRuntimeState,
    resp: cb::Sender<Result<SyncStatus, NodeError>>,
) {
    let state = state.clone();
    executor.spawn(lane, RuntimeCommandClass::Local, resp, async move {
        Ok(from_sdk_sync_status(
            state.messaging.lock().await.sync_status(),
        ))
    });
}

fn spawn_hub_refresh_command(
    executor: &RuntimeCommandExecutor,
    lane: RuntimeCommandLane,
    config: &NodeConfig,
    state: &NodeRuntimeState,
    bus: &EventBus,
    resp: cb::Sender<Result<(), NodeError>>,
) {
    let config = config.clone();
    let state = state.clone();
    let bus = bus.clone();
    executor.spawn(lane, RuntimeCommandClass::Work, resp, async move {
        let result = match config.hub_mode {
            HubMode::Autonomous {} => return Err(NodeError::InvalidConfig {}),
            HubMode::SemiAutonomous {} | HubMode::Connected {} => {
                refresh_hub_directory_lxmf(&config, &state).await
            }
        };
        match result {
            Ok(snapshot) => {
                publish_hub_directory_snapshot(&state, &bus, snapshot).await;
                Ok(())
            }
            Err(error) => {
                publish_failed_hub_directory_refresh(&bus, &error).await;
                Err(error)
            }
        }
    });
}
