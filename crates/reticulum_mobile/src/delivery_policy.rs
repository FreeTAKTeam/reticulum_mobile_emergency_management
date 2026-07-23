use crate::messaging_compat as sdkmsg;
use crate::types;
use std::collections::HashSet;

pub(crate) fn normalize_hex_32(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() == 32 && trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

pub(crate) fn inbound_message_matches_destinations(
    message: &types::MessageRecord,
    destinations: &HashSet<String>,
) -> bool {
    if !matches!(message.direction, types::MessageDirection::Inbound {}) {
        return false;
    }

    [
        Some(message.conversation_id.as_str()),
        Some(message.destination_hex.as_str()),
        message.source_hex.as_deref(),
        message.requested_destination_hex.as_deref(),
        message.delivery_destination_hex.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(normalize_hex_32)
    .any(|destination| destinations.contains(destination.as_str()))
}

pub(crate) trait PeerDeliveryState {
    fn destination_hex(&self) -> &str;
    fn lxmf_destination_hex(&self) -> Option<&str>;
    fn active_link(&self) -> bool;
    fn connected_state(&self) -> bool;
    fn saved(&self) -> bool;
    fn stale(&self) -> bool;
    fn announce_last_seen_at_ms(&self) -> Option<u64>;
    fn lxmf_last_seen_at_ms(&self) -> Option<u64>;
}

impl PeerDeliveryState for types::PeerRecord {
    fn destination_hex(&self) -> &str {
        self.destination_hex.as_str()
    }

    fn lxmf_destination_hex(&self) -> Option<&str> {
        self.lxmf_destination_hex.as_deref()
    }

    fn active_link(&self) -> bool {
        self.active_link
    }

    fn connected_state(&self) -> bool {
        matches!(self.state, types::PeerState::Connected {})
    }

    fn saved(&self) -> bool {
        self.saved
    }

    fn stale(&self) -> bool {
        self.stale
    }

    fn announce_last_seen_at_ms(&self) -> Option<u64> {
        self.announce_last_seen_at_ms
    }

    fn lxmf_last_seen_at_ms(&self) -> Option<u64> {
        self.lxmf_last_seen_at_ms
    }
}

impl PeerDeliveryState for sdkmsg::PeerRecord {
    fn destination_hex(&self) -> &str {
        self.destination_hex.as_str()
    }

    fn lxmf_destination_hex(&self) -> Option<&str> {
        self.lxmf_destination_hex.as_deref()
    }

    fn active_link(&self) -> bool {
        self.active_link
    }

    fn connected_state(&self) -> bool {
        matches!(self.state, sdkmsg::PeerState::Connected)
    }

    fn saved(&self) -> bool {
        self.saved
    }

    fn stale(&self) -> bool {
        self.stale
    }

    fn announce_last_seen_at_ms(&self) -> Option<u64> {
        self.announce_last_seen_at_ms
    }

    fn lxmf_last_seen_at_ms(&self) -> Option<u64> {
        self.lxmf_last_seen_at_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PeerConnectivityModel {
    pub(crate) seen_recent: bool,
    pub(crate) saved: bool,
    pub(crate) connected_link: bool,
    pub(crate) desired_link: bool,
    pub(crate) direct_cooldown: bool,
    pub(crate) propagation_eligible: bool,
}

impl PeerConnectivityModel {
    #[cfg(test)]
    pub(crate) fn from_peer<P: PeerDeliveryState + ?Sized>(
        peer: &P,
        has_active_relay: bool,
        desired_link: bool,
        direct_cooldown: bool,
        now_ms: u64,
        stale_after_ms: u64,
    ) -> Self {
        Self::from_peer_with_saved(
            peer,
            peer.saved(),
            has_active_relay,
            desired_link,
            direct_cooldown,
            now_ms,
            stale_after_ms,
        )
    }

    pub(crate) fn from_peer_with_saved<P: PeerDeliveryState + ?Sized>(
        peer: &P,
        saved: bool,
        has_active_relay: bool,
        desired_link: bool,
        direct_cooldown: bool,
        now_ms: u64,
        stale_after_ms: u64,
    ) -> Self {
        Self {
            seen_recent: peer_is_current_replication_target(peer)
                || peer_has_observed_lxmf_delivery_route(peer, now_ms, stale_after_ms),
            saved,
            connected_link: peer_is_directly_reachable(peer),
            desired_link,
            direct_cooldown,
            propagation_eligible: has_active_relay && peer_has_known_lxmf_route(peer),
        }
    }

    pub(crate) fn direct_delivery_available(self) -> bool {
        self.connected_link && !self.direct_cooldown
    }

    pub(crate) fn stored_propagation_available(self) -> bool {
        self.saved && self.propagation_eligible
    }

    pub(crate) fn current_or_stored_route_available(self) -> bool {
        self.seen_recent || self.stored_propagation_available()
    }
}

pub(crate) fn peer_has_known_lxmf_route<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    if normalize_hex_32(peer.destination_hex()).is_none() {
        return false;
    }
    if peer
        .lxmf_destination_hex()
        .and_then(normalize_hex_32)
        .is_none()
    {
        return false;
    }
    true
}

pub(crate) fn peer_has_observed_lxmf_delivery_route<P: PeerDeliveryState + ?Sized>(
    peer: &P,
    now_ms: u64,
    stale_after_ms: u64,
) -> bool {
    peer_has_known_lxmf_route(peer)
        && peer
            .lxmf_last_seen_at_ms()
            .is_some_and(|seen_at_ms| now_ms.saturating_sub(seen_at_ms) <= stale_after_ms)
}

pub(crate) fn peer_is_directly_reachable<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    peer.active_link() && peer.connected_state()
}

pub(crate) fn peer_is_direct_delivery_ready<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    peer_is_directly_reachable(peer)
}

pub(crate) fn peer_is_current_replication_target<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    !peer.stale() && (peer.active_link() || peer.announce_last_seen_at_ms().is_some())
}

