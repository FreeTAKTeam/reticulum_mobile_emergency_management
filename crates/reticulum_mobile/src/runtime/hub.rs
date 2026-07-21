const HUB_TEAM_DIRECTORY_COMMAND: &str = "rem.registry.team_peers.list";
const HUB_TEAM_DIRECTORY_SCOPE: &str = "shared_teams";

fn canonical_team_color(team_uid: &str) -> Option<&'static str> {
    canonical_team_color_for_uid(team_uid)
}

fn yellow_team_record() -> HubTeamRecord {
    HubTeamRecord {
        uid: YELLOW_TEAM_UID.to_string(),
        color: "YELLOW".to_string(),
        team_name: "YELLOW".to_string(),
    }
}

fn parse_hub_directory_peer_record(
    value: &MsgPackValue,
) -> Result<HubDirectoryPeerRecord, NodeError> {
    let entries = msgpack_map_entries(value).ok_or(NodeError::InternalError {})?;
    let identity = msgpack_get_named(entries, &["identity"])
        .and_then(msgpack_string)
        .and_then(|value| normalize_hex_32(&value))
        .ok_or(NodeError::InternalError {})?;
    let destination_hash = msgpack_get_named(entries, &["destination_hash"])
        .and_then(msgpack_string)
        .and_then(|value| normalize_hex_32(&value))
        .ok_or(NodeError::InternalError {})?;
    let announce_capabilities = msgpack_get_named(entries, &["announce_capabilities"])
        .and_then(msgpack_string_vec)
        .ok_or(NodeError::InternalError {})?
        .into_iter()
        .map(|capability| capability.trim().to_ascii_lowercase())
        .filter(|capability| !capability.is_empty())
        .collect::<Vec<_>>();
    if !announce_capabilities
        .iter()
        .any(|capability| capability == "r3akt")
        || !announce_capabilities
            .iter()
            .any(|capability| capability == "emergencymessages")
    {
        return Err(NodeError::InternalError {});
    }
    let client_type = msgpack_get_named(entries, &["client_type"])
        .and_then(msgpack_string)
        .filter(|value| value.eq_ignore_ascii_case("rem"))
        .ok_or(NodeError::InternalError {})?;
    let status = msgpack_get_named(entries, &["status"])
        .and_then(msgpack_string)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    Ok(HubDirectoryPeerRecord {
        identity,
        destination_hash,
        display_name: msgpack_get_named(entries, &["display_name"]).and_then(msgpack_string),
        announce_capabilities,
        client_type: Some(client_type.to_ascii_lowercase()),
        registered_mode: msgpack_get_named(entries, &["registered_mode"]).and_then(msgpack_string),
        last_seen: msgpack_get_named(entries, &["last_seen"]).and_then(msgpack_string),
        status,
    })
}

fn parse_hub_team_record(value: &MsgPackValue) -> Result<Option<HubTeamRecord>, NodeError> {
    let entries = msgpack_map_entries(value).ok_or(NodeError::InternalError {})?;
    let uid = msgpack_get_named(entries, &["uid", "team_uid"])
        .and_then(msgpack_string)
        .ok_or(NodeError::InternalError {})?;
    let Some(canonical_color) = canonical_team_color(uid.trim()) else {
        return Ok(None);
    };
    let supplied_color = msgpack_get_named(entries, &["color"])
        .and_then(msgpack_string)
        .unwrap_or_else(|| canonical_color.to_string());
    if supplied_color.trim().to_ascii_uppercase() != canonical_color {
        return Err(NodeError::InternalError {});
    }
    let team_name = msgpack_get_named(entries, &["team_name", "name"])
        .and_then(msgpack_string)
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| canonical_color.to_string());
    Ok(Some(HubTeamRecord {
        uid,
        color: canonical_color.to_string(),
        team_name,
    }))
}

