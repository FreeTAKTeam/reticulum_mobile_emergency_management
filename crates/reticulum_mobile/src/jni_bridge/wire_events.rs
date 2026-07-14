fn send_outcome_to_str(outcome: SendOutcome) -> &'static str {
    match outcome {
        SendOutcome::SentDirect {} => "SentDirect",
        SendOutcome::SentBroadcast {} => "SentBroadcast",
        SendOutcome::DroppedMissingDestinationIdentity {} => "DroppedMissingDestinationIdentity",
        SendOutcome::DroppedCiphertextTooLarge {} => "DroppedCiphertextTooLarge",
        SendOutcome::DroppedEncryptFailed {} => "DroppedEncryptFailed",
        SendOutcome::DroppedNoRoute {} => "DroppedNoRoute",
    }
}

fn lxmf_delivery_status_to_str(status: LxmfDeliveryStatus) -> &'static str {
    match status {
        LxmfDeliveryStatus::Sent {} => "Sent",
        LxmfDeliveryStatus::SentToPropagation {} => "SentToPropagation",
        LxmfDeliveryStatus::Acknowledged {} => "Acknowledged",
        LxmfDeliveryStatus::Failed {} => "Failed",
        LxmfDeliveryStatus::TimedOut {} => "TimedOut",
    }
}

fn send_mode_from_input(send_mode: Option<&str>, use_propagation_node: bool) -> SendMode {
    if use_propagation_node {
        return SendMode::PropagationOnly {};
    }
    match send_mode.unwrap_or("").trim() {
        "DirectOnly" => SendMode::DirectOnly {},
        "PropagationOnly" => SendMode::PropagationOnly {},
        _ => SendMode::Auto {},
    }
}

fn lxmf_delivery_method_to_str(method: LxmfDeliveryMethod) -> &'static str {
    match method {
        LxmfDeliveryMethod::Direct {} => "Direct",
        LxmfDeliveryMethod::Opportunistic {} => "Opportunistic",
        LxmfDeliveryMethod::Propagated {} => "Propagated",
    }
}

fn lxmf_delivery_representation_to_str(representation: LxmfDeliveryRepresentation) -> &'static str {
    match representation {
        LxmfDeliveryRepresentation::Packet {} => "Packet",
        LxmfDeliveryRepresentation::Resource {} => "Resource",
    }
}

fn lxmf_fallback_stage_to_str(stage: LxmfFallbackStage) -> &'static str {
    match stage {
        LxmfFallbackStage::AfterDirectRetryBudget {} => "AfterDirectRetryBudget",
    }
}

fn message_method_to_str(method: MessageMethod) -> &'static str {
    match method {
        MessageMethod::Direct {} => "Direct",
        MessageMethod::Opportunistic {} => "Opportunistic",
        MessageMethod::Propagated {} => "Propagated",
        MessageMethod::Resource {} => "Resource",
    }
}

fn message_state_to_str(state: MessageState) -> &'static str {
    match state {
        MessageState::Queued {} => "Queued",
        MessageState::PathRequested {} => "PathRequested",
        MessageState::LinkEstablishing {} => "LinkEstablishing",
        MessageState::Sending {} => "Sending",
        MessageState::SentDirect {} => "SentDirect",
        MessageState::SentToPropagation {} => "SentToPropagation",
        MessageState::Delivered {} => "Delivered",
        MessageState::Failed {} => "Failed",
        MessageState::TimedOut {} => "TimedOut",
        MessageState::Cancelled {} => "Cancelled",
        MessageState::Received {} => "Received",
    }
}

fn transport_delivery_state_to_str(state: TransportDeliveryState) -> &'static str {
    match state {
        TransportDeliveryState::Queued {} => "Queued",
        TransportDeliveryState::Sending {} => "Sending",
        TransportDeliveryState::SentDirect {} => "SentDirect",
        TransportDeliveryState::SentToPropagation {} => "SentToPropagation",
        TransportDeliveryState::TransportDelivered {} => "TransportDelivered",
        TransportDeliveryState::Failed {} => "Failed",
        TransportDeliveryState::TimedOut {} => "TimedOut",
        TransportDeliveryState::Cancelled {} => "Cancelled",
    }
}

fn application_ack_state_to_str(state: ApplicationAckState) -> &'static str {
    match state {
        ApplicationAckState::NotRequired {} => "NotRequired",
        ApplicationAckState::Waiting {} => "Waiting",
        ApplicationAckState::Accepted {} => "Accepted",
        ApplicationAckState::Completed {} => "Completed",
        ApplicationAckState::Rejected {} => "Rejected",
        ApplicationAckState::Failed {} => "Failed",
    }
}

fn message_direction_to_str(direction: MessageDirection) -> &'static str {
    match direction {
        MessageDirection::Inbound {} => "Inbound",
        MessageDirection::Outbound {} => "Outbound",
    }
}

