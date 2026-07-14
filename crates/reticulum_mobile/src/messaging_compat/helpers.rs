fn message_preview(body_utf8: &str) -> Option<String> {
    let trimmed = body_utf8.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(80).collect())
}

fn peer_display_name_for(peer: &PeerRecord) -> Option<String> {
    peer.display_name
        .clone()
        .or_else(|| peer.identity_hex.clone())
        .or_else(|| Some(peer.destination_hex.clone()))
}

fn normalize_hex(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn peer_is_stale(
    saved: bool,
    active_link: bool,
    announce_last_seen_at_ms: Option<u64>,
    now_ms: u64,
    stale_after_ms: u64,
) -> bool {
    if !saved || active_link {
        return false;
    }

    announce_last_seen_at_ms
        .is_some_and(|seen_at_ms| now_ms.saturating_sub(seen_at_ms) > stale_after_ms)
}

fn compatibility_peer_state(
    saved: bool,
    availability_state: PeerAvailabilityState,
    active_link: bool,
) -> PeerState {
    if active_link {
        return PeerState::Connected;
    }

    match availability_state {
        _ if !saved => PeerState::Disconnected,
        PeerAvailabilityState::Unseen => PeerState::Connecting,
        _ => PeerState::Disconnected,
    }
}

fn peer_availability_state(
    has_app_announce: bool,
    identity_hex: Option<&String>,
    lxmf_destination_hex: Option<&String>,
    stale: bool,
) -> PeerAvailabilityState {
    if identity_hex.is_some() && lxmf_destination_hex.is_some() {
        if !has_app_announce {
            return PeerAvailabilityState::Resolved;
        }
        return if stale {
            PeerAvailabilityState::Resolved
        } else {
            PeerAvailabilityState::Ready
        };
    }
    if has_app_announce || identity_hex.is_some() {
        return PeerAvailabilityState::Discovered;
    }
    PeerAvailabilityState::Unseen
}

fn peer_change_from_record(record: PeerRecord) -> PeerChange {
    PeerChange {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: record.state,
        saved: record.saved,
        stale: record.stale,
        active_link: record.active_link,
        last_error: record.last_resolution_error.clone(),
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}