fn parse_hub_caller_membership_record(
    value: &MsgPackValue,
) -> Result<Option<HubCallerMembershipRecord>, NodeError> {
    let entries = msgpack_map_entries(value).ok_or(NodeError::InternalError {})?;
    let team_uid = msgpack_get_named(entries, &["team_uid"])
        .and_then(msgpack_string)
        .ok_or(NodeError::InternalError {})?;
    if canonical_team_color(team_uid.trim()).is_none() {
        return Ok(None);
    }
    let team_member_uid = msgpack_get_named(entries, &["team_member_uid"])
        .and_then(msgpack_string)
        .map(|uid| uid.trim().to_string())
        .filter(|uid| !uid.is_empty())
        .ok_or(NodeError::InternalError {})?;
    Ok(Some(HubCallerMembershipRecord {
        team_uid,
        team_member_uid,
    }))
}

fn parse_hub_team_member_record(
    value: &MsgPackValue,
) -> Result<Option<HubTeamMemberRecord>, NodeError> {
    let entries = msgpack_map_entries(value).ok_or(NodeError::InternalError {})?;
    let team_uid = msgpack_get_named(entries, &["team_uid"])
        .and_then(msgpack_string)
        .ok_or(NodeError::InternalError {})?;
    if canonical_team_color(team_uid.trim()).is_none() {
        return Ok(None);
    }
    let team_member_uid = msgpack_get_named(entries, &["team_member_uid"])
        .and_then(msgpack_string)
        .map(|uid| uid.trim().to_string())
        .filter(|uid| !uid.is_empty())
        .ok_or(NodeError::InternalError {})?;
    let peer = parse_hub_directory_peer_record(value)?;
    Ok(Some(HubTeamMemberRecord {
        team_uid,
        team_member_uid,
        identity: peer.identity,
        destination_hash: peer.destination_hash,
        display_name: peer.display_name,
        announce_capabilities: peer.announce_capabilities,
        client_type: peer.client_type,
        registered_mode: peer.registered_mode,
        last_seen: peer.last_seen,
        status: peer.status,
    }))
}

fn parse_optional_array<T>(
    entries: &[(MsgPackValue, MsgPackValue)],
    key: &str,
    parser: impl Fn(&MsgPackValue) -> Result<Option<T>, NodeError>,
) -> Result<Vec<T>, NodeError> {
    let Some(value) = msgpack_get_named(entries, &[key]) else {
        return Ok(Vec::new());
    };
    let MsgPackValue::Array(values) = value else {
        return Err(NodeError::InternalError {});
    };
    values
        .iter()
        .filter_map(|value| parser(value).transpose())
        .collect()
}

