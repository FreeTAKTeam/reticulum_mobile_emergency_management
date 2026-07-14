fn emit_operational_notice(bus: &EventBus, level: LogLevel, message: impl Into<String>) {
    bus.emit(NodeEvent::OperationalNotice {
        notice: OperationalNotice {
            level,
            message: message.into(),
            at_ms: now_ms(),
        },
    });
}

fn send_outcome_to_udl(outcome: RnsSendOutcome) -> SendOutcome {
    match outcome {
        RnsSendOutcome::SentDirect => SendOutcome::SentDirect {},
        RnsSendOutcome::SentBroadcast => SendOutcome::SentBroadcast {},
        RnsSendOutcome::DroppedMissingDestinationIdentity => {
            SendOutcome::DroppedMissingDestinationIdentity {}
        }
        RnsSendOutcome::DroppedCiphertextTooLarge => SendOutcome::DroppedCiphertextTooLarge {},
        RnsSendOutcome::DroppedEncryptFailed => SendOutcome::DroppedEncryptFailed {},
        RnsSendOutcome::DroppedNoRoute => SendOutcome::DroppedNoRoute {},
    }
}

fn from_sdk_peer_state(state: sdkmsg::PeerState) -> PeerState {
    match state {
        sdkmsg::PeerState::Connecting => PeerState::Connecting {},
        sdkmsg::PeerState::Connected => PeerState::Connected {},
        sdkmsg::PeerState::Disconnected => PeerState::Disconnected {},
    }
}

fn to_sdk_message_method(method: MessageMethod) -> sdkmsg::MessageMethod {
    match method {
        MessageMethod::Direct {} => sdkmsg::MessageMethod::Direct,
        MessageMethod::Opportunistic {} => sdkmsg::MessageMethod::Opportunistic,
        MessageMethod::Propagated {} => sdkmsg::MessageMethod::Propagated,
        MessageMethod::Resource {} => sdkmsg::MessageMethod::Resource,
    }
}

fn from_sdk_message_method(method: sdkmsg::MessageMethod) -> MessageMethod {
    match method {
        sdkmsg::MessageMethod::Direct => MessageMethod::Direct {},
        sdkmsg::MessageMethod::Opportunistic => MessageMethod::Opportunistic {},
        sdkmsg::MessageMethod::Propagated => MessageMethod::Propagated {},
        sdkmsg::MessageMethod::Resource => MessageMethod::Resource {},
    }
}

fn to_sdk_message_state(state: MessageState) -> sdkmsg::MessageState {
    match state {
        MessageState::Queued {} => sdkmsg::MessageState::Queued,
        MessageState::PathRequested {} => sdkmsg::MessageState::PathRequested,
        MessageState::LinkEstablishing {} => sdkmsg::MessageState::LinkEstablishing,
        MessageState::Sending {} => sdkmsg::MessageState::Sending,
        MessageState::SentDirect {} => sdkmsg::MessageState::SentDirect,
        MessageState::SentToPropagation {} => sdkmsg::MessageState::SentToPropagation,
        MessageState::Delivered {} => sdkmsg::MessageState::Delivered,
        MessageState::Failed {} => sdkmsg::MessageState::Failed,
        MessageState::TimedOut {} => sdkmsg::MessageState::TimedOut,
        MessageState::Cancelled {} => sdkmsg::MessageState::Cancelled,
        MessageState::Received {} => sdkmsg::MessageState::Received,
    }
}

fn to_sdk_transport_delivery_state(
    state: TransportDeliveryState,
) -> sdkmsg::TransportDeliveryState {
    match state {
        TransportDeliveryState::Queued {} => sdkmsg::TransportDeliveryState::Queued,
        TransportDeliveryState::Sending {} => sdkmsg::TransportDeliveryState::Sending,
        TransportDeliveryState::SentDirect {} => sdkmsg::TransportDeliveryState::SentDirect,
        TransportDeliveryState::SentToPropagation {} => {
            sdkmsg::TransportDeliveryState::SentToPropagation
        }
        TransportDeliveryState::TransportDelivered {} => {
            sdkmsg::TransportDeliveryState::TransportDelivered
        }
        TransportDeliveryState::Failed {} => sdkmsg::TransportDeliveryState::Failed,
        TransportDeliveryState::TimedOut {} => sdkmsg::TransportDeliveryState::TimedOut,
        TransportDeliveryState::Cancelled {} => sdkmsg::TransportDeliveryState::Cancelled,
    }
}

