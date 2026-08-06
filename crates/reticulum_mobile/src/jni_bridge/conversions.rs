fn to_app_settings_record(input: AppSettingsInput) -> Result<AppSettingsRecord, NodeError> {
    Ok(AppSettingsRecord {
        display_name: input.display_name,
        auto_connect_saved: input.auto_connect_saved,
        announce_capabilities: input.announce_capabilities,
        tcp_clients: input.tcp_clients,
        broadcast: input.broadcast,
        transport_node_enabled: input.transport_node_enabled,
        announce_interval_seconds: input.announce_interval_seconds,
        telemetry: TelemetrySettingsRecord {
            enabled: input.telemetry.enabled,
            publish_interval_seconds: input.telemetry.publish_interval_seconds,
            accuracy_threshold_meters: input.telemetry.accuracy_threshold_meters,
            stale_after_minutes: input.telemetry.stale_after_minutes,
            expire_after_minutes: input.telemetry.expire_after_minutes,
        },
        hub: HubSettingsRecord {
            mode: parse_hub_mode(Some(input.hub.mode.as_str())),
            identity_hash: input.hub.identity_hash,
            api_base_url: input.hub.api_base_url,
            api_key: input.hub.api_key,
            refresh_interval_seconds: input.hub.refresh_interval_seconds,
        },
        teams: to_team_settings_record(input.teams)?,
        checklists: ChecklistSettingsRecord {
            default_task_due_step_minutes: input
                .checklists
                .default_task_due_step_minutes
                .unwrap_or(crate::types::DEFAULT_CHECKLIST_TASK_DUE_STEP_MINUTES)
                .max(1),
        },
        rnode: to_rnode_settings_record(Some(input.rnode))?,
    })
}

fn to_eam_projection_record(input: EamProjectionInput) -> EamProjectionRecord {
    EamProjectionRecord {
        callsign: input.callsign,
        group_name: input.group_name,
        security_status: input.security_status,
        capability_status: input.capability_status,
        preparedness_status: input.preparedness_status,
        medical_status: input.medical_status,
        mobility_status: input.mobility_status,
        comms_status: input.comms_status,
        notes: input.notes,
        updated_at_ms: input.updated_at,
        deleted_at_ms: input.deleted_at,
        eam_uid: input.eam_uid,
        team_member_uid: input.team_member_uid,
        team_uid: input.team_uid,
        reported_at: input.reported_at,
        reported_by: input.reported_by,
        overall_status: input.overall_status,
        confidence: input.confidence,
        ttl_seconds: input.ttl_seconds,
        source: input.source.map(|source| crate::types::EamSourceRecord {
            rns_identity: source.rns_identity,
            display_name: source.display_name,
        }),
        sync_state: input.sync_state,
        sync_error: input.sync_error,
        draft_created_at_ms: input.draft_created_at,
        last_synced_at_ms: input.last_synced_at,
    }
}

fn to_event_projection_record(input: EventProjectionInput) -> EventProjectionRecord {
    EventProjectionRecord {
        uid: input.uid,
        command_id: input.command_id,
        source_identity: input.source_identity,
        source_display_name: input.source_display_name,
        timestamp: input.timestamp,
        command_type: input.command_type,
        mission_uid: input.mission_uid,
        content: input.content,
        callsign: input.callsign,
        server_time: input.server_time,
        client_time: input.client_time,
        keywords: input.keywords,
        content_hashes: input.content_hashes,
        updated_at_ms: input.updated_at,
        deleted_at_ms: input.deleted_at,
        correlation_id: input.correlation_id,
        topics: input.topics,
    }
}

fn to_message_record(input: MessageRecordInput) -> Result<MessageRecord, NodeError> {
    Ok(MessageRecord {
        message_id_hex: input.message_id_hex,
        conversation_id: input.conversation_id,
        direction: parse_message_direction(&input.direction)?,
        destination_hex: input.destination_hex,
        source_hex: input.source_hex,
        requested_destination_hex: input.requested_destination_hex,
        delivery_destination_hex: input.delivery_destination_hex,
        recipient_identity_hex: input.recipient_identity_hex,
        last_wire_message_id_hex: input.last_wire_message_id_hex,
        title: input.title,
        body_utf8: input.body_utf8,
        method: parse_message_method(&input.method)?,
        state: parse_message_state(&input.state)?,
        transport_state: parse_transport_delivery_state(input.transport_state.as_deref())?,
        application_ack_state: parse_application_ack_state(input.application_ack_state.as_deref())?,
        detail: input.detail,
        sent_at_ms: input.sent_at,
        received_at_ms: input.received_at,
        updated_at_ms: input.updated_at,
    })
}

