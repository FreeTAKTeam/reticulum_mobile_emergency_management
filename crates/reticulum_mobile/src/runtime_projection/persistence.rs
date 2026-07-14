fn persisted_peer_from_runtime(record: &PeerRecord) -> Option<PersistedPeerRecord> {
    Some(PersistedPeerRecord {
        destination_hex: record.destination_hex.clone(),
        identity_hex: record.identity_hex.clone(),
        lxmf_destination_hex: record.lxmf_destination_hex.clone(),
        display_name: record.display_name.clone(),
        app_data: record.app_data.clone(),
        state: peer_state_to_string(PeerState::Disconnected {}),
        saved: Some(record.saved),
        management_state: None,
        stale: record.stale,
        active_link: false,
        hub_derived: record.hub_derived,
        last_resolution_error: record.last_resolution_error.clone(),
        last_resolution_attempt_at_ms: None,
        last_seen_at_ms: 0,
        announce_last_seen_at_ms: None,
        lxmf_last_seen_at_ms: None,
    })
}

fn persisted_saved_peers(records: &[PeerRecord]) -> Option<Vec<PersistedPeerRecord>> {
    records
        .iter()
        .filter(|record| record.saved)
        .map(persisted_peer_from_runtime)
        .collect::<Option<Vec<_>>>()
}

fn persisted_message_from_runtime(record: &MessageRecord) -> Option<PersistedMessageRecord> {
    Some(PersistedMessageRecord {
        message_id_hex: record.message_id_hex.clone(),
        conversation_id: record.conversation_id.clone(),
        direction: message_direction_to_string(record.direction),
        destination_hex: record.destination_hex.clone(),
        source_hex: record.source_hex.clone(),
        requested_destination_hex: record.requested_destination_hex.clone(),
        delivery_destination_hex: record.delivery_destination_hex.clone(),
        recipient_identity_hex: record.recipient_identity_hex.clone(),
        last_wire_message_id_hex: record.last_wire_message_id_hex.clone(),
        title: record.title.clone(),
        body_utf8: record.body_utf8.clone(),
        method: message_method_to_string(record.method),
        state: message_state_to_string(record.state),
        transport_state: record.transport_state,
        application_ack_state: record.application_ack_state,
        detail: record.detail.clone(),
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    })
}

fn persisted_sync_from_runtime(status: &SyncStatus) -> Option<PersistedSyncStatus> {
    Some(PersistedSyncStatus {
        phase: sync_phase_to_string(status.phase),
        active_propagation_node_hex: status.active_propagation_node_hex.clone(),
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail.clone(),
    })
}

fn runtime_peer_from_persisted(record: PersistedPeerRecord) -> PeerRecord {
    PeerRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: PeerState::Disconnected {},
        saved: record.saved.unwrap_or_else(|| {
            record
                .management_state
                .as_deref()
                .is_some_and(|value| value == "managed")
        }),
        stale: record.stale,
        active_link: false,
        hub_derived: record.hub_derived,
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: None,
        last_seen_at_ms: 0,
        announce_last_seen_at_ms: None,
        lxmf_last_seen_at_ms: None,
    }
}

