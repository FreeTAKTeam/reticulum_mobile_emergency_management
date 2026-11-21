// RCH hub presence in semi-autonomous and connected mode (#366): REM
// announces when a hub-capable interface comes up, keeps a maintained Link
// to the hub's LXMF delivery destination, and lets DirectOnly sends wait
// for a requested path instead of failing at once.

const HUB_INTERFACE_POLL_INTERVAL: Duration = Duration::from_secs(5);
const HUB_INTERFACE_ANNOUNCE_MIN_GAP_MS: u64 = 60_000;
const DIRECT_PATH_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// The hub's LXMF delivery Link is maintained infrastructure in
/// semi-autonomous and connected mode, whatever the team membership.
fn hub_link_target(
    hub_mode: HubMode,
    hub_identity_hash: Option<&str>,
) -> Option<ManagedPeerLinkTarget> {
    if !matches!(hub_mode, HubMode::SemiAutonomous {} | HubMode::Connected {}) {
        return None;
    }
    Some(ManagedPeerLinkTarget {
        destination_hex: normalize_hex_32(hub_identity_hash?)?,
        kind: ManagedPeerLinkKind::LxmfDelivery,
    })
}

fn configured_hub_link_target(state: &NodeRuntimeState) -> Option<ManagedPeerLinkTarget> {
    hub_link_target(state.config.hub_mode, state.config.hub_identity_hash.as_deref())
}

fn is_hub_link_destination(state: &NodeRuntimeState, destination_hex: &str) -> bool {
    configured_hub_link_target(state).is_some_and(|target| {
        normalize_hex_32(destination_hex).as_deref() == Some(target.destination_hex.as_str())
    })
}

/// Announce when a relay-capable interface appears, at most once a minute so
/// a flapping TCP connection cannot flood the hub with announces.
fn hub_interface_announce_due(
    was_up: bool,
    is_up: bool,
    last_announce_ms: Option<u64>,
    now_ms: u64,
) -> bool {
    !was_up
        && is_up
        && last_announce_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= HUB_INTERFACE_ANNOUNCE_MIN_GAP_MS)
}

fn should_wait_for_direct_path(send_mode: SendMode, err: &NodeError) -> bool {
    matches!(send_mode, SendMode::DirectOnly {}) && matches!(err, NodeError::NetworkError {})
}

/// A DirectOnly send with no current route asks Reticulum for a path and
/// waits briefly for the path response before failing.
async fn await_direct_lxmf_route(
    state: &NodeRuntimeState,
    destination_hex: &str,
    require_current_peer: bool,
) -> Result<String, NodeError> {
    let resolved = resolve_lxmf_destination_hex(state, destination_hex).await;
    let destination = parse_address_hash(resolved.as_str())?;
    if output_link_is_active(state, &destination).await {
        return Ok(resolved);
    }
    info!(
        "[lxmf][mission] no current route destination={destination_hex} resolved={resolved}; requesting path wait_ms={}",
        DEFAULT_IDENTITY_WAIT_TIMEOUT.as_millis(),
    );
    state.transport.request_path(&destination, None, None).await;
    let deadline = tokio::time::Instant::now() + DEFAULT_IDENTITY_WAIT_TIMEOUT;
    loop {
        tokio::time::sleep(DIRECT_PATH_POLL_INTERVAL).await;
        let result =
            resolve_lxmf_destination_for_send(state, destination_hex, require_current_peer).await;
        if result.is_ok() || tokio::time::Instant::now() >= deadline {
            return result;
        }
    }
}

fn spawn_hub_interface_announcer(
    state: &NodeRuntimeState,
    app_destination: &Arc<TokioMutex<SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<AnnounceProfile>>,
    power_saver_rx: watch::Receiver<bool>,
) {
    if configured_hub_link_target(state).is_none() {
        return;
    }
    let state = state.clone();
    let app_destination = app_destination.clone();
    let announce_capabilities = announce_capabilities.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HUB_INTERFACE_POLL_INTERVAL);
        // Interfaces already up at start are covered by the startup burst.
        let mut was_up = has_active_relay_transport_interface(&state).await;
        let mut last_announce_ms = None;
        loop {
            interval.tick().await;
            let is_up = has_active_relay_transport_interface(&state).await;
            let now = now_ms();
            if !*power_saver_rx.borrow()
                && hub_interface_announce_due(was_up, is_up, last_announce_ms, now)
            {
                last_announce_ms = Some(now);
                announce_destinations(
                    &state.transport,
                    &app_destination,
                    &state.lxmf_destination,
                    &announce_capabilities,
                    "hub-interface-up",
                )
                .await;
            }
            was_up = is_up;
        }
    });
}