fn to_telemetry_position_record(input: TelemetryPositionInput) -> TelemetryPositionRecord {
    TelemetryPositionRecord {
        callsign: input.callsign,
        lat: input.lat,
        lon: input.lon,
        alt: input.alt,
        course: input.course,
        speed: input.speed,
        accuracy: input.accuracy,
        updated_at_ms: input.updated_at,
    }
}

fn to_checklist_create_request(input: ChecklistCreateInput) -> ChecklistCreateOnlineRequest {
    ChecklistCreateOnlineRequest {
        checklist_uid: input.checklist_uid,
        mission_uid: input.mission_uid,
        template_uid: input.template_uid,
        name: input.name,
        description: input.description,
        start_time: input.start_time,
        created_by_team_member_rns_identity: input.created_by_team_member_rns_identity,
        created_by_team_member_display_name: input.created_by_team_member_display_name,
    }
}

fn to_checklist_template_import_request(
    input: ChecklistTemplateImportInput,
) -> ChecklistTemplateImportCsvRequest {
    ChecklistTemplateImportCsvRequest {
        template_uid: input.template_uid,
        name: input.name,
        description: input.description,
        csv_text: input.csv_text,
        source_filename: input.source_filename,
    }
}

fn to_checklist_update_request(input: ChecklistUpdateInput) -> ChecklistUpdateRequest {
    ChecklistUpdateRequest {
        checklist_uid: input.checklist_uid,
        patch: ChecklistUpdatePatch {
            mission_uid: input.patch.mission_uid,
            template_uid: input.patch.template_uid,
            name: input.patch.name,
            description: input.patch.description,
            start_time: input.patch.start_time,
        },
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_status_request(
    input: ChecklistTaskStatusInput,
) -> Result<ChecklistTaskStatusSetRequest, NodeError> {
    let user_status = match input.user_status.trim() {
        "COMPLETE" => crate::types::ChecklistUserTaskStatus::Complete {},
        "PENDING" => crate::types::ChecklistUserTaskStatus::Pending {},
        _ => return Err(NodeError::InvalidConfig {}),
    };
    Ok(ChecklistTaskStatusSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        user_status,
        changed_by_team_member_rns_identity: input.changed_by_team_member_rns_identity,
    })
}

fn to_checklist_task_row_add_request(
    input: ChecklistTaskRowAddInput,
) -> ChecklistTaskRowAddRequest {
    ChecklistTaskRowAddRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        number: input.number,
        due_relative_minutes: input.due_relative_minutes,
        legacy_value: input.legacy_value,
        changed_by_team_member_rns_identity: input.changed_by_team_member_rns_identity,
    }
}

fn to_checklist_task_row_delete_request(
    input: ChecklistTaskRowDeleteInput,
) -> ChecklistTaskRowDeleteRequest {
    ChecklistTaskRowDeleteRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        changed_by_team_member_rns_identity: input.changed_by_team_member_rns_identity,
    }
}

fn to_checklist_task_row_style_request(
    input: ChecklistTaskRowStyleInput,
) -> ChecklistTaskRowStyleSetRequest {
    ChecklistTaskRowStyleSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        row_background_color: input.row_background_color,
        line_break_enabled: input.line_break_enabled,
        changed_by_team_member_rns_identity: None,
    }
}

fn to_checklist_task_cell_request(input: ChecklistTaskCellInput) -> ChecklistTaskCellSetRequest {
    ChecklistTaskCellSetRequest {
        checklist_uid: input.checklist_uid,
        task_uid: input.task_uid,
        column_uid: input.column_uid,
        value: input.value,
        updated_by_team_member_rns_identity: input.updated_by_team_member_rns_identity,
    }
}

fn status_to_json(status: NodeStatus) -> String {
    json!({
        "running": status.running,
        "name": status.name,
        "identityHex": status.identity_hex,
        "appDestinationHex": status.app_destination_hex,
        "lxmfDestinationHex": status.lxmf_destination_hex,
        "readiness": runtime_readiness_json(status.readiness),
        "interfaces": status.interfaces.into_iter().map(interface_status_json).collect::<Vec<_>>()
    })
    .to_string()
}

