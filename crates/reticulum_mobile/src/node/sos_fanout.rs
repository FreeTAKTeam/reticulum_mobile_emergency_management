fn emit_sos_status(
    app_state: &AppStateStore,
    bus: &EventBus,
    status: &SosStatusRecord,
    reason: &str,
) -> Result<(), NodeError> {
    let invalidation = app_state.set_sos_status(status, reason)?;
    emit_projection_invalidation(bus, invalidation);
    bus.emit(NodeEvent::SosStatusChanged {
        status: status.clone(),
    });
    Ok(())
}

fn is_pending_sos_countdown_for_incident(status: &SosStatusRecord, incident_id: &str) -> bool {
    matches!(status.state, SosState::Countdown {})
        && status.incident_id.as_deref() == Some(incident_id)
}

fn app_state_has_pending_sos_countdown(app_state: &AppStateStore, incident_id: &str) -> bool {
    matches!(
        app_state.get_sos_status(),
        Ok(Some(status)) if is_pending_sos_countdown_for_incident(&status, incident_id)
    )
}

#[allow(clippy::too_many_arguments)]
fn run_sos_fanout(
    app_state: AppStateStore,
    bus: EventBus,
    tx: mpsc::Sender<Command>,
    status: NodeStatus,
    settings: SosSettingsRecord,
    saved_peers: Vec<SavedPeerRecord>,
    peers: Vec<PeerRecord>,
    active_propagation_node_hex: Option<String>,
    _active_config: Option<NodeConfigFingerprint>,
    _hub_directory_snapshot: Option<HubDirectorySnapshot>,
    telemetry: Option<SosDeviceTelemetryRecord>,
    incident_id: String,
    trigger_source: SosTriggerSource,
    kind: SosMessageKind,
) -> Option<SosStatusRecord> {
    let now = now_ms();
    let sending = SosStatusRecord {
        state: SosState::Sending {},
        incident_id: Some(incident_id.clone()),
        trigger_source: Some(trigger_source),
        countdown_deadline_ms: None,
        activated_at_ms: if matches!(kind, SosMessageKind::Cancelled {}) {
            None
        } else {
            Some(now)
        },
        last_sent_at_ms: None,
        last_update_at_ms: None,
        updated_at_ms: now,
    };
    if emit_sos_status(&app_state, &bus, &sending, "sos-sending").is_err() {
        return None;
    }

    if matches!(kind, SosMessageKind::Active {}) && settings.audio_recording {
        bus.emit(NodeEvent::SosAudioRecordingRequested {
            incident_id: incident_id.clone(),
            duration_seconds: settings.audio_duration_seconds,
        });
    }

    let body = compose_sos_body(&settings, kind, telemetry.as_ref());
    let mut targets = build_sos_replication_targets(
        &status,
        peers.as_slice(),
        saved_peers.as_slice(),
        active_propagation_node_hex.as_deref(),
    );
    let route_hops = route_hops_for_replication(&app_state, &bus, "sos");
    prioritize_sos_replication_targets(
        targets.as_mut_slice(),
        peers.as_slice(),
        saved_peers.as_slice(),
        &route_hops,
    );
    if targets.is_empty() {
        bus.emit(NodeEvent::Log {
            level: LogLevel::Warn {},
            message: "sos fanout has no eligible saved or active mission-capable peer targets"
                .to_string(),
        });
    }
    for target in targets {
        let destination_hex = target.app_destination_hex.clone();
        let command = SosCommand {
            state: kind,
            incident_id: incident_id.clone(),
            trigger_source,
            sent_at_ms: now,
            audio_id: None,
        };
        let fields = match build_sos_fields(&command, telemetry.as_ref()) {
            Ok(fields) => fields,
            Err(err) => {
                bus.emit(NodeEvent::Error {
                    code: "InternalError".to_string(),
                    message: format!(
                        "sos field encode failed destination={destination_hex} reason={err}"
                    ),
                });
                continue;
            }
        };
        let message_id_hex = format!(
            "{}-{}-{}",
            incident_id,
            destination_hex.chars().take(8).collect::<String>(),
            now
        );
        let record = canonicalize_chat_message(&MessageRecord {
            message_id_hex: message_id_hex.clone(),
            conversation_id: sdkmsg::MessagingStore::conversation_id_for(destination_hex.as_str()),
            direction: MessageDirection::Outbound {},
            destination_hex: destination_hex.clone(),
            source_hex: Some(status.lxmf_destination_hex.clone()),
            requested_destination_hex: Some(destination_hex.clone()),
            delivery_destination_hex: Some(destination_hex.clone()),
            recipient_identity_hex: None,
            last_wire_message_id_hex: Some(message_id_hex.clone()),
            title: Some("SOS Emergency".to_string()),
            body_utf8: body.clone(),
            method: if matches!(target.send_mode, SendMode::PropagationOnly {}) {
                MessageMethod::Propagated {}
            } else {
                MessageMethod::Direct {}
            },
            state: MessageState::Queued {},
            transport_state: TransportDeliveryState::Queued {},
            application_ack_state: ApplicationAckState::Waiting {},
            detail: Some(format!("sos:{}", crate::sos::sos_kind_label(kind))),
            sent_at_ms: Some(now),
            received_at_ms: None,
            updated_at_ms: now,
        });
        match app_state.upsert_message(&record) {
            Ok(invalidations) => {
                for invalidation in invalidations {
                    emit_projection_invalidation(&bus, invalidation);
                }
                bus.emit(NodeEvent::MessageUpdated { message: record });
            }
            Err(error) => {
                bus.emit(NodeEvent::Error {
                    code: "IoError".to_string(),
                    message: format!(
                        "failed to persist sos message destination={destination_hex} reason={error}"
                    ),
                });
            }
        }

        let (resp_tx, _resp_rx) = cb::bounded(1);
        if let Err(err) = dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex: destination_hex.clone(),
                bytes: body.as_bytes().to_vec(),
                fields_bytes: Some(fields),
                send_mode: target.send_mode,
                resp: resp_tx,
            },
        ) {
            bus.emit(NodeEvent::Error {
                code: "NotRunning".to_string(),
                message: format!(
                    "sos send enqueue failed destination={destination_hex} reason={err}"
                ),
            });
        }
    }

    let next = if matches!(kind, SosMessageKind::Cancelled {}) {
        idle_status()
    } else {
        active_status(incident_id, trigger_source, now)
    };
    if emit_sos_status(&app_state, &bus, &next, "sos-fanout-complete").is_err() {
        return None;
    }
    Some(next)
}
