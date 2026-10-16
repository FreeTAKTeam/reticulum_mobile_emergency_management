use crate::delivery_policy::normalize_hex_32;
use crate::types::{CircleTier, LocalTeamRecord, SavedPeerRecord};

pub(crate) fn destinations_include_peer(
    destinations: &[String],
    destination: &str,
    lxmf_destination: Option<&str>,
    identity: Option<&str>,
) -> bool {
    [Some(destination), lxmf_destination, identity]
        .into_iter()
        .flatten()
        .filter_map(normalize_hex_32)
        .any(|alias| {
            destinations
                .iter()
                .any(|member| normalize_hex_32(member).as_deref() == Some(alias.as_str()))
        })
}

pub(crate) fn saved_peer_is_local_team_member(
    local_teams: &[LocalTeamRecord],
    peer: &SavedPeerRecord,
) -> bool {
    local_teams.iter().any(|team| {
        destinations_include_peer(
            &team.member_destinations,
            &peer.destination_hex,
            peer.lxmf_destination_hex.as_deref(),
            peer.identity_hex.as_deref(),
        )
    })
}

pub(crate) fn apply_derived_circle_tiers(
    peers: &mut [SavedPeerRecord],
    local_teams: &[LocalTeamRecord],
) {
    for peer in peers {
        peer.circle_tier = if saved_peer_is_local_team_member(local_teams, peer) {
            CircleTier::Inner {}
        } else {
            CircleTier::Outer {}
        };
    }
}