pub(crate) fn peer_has_current_known_lxmf_route<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    peer_is_current_replication_target(peer) && peer_has_known_lxmf_route(peer)
}

pub(crate) fn peer_can_use_propagation_fallback<P: PeerDeliveryState + ?Sized>(peer: &P) -> bool {
    peer_is_current_replication_target(peer) && peer_has_known_lxmf_route(peer)
}

pub(crate) fn saved_route_prefers_propagation<P: PeerDeliveryState + ?Sized>(
    peer: &P,
    has_active_relay: bool,
    direct_delivery_available: bool,
    direct_priority_hops: Option<u8>,
    direct_priority_free_hops: u8,
) -> bool {
    if !has_active_relay || !peer.saved() {
        return false;
    }
    if !direct_delivery_available {
        return direct_priority_hops.is_some_and(|hops| hops > direct_priority_free_hops);
    }
    peer_has_known_lxmf_route(peer) && !peer_is_direct_delivery_ready(peer)
        || direct_priority_hops.is_some_and(|hops| hops > direct_priority_free_hops)
            && peer_has_known_lxmf_route(peer)
            && !peer_is_directly_reachable(peer)
}

pub(crate) struct DirectAttemptBudget {
    pub send_mode: types::SendMode,
    pub has_active_relay: bool,
    pub can_try_stored_lxmf_route: bool,
    pub has_current_lxmf_route: bool,
    pub direct_delivery_ready: bool,
    pub direct_priority_hops: Option<u8>,
    pub direct_priority_free_hops: u8,
    pub lxmf_direct_attempts: usize,
}

pub(crate) fn direct_attempt_budget_for_send(input: DirectAttemptBudget) -> usize {
    if matches!(input.send_mode, types::SendMode::Auto {})
        && input.has_active_relay
        && input.can_try_stored_lxmf_route
        && !input.has_current_lxmf_route
        && !input.direct_delivery_ready
        && input
            .direct_priority_hops
            .is_some_and(|hops| hops > input.direct_priority_free_hops)
    {
        return 0;
    }

    input.lxmf_direct_attempts
}

#[cfg(test)]
mod tests {
    use super::{
        direct_attempt_budget_for_send, saved_route_prefers_propagation, DirectAttemptBudget,
        PeerConnectivityModel, PeerDeliveryState,
    };
    use crate::types;

    struct TestPeer {
        destination_hex: &'static str,
        lxmf_destination_hex: Option<&'static str>,
        active_link: bool,
        connected_state: bool,
        saved: bool,
        stale: bool,
        announce_last_seen_at_ms: Option<u64>,
        lxmf_last_seen_at_ms: Option<u64>,
    }

