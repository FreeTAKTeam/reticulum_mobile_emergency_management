fn sos_state_to_str(state: SosState) -> &'static str {
    crate::sos::sos_status_label(state)
}

fn sos_trigger_to_str(source: SosTriggerSource) -> &'static str {
    crate::sos::sos_trigger_label(source)
}

fn sos_kind_to_str(kind: SosMessageKind) -> &'static str {
    crate::sos::sos_kind_label(kind)
}

fn sos_settings_json(settings: &SosSettingsRecord) -> serde_json::Value {
    json!({
        "enabled": settings.enabled,
        "messageTemplate": settings.message_template,
        "cancelMessageTemplate": settings.cancel_message_template,
        "countdownSeconds": settings.countdown_seconds,
        "includeLocation": settings.include_location,
        "triggerShake": settings.trigger_shake,
        "triggerTapPattern": settings.trigger_tap_pattern,
        "triggerPowerButton": settings.trigger_power_button,
        "shakeSensitivity": settings.shake_sensitivity,
        "audioRecording": settings.audio_recording,
        "audioDurationSeconds": settings.audio_duration_seconds,
        "periodicUpdates": settings.periodic_updates,
        "updateIntervalSeconds": settings.update_interval_seconds,
        "floatingButton": settings.floating_button,
        "silentAutoAnswer": settings.silent_auto_answer,
        "deactivationPinHash": settings.deactivation_pin_hash,
        "deactivationPinSalt": settings.deactivation_pin_salt,
        "floatingButtonX": settings.floating_button_x,
        "floatingButtonY": settings.floating_button_y,
        "activePillX": settings.active_pill_x,
        "activePillY": settings.active_pill_y
    })
}

fn sos_status_json(status: &SosStatusRecord) -> serde_json::Value {
    json!({
        "state": sos_state_to_str(status.state),
        "incidentId": status.incident_id,
        "triggerSource": status.trigger_source.map(sos_trigger_to_str),
        "countdownDeadlineMs": status.countdown_deadline_ms,
        "activatedAtMs": status.activated_at_ms,
        "lastSentAtMs": status.last_sent_at_ms,
        "lastUpdateAtMs": status.last_update_at_ms,
        "updatedAtMs": status.updated_at_ms
    })
}

fn sos_alert_json(alert: &SosAlertRecord) -> serde_json::Value {
    json!({
        "incidentId": alert.incident_id,
        "sourceHex": alert.source_hex,
        "conversationId": alert.conversation_id,
        "state": sos_kind_to_str(alert.state),
        "active": alert.active,
        "bodyUtf8": alert.body_utf8,
        "lat": alert.lat,
        "lon": alert.lon,
        "batteryPercent": alert.battery_percent,
        "audioId": alert.audio_id,
        "messageIdHex": alert.message_id_hex,
        "receivedAtMs": alert.received_at_ms,
        "updatedAtMs": alert.updated_at_ms
    })
}

fn sos_location_json(location: &SosLocationRecord) -> serde_json::Value {
    json!({
        "incidentId": location.incident_id,
        "sourceHex": location.source_hex,
        "lat": location.lat,
        "lon": location.lon,
        "alt": location.alt,
        "accuracy": location.accuracy,
        "batteryPercent": location.battery_percent,
        "recordedAtMs": location.recorded_at_ms
    })
}

fn sos_audio_json(audio: &SosAudioRecord) -> serde_json::Value {
    json!({
        "audioId": audio.audio_id,
        "incidentId": audio.incident_id,
        "sourceHex": audio.source_hex,
        "path": audio.path,
        "mimeType": audio.mime_type,
        "durationSeconds": audio.duration_seconds,
        "createdAtMs": audio.created_at_ms
    })
}

fn to_sos_audio_record(input: SosAudioInput) -> SosAudioRecord {
    SosAudioRecord {
        audio_id: input.audio_id,
        incident_id: input.incident_id,
        source_hex: input.source_hex,
        path: input.path,
        mime_type: input.mime_type,
        duration_seconds: input.duration_seconds,
        created_at_ms: input.created_at_ms,
    }
}

fn saved_peer_json(peer: &SavedPeerRecord) -> serde_json::Value {
    json!({
        "destination": peer.destination_hex,
        "label": peer.label,
        "savedAt": peer.saved_at_ms,
        "identityHex": peer.identity_hex,
        "lxmfDestinationHex": peer.lxmf_destination_hex,
        "appData": peer.app_data,
        "displayName": peer.display_name,
        "lastRouteSeenAtMs": peer.last_route_seen_at_ms,
        "lastHops": peer.last_hops
    })
}