fn sync_phase_to_str(phase: SyncPhase) -> &'static str {
    match phase {
        SyncPhase::Idle {} => "Idle",
        SyncPhase::PathRequested {} => "PathRequested",
        SyncPhase::LinkEstablishing {} => "LinkEstablishing",
        SyncPhase::RequestSent {} => "RequestSent",
        SyncPhase::Receiving {} => "Receiving",
        SyncPhase::Complete {} => "Complete",
        SyncPhase::Failed {} => "Failed",
    }
}

fn log_level_to_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace {} => "Trace",
        LogLevel::Debug {} => "Debug",
        LogLevel::Info {} => "Info",
        LogLevel::Warn {} => "Warn",
        LogLevel::Error {} => "Error",
    }
}

fn projection_scope_to_str(scope: ProjectionScope) -> &'static str {
    match scope {
        ProjectionScope::AppSettings {} => "AppSettings",
        ProjectionScope::SavedPeers {} => "SavedPeers",
        ProjectionScope::OperationalSummary {} => "OperationalSummary",
        ProjectionScope::Peers {} => "Peers",
        ProjectionScope::SyncStatus {} => "SyncStatus",
        ProjectionScope::HubRegistration {} => "HubRegistration",
        ProjectionScope::Checklists {} => "Checklists",
        ProjectionScope::ChecklistDetail {} => "ChecklistDetail",
        ProjectionScope::Eams {} => "Eams",
        ProjectionScope::Events {} => "Events",
        ProjectionScope::Conversations {} => "Conversations",
        ProjectionScope::Messages {} => "Messages",
        ProjectionScope::Telemetry {} => "Telemetry",
        ProjectionScope::Sos {} => "Sos",
        ProjectionScope::Plugins {} => "Plugins",
        ProjectionScope::PluginSensors {} => "PluginSensors",
    }
}

fn message_record_json(message: &MessageRecord) -> Value {
    json!({
        "messageIdHex": message.message_id_hex,
        "conversationId": message.conversation_id,
        "direction": message_direction_to_str(message.direction),
        "destinationHex": message.destination_hex,
        "sourceHex": message.source_hex,
        "requestedDestinationHex": message.requested_destination_hex,
        "deliveryDestinationHex": message.delivery_destination_hex,
        "recipientIdentityHex": message.recipient_identity_hex,
        "lastWireMessageIdHex": message.last_wire_message_id_hex,
        "title": message.title,
        "bodyUtf8": message.body_utf8,
        "method": message_method_to_str(message.method),
        "state": message_state_to_str(message.state),
        "transportState": transport_delivery_state_to_str(message.transport_state),
        "applicationAckState": application_ack_state_to_str(message.application_ack_state),
        "detail": message.detail,
        "sentAtMs": message.sent_at_ms,
        "receivedAtMs": message.received_at_ms,
        "updatedAtMs": message.updated_at_ms
    })
}

fn conversation_record_json(conversation: &ConversationRecord) -> Value {
    json!({
        "conversationId": conversation.conversation_id,
        "peerDestinationHex": conversation.peer_destination_hex,
        "peerDisplayName": conversation.peer_display_name,
        "lastMessagePreview": conversation.last_message_preview,
        "lastMessageAtMs": conversation.last_message_at_ms,
        "unreadCount": conversation.unread_count,
        "lastMessageState": conversation.last_message_state.map(message_state_to_str)
    })
}