    impl PeerDeliveryState for TestPeer {
        fn destination_hex(&self) -> &str {
            self.destination_hex
        }

        fn lxmf_destination_hex(&self) -> Option<&str> {
            self.lxmf_destination_hex
        }

        fn active_link(&self) -> bool {
            self.active_link
        }

        fn connected_state(&self) -> bool {
            self.connected_state
        }

        fn saved(&self) -> bool {
            self.saved
        }

        fn stale(&self) -> bool {
            self.stale
        }

        fn announce_last_seen_at_ms(&self) -> Option<u64> {
            self.announce_last_seen_at_ms
        }

        fn lxmf_last_seen_at_ms(&self) -> Option<u64> {
            self.lxmf_last_seen_at_ms
        }
    }

    #[test]
    fn connectivity_model_derives_connected_reachable_cooldown_and_propagation() {
        let peer = TestPeer {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            lxmf_destination_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            active_link: true,
            connected_state: true,
            saved: true,
            stale: false,
            announce_last_seen_at_ms: Some(1_000),
            lxmf_last_seen_at_ms: Some(1_050),
        };

        let healthy = PeerConnectivityModel::from_peer(&peer, true, true, false, 1_100, 500);
        assert!(healthy.seen_recent);
        assert!(healthy.saved);
        assert!(healthy.connected_link);
        assert!(healthy.desired_link);
        assert!(!healthy.direct_cooldown);
        assert!(healthy.propagation_eligible);
        assert!(healthy.direct_delivery_available());
        assert!(healthy.stored_propagation_available());
        assert!(healthy.current_or_stored_route_available());

        let cooled_down = PeerConnectivityModel::from_peer(&peer, true, true, true, 1_100, 500);
        assert!(!cooled_down.direct_delivery_available());
        assert!(cooled_down.stored_propagation_available());
    }

    #[test]
    fn connectivity_model_allows_saved_override_for_persisted_peers() {
        let peer = TestPeer {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            lxmf_destination_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            active_link: false,
            connected_state: false,
            saved: false,
            stale: true,
            announce_last_seen_at_ms: None,
            lxmf_last_seen_at_ms: None,
        };

        let model =
            PeerConnectivityModel::from_peer_with_saved(&peer, true, true, true, false, 2_000, 500);

        assert!(!model.seen_recent);
        assert!(model.saved);
        assert!(!model.direct_delivery_available());
        assert!(model.stored_propagation_available());
        assert!(model.current_or_stored_route_available());
    }

    #[test]
    fn current_saved_lxmf_route_does_not_prefer_propagation_without_priority_hops() {
        let peer = TestPeer {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            lxmf_destination_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            active_link: false,
            connected_state: false,
            saved: true,
            stale: false,
            announce_last_seen_at_ms: Some(1_000),
            lxmf_last_seen_at_ms: Some(1_050),
        };

        assert!(!saved_route_prefers_propagation(
            &peer, true, false, None, 2
        ));
    }

    #[test]
    fn auto_send_keeps_direct_budget_for_current_saved_lxmf_route() {
        assert_eq!(
            direct_attempt_budget_for_send(DirectAttemptBudget {
                send_mode: types::SendMode::Auto {},
                has_active_relay: true,
                can_try_stored_lxmf_route: true,
                has_current_lxmf_route: true,
                direct_delivery_ready: false,
                direct_priority_hops: None,
                direct_priority_free_hops: 2,
                lxmf_direct_attempts: 3,
            }),
            3,
        );
    }

    #[test]
    fn saved_route_without_priority_keeps_direct_lane_when_current_route_is_stale() {
        let peer = TestPeer {
            destination_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            lxmf_destination_hex: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            active_link: false,
            connected_state: false,
            saved: true,
            stale: true,
            announce_last_seen_at_ms: None,
            lxmf_last_seen_at_ms: None,
        };

        assert!(!saved_route_prefers_propagation(
            &peer, true, false, None, 2
        ));
        assert_eq!(
            direct_attempt_budget_for_send(DirectAttemptBudget {
                send_mode: types::SendMode::Auto {},
                has_active_relay: true,
                can_try_stored_lxmf_route: true,
                has_current_lxmf_route: false,
                direct_delivery_ready: false,
                direct_priority_hops: None,
                direct_priority_free_hops: 2,
                lxmf_direct_attempts: 3,
            },),
            3,
        );
    }
}
