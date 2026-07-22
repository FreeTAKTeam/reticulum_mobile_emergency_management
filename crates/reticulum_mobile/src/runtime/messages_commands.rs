async fn upsert_message_record(
    state: &NodeRuntimeState,
    bus: &EventBus,
    message: MessageRecord,
    emit_received: bool,
) {
    let message = canonicalize_chat_message(&message);
    match state.app_state.upsert_message(&message) {
        Ok(invalidations) => {
            for invalidation in invalidations {
                bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
            }
        }
        Err(error) => {
            bus.emit(NodeEvent::Error {
                code: "IoError".to_string(),
                message: format!(
                    "failed to persist message id={} reason={error}",
                    message.message_id_hex
                ),
            });
        }
    }
    let changed = state
        .projection_journal
        .record_message(message.clone(), Some("message-upsert"));
    state
        .messaging
        .lock()
        .await
        .upsert_message(to_sdk_message_record(message.clone()));

    if changed {
        if emit_received {
            bus.emit(NodeEvent::MessageReceived {
                message: message.clone(),
            });
        }
        bus.emit(NodeEvent::MessageUpdated { message });
    }
}

async fn delete_conversation_records(
    state: &NodeRuntimeState,
    bus: &EventBus,
    conversation_id: &str,
) -> Result<(), NodeError> {
    let peers = state
        .peers_snapshot
        .lock()
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
        .clone();
    let resolver = conversation_peer_resolver(&peers);
    for invalidation in state
        .app_state
        .delete_conversation_resolved(conversation_id, &resolver)?
    {
        bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    }

    let delete_keys = conversation_delete_keys(conversation_id, &peers);
    let projection_changed = state.projection_journal.remove_conversation_messages(
        delete_keys.iter().map(String::as_str),
        Some("conversation-deleted"),
    );
    state
        .messaging
        .lock()
        .await
        .delete_conversation_messages(delete_keys.iter().map(String::as_str));
    if projection_changed {
        state.projection_journal.flush_now().await;
    }
    Ok(())
}

async fn message_records_snapshot(
    state: &NodeRuntimeState,
    conversation_id: Option<&str>,
) -> Vec<MessageRecord> {
    state
        .messaging
        .lock()
        .await
        .list_messages(conversation_id)
        .into_iter()
        .map(from_sdk_message_record)
        .collect()
}

async fn conversation_records_snapshot(state: &NodeRuntimeState) -> Vec<ConversationRecord> {
    state
        .messaging
        .lock()
        .await
        .list_conversations()
        .into_iter()
        .map(from_sdk_conversation_record)
        .collect()
}

pub enum Command {
    Stop {
        resp: cb::Sender<Result<(), NodeError>>,
    },
    AnnounceNow {},
    ConnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    DisconnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetSavedPeers {
        peers: Vec<SavedPeerRecord>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendBytes {
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    BroadcastBytes {
        bytes: Vec<u8>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    RequestPeerIdentity {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendLxmf {
        request: SendLxmfRequest,
        fields_bytes: Vec<u8>,
        resp: cb::Sender<Result<String, NodeError>>,
    },
    RetryLxmf {
        message_id_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    CancelLxmf {
        message_id_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetActivePropagationNode {
        destination_hex: Option<String>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    RequestLxmfSync {
        limit: Option<u32>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    ListAnnounces {
        resp: cb::Sender<Result<Vec<AnnounceRecord>, NodeError>>,
    },
    ListPeers {
        resp: cb::Sender<Result<Vec<PeerRecord>, NodeError>>,
    },
    ListConversations {
        resp: cb::Sender<Result<Vec<ConversationRecord>, NodeError>>,
    },
    ListMessages {
        conversation_id: Option<String>,
        resp: cb::Sender<Result<Vec<MessageRecord>, NodeError>>,
    },
    DeleteConversation {
        conversation_id: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    GetLxmfSyncStatus {
        resp: cb::Sender<Result<SyncStatus, NodeError>>,
    },
    SetAnnounceCapabilities {
        capability_string: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetLogLevel {
        level: crate::types::LogLevel,
    },
    RefreshHubDirectory {
        resp: cb::Sender<Result<(), NodeError>>,
    },
}

#[derive(Clone)]
struct NodeRuntimeState {
    app_state: AppStateStore,
    identity: PrivateIdentity,
    app_destination_hex: String,
    transport: Arc<Transport>,
    lxmf_destination: Arc<TokioMutex<SingleInputDestination>>,
    peer_resolution_inflight: Arc<TokioMutex<HashSet<String>>>,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<Link>>>>>,
    active_interface_registry: ActiveInterfaceRegistry,
    connected_peers: Arc<TokioMutex<HashSet<AddressHash>>>,
    pending_lxmf_deliveries: Arc<TokioMutex<HashMap<String, PendingLxmfDelivery>>>,
    pending_lxmf_acknowledgements: Arc<TokioMutex<HashMap<String, PendingLxmfAcknowledgement>>>,
    messaging: Arc<TokioMutex<sdkmsg::MessagingStore>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    hub_directory_snapshot: Arc<Mutex<Option<HubDirectorySnapshot>>>,
    projection_journal: Arc<RuntimeProjectionJournal>,
    sdk: Arc<RuntimeLxmfSdk>,
    active_propagation_node_hex: Arc<TokioMutex<Option<String>>>,
    preferred_propagation_node_hex: Option<String>,
    propagation_sync_inflight: Arc<AtomicBool>,
    direct_delivery_health: DirectDeliveryHealth,
    managed_peer_links: ManagedPeerLinks,
    ignored_peer_destinations: Arc<TokioMutex<HashSet<String>>>,
    send_task_permits: SendTaskPermits,
    mission_destination_locks: MissionDestinationLocks,
}

fn prune_expired_buffered_acknowledgements(
    pending_lxmf_acknowledgements: &mut HashMap<String, PendingLxmfAcknowledgement>,
    now_ms: u64,
) -> usize {
    let before = pending_lxmf_acknowledgements.len();
    pending_lxmf_acknowledgements.retain(|_, pending| {
        now_ms.saturating_sub(pending.buffered_at_ms)
            < crate::numeric::u128_to_u64_saturating(DEFAULT_BUFFERED_ACK_TTL.as_millis())
    });
    before.saturating_sub(pending_lxmf_acknowledgements.len())
}

fn prune_expired_receipt_tracking(
    receipt_message_ids: &mut HashMap<String, ReceiptMessageTracking>,
    now_ms: u64,
) -> usize {
    let before = receipt_message_ids.len();
    receipt_message_ids.retain(|_, tracking| {
        let ttl = match tracking {
            ReceiptMessageTracking::Pending { .. } => DEFAULT_RECEIPT_TRACKING_TTL,
            ReceiptMessageTracking::Observed { .. } => RECEIPT_REGISTRATION_RACE_TTL,
        };
        now_ms.saturating_sub(tracking.recorded_at_ms())
            < crate::numeric::u128_to_u64_saturating(ttl.as_millis())
    });
    before.saturating_sub(receipt_message_ids.len())
}
