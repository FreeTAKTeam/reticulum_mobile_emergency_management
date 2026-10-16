pub(crate) fn maintained_link_destinations(
    app_state: &AppStateStore,
) -> Result<HashSet<String>, NodeError> {
    let teams = app_state
        .get_app_settings()?
        .map(|settings| settings.teams)
        .unwrap_or_default();
    let mut destinations = teams
        .local_teams
        .iter()
        .flat_map(|team| &team.member_destinations)
        .filter_map(|member| normalize_hex_32(member))
        .collect::<HashSet<_>>();
    destinations.extend(
        app_state
            .chat_correspondent_destinations()?
            .iter()
            .filter_map(|value| normalize_hex_32(value)),
    );
    Ok(destinations)
}

fn peer_has_maintained_link_membership(
    peer: &sdkmsg::PeerRecord,
    destinations: &HashSet<String>,
) -> bool {
    equivalent_peer_destinations(peer)
        .filter_map(normalize_hex_32)
        .any(|destination| destinations.contains(&destination))
}

async fn should_maintain_peer_link(state: &NodeRuntimeState, destination: &str) -> bool {
    let destinations = match maintained_link_destinations(&state.app_state) {
        Ok(destinations) => destinations,
        Err(error) => {
            log::warn!("link membership unavailable: {error}");
            return false;
        }
    };
    peer_for_any_destination_hex(state, destination)
        .await
        .is_some_and(|peer| peer_has_maintained_link_membership(&peer, &destinations))
}

async fn reconcile_team_link_targets(state: &NodeRuntimeState) -> Result<(), NodeError> {
    let destinations = maintained_link_destinations(&state.app_state)?;
    let peers = state.messaging.lock().await.list_peers();
    let desired = peers
        .iter()
        .filter(|peer| peer_has_maintained_link_membership(peer, &destinations))
        .filter_map(managed_peer_link_target)
        .collect::<Vec<_>>();
    let stale = state
        .managed_peer_links
        .desired_targets()
        .await
        .into_iter()
        .filter(|target| {
            !desired
                .iter()
                .any(|next| next.destination_hex == target.destination_hex)
        })
        .map(|target| target.destination_hex)
        .collect::<Vec<_>>();
    state
        .managed_peer_links
        .remove_desired(stale.iter().map(String::as_str))
        .await;
    for target in desired {
        state.managed_peer_links.add_desired(target).await;
    }
    Ok(())
}
