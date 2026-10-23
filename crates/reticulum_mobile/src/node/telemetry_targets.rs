// Persisted contacts are authoritative; a removed contact cannot survive in a live snapshot.
fn telemetry_peers_with_saved(
    peers: &[PeerRecord],
    saved_peers: &[SavedPeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
) -> Vec<PeerRecord> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    let members = snapshot
        .map(active_local_team_destinations)
        .unwrap_or_default();
    for saved in saved_peers {
        let aliases = [
            Some(saved.destination_hex.as_str()),
            saved.lxmf_destination_hex.as_deref(),
            saved.identity_hex.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(normalize_hex_32)
        .collect::<Vec<_>>();
        let live = peers.iter().find(|peer| {
            crate::team_membership::destinations_include_peer(
                &aliases,
                &peer.destination_hex,
                peer.lxmf_destination_hex.as_deref(),
                peer.identity_hex.as_deref(),
            )
        });
        let destination = live
            .and_then(|peer| normalize_hex_32(&peer.destination_hex))
            .or_else(|| normalize_hex_32(&saved.destination_hex));
        let Some(destination_hex) = destination else {
            continue;
        };
        let lxmf = live
            .and_then(|peer| peer.lxmf_destination_hex.as_deref())
            .or(saved.lxmf_destination_hex.as_deref())
            .and_then(normalize_hex_32);
        let key = lxmf.clone().unwrap_or_else(|| destination_hex.clone());

        let mut peer = live.cloned().unwrap_or(PeerRecord {
            destination_hex: destination_hex.clone(),
            identity_hex: saved.identity_hex.clone(),
            lxmf_destination_hex: lxmf.clone(),
            display_name: saved.display_name.clone(),
            app_data: saved.app_data.clone(),
            state: crate::types::PeerState::Disconnected {},
            saved: true,
            stale: true,
            active_link: false,
            hub_derived: false,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: None,
            last_seen_at_ms: saved.last_route_seen_at_ms.unwrap_or(0),
            announce_last_seen_at_ms: None,
            lxmf_last_seen_at_ms: None,
        });
        peer.destination_hex = destination_hex;
        peer.lxmf_destination_hex = lxmf;
        peer.saved = true;
        // A current explicit capability list overrides persisted capabilities, including revocation.
        if peer.app_data.is_none() {
            peer.app_data = saved.app_data.clone();
        }
        if !crate::team_membership::destinations_include_peer(
            &members,
            &peer.destination_hex,
            peer.lxmf_destination_hex.as_deref(),
            peer.identity_hex.as_deref(),
        ) || !seen.insert(key)
        {
            continue;
        }
        merged.push(peer);
    }
    merged
}

fn ensure_telemetry_destination_authorized(
    inner: &NodeInner,
    class: OutboundTrafficClass,
    destination: &str,
    peers: &[PeerRecord],
    saved: &[SavedPeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
) -> Result<(), NodeError> {
    if !matches!(class, OutboundTrafficClass::Telemetry {}) {
        return Ok(());
    }
    let status = inner
        .status
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    let sync = inner
        .sync_status_snapshot
        .lock()
        .map_err(|_| NodeError::InternalError {})?
        .clone();
    let targets = build_runtime_telemetry_destinations(
        &status,
        peers,
        saved,
        sync.active_propagation_node_hex.as_deref(),
        inner.active_config.as_ref(),
        snapshot,
    )?;
    if targets.iter().any(|target| {
        destination_matches_peer_set(
            destination,
            &[target.app_destination_hex.clone()],
            saved,
            peers,
        )
    }) {
        Ok(())
    } else {
        Err(NodeError::InvalidConfig {})
    }
}

pub(crate) fn telemetry_delivery_is_current(
    app_state: &AppStateStore,
    status: &NodeStatus,
    config: &NodeConfig,
    peers: &[PeerRecord],
    snapshot: Option<&HubDirectorySnapshot>,
    request: (&str, &str),
) -> Result<bool, NodeError> {
    let publication = app_state.telemetry_publication()?;
    let Some(fix) = publication.fix.as_ref() else {
        return Ok(false);
    };
    if app_state
        .get_app_settings()?
        .as_ref()
        .is_some_and(|settings| {
            !settings.telemetry.enabled
                || now_ms().saturating_sub(fix.updated_at_ms)
                    > u64::from(settings.telemetry.expire_after_minutes) * 60_000
        })
    {
        return Ok(false);
    }
    let Some(row) = publication.recipients.iter().find(|row| {
        row.command_id == request.1
            && row.fix_updated_at_ms == fix.updated_at_ms
            && !matches!(
                row.state,
                PublicationState::Acknowledged | PublicationState::Rejected
            )
    }) else {
        return Ok(false);
    };
    let expected_target = MissionReplicationTarget {
        app_destination_hex: row.destination_hex.clone(),
        send_mode: SendMode::Auto {},
    };
    let (_, fields) = build_telemetry_replication_payload(fix, &expected_target)?;
    if parse_mission_sync_metadata(&fields)
        .and_then(|metadata| metadata.command_id)
        .as_deref()
        != Some(request.1)
    {
        return Ok(false);
    }
    let saved = app_state.get_saved_peers()?;
    let config = NodeConfigFingerprint::from_config(config)?;
    Ok(
        build_runtime_telemetry_destinations(status, peers, &saved, None, Some(&config), snapshot)?
            .iter()
            .any(|target| {
                destination_matches_peer_set(
                    request.0,
                    &[target.app_destination_hex.clone()],
                    &saved,
                    peers,
                )
            }),
    )
}