fn eam_projection_json(record: &EamProjectionRecord) -> serde_json::Value {
    json!({
        "callsign": record.callsign,
        "groupName": record.group_name,
        "securityStatus": record.security_status,
        "capabilityStatus": record.capability_status,
        "preparednessStatus": record.preparedness_status,
        "medicalStatus": record.medical_status,
        "mobilityStatus": record.mobility_status,
        "commsStatus": record.comms_status,
        "notes": record.notes,
        "updatedAt": record.updated_at_ms,
        "deletedAt": record.deleted_at_ms,
        "eamUid": record.eam_uid,
        "teamMemberUid": record.team_member_uid,
        "teamUid": record.team_uid,
        "reportedAt": record.reported_at,
        "reportedBy": record.reported_by,
        "overallStatus": record.overall_status,
        "confidence": record.confidence,
        "ttlSeconds": record.ttl_seconds,
        "source": record.source.as_ref().map(|source| json!({
            "rns_identity": source.rns_identity,
            "display_name": source.display_name
        })),
        "syncState": record.sync_state,
        "syncError": record.sync_error,
        "draftCreatedAt": record.draft_created_at_ms,
        "lastSyncedAt": record.last_synced_at_ms
    })
}

fn event_projection_json(record: &EventProjectionRecord) -> serde_json::Value {
    json!({
        "command_id": record.command_id,
        "source": {
            "rns_identity": record.source_identity,
            "display_name": record.source_display_name
        },
        "timestamp": record.timestamp,
        "command_type": record.command_type,
        "args": {
            "entry_uid": record.uid,
            "mission_uid": record.mission_uid,
            "content": record.content,
            "callsign": record.callsign,
            "server_time": record.server_time,
            "client_time": record.client_time,
            "keywords": record.keywords,
            "content_hashes": record.content_hashes,
            "source_identity": record.source_identity,
            "source_display_name": record.source_display_name
        },
        "correlation_id": record.correlation_id,
        "topics": record.topics,
        "deleted_at": record.deleted_at_ms,
        "updatedAt": record.updated_at_ms
    })
}

fn checklist_column_json(column: &crate::types::ChecklistColumnRecord) -> serde_json::Value {
    json!({
        "columnUid": column.column_uid,
        "columnName": column.column_name,
        "columnType": column.column_type.as_str(),
        "columnEditable": column.column_editable,
        "backgroundColor": column.background_color,
        "textColor": column.text_color,
        "isRemovable": column.is_removable,
        "systemKey": column.system_key.map(|key| key.as_str()),
        "displayOrder": column.display_order
    })
}

fn checklist_cell_json(cell: &crate::types::ChecklistCellRecord) -> serde_json::Value {
    json!({
        "cellUid": cell.cell_uid,
        "taskUid": cell.task_uid,
        "columnUid": cell.column_uid,
        "value": cell.value,
        "updatedAt": cell.updated_at,
        "updatedByTeamMemberRnsIdentity": cell.updated_by_team_member_rns_identity
    })
}

fn checklist_task_json(task: &crate::types::ChecklistTaskRecord) -> serde_json::Value {
    json!({
        "taskUid": task.task_uid,
        "number": task.number,
        "userStatus": task.user_status.as_str(),
        "taskStatus": task.task_status.as_str(),
        "isLate": task.is_late,
        "updatedAt": task.updated_at,
        "deletedAt": task.deleted_at,
        "customStatus": task.custom_status,
        "dueRelativeMinutes": task.due_relative_minutes,
        "dueDtg": task.due_dtg,
        "notes": task.notes,
        "rowBackgroundColor": task.row_background_color,
        "lineBreakEnabled": task.line_break_enabled,
        "completedAt": task.completed_at,
        "completedByTeamMemberRnsIdentity": task.completed_by_team_member_rns_identity,
        "legacyValue": task.legacy_value,
        "cells": task.cells.iter().map(checklist_cell_json).collect::<Vec<_>>()
    })
}

fn checklist_feed_publication_json(
    publication: &crate::types::ChecklistFeedPublicationRecord,
) -> serde_json::Value {
    json!({
        "publicationUid": publication.publication_uid,
        "checklistUid": publication.checklist_uid,
        "missionFeedUid": publication.mission_feed_uid,
        "publishedAt": publication.published_at,
        "publishedByTeamMemberRnsIdentity": publication.published_by_team_member_rns_identity
    })
}