fn parse_hub_directory_snapshot_value(
    value: &MsgPackValue,
    received_at_ms: u64,
) -> Result<HubDirectorySnapshot, NodeError> {
    let entries = msgpack_map_entries(value).ok_or(NodeError::InternalError {})?;
    msgpack_get_named(entries, &["scope"])
        .and_then(msgpack_string)
        .filter(|value| value == HUB_TEAM_DIRECTORY_SCOPE)
        .ok_or(NodeError::InternalError {})?;
    let effective_connected_mode = msgpack_get_named(entries, &["effective_connected_mode"])
        .and_then(msgpack_bool)
        .ok_or(NodeError::InternalError {})?;
    let items = match msgpack_get_named(entries, &["items"]) {
        Some(MsgPackValue::Array(items)) => items
            .iter()
            .map(parse_hub_directory_peer_record)
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(NodeError::InternalError {}),
    };
    let schema_version = msgpack_get_named(entries, &["schema_version"])
        .and_then(MsgPackValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let mut teams = parse_optional_array(entries, "teams", parse_hub_team_record)?;
    if !teams.iter().any(|team| team.uid == YELLOW_TEAM_UID) {
        teams.push(yellow_team_record());
    }
    teams.sort_by_key(|team| (team.uid != YELLOW_TEAM_UID, team.color.clone()));
    let caller_memberships = parse_optional_array(
        entries,
        "caller_memberships",
        parse_hub_caller_membership_record,
    )?;
    let mut members = parse_optional_array(entries, "members", parse_hub_team_member_record)?;
    if schema_version < HUB_DIRECTORY_SCHEMA_VERSION {
        members = items
            .iter()
            .map(|peer| HubTeamMemberRecord {
                team_uid: YELLOW_TEAM_UID.to_string(),
                team_member_uid: peer.identity.clone(),
                identity: peer.identity.clone(),
                destination_hash: peer.destination_hash.clone(),
                display_name: peer.display_name.clone(),
                announce_capabilities: peer.announce_capabilities.clone(),
                client_type: peer.client_type.clone(),
                registered_mode: peer.registered_mode.clone(),
                last_seen: peer.last_seen.clone(),
                status: peer.status.clone(),
            })
            .collect();
    }
    Ok(HubDirectorySnapshot {
        schema_version,
        hub_identity_hash: None,
        active_team_uid: YELLOW_TEAM_UID.to_string(),
        effective_connected_mode,
        teams,
        caller_memberships,
        members,
        local_teams: Vec::new(),
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
) -> Result<Option<HubDirectoryResultState>, NodeError> {
    let outer_entries = match msgpack_map_entries(value) {
        Some(entries) => entries,
        None => return Ok(None),
    };
    let result_value = msgpack_get_indexed(outer_entries, FIELD_RESULTS).unwrap_or(value);
    let entries = match msgpack_map_entries(result_value) {
        Some(entries) => entries,
        None => return Ok(None),
    };
    let command_id = msgpack_get_named(entries, &["command_id"]).and_then(msgpack_string);
    if command_id.as_deref() != Some(expected_command_id) {
        return Ok(None);
    }

    let status = msgpack_get_named(entries, &["status"])
        .and_then(msgpack_string)
        .map(|value| value.to_ascii_lowercase())
        .ok_or(NodeError::InternalError {})?;
    if status == "accepted" {
        return Ok(Some(HubDirectoryResultState::Accepted));
    }
    if status != "result" && status != "completed" {
        return Err(NodeError::NetworkError {});
    }

    let payload = msgpack_get_named(entries, &["payload", "result", "data"])
        .ok_or(NodeError::InternalError {})?;
    parse_hub_directory_snapshot_value(payload, received_at_ms)
        .map(HubDirectoryResultState::Snapshot)
        .map(Some)
}

fn hub_team_directory_command_fields(command_id: &str, source_identity: &str) -> MsgPackValue {
    MsgPackValue::Map(vec![(
        MsgPackValue::from(FIELD_COMMANDS),
        MsgPackValue::Array(vec![MsgPackValue::Map(vec![
            (
                MsgPackValue::from("command_id"),
                MsgPackValue::from(command_id),
            ),
            (
                MsgPackValue::from("command_type"),
                MsgPackValue::from(HUB_TEAM_DIRECTORY_COMMAND),
            ),
            (
                MsgPackValue::from("timestamp"),
                MsgPackValue::from(current_timestamp_rfc3339()),
            ),
            (
                MsgPackValue::from("source"),
                MsgPackValue::Map(vec![(
                    MsgPackValue::from("rns_identity"),
                    MsgPackValue::from(source_identity),
                )]),
            ),
            (MsgPackValue::from("args"), MsgPackValue::Map(vec![])),
        ])]),
    )])
}

async fn publish_hub_directory_snapshot(
    state: &NodeRuntimeState,
    bus: &EventBus,
    mut snapshot: HubDirectorySnapshot,
) {
    let mut settings = state.app_state.get_app_settings().ok().flatten();
    let selected_team_uid = settings
        .as_ref()
        .map(|settings| settings.teams.active_team_uid.trim())
        .filter(|team_uid| canonical_team_color(team_uid).is_some())
        .unwrap_or(YELLOW_TEAM_UID);
    let selected_is_available = settings.as_ref().is_some_and(|settings| {
        settings
            .teams
            .local_teams
            .iter()
            .any(|team| team.team_uid == selected_team_uid)
    }) || hub_directory_contains_active_team(&snapshot, selected_team_uid);
    snapshot.active_team_uid = if selected_is_available {
        selected_team_uid.to_string()
    } else {
        if let Some(settings) = settings.as_mut() {
            settings.teams.active_team_uid = YELLOW_TEAM_UID.to_string();
            if let Err(error) = state.app_state.set_app_settings(settings) {
                bus.emit(NodeEvent::Error {
                    code: node_error_code(&error).to_string(),
                    message: format!(
                        "Active TEAM disappeared and Yellow fallback could not be persisted: {error}"
                    ),
                });
            }
        }
        emit_operational_notice(
            bus,
            LogLevel::Warn {},
            "The selected TEAM is no longer assigned by RCH; REM switched to Yellow",
        );
        YELLOW_TEAM_UID.to_string()
    };
    if let Some(settings) = settings.as_ref() {
        crate::node::apply_local_team_settings(&mut snapshot, &settings.teams);
    }
    if let Some(hub_identity_hash) = snapshot.hub_identity_hash.as_deref() {
        if let Err(error) = state
            .app_state
            .set_hub_directory(hub_identity_hash, &snapshot)
        {
            bus.emit(NodeEvent::Error {
                code: node_error_code(&error).to_string(),
                message: format!("RCH TEAM directory cache could not be persisted: {error}"),
            });
        }
    }
    if let Ok(mut guard) = state.hub_directory_snapshot.lock() {
        *guard = Some(snapshot.clone());
    }
    let _ = refresh_peer_snapshot(state).await;
    state.sdk.record_hub_directory_updated(&snapshot);
    bus.emit(NodeEvent::HubDirectoryUpdated { snapshot });
}

async fn publish_failed_hub_directory_refresh(bus: &EventBus, error: &NodeError) {
    bus.emit(NodeEvent::Error {
        code: node_error_code(error).to_string(),
        message: format!(
            "RCH TEAM peer directory refresh failed; retaining the last successful directory: {error}"
        ),
    });
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
    let fields = hub_team_directory_command_fields(
        &command_id,
        &state.identity.address_hash().to_hex_string(),
    );

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(destination);
    message.set_title_from_string(HUB_TEAM_DIRECTORY_COMMAND);
    message.fields = Some(fields);

    let signer = lxmf_private_identity(&state.identity)?;
    let wire = message
        .to_wire(Some(&signer))
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;

    // Subscribe before transmitting so a fast hub response cannot arrive
    // between the send and creation of the response receiver.
    let mut data_rx = state.transport.received_data_events();
    let mut resource_rx = state.transport.resource_events();
    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    let outcome = state.transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(NodeError::NetworkError {});
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        let received_data = tokio::select! {
            received = data_rx.recv() => match received {
                Ok(event) => event.data.as_slice().to_vec(),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Err(NodeError::InternalError {});
                }
            },
            received = resource_rx.recv() => match received {
                Ok(event) => match event.kind {
                    ResourceEventKind::Complete(complete) => complete.data,
                    _ => continue,
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Err(NodeError::InternalError {});
                }
            },
            () = tokio::time::sleep(Duration::from_millis(500)) => continue,
        };

        let Ok(reply) = LxmfMessage::from_wire(received_data.as_slice()) else {
            continue;
        };
        if reply.source_hash != Some(destination) || reply.destination_hash != Some(source) {
            continue;
        }

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
                Ok(Some(HubDirectoryResultState::Accepted)) => continue,
                Ok(Some(HubDirectoryResultState::Snapshot(mut snapshot))) => {
                    snapshot.hub_identity_hash = Some(hub_hex.clone());
                    return Ok(snapshot);
                }
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }
    }
}