fn runtime_readiness_state_str(state: RuntimeReadinessState) -> &'static str {
    match state {
        RuntimeReadinessState::Pending => "Pending",
        RuntimeReadinessState::Ready => "Ready",
        RuntimeReadinessState::Failed => "Failed",
        RuntimeReadinessState::Unsupported => "Unsupported",
        RuntimeReadinessState::Disabled => "Disabled",
    }
}

fn runtime_interface_readiness_json(
    readiness: RuntimeInterfaceReadinessRecord,
) -> serde_json::Value {
    json!({
        "id": readiness.id,
        "label": readiness.label,
        "state": runtime_readiness_state_str(readiness.state),
        "detail": readiness.detail,
        "lastError": readiness.last_error,
    })
}

fn runtime_readiness_json(readiness: RuntimeReadinessSnapshot) -> serde_json::Value {
    json!({
        "state": runtime_readiness_state_str(readiness.state),
        "interfaces": readiness
            .interfaces
            .into_iter()
            .map(runtime_interface_readiness_json)
            .collect::<Vec<_>>(),
    })
}

fn interface_status_json(status: InterfaceStatusRecord) -> serde_json::Value {
    json!({
        "interfaceHex": status.interface_hex,
        "label": status.label,
        "kind": status.kind,
        "state": status.state,
        "lastError": status.last_error,
        "rxPackets": status.rx_packets,
        "rxBytes": status.rx_bytes,
        "lastActivityMs": status.last_activity_ms
    })
}

fn peer_state_to_str(state: PeerState) -> &'static str {
    match state {
        PeerState::Connecting {} => "Connecting",
        PeerState::Connected {} => "Connected",
        PeerState::Disconnected {} => "Disconnected",
    }
}

fn announce_class_to_str(class: crate::types::AnnounceClass) -> &'static str {
    match class {
        crate::types::AnnounceClass::PeerApp {} => "PeerApp",
        crate::types::AnnounceClass::RchHubServer {} => "RchHubServer",
        crate::types::AnnounceClass::PropagationNode {} => "PropagationNode",
        crate::types::AnnounceClass::LxmfDelivery {} => "LxmfDelivery",
        crate::types::AnnounceClass::Other {} => "Other",
    }
}

fn peer_change_json(change: &PeerChange) -> serde_json::Value {
    json!({
        "destinationHex": change.destination_hex,
        "identityHex": change.identity_hex,
        "lxmfDestinationHex": change.lxmf_destination_hex,
        "displayName": change.display_name,
        "appData": change.app_data,
        "state": peer_state_to_str(change.state),
        "saved": change.saved,
        "stale": change.stale,
        "activeLink": change.active_link,
        "lastError": change.last_error,
        "lastResolutionError": change.last_resolution_error,
        "lastResolutionAttemptAtMs": change.last_resolution_attempt_at_ms,
        "lastSeenAtMs": change.last_seen_at_ms,
        "announceLastSeenAtMs": change.announce_last_seen_at_ms,
        "lxmfLastSeenAtMs": change.lxmf_last_seen_at_ms
    })
}

fn peer_record_json(peer: &PeerRecord) -> serde_json::Value {
    json!({
        "destinationHex": peer.destination_hex,
        "identityHex": peer.identity_hex,
        "lxmfDestinationHex": peer.lxmf_destination_hex,
        "displayName": peer.display_name,
        "appData": peer.app_data,
        "state": peer_state_to_str(peer.state),
        "saved": peer.saved,
        "stale": peer.stale,
        "activeLink": peer.active_link,
        "hubDerived": peer.hub_derived,
        "lastResolutionError": peer.last_resolution_error,
        "lastResolutionAttemptAtMs": peer.last_resolution_attempt_at_ms,
        "lastSeenAtMs": peer.last_seen_at_ms,
        "announceLastSeenAtMs": peer.announce_last_seen_at_ms,
        "lxmfLastSeenAtMs": peer.lxmf_last_seen_at_ms
    })
}

fn hub_directory_peer_json(peer: &HubDirectoryPeerRecord) -> serde_json::Value {
    json!({
        "identity": peer.identity,
        "destinationHash": peer.destination_hash,
        "displayName": peer.display_name,
        "announceCapabilities": peer.announce_capabilities,
        "clientType": peer.client_type,
        "registeredMode": peer.registered_mode,
        "lastSeen": peer.last_seen,
        "status": peer.status
    })
}