fn event_to_wire_json(event: NodeEvent) -> String {
    let (event_name, payload) = match event {
        NodeEvent::StatusChanged { status } => (
            "statusChanged",
            json!({
                "status": {
                    "running": status.running,
                    "name": status.name,
                    "identityHex": status.identity_hex,
                    "appDestinationHex": status.app_destination_hex,
                    "lxmfDestinationHex": status.lxmf_destination_hex,
                    "readiness": runtime_readiness_json(status.readiness),
                    "interfaces": status.interfaces.into_iter().map(interface_status_json).collect::<Vec<_>>()
                }
            }),
        ),
        NodeEvent::InterfaceStatusChanged { status } => (
            "interfaceStatusChanged",
            json!({
                "status": interface_status_json(status)
            }),
        ),
        NodeEvent::AnnounceReceived {
            destination_hex,
            identity_hex,
            destination_kind,
            announce_class,
            app_data,
            display_name,
            hops,
            interface_hex,
            received_at_ms,
        } => (
            "announceReceived",
            json!({
                "destinationHex": destination_hex,
                "identityHex": identity_hex,
                "destinationKind": destination_kind,
                "announceClass": announce_class_to_str(announce_class),
                "appData": app_data,
                "displayName": display_name,
                "hops": hops,
                "interfaceHex": interface_hex,
                "receivedAtMs": received_at_ms
            }),
        ),
        NodeEvent::PeerChanged { change } => (
            "peerChanged",
            json!({
                "change": peer_change_json(&change)
            }),
        ),
        NodeEvent::PacketReceived {
            destination_hex,
            source_hex,
            bytes,
            fields_bytes,
        } => (
            "packetReceived",
            json!({
                "destinationHex": destination_hex,
                "sourceHex": source_hex,
                "bytesBase64": BASE64_STANDARD.encode(bytes),
                "fieldsBase64": fields_bytes.map(|bytes| BASE64_STANDARD.encode(bytes))
            }),
        ),
        NodeEvent::PacketSent {
            destination_hex,
            bytes,
            outcome,
        } => (
            "packetSent",
            json!({
                "destinationHex": destination_hex,
                "bytesBase64": BASE64_STANDARD.encode(bytes),
                "outcome": send_outcome_to_str(outcome)
            }),
        ),
        NodeEvent::LxmfDelivery { update } => (
            "lxmfDelivery",
            json!({
                "messageIdHex": update.message_id_hex,
                "destinationHex": update.destination_hex,
                "sourceHex": update.source_hex,
                "correlationId": update.correlation_id,
                "commandId": update.command_id,
                "commandType": update.command_type,
                "eventUid": update.event_uid,
                "missionUid": update.mission_uid,
                "status": lxmf_delivery_status_to_str(update.status),
                "transportState": transport_delivery_state_to_str(update.transport_state),
                "applicationAckState": application_ack_state_to_str(update.application_ack_state),
                "method": lxmf_delivery_method_to_str(update.method),
                "representation": lxmf_delivery_representation_to_str(update.representation),
                "relayDestinationHex": update.relay_destination_hex,
                "fallbackStage": update.fallback_stage.map(lxmf_fallback_stage_to_str),
                "detail": update.detail,
                "sentAtMs": update.sent_at_ms,
                "updatedAtMs": update.updated_at_ms
            }),
        ),
        NodeEvent::PeerResolved { peer } => ("peerResolved", peer_record_json(&peer)),
        NodeEvent::MessageReceived { message } => {
            ("messageReceived", message_record_json(&message))
        }
        NodeEvent::MessageUpdated { message } => ("messageUpdated", message_record_json(&message)),
        NodeEvent::SyncUpdated { status } => (
            "syncUpdated",
            json!({
                "phase": sync_phase_to_str(status.phase),
                "activePropagationNodeHex": status.active_propagation_node_hex,
                "requestedAtMs": status.requested_at_ms,
                "completedAtMs": status.completed_at_ms,
                "messagesReceived": status.messages_received,
                "detail": status.detail
            }),
        ),
        NodeEvent::HubDirectoryUpdated { snapshot } => (
            "hubDirectoryUpdated",
            hub_directory_snapshot_json(&snapshot),
        ),
        NodeEvent::OperationalNotice { notice } => {
            ("operationalNotice", operational_notice_json(&notice))
        }
        NodeEvent::ProjectionInvalidated { invalidation } => (
            "projectionInvalidated",
            json!({
                "scope": projection_scope_to_str(invalidation.scope),
                "key": invalidation.key,
                "revision": invalidation.revision,
                "updatedAtMs": invalidation.updated_at_ms,
                "reason": invalidation.reason
            }),
        ),
        NodeEvent::PluginEventPublished { event } => (
            "pluginEventPublished",
            json!({
                "pluginId": event.plugin_id,
                "event": serde_json::from_str::<Value>(event.event_json.as_str()).unwrap_or(Value::Null)
            }),
        ),
        NodeEvent::SosStatusChanged { status } => ("sosStatusChanged", sos_status_json(&status)),
        NodeEvent::SosAlertChanged { alert } => ("sosAlertChanged", sos_alert_json(&alert)),
        NodeEvent::SosTelemetryRequested {} => ("sosTelemetryRequested", json!({})),
        NodeEvent::SosAudioRecordingRequested {
            incident_id,
            duration_seconds,
        } => (
            "sosAudioRecordingRequested",
            json!({
                "incidentId": incident_id,
                "durationSeconds": duration_seconds
            }),
        ),
        NodeEvent::Log { level, message } => (
            "log",
            json!({
                "level": log_level_to_str(level),
                "message": message
            }),
        ),
        NodeEvent::Error { code, message } => (
            "error",
            json!({
                "code": code,
                "message": message
            }),
        ),
    };

    json!({
        "event": event_name,
        "payload": payload
    })
    .to_string()
}

fn ok_result() -> jint {
    clear_last_error();
    RESULT_OK
}

fn err_result(code: impl Into<String>, message: impl Into<String>) -> jint {
    set_last_error(code, message);
    RESULT_ERR
}

fn ok_json_result<T: Serialize>(env: &mut JNIEnv, value: &T) -> jstring {
    clear_last_error();
    match serde_json::to_string(value) {
        Ok(payload) => make_jstring_or_null(env, payload),
        Err(e) => {
            set_last_error("InternalError", format!("JSON serialization failed: {e}"));
            ptr::null_mut()
        }
    }
}