fn checklist_record_json(record: &ChecklistRecord) -> serde_json::Value {
    json!({
        "uid": record.uid,
        "missionUid": record.mission_uid,
        "templateUid": record.template_uid,
        "templateVersion": record.template_version,
        "templateName": record.template_name,
        "name": record.name,
        "description": record.description,
        "startTime": record.start_time,
        "mode": record.mode.as_str(),
        "syncState": record.sync_state.as_str(),
        "originType": record.origin_type.as_str(),
        "checklistStatus": record.checklist_status.as_str(),
        "createdAt": record.created_at,
        "createdByTeamMemberRnsIdentity": record.created_by_team_member_rns_identity,
        "createdByTeamMemberDisplayName": record.created_by_team_member_display_name,
        "updatedAt": record.updated_at,
        "lastChangedByTeamMemberRnsIdentity": record.last_changed_by_team_member_rns_identity,
        "deletedAt": record.deleted_at,
        "uploadedAt": record.uploaded_at,
        "participantRnsIdentities": record.participant_rns_identities,
        "expectedTaskCount": record.expected_task_count,
        "progressPercent": record.progress_percent,
        "counts": {
            "pendingCount": record.counts.pending_count,
            "lateCount": record.counts.late_count,
            "completeCount": record.counts.complete_count
        },
        "columns": record.columns.iter().map(checklist_column_json).collect::<Vec<_>>(),
        "tasks": record.tasks.iter().map(checklist_task_json).collect::<Vec<_>>(),
        "feedPublications": record
            .feed_publications
            .iter()
            .map(checklist_feed_publication_json)
            .collect::<Vec<_>>()
    })
}

fn checklist_template_json(record: &ChecklistTemplateRecord) -> serde_json::Value {
    json!({
        "uid": record.uid,
        "name": record.name,
        "description": record.description,
        "version": record.version,
        "originType": record.origin_type.as_str(),
        "createdAt": record.created_at,
        "updatedAt": record.updated_at,
        "sourceFilename": record.source_filename,
        "columns": record.columns.iter().map(checklist_column_json).collect::<Vec<_>>(),
        "tasks": record.tasks.iter().map(checklist_task_json).collect::<Vec<_>>()
    })
}

fn telemetry_position_json(record: &TelemetryPositionRecord) -> serde_json::Value {
    json!({
        "callsign": record.callsign,
        "lat": record.lat,
        "lon": record.lon,
        "alt": record.alt,
        "course": record.course,
        "speed": record.speed,
        "accuracy": record.accuracy,
        "updatedAt": record.updated_at_ms
    })
}

fn eam_team_summary_json(summary: &crate::types::EamTeamSummaryRecord) -> serde_json::Value {
    json!({
        "teamUid": summary.team_uid,
        "total": summary.total,
        "activeTotal": summary.active_total,
        "deletedTotal": summary.deleted_total,
        "overallStatus": summary.overall_status,
        "greenTotal": summary.green_total,
        "yellowTotal": summary.yellow_total,
        "redTotal": summary.red_total,
        "updatedAt": summary.updated_at_ms
    })
}

fn eam_readiness_summary_json(
    summary: &crate::types::EamReadinessSummaryRecord,
) -> serde_json::Value {
    json!({
        "activeTotal": summary.active_total,
        "updatedAt": summary.updated_at_ms,
        "statusMetrics": summary.status_metrics.iter().map(|metric| json!({
            "field": metric.field,
            "label": metric.label,
            "score": metric.score,
            "band": metric.band,
            "ringColor": metric.ring_color
        })).collect::<Vec<_>>(),
        "messages": summary.messages.iter().map(|message| json!({
            "callsign": message.callsign,
            "overallScore": message.overall_score,
            "overallBand": message.overall_band,
            "overallRingColor": message.overall_ring_color
        })).collect::<Vec<_>>()
    })
}

fn operational_summary_json(summary: &crate::types::OperationalSummary) -> serde_json::Value {
    json!({
        "running": summary.running,
        "peerCountTotal": summary.peer_count_total,
        "savedPeerCount": summary.saved_peer_count,
        "connectedPeerCount": summary.connected_peer_count,
        "conversationCount": summary.conversation_count,
        "messageCount": summary.message_count,
        "eamCount": summary.eam_count,
        "eventCount": summary.event_count,
        "telemetryCount": summary.telemetry_count,
        "activePropagationNodeHex": summary.active_propagation_node_hex,
        "updatedAtMs": summary.updated_at_ms
    })
}