fn from_sdk_transport_delivery_state(
    state: sdkmsg::TransportDeliveryState,
) -> TransportDeliveryState {
    match state {
        sdkmsg::TransportDeliveryState::Queued => TransportDeliveryState::Queued {},
        sdkmsg::TransportDeliveryState::Sending => TransportDeliveryState::Sending {},
        sdkmsg::TransportDeliveryState::SentDirect => TransportDeliveryState::SentDirect {},
        sdkmsg::TransportDeliveryState::SentToPropagation => {
            TransportDeliveryState::SentToPropagation {}
        }
        sdkmsg::TransportDeliveryState::TransportDelivered => {
            TransportDeliveryState::TransportDelivered {}
        }
        sdkmsg::TransportDeliveryState::Failed => TransportDeliveryState::Failed {},
        sdkmsg::TransportDeliveryState::TimedOut => TransportDeliveryState::TimedOut {},
        sdkmsg::TransportDeliveryState::Cancelled => TransportDeliveryState::Cancelled {},
    }
}

fn to_sdk_application_ack_state(state: ApplicationAckState) -> sdkmsg::ApplicationAckState {
    match state {
        ApplicationAckState::NotRequired {} => sdkmsg::ApplicationAckState::NotRequired,
        ApplicationAckState::Waiting {} => sdkmsg::ApplicationAckState::Waiting,
        ApplicationAckState::Accepted {} => sdkmsg::ApplicationAckState::Accepted,
        ApplicationAckState::Completed {} => sdkmsg::ApplicationAckState::Completed,
        ApplicationAckState::Rejected {} => sdkmsg::ApplicationAckState::Rejected,
        ApplicationAckState::Failed {} => sdkmsg::ApplicationAckState::Failed,
    }
}

fn from_sdk_application_ack_state(state: sdkmsg::ApplicationAckState) -> ApplicationAckState {
    match state {
        sdkmsg::ApplicationAckState::NotRequired => ApplicationAckState::NotRequired {},
        sdkmsg::ApplicationAckState::Waiting => ApplicationAckState::Waiting {},
        sdkmsg::ApplicationAckState::Accepted => ApplicationAckState::Accepted {},
        sdkmsg::ApplicationAckState::Completed => ApplicationAckState::Completed {},
        sdkmsg::ApplicationAckState::Rejected => ApplicationAckState::Rejected {},
        sdkmsg::ApplicationAckState::Failed => ApplicationAckState::Failed {},
    }
}

fn from_sdk_message_state(state: sdkmsg::MessageState) -> MessageState {
    match state {
        sdkmsg::MessageState::Queued => MessageState::Queued {},
        sdkmsg::MessageState::PathRequested => MessageState::PathRequested {},
        sdkmsg::MessageState::LinkEstablishing => MessageState::LinkEstablishing {},
        sdkmsg::MessageState::Sending => MessageState::Sending {},
        sdkmsg::MessageState::SentDirect => MessageState::SentDirect {},
        sdkmsg::MessageState::SentToPropagation => MessageState::SentToPropagation {},
        sdkmsg::MessageState::Delivered => MessageState::Delivered {},
        sdkmsg::MessageState::Failed => MessageState::Failed {},
        sdkmsg::MessageState::TimedOut => MessageState::TimedOut {},
        sdkmsg::MessageState::Cancelled => MessageState::Cancelled {},
        sdkmsg::MessageState::Received => MessageState::Received {},
    }
}

fn to_sdk_send_mode(mode: SendMode) -> sdkmsg::SendMode {
    match mode {
        SendMode::Auto {} => sdkmsg::SendMode::Auto,
        SendMode::DirectOnly {} => sdkmsg::SendMode::DirectOnly,
        SendMode::PropagationOnly {} => sdkmsg::SendMode::PropagationOnly,
    }
}

fn to_sdk_message_direction(direction: MessageDirection) -> sdkmsg::MessageDirection {
    match direction {
        MessageDirection::Inbound {} => sdkmsg::MessageDirection::Inbound,
        MessageDirection::Outbound {} => sdkmsg::MessageDirection::Outbound,
    }
}

fn from_sdk_message_direction(direction: sdkmsg::MessageDirection) -> MessageDirection {
    match direction {
        sdkmsg::MessageDirection::Inbound => MessageDirection::Inbound {},
        sdkmsg::MessageDirection::Outbound => MessageDirection::Outbound {},
    }
}

fn from_sdk_sync_phase(phase: sdkmsg::SyncPhase) -> SyncPhase {
    match phase {
        sdkmsg::SyncPhase::Idle => SyncPhase::Idle {},
        sdkmsg::SyncPhase::PathRequested => SyncPhase::PathRequested {},
        sdkmsg::SyncPhase::LinkEstablishing => SyncPhase::LinkEstablishing {},
        sdkmsg::SyncPhase::RequestSent => SyncPhase::RequestSent {},
        sdkmsg::SyncPhase::Receiving => SyncPhase::Receiving {},
        sdkmsg::SyncPhase::Complete => SyncPhase::Complete {},
        sdkmsg::SyncPhase::Failed => SyncPhase::Failed {},
    }
}

