fn parse_hub_directory_peer_record(value: &MsgPackValue) -> Option<HubDirectoryPeerRecord> {
    let entries = msgpack_map_entries(value)?;
    Some(HubDirectoryPeerRecord {
        identity: msgpack_get_named(entries, &["identity"]).and_then(msgpack_string)?,
        destination_hash: msgpack_get_named(entries, &["destination_hash"])
            .and_then(msgpack_string)?,
        display_name: msgpack_get_named(entries, &["display_name"]).and_then(msgpack_string),
        announce_capabilities: msgpack_get_named(entries, &["announce_capabilities"])
            .and_then(msgpack_string_vec)
            .unwrap_or_default(),
        client_type: msgpack_get_named(entries, &["client_type"]).and_then(msgpack_string),
        registered_mode: msgpack_get_named(entries, &["registered_mode"]).and_then(msgpack_string),
        last_seen: msgpack_get_named(entries, &["last_seen"]).and_then(msgpack_string),
        status: msgpack_get_named(entries, &["status"]).and_then(msgpack_string),
    })
}

fn parse_hub_directory_snapshot_value(
    value: &MsgPackValue,
    received_at_ms: u64,
) -> Option<HubDirectorySnapshot> {
    let entries = msgpack_map_entries(value)?;
    let effective_connected_mode = msgpack_get_named(entries, &["effective_connected_mode"])
        .and_then(msgpack_bool)
        .unwrap_or(false);
    let items = match msgpack_get_named(entries, &["items"]) {
        Some(MsgPackValue::Array(items)) => items
            .iter()
            .filter_map(parse_hub_directory_peer_record)
            .collect(),
        _ => Vec::new(),
    };
    Some(HubDirectorySnapshot {
        effective_connected_mode,
        items,
        received_at_ms,
    })
}

enum HubDirectoryResultState {
    Accepted,
    Snapshot(HubDirectorySnapshot),
}

fn parse_hub_directory_result_state(
    value: &MsgPackValue,
    expected_command_id: &str,
    received_at_ms: u64,
) -> Option<HubDirectoryResultState> {
    let entries = msgpack_map_entries(value)?;
    let command_id = msgpack_get_named(entries, &["command_id"]).and_then(msgpack_string);
    if command_id
        .as_deref()
        .is_some_and(|value| value != expected_command_id)
    {
        return None;
    }

    let status = msgpack_get_named(entries, &["status"])
        .and_then(msgpack_string)
        .map(|value| value.to_ascii_lowercase());
    if status.as_deref() == Some("accepted") {
        return Some(HubDirectoryResultState::Accepted);
    }

    let payload = msgpack_get_named(entries, &["payload", "result", "data"]).unwrap_or(value);
    parse_hub_directory_snapshot_value(payload, received_at_ms)
        .map(HubDirectoryResultState::Snapshot)
}

async fn publish_hub_directory_snapshot(
    state: &NodeRuntimeState,
    bus: &EventBus,
    snapshot: HubDirectorySnapshot,
) {
    if let Ok(mut guard) = state.hub_directory_snapshot.lock() {
        *guard = Some(snapshot.clone());
    }
    let _ = refresh_peer_snapshot(state).await;
    state.sdk.record_hub_directory_updated(&snapshot);
    bus.emit(NodeEvent::HubDirectoryUpdated { snapshot });
}

async fn refresh_hub_directory_lxmf(
    config: &NodeConfig,
    state: &NodeRuntimeState,
) -> Result<HubDirectorySnapshot, NodeError> {
    let hub_hex = config
        .hub_identity_hash
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let hub_hex = normalize_hex_32(hub_hex).ok_or(NodeError::InvalidConfig {})?;
    let hub = parse_address_hash(&hub_hex)?;

    let hub_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    let hub_desc = ensure_destination_desc(state, hub, Some(hub_name)).await?;

    let link = {
        let mut links = state.out_links.lock().await;
        if let Some(existing) = links.get(&hub).cloned() {
            existing
        } else {
            let created = state.transport.link(hub_desc).await;
            links.insert(hub, created.clone());
            created
        }
    };

    wait_for_link_active(&state.transport, &link, DEFAULT_LINK_CONNECT_TIMEOUT).await?;

    let mut source = [0u8; 16];
    source.copy_from_slice(
        state
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .as_slice(),
    );
    let mut destination = [0u8; 16];
    destination.copy_from_slice(hub.as_slice());

    let command_id = format!("hub-directory-{}", now_ms());
    let fields = MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from(command_id.as_str()),
            ),
            (
                MsgPackValue::from("command_type"),
                MsgPackValue::from("rem.registry.peers.list"),
            ),
            (
                MsgPackValue::from("timestamp"),
                MsgPackValue::from(current_timestamp_rfc3339()),
            ),
            (
                MsgPackValue::from("source"),
                MsgPackValue::Map(vec![(
                    MsgPackValue::from("rns_identity"),
                    MsgPackValue::from(state.identity.address_hash().to_hex_string()),
                )]),
            ),
            (MsgPackValue::from("args"), MsgPackValue::Map(vec![])),
        ])]),
    )]);

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(destination);
    message.set_title_from_string("rem.registry.peers.list");
    message.fields = Some(fields);

    let signer = lxmf_private_identity(&state.identity)?;
    let wire = message
        .to_wire(Some(&signer))
        .map_err(|_| NodeError::InternalError {})?;

    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|_| NodeError::InternalError {})?;
    let outcome = state.transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(NodeError::NetworkError {});
    }

    let mut rx = state.transport.received_data_events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        let received = match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(event)) => event,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        };

        if received.destination != hub {
            continue;
        }

        let Ok(reply) = LxmfMessage::from_wire(received.data.as_slice()) else {
            continue;
        };

        let mut text = String::new();
        if !reply.title.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.title));
            text.push('\n');
        }
        if !reply.content.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.content));
            text.push('\n');
        }
        if let Some(fields) = &reply.fields {
            text.push_str(&format!("{fields:?}"));
        }

        if let Some(fields) = reply.fields.as_ref() {
            match parse_hub_directory_result_state(fields, &command_id, now_ms()) {
                Some(HubDirectoryResultState::Accepted) => continue,
                Some(HubDirectoryResultState::Snapshot(snapshot)) => return Ok(snapshot),
                None => {}
            }
        }
    }
}