fn hub_directory_snapshot_json(snapshot: &HubDirectorySnapshot) -> serde_json::Value {
    json!({
        "schemaVersion": snapshot.schema_version,
        "hubIdentityHash": snapshot.hub_identity_hash,
        "activeTeamUid": snapshot.active_team_uid,
        "effectiveConnectedMode": snapshot.effective_connected_mode,
        "teams": snapshot.teams.iter().map(|team| json!({
            "uid": team.uid,
            "color": team.color,
            "teamName": team.team_name,
        })).collect::<Vec<_>>(),
        "callerMemberships": snapshot.caller_memberships.iter().map(|membership| json!({
            "teamUid": membership.team_uid,
            "teamMemberUid": membership.team_member_uid,
        })).collect::<Vec<_>>(),
        "members": snapshot.members.iter().map(|member| json!({
            "teamUid": member.team_uid,
            "teamMemberUid": member.team_member_uid,
            "identity": member.identity,
            "destinationHash": member.destination_hash,
            "displayName": member.display_name,
            "announceCapabilities": member.announce_capabilities,
            "clientType": member.client_type,
            "registeredMode": member.registered_mode,
            "lastSeen": member.last_seen,
            "status": member.status,
        })).collect::<Vec<_>>(),
        "localTeams": snapshot.local_teams.iter().map(|team| json!({
            "teamUid": team.team_uid,
            "memberDestinations": team.member_destinations,
        })).collect::<Vec<_>>(),
        "items": snapshot
            .items
            .iter()
            .map(hub_directory_peer_json)
            .collect::<Vec<_>>(),
        "receivedAtMs": snapshot.received_at_ms
    })
}

fn operational_notice_json(notice: &crate::types::OperationalNotice) -> serde_json::Value {
    json!({
        "level": log_level_to_str(notice.level),
        "message": notice.message,
        "atMs": notice.at_ms
    })
}

fn hub_settings_json(settings: &HubSettingsRecord) -> serde_json::Value {
    json!({
        "mode": settings.mode.as_str(),
        "identityHash": settings.identity_hash,
        "apiBaseUrl": settings.api_base_url,
        "apiKey": settings.api_key,
        "refreshIntervalSeconds": settings.refresh_interval_seconds
    })
}

fn telemetry_settings_json(settings: &TelemetrySettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "publishIntervalSeconds": settings.publish_interval_seconds,
        "accuracyThresholdMeters": settings.accuracy_threshold_meters,
        "staleAfterMinutes": settings.stale_after_minutes,
        "expireAfterMinutes": settings.expire_after_minutes
    })
}

fn rnode_settings_json(settings: &RnodeSettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "connectionMode": settings.connection_mode,
        "peripheralId": settings.peripheral_id,
        "displayName": settings.display_name,
        "region": settings.region,
        "profile": settings.profile,
        "frequencyHz": settings.frequency_hz
    })
}

fn app_settings_json(settings: &AppSettingsRecord) -> serde_json::Value {
    json!({
        "displayName": settings.display_name,
        "autoConnectSaved": settings.auto_connect_saved,
        "announceCapabilities": settings.announce_capabilities,
        "tcpClients": settings.tcp_clients,
        "broadcast": settings.broadcast,
        "transportNodeEnabled": settings.transport_node_enabled,
        "announceIntervalSeconds": settings.announce_interval_seconds,
        "telemetry": telemetry_settings_json(&settings.telemetry),
        "hub": hub_settings_json(&settings.hub),
        "teams": {
            "activeTeamUid": settings.teams.active_team_uid,
            "aliases": settings.teams.aliases.iter().map(|alias| json!({
                "teamUid": alias.team_uid,
                "alias": alias.alias,
            })).collect::<Vec<_>>(),
            "localTeams": settings.teams.local_teams.iter().map(|team| json!({
                "teamUid": team.team_uid,
                "memberDestinations": team.member_destinations,
            })).collect::<Vec<_>>(),
            "localTeamsInitialized": settings.teams.local_teams_initialized
        },
        "checklists": {
            "defaultTaskDueStepMinutes": settings.checklists.default_task_due_step_minutes
        },
        "rnode": rnode_settings_json(&settings.rnode)
    })
}