fn to_sdk_announce_record(record: AnnounceRecord) -> sdkmsg::AnnounceRecord {
    sdkmsg::AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        app_data: record.app_data,
        display_name: record.display_name,
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn from_sdk_announce_record(record: sdkmsg::AnnounceRecord) -> AnnounceRecord {
    let parsed_display_name = parse_announce_metadata(&record.app_data).display_name;
    let announce_class = classify_announce(&record.destination_kind, &record.app_data);
    AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        announce_class,
        app_data: record.app_data,
        display_name: record.display_name.or(parsed_display_name),
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn normalize_announce_app_data(app_data: &[u8]) -> String {
    String::from_utf8(app_data.to_vec()).unwrap_or_else(|_| hex::encode(app_data))
}

fn lxmf_sdk_announce_record_from_raw(
    destination_hex: impl Into<String>,
    identity_hex: impl Into<String>,
    destination_kind: impl Into<String>,
    app_data: &[u8],
    hops: u8,
    interface_hex: impl Into<String>,
    received_at_ms: u64,
) -> LxmfSdkAnnounceRecord {
    let destination_kind = destination_kind.into();
    let display_name = if destination_kind == DESTINATION_KIND_LXMF_DELIVERY {
        display_name_from_delivery_app_data(app_data).into_display_name_option()
    } else {
        None
    };
    LxmfSdkAnnounceRecord {
        destination_hex: destination_hex.into(),
        identity_hex: identity_hex.into(),
        destination_kind,
        app_data: normalize_announce_app_data(app_data),
        display_name,
        hops,
        interface_hex: interface_hex.into(),
        received_at_ms,
    }
}

trait IntoDisplayNameOption {
    fn into_display_name_option(self) -> Option<String>;
}

impl IntoDisplayNameOption for Option<String> {
    fn into_display_name_option(self) -> Option<String> {
        self
    }
}

impl<E> IntoDisplayNameOption for Result<Option<String>, E> {
    fn into_display_name_option(self) -> Option<String> {
        self.unwrap_or(None)
    }
}

fn to_compat_announce_record(record: &LxmfSdkAnnounceRecord) -> sdkmsg::AnnounceRecord {
    sdkmsg::AnnounceRecord {
        destination_hex: record.destination_hex.clone(),
        identity_hex: record.identity_hex.clone(),
        destination_kind: record.destination_kind.clone(),
        app_data: record.app_data.clone(),
        display_name: record.display_name.clone(),
        hops: record.hops,
        interface_hex: record.interface_hex.clone(),
        received_at_ms: record.received_at_ms,
    }
}

fn from_lxmf_sdk_announce_record(record: LxmfSdkAnnounceRecord) -> AnnounceRecord {
    let parsed_display_name = parse_announce_metadata(&record.app_data).display_name;
    let announce_class = classify_announce(&record.destination_kind, &record.app_data);
    AnnounceRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        destination_kind: record.destination_kind,
        announce_class,
        app_data: record.app_data,
        display_name: record.display_name.or(parsed_display_name),
        hops: record.hops,
        interface_hex: record.interface_hex,
        received_at_ms: record.received_at_ms,
    }
}

fn to_sdk_message_record(record: MessageRecord) -> sdkmsg::MessageRecord {
    sdkmsg::MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: to_sdk_message_direction(record.direction),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        requested_destination_hex: record.requested_destination_hex,
        delivery_destination_hex: record.delivery_destination_hex,
        recipient_identity_hex: record.recipient_identity_hex,
        last_wire_message_id_hex: record.last_wire_message_id_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: to_sdk_message_method(record.method),
        state: to_sdk_message_state(record.state),
        transport_state: to_sdk_transport_delivery_state(record.transport_state),
        application_ack_state: to_sdk_application_ack_state(record.application_ack_state),
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn from_sdk_message_record(record: sdkmsg::MessageRecord) -> MessageRecord {
    MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: from_sdk_message_direction(record.direction),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        requested_destination_hex: record.requested_destination_hex,
        delivery_destination_hex: record.delivery_destination_hex,
        recipient_identity_hex: record.recipient_identity_hex,
        last_wire_message_id_hex: record.last_wire_message_id_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: from_sdk_message_method(record.method),
        state: from_sdk_message_state(record.state),
        transport_state: from_sdk_transport_delivery_state(record.transport_state),
        application_ack_state: from_sdk_application_ack_state(record.application_ack_state),
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn from_sdk_peer_record(record: sdkmsg::PeerRecord) -> PeerRecord {
    PeerRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: from_sdk_peer_state(record.state),
        saved: record.saved,
        stale: record.stale,
        active_link: record.active_link,
        hub_derived: false,
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}

fn from_sdk_peer_change(change: sdkmsg::PeerChange) -> PeerChange {
    PeerChange {
        destination_hex: change.destination_hex,
        identity_hex: change.identity_hex,
        lxmf_destination_hex: change.lxmf_destination_hex,
        display_name: change.display_name,
        app_data: change.app_data,
        state: from_sdk_peer_state(change.state),
        saved: change.saved,
        stale: change.stale,
        active_link: change.active_link,
        last_error: change.last_error,
        last_resolution_error: change.last_resolution_error,
        last_resolution_attempt_at_ms: change.last_resolution_attempt_at_ms,
        last_seen_at_ms: change.last_seen_at_ms,
        announce_last_seen_at_ms: change.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: change.lxmf_last_seen_at_ms,
    }
}

fn from_sdk_conversation_record(record: sdkmsg::ConversationRecord) -> ConversationRecord {
    ConversationRecord {
        conversation_id: record.conversation_id,
        peer_destination_hex: record.peer_destination_hex,
        peer_display_name: record.peer_display_name,
        last_message_preview: record.last_message_preview,
        last_message_at_ms: record.last_message_at_ms,
        unread_count: record.unread_count,
        last_message_state: record.last_message_state.map(from_sdk_message_state),
    }
}

fn from_sdk_sync_status(status: sdkmsg::SyncStatus) -> SyncStatus {
    SyncStatus {
        phase: from_sdk_sync_phase(status.phase),
        active_propagation_node_hex: status.active_propagation_node_hex,
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail,
    }
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn conversation_peer_resolver(peers: &[PeerRecord]) -> ConversationPeerResolver {
    let mut resolver = ConversationPeerResolver::default();
    for peer in peers {
        let destination_hex = match trimmed_non_empty(Some(peer.destination_hex.as_str())) {
            Some(value) => value,
            None => continue,
        };
        let lxmf_destination_hex = trimmed_non_empty(peer.lxmf_destination_hex.as_deref());
        let identity_hex = trimmed_non_empty(peer.identity_hex.as_deref());
        let canonical_id = identity_hex
            .clone()
            .or_else(|| lxmf_destination_hex.clone())
            .unwrap_or_else(|| destination_hex.clone());
        let peer_destination_hex = lxmf_destination_hex
            .clone()
            .unwrap_or_else(|| destination_hex.clone());
        let mut aliases = vec![destination_hex];
        if let Some(lxmf_destination_hex) = lxmf_destination_hex {
            aliases.push(lxmf_destination_hex);
        }
        if let Some(identity_hex) = identity_hex {
            aliases.push(identity_hex);
        }
        resolver.insert(
            aliases,
            canonical_id,
            peer_destination_hex,
            peer.display_name.clone(),
        );
    }
    resolver
}

fn conversation_delete_keys(conversation_id: &str, peers: &[PeerRecord]) -> Vec<String> {
    let normalized_conversation_id = conversation_id.trim().to_ascii_lowercase();
    if normalized_conversation_id.is_empty() {
        return Vec::new();
    }

    let mut keys = HashSet::from([normalized_conversation_id.clone()]);
    for peer in peers {
        let aliases = [
            trimmed_non_empty(Some(peer.destination_hex.as_str())),
            trimmed_non_empty(peer.lxmf_destination_hex.as_deref()),
            trimmed_non_empty(peer.identity_hex.as_deref()),
        ];
        let matches_peer = aliases.iter().flatten().any(|alias| {
            alias
                .trim()
                .eq_ignore_ascii_case(normalized_conversation_id.as_str())
        });
        if matches_peer {
            for alias in aliases.into_iter().flatten() {
                keys.insert(alias.trim().to_ascii_lowercase());
            }
        }
    }
    let mut out = keys.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn to_sdk_sync_status(status: SyncStatus) -> Option<sdkmsg::SyncStatus> {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn to_sdk_send_request(request: &SendLxmfRequest) -> sdkmsg::SendMessageRequest {
    sdkmsg::SendMessageRequest {
        destination_hex: request.destination_hex.clone(),
        body_utf8: request.body_utf8.clone(),
        title: request.title.clone(),
        send_mode: to_sdk_send_mode(request.send_mode),
        use_propagation_node: matches!(request.send_mode, SendMode::PropagationOnly {}),
    }
}
