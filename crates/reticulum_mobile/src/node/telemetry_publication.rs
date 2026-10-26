type TelemetryInflight = HashMap<
    String,
    (
        String,
        cb::Receiver<Result<(), NodeError>>,
        std::time::Instant,
    ),
>;
use crate::mission_sync::parse_mission_sync_metadata;
use crate::telemetry_publication::{
    TelemetryPublicationState as PublicationState, TelemetryRecipient,
};

fn emit_telemetry_projection(
    app_state: &AppStateStore,
    bus: &EventBus,
    update: impl FnOnce(&mut crate::telemetry_publication::TelemetryPublication),
) -> Result<(), NodeError> {
    if let Some(invalidation) = app_state.update_telemetry_publication(update)? {
        emit_projection_invalidation(bus, invalidation);
    }
    Ok(())
}

impl Node {
    pub(crate) fn telemetry_publication_json(&self) -> Result<JsonValue, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        Ok(inner.app_state.telemetry_publication()?.snapshot_json())
    }

    fn retain_telemetry_fix(&self, position: TelemetryPositionRecord) -> Result<(), NodeError> {
        if position.callsign.trim().is_empty()
            || !(-90.0..=90.0).contains(&position.lat)
            || !(-180.0..=180.0).contains(&position.lon)
            || [
                position.alt,
                position.course,
                position.speed,
                position.accuracy,
            ]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(NodeError::InvalidConfig {});
        }
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        let invalidation = inner.app_state.record_local_telemetry_fix(&position)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        inner.app_state.record_local_observer_fix(&position)?;
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("telemetry-upserted".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        let due = inner
            .next_telemetry_publish_at_ms
            .is_none_or(|deadline| now_ms() >= deadline);
        let normal_cadence = inner
            .app_state
            .get_app_settings()?
            .map(|settings| settings.telemetry.publish_interval_seconds)
            .unwrap_or(60);
        if due {
            let cadence =
                effective_power_cadence_seconds(normal_cadence, inner.power_state.saver_active);
            inner.next_telemetry_publish_at_ms =
                Some(now_ms().saturating_add(u64::from(cadence) * 1_000));
        }
        emit_telemetry_projection(&inner.app_state, &inner.bus, |publication| {
            if publication
                .fix
                .as_ref()
                .is_some_and(|fix| fix.updated_at_ms > position.updated_at_ms)
            {
                return;
            }
            let fix_changed = publication.fix.as_ref() != Some(&position);
            publication.fix = Some(position.clone());
            publication
                .recipients
                .retain(|recipient| recipient.callsign == position.callsign);
            for recipient in &mut publication.recipients {
                if recipient.custody_message_id_hex.is_some() && !fix_changed {
                    continue;
                }
                if due
                    || fix_changed
                        && !matches!(
                            recipient.state,
                            PublicationState::Acknowledged | PublicationState::Rejected
                        )
                {
                    recipient.fix_updated_at_ms = position.updated_at_ms;
                    recipient.command_id.clear();
                    recipient.custody_message_id_hex = None;
                    recipient.state = PublicationState::QueuedNoRoute;
                    recipient.message_id_hex = None;
                    recipient.representation = None;
                    recipient.detail = None;
                    recipient.attempt_route = None;
                }
            }
        })
    }

    fn flush_pending_telemetry(&self, inflight: &mut TelemetryInflight) -> Result<(), NodeError> {
        let (app_state, bus, status, peers, sync, snapshot, config, saver) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            if inner.cmd_tx.is_none() {
                return Ok(());
            }
            let status = inner
                .status
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let peers = inner
                .peers_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let sync = inner
                .sync_status_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            let snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.app_state.clone(),
                inner.bus.clone(),
                status,
                peers,
                sync,
                snapshot,
                inner.active_config.clone(),
                inner.power_state.saver_active,
            )
        };
        let mut completed = Vec::new();
        for (destination, (command_id, response, started)) in inflight.iter() {
            let result = match response.try_recv() {
                Ok(result) => Some(result),
                Err(cb::TryRecvError::Disconnected) => Some(Err(NodeError::NotRunning {})),
                Err(cb::TryRecvError::Empty) if started.elapsed() >= SEND_COMMAND_TIMEOUT => {
                    Some(Err(NodeError::Timeout {}))
                }
                Err(cb::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                if let Err(error) = result {
                    emit_telemetry_projection(&app_state, &bus, |publication| {
                        if let Some(row) = publication
                            .recipients
                            .iter_mut()
                            .find(|row| row.command_id == *command_id)
                        {
                            if row.custody_message_id_hex.is_none()
                                && !matches!(
                                    row.state,
                                    PublicationState::Acknowledged | PublicationState::Rejected
                                )
                            {
                                row.state = if matches!(error, NodeError::Timeout {}) {
                                    PublicationState::TimedOut
                                } else {
                                    PublicationState::Failed
                                };
                                row.detail = Some(format!("{error}"));
                                row.updated_at_ms = now_ms();
                            }
                        }
                    })?;
                }
                completed.push(destination.clone());
            }
        }
        for destination in completed {
            inflight.remove(&destination);
        }
        let publication = app_state.telemetry_publication()?;
        let Some(position) = publication.fix else {
            return Ok(());
        };
        let settings = app_state.get_app_settings()?;
        let disabled_or_expired = settings.as_ref().is_some_and(|settings| {
            !settings.telemetry.enabled
                || now_ms().saturating_sub(position.updated_at_ms)
                    > u64::from(settings.telemetry.expire_after_minutes) * 60_000
        });
        let saved = app_state.get_saved_peers()?;
        let targets = if disabled_or_expired {
            Vec::new()
        } else {
            build_runtime_telemetry_destinations(
                &status,
                &peers,
                &saved,
                sync.active_propagation_node_hex.as_deref(),
                config.as_ref(),
                snapshot.as_ref(),
            )?
        };
        let mut current_fix = true;
        emit_telemetry_projection(&app_state, &bus, |publication| {
            if publication.fix.as_ref() != Some(&position) {
                current_fix = false;
                return;
            }
            publication.recipients.retain(|row| {
                targets
                    .iter()
                    .any(|target| target.app_destination_hex == row.destination_hex)
            });
            for target in &targets {
                if publication
                    .recipients
                    .iter()
                    .any(|row| row.destination_hex == target.app_destination_hex)
                {
                    continue;
                }
                publication.recipients.push(TelemetryRecipient {
                    destination_hex: target.app_destination_hex.clone(),
                    callsign: position.callsign.clone(),
                    fix_updated_at_ms: position.updated_at_ms,
                    command_id: String::new(),
                    state: PublicationState::QueuedNoRoute,
                    message_id_hex: None,
                    representation: None,
                    detail: None,
                    updated_at_ms: now_ms(),
                    attempt_route: None,
                    custody_message_id_hex: None,
                });
            }
        })?;
        if !current_fix {
            return Ok(());
        }
        if !status.running {
            return Ok(());
        }
        let publication = app_state.telemetry_publication()?;
        if publication.fix.as_ref() != Some(&position) {
            return Ok(());
        }
        for row in publication.recipients {
            if row.custody_message_id_hex.is_some()
                || inflight.contains_key(&row.destination_hex)
                || row.fix_updated_at_ms != position.updated_at_ms
                || matches!(
                    row.state,
                    PublicationState::Acknowledged
                        | PublicationState::Rejected
                        | PublicationState::Transmitted
                        | PublicationState::TransportDelivered
                        | PublicationState::QueuedForDelivery
                )
            {
                continue;
            }
            let Some(target) = targets
                .iter()
                .find(|target| target.app_destination_hex == row.destination_hex)
            else {
                continue;
            };
            let peer = peers.iter().find(|peer| {
                crate::team_membership::destinations_include_peer(
                    std::slice::from_ref(&target.app_destination_hex),
                    &peer.destination_hex,
                    peer.lxmf_destination_hex.as_deref(),
                    peer.identity_hex.as_deref(),
                )
            });
            let route_ready = peer.is_some_and(|peer| peer.active_link || peer_has_current_known_lxmf_route(peer));
            let stored_route = saved.iter().any(|saved_peer| {
                crate::team_membership::destinations_include_peer(
                    std::slice::from_ref(&target.app_destination_hex), &saved_peer.destination_hex,
                    saved_peer.lxmf_destination_hex.as_deref(), saved_peer.identity_hex.as_deref(),
                ) && saved_peer.lxmf_destination_hex.as_deref().and_then(normalize_hex_32).is_some()
            });
            let offline = offline_saved_targets(&status, &peers, &saved, &[], true).iter()
                .any(|candidate| candidate.app_destination_hex == target.app_destination_hex);
            if !(route_ready || stored_route || matches!(target.send_mode, SendMode::PropagationOnly {}) || offline)
                || status.interfaces.is_empty() && !offline {
                continue;
            }
            let route = format!(
                "{:?}:{:?}:{}:{:?}:{:?}:{:?}",
                status
                    .interfaces
                    .iter()
                    .map(|interface| (&interface.interface_hex, &interface.state))
                    .collect::<Vec<_>>(),
                peer.map(|peer| (
                    peer.announce_last_seen_at_ms,
                    peer.lxmf_last_seen_at_ms,
                    peer.active_link
                )),
                saver,
                sync.active_propagation_node_hex,
                target.send_mode,
                saved
                    .iter()
                    .map(|peer| (&peer.destination_hex, &peer.lxmf_destination_hex))
                    .collect::<Vec<_>>()
            );
            if row.attempt_route.as_deref() == Some(&route) {
                continue;
            }
            let (body, fields) = build_telemetry_replication_payload(&position, target)?;
            let metadata =
                parse_mission_sync_metadata(&fields).ok_or(NodeError::InvalidConfig {})?;
            let command_id = metadata.command_id.ok_or(NodeError::InvalidConfig {})?;
            let mut queued = false;
            emit_telemetry_projection(&app_state, &bus, |publication| {
                if publication.fix.as_ref() != Some(&position) {
                    return;
                }
                if let Some(row) = publication.recipients.iter_mut().find(|row| {
                    row.destination_hex == target.app_destination_hex
                        && row.fix_updated_at_ms == position.updated_at_ms
                }) {
                    if row.custody_message_id_hex.is_some()
                        || matches!(
                            row.state,
                            PublicationState::Acknowledged | PublicationState::Rejected
                        )
                    {
                        return;
                    }
                    queued = true;
                    row.command_id = command_id.clone();
                    row.state = PublicationState::QueuedForDelivery;
                    row.attempt_route = Some(route);
                    row.updated_at_ms = now_ms();
                }
            })?;
            if !queued {
                continue;
            }
            match self.queue_bytes_with_class(
                target.app_destination_hex.clone(),
                body,
                Some(fields),
                target.send_mode,
                OutboundTrafficClass::Telemetry {},
            ) {
                Ok(response) => {
                    inflight.insert(
                        target.app_destination_hex.clone(),
                        (command_id, response, std::time::Instant::now()),
                    );
                }
                Err(error) => {
                    emit_telemetry_projection(&app_state, &bus, |publication| {
                        if let Some(row) = publication
                            .recipients
                            .iter_mut()
                            .find(|row| row.command_id == command_id)
                        {
                            row.state = PublicationState::QueuedNoRoute;
                            row.detail = Some(format!("{error}"));
                        }
                    })?;
                }
            }
        }
        Ok(())
    }
}

fn spawn_telemetry_publication_worker(runtime: &Runtime, node: std::sync::Weak<Mutex<NodeInner>>) {
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut inflight = HashMap::new();
        if let Some(inner) = node.upgrade() {
            if let Ok(inner) = inner.lock() {
                let _ = emit_telemetry_projection(&inner.app_state, &inner.bus, |publication| {
                    publication.recover_uncertain_sends();
                });
            }
        }
        loop {
            interval.tick().await;
            let Some(inner) = node.upgrade() else {
                break;
            };
            let node = Node { inner };
            if let Err(error) = node.flush_pending_telemetry(&mut inflight) {
                if let Ok(inner) = node.inner.lock() {
                    inner.bus.emit(NodeEvent::Error {
                        code: "TelemetryPublicationError".to_string(),
                        message: format!("telemetry recovery failed: {error}"),
                    });
                }
            }
        }
    });
}