fn runtime_message_from_persisted(record: PersistedMessageRecord) -> MessageRecord {
    MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: message_direction_from_string(record.direction)
            .unwrap_or(MessageDirection::Outbound {}),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        requested_destination_hex: record.requested_destination_hex,
        delivery_destination_hex: record.delivery_destination_hex,
        recipient_identity_hex: record.recipient_identity_hex,
        last_wire_message_id_hex: record.last_wire_message_id_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: message_method_from_string(record.method).unwrap_or(MessageMethod::Direct {}),
        state: message_state_from_string(record.state).unwrap_or(MessageState::Queued {}),
        transport_state: record.transport_state,
        application_ack_state: record.application_ack_state,
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn runtime_sync_from_persisted(status: PersistedSyncStatus) -> SyncStatus {
    SyncStatus {
        phase: sync_phase_from_string(status.phase).unwrap_or(SyncPhase::Idle {}),
        active_propagation_node_hex: status.active_propagation_node_hex,
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail,
    }
}

fn peer_state_to_string(state: PeerState) -> String {
    match state {
        PeerState::Connecting {} => "connecting".to_string(),
        PeerState::Connected {} => "connected".to_string(),
        PeerState::Disconnected {} => "disconnected".to_string(),
    }
}

fn message_direction_to_string(direction: MessageDirection) -> String {
    match direction {
        MessageDirection::Inbound {} => "inbound".to_string(),
        MessageDirection::Outbound {} => "outbound".to_string(),
    }
}

fn message_direction_from_string(value: String) -> Option<MessageDirection> {
    match value.as_str() {
        "inbound" => Some(MessageDirection::Inbound {}),
        "outbound" => Some(MessageDirection::Outbound {}),
        _ => None,
    }
}

fn message_method_to_string(method: MessageMethod) -> String {
    match method {
        MessageMethod::Direct {} => "direct".to_string(),
        MessageMethod::Opportunistic {} => "opportunistic".to_string(),
        MessageMethod::Propagated {} => "propagated".to_string(),
        MessageMethod::Resource {} => "resource".to_string(),
    }
}

fn message_method_from_string(value: String) -> Option<MessageMethod> {
    match value.as_str() {
        "direct" => Some(MessageMethod::Direct {}),
        "opportunistic" => Some(MessageMethod::Opportunistic {}),
        "propagated" => Some(MessageMethod::Propagated {}),
        "resource" => Some(MessageMethod::Resource {}),
        _ => None,
    }
}

fn message_state_to_string(state: MessageState) -> String {
    match state {
        MessageState::Queued {} => "queued".to_string(),
        MessageState::PathRequested {} => "path-requested".to_string(),
        MessageState::LinkEstablishing {} => "link-establishing".to_string(),
        MessageState::Sending {} => "sending".to_string(),
        MessageState::SentDirect {} => "sent-direct".to_string(),
        MessageState::SentToPropagation {} => "sent-to-propagation".to_string(),
        MessageState::Delivered {} => "delivered".to_string(),
        MessageState::Failed {} => "failed".to_string(),
        MessageState::TimedOut {} => "timed-out".to_string(),
        MessageState::Cancelled {} => "cancelled".to_string(),
        MessageState::Received {} => "received".to_string(),
    }
}

fn message_state_from_string(value: String) -> Option<MessageState> {
    match value.as_str() {
        "queued" => Some(MessageState::Queued {}),
        "path-requested" => Some(MessageState::PathRequested {}),
        "link-establishing" => Some(MessageState::LinkEstablishing {}),
        "sending" => Some(MessageState::Sending {}),
        "sent-direct" => Some(MessageState::SentDirect {}),
        "sent-to-propagation" => Some(MessageState::SentToPropagation {}),
        "delivered" => Some(MessageState::Delivered {}),
        "failed" => Some(MessageState::Failed {}),
        "timed-out" => Some(MessageState::TimedOut {}),
        "cancelled" => Some(MessageState::Cancelled {}),
        "received" => Some(MessageState::Received {}),
        _ => None,
    }
}

fn sync_phase_to_string(phase: SyncPhase) -> String {
    match phase {
        SyncPhase::Idle {} => "idle".to_string(),
        SyncPhase::PathRequested {} => "path-requested".to_string(),
        SyncPhase::LinkEstablishing {} => "link-establishing".to_string(),
        SyncPhase::RequestSent {} => "request-sent".to_string(),
        SyncPhase::Receiving {} => "receiving".to_string(),
        SyncPhase::Complete {} => "complete".to_string(),
        SyncPhase::Failed {} => "failed".to_string(),
    }
}

fn sync_phase_from_string(value: String) -> Option<SyncPhase> {
    match value.as_str() {
        "idle" => Some(SyncPhase::Idle {}),
        "path-requested" => Some(SyncPhase::PathRequested {}),
        "link-establishing" => Some(SyncPhase::LinkEstablishing {}),
        "request-sent" => Some(SyncPhase::RequestSent {}),
        "receiving" => Some(SyncPhase::Receiving {}),
        "complete" => Some(SyncPhase::Complete {}),
        "failed" => Some(SyncPhase::Failed {}),
        _ => None,
    }
}

fn message_matches(a: &MessageRecord, b: &MessageRecord) -> bool {
    serde_json::to_string(a)
        .ok()
        .zip(serde_json::to_string(b).ok())
        .is_some_and(|(left, right)| left == right)
}

fn normalize_message_key(value: impl AsRef<str>) -> String {
    value.as_ref().trim().to_ascii_lowercase()
}

fn message_key_matches(keys: &HashSet<String>, value: &str) -> bool {
    keys.contains(normalize_message_key(value).as_str())
}

fn message_matches_conversation_keys(
    message: &PersistedMessageRecord,
    keys: &HashSet<String>,
) -> bool {
    message_key_matches(keys, message.conversation_id.as_str())
        || message_key_matches(keys, message.destination_hex.as_str())
        || message
            .source_hex
            .as_deref()
            .is_some_and(|source_hex| message_key_matches(keys, source_hex))
}

fn peers_match(left: &[PeerRecord], right: &[PersistedPeerRecord]) -> bool {
    let left = serde_json::to_string(left).ok();
    let right = serde_json::to_string(right).ok();
    left.zip(right).is_some_and(|(l, r)| l == r)
}

fn sync_match(left: &SyncStatus, right: &PersistedSyncStatus) -> bool {
    let left = serde_json::to_string(left).ok();
    let right = serde_json::to_string(right).ok();
    left.zip(right).is_some_and(|(l, r)| l == r)
}
