/// Uses the current telemetry recipient/team policy for R3S ingress as well as retries.
pub(crate) fn sensor_ingress_policy(
    store: &AppStateStore,
    status: &NodeStatus,
    config: &NodeConfig,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
    source: &str,
) -> Result<(bool, bool), NodeError> {
    let saved = store.get_saved_peers()?;
    let fingerprint = NodeConfigFingerprint::from_config(config)?;
    if destination_matches_peer_set(
        source,
        &store.get_ignored_peer_destinations()?,
        &saved,
        peers,
    ) {
        return Ok((false, false));
    }
    if store
        .get_app_settings()?
        .is_none_or(|s| !s.telemetry.enabled)
    {
        return Ok((false, false));
    }
    let relay = destination_is_configured_hub(source, Some(&fingerprint), peers, &saved)
        && matches!(
            effective_hub_mode(fingerprint.hub_mode, snapshot),
            HubMode::Connected {}
        )
        && snapshot.is_some_and(active_team_has_caller_membership);
    let targets = merge_replication_target_sets(
        build_runtime_telemetry_destinations(
            status,
            peers,
            &saved,
            None,
            Some(&fingerprint),
            snapshot,
        )?,
        explicit_telemetry_targets(store, status, &saved)?,
    );
    let allowed = relay
        || targets.iter().any(|t| {
            destination_matches_peer_set(
                source,
                std::slice::from_ref(&t.app_destination_hex),
                &saved,
                peers,
            )
        });
    Ok((allowed, relay))
}

fn sensor_targets(
    store: &AppStateStore,
    status: &NodeStatus,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    config: Option<&NodeConfigFingerprint>,
    snapshot: Option<&HubDirectorySnapshot>,
) -> Result<Vec<MissionReplicationTarget>, NodeError> {
    let mut targets = merge_replication_target_sets(
        build_runtime_telemetry_destinations(status, peers, saved, None, config, snapshot)?,
        explicit_telemetry_targets(store, status, saved)?,
    );
    if let Some(config) = config.filter(|c| {
        matches!(
            effective_hub_mode(c.hub_mode, snapshot),
            HubMode::Connected {}
        )
    }) {
        if snapshot.is_some_and(active_team_has_caller_membership) {
            targets = merge_replication_target_sets(
                targets,
                vec![MissionReplicationTarget {
                    app_destination_hex: configured_hub_destination(config)?,
                    send_mode: SendMode::Auto {},
                }],
            );
        }
    }
    Ok(targets)
}

fn sensor_hub_queue_authorized(
    inner: &NodeInner,
    destination: &str,
    fields: Option<&[u8]>,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
) -> Result<bool, NodeError> {
    let (Some(config), Some(fields)) = (inner.active_config.as_ref(), fields) else {
        return Ok(false);
    };
    let Some(metadata) = parse_mission_sync_metadata(fields)
        .filter(|m| m.command_type.as_deref() == Some("r3s.telemetry"))
    else {
        return Ok(false);
    };
    let Some(command) = metadata.command_id else {
        return Ok(false);
    };
    let status = inner
        .status
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    if !sensor_delivery_is_current_for_config(
        &inner.app_state,
        &status,
        config,
        peers,
        snapshot,
        destination,
        &command,
    )? {
        return Ok(false);
    }
    let Some(id) = command
        .strip_prefix("r3s:")
        .and_then(|s| s.split_once(':'))
        .map(|v| v.0)
    else {
        return Ok(false);
    };
    let Some(origin) = inner.app_state.sensor_origin(id)? else {
        return Ok(false);
    };
    let Some(current) = sensor_snapshot(&inner.app_state, origin)? else {
        return Ok(false);
    };
    Ok([true, false].into_iter().any(|full| {
        sensor_fields(&current, &command, full, active_team_uid(snapshot))
            .is_ok_and(|expected| expected == fields)
    }))
}
