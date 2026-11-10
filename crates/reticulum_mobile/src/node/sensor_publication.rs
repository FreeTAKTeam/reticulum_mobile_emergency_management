struct SensorPending {
    origin: String,
    destination: String,
    response: cb::Receiver<Result<(), NodeError>>,
    started: std::time::Instant,
}
type SensorInflight = HashMap<String, SensorPending>;
impl Node {
    fn flush_sensor_telemetry(&self, inflight: &mut SensorInflight) -> Result<(), NodeError> {
        let (store, status, peers, config, hub_snapshot, saver, running) = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
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
            let snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|_| NodeError::InternalError {})?
                .clone();
            (
                inner.app_state.clone(),
                status,
                peers,
                inner.active_config.clone(),
                snapshot,
                inner.power_state.saver_active,
                inner.cmd_tx.is_some(),
            )
        };
        let mut done = Vec::new();
        for (key, pending) in inflight.iter() {
            match pending.response.try_recv() {
                Ok(Ok(())) => {
                    done.push(key.clone());
                }
                Ok(Err(_)) | Err(cb::TryRecvError::Disconnected) => {
                    store.retry_sensor_attempt(&pending.origin, &pending.destination)?;
                    done.push(key.clone());
                }
                Err(cb::TryRecvError::Empty)
                    if pending.started.elapsed() >= SEND_COMMAND_TIMEOUT =>
                {
                    store.retry_sensor_attempt(&pending.origin, &pending.destination)?;
                    done.push(key.clone());
                }
                Err(cb::TryRecvError::Empty) => {}
            }
        }
        for key in done {
            inflight.remove(&key);
        }
        if !running
            || !status.running
            || store
                .get_app_settings()?
                .is_none_or(|s| !s.telemetry.enabled)
        {
            return Ok(());
        }
        let saved = store.get_saved_peers()?;
        let targets = sensor_targets(
            &store,
            &status,
            &peers,
            &saved,
            config.as_ref(),
            hub_snapshot.as_ref(),
        )?;
        let scope = sensor_scope(config.as_ref(), hub_snapshot.as_ref());
        let route = sensor_route(&status, &peers)?;
        // A mixed/unknown radio route uses the conservative cadence until a route is known.
        let cadence = if saver {
            300_000
        } else if status.interfaces.is_empty()
            || status
                .interfaces
                .iter()
                .any(|i| i.kind.starts_with("rnode"))
        {
            30_000
        } else {
            1_000
        };
        let ignored = store.get_ignored_peer_destinations()?;
        let mut queued = 0;
        for origin in store.local_sensor_origins()? {
            let Some(snapshot) = sensor_snapshot(&store, origin)? else {
                continue;
            };
            let metadata_hash = sensor_metadata_hash(&snapshot)?;
            for target in &targets {
                if queued >= 16 {
                    return Ok(());
                }
                if destination_matches_peer_set(
                    &target.app_destination_hex,
                    &ignored,
                    &saved,
                    &peers,
                ) {
                    continue;
                }
                let key = format!(
                    "{}:{}",
                    snapshot.origin.origin_id, target.app_destination_hex
                );
                if inflight.contains_key(&key) {
                    continue;
                }
                let command =
                    sensor_command(&snapshot, &target.app_destination_hex, &scope, &route)?;
                let prior = store.sensor_publication_attempt(
                    &snapshot.origin.origin_id,
                    &target.app_destination_hex,
                )?;
                if now_ms().saturating_sub(prior.attempted_at_ms) < cadence {
                    continue;
                }
                // Keep the runtime's receipt tracking until a terminal event or a newer snapshot.
                if prior.command_id == command && prior.route == route {
                    continue;
                }
                let fields = sensor_fields(
                    &snapshot,
                    &command,
                    prior.metadata_hash != metadata_hash || prior.route != route,
                    active_team_uid(hub_snapshot.as_ref()),
                )?;
                store.record_sensor_attempt(
                    &snapshot.origin.origin_id,
                    &target.app_destination_hex,
                    &command,
                    &route,
                    &metadata_hash,
                )?;
                let response = match self.queue_bytes_with_class(
                    target.app_destination_hex.clone(),
                    b"R3S/1".to_vec(),
                    Some(fields),
                    target.send_mode,
                    OutboundTrafficClass::Telemetry {},
                ) {
                    Ok(response) => response,
                    Err(error) => {
                        store.retry_sensor_attempt(
                            &snapshot.origin.origin_id,
                            &target.app_destination_hex,
                        )?;
                        return Err(error);
                    }
                };
                inflight.insert(
                    key,
                    SensorPending {
                        origin: snapshot.origin.origin_id.clone(),
                        destination: target.app_destination_hex.clone(),
                        response,
                        started: std::time::Instant::now(),
                    },
                );
                queued += 1;
            }
        }
        Ok(())
    }
}
fn sensor_delivery_is_current(
    store: &AppStateStore,
    status: &NodeStatus,
    config: &NodeConfig,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
    destination: &str,
    command: &str,
) -> Result<bool, NodeError> {
    sensor_delivery_is_current_for_config(
        store,
        status,
        &NodeConfigFingerprint::from_config(config)?,
        peers,
        snapshot,
        destination,
        command,
    )
}
fn sensor_delivery_is_current_for_config(
    store: &AppStateStore,
    status: &NodeStatus,
    config: &NodeConfigFingerprint,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
    destination: &str,
    command: &str,
) -> Result<bool, NodeError> {
    let Some(id) = command
        .strip_prefix("r3s:")
        .and_then(|s| s.split_once(':'))
        .map(|v| v.0)
    else {
        return Ok(false);
    };
    if store
        .get_app_settings()?
        .is_none_or(|s| !s.telemetry.enabled)
    {
        return Ok(false);
    }
    let saved = store.get_saved_peers()?;
    if destination_matches_peer_set(
        destination,
        &store.get_ignored_peer_destinations()?,
        &saved,
        peers,
    ) {
        return Ok(false);
    }
    let targets = sensor_targets(store, status, peers, &saved, Some(config), snapshot)?;
    let Some(target) = targets.iter().find(|t| {
        destination_matches_peer_set(
            destination,
            std::slice::from_ref(&t.app_destination_hex),
            &saved,
            peers,
        )
    }) else {
        return Ok(false);
    };
    let Some(origin) = store.sensor_origin(id)? else {
        return Ok(false);
    };
    let Some(current) = sensor_snapshot(store, origin)? else {
        return Ok(false);
    };
    Ok(sensor_command(
        &current,
        &target.app_destination_hex,
        &sensor_scope(Some(config), snapshot),
        &sensor_route(status, peers)?,
    )? == command)
}
