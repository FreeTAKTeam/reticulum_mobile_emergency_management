fn projection_journal_path(storage_dir: Option<&str>) -> Option<PathBuf> {
    storage_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|dir| PathBuf::from(dir).join(runtime_projection::PERSIST_FILENAME))
}

struct RestoredSavedPeerManagement {
    route_request_destinations: Vec<String>,
    link_targets: Vec<ManagedPeerLinkTarget>,
    pruned_destinations: Vec<String>,
}

fn record_saved_peer_profile(messaging: &mut sdkmsg::MessagingStore, peer: &SavedPeerRecord) {
    messaging.record_saved_peer_profile(sdkmsg::SavedPeerProfileInput {
        destination_hex: peer.destination_hex.as_str(),
        identity_hex: peer.identity_hex.as_deref(),
        lxmf_destination_hex: peer.lxmf_destination_hex.as_deref(),
        app_data: peer.app_data.as_deref(),
        display_name: peer.display_name.as_deref().or(peer.label.as_deref()),
        last_route_seen_at_ms: peer.last_route_seen_at_ms,
    });
}

fn saved_peer_matches_selected_destination(
    peer: &SavedPeerRecord,
    selected_hex: &str,
    selected_identity_hex: Option<&str>,
) -> bool {
    peer.destination_hex.eq_ignore_ascii_case(selected_hex)
        || peer
            .lxmf_destination_hex
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(selected_hex))
        || selected_identity_hex.is_some_and(|identity_hex| {
            peer.identity_hex
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(identity_hex))
        })
}

async fn selected_saved_peer_record(
    state: &NodeRuntimeState,
    selected_destination_hex: &str,
) -> Result<SavedPeerRecord, NodeError> {
    let normalized_selected =
        normalize_hex_32(selected_destination_hex).ok_or(NodeError::InvalidConfig {})?;
    let current_peer = peer_for_any_destination_hex(state, normalized_selected.as_str()).await;
    let selected_identity_hex = current_peer
        .as_ref()
        .and_then(|peer| peer.identity_hex.as_deref());
    let destination_hex = current_peer
        .as_ref()
        .map(|peer| peer.destination_hex.clone())
        .unwrap_or_else(|| normalized_selected.clone());
    let mut existing = state.app_state.get_saved_peers()?.into_iter().find(|peer| {
        saved_peer_matches_selected_destination(
            peer,
            normalized_selected.as_str(),
            selected_identity_hex,
        ) || peer
            .destination_hex
            .eq_ignore_ascii_case(destination_hex.as_str())
    });
    let now = now_ms();
    let mut record = existing.take().unwrap_or(SavedPeerRecord {
        destination_hex: destination_hex.clone(),
        label: None,
        saved_at_ms: now,
        identity_hex: None,
        lxmf_destination_hex: None,
        app_data: None,
        display_name: None,
        last_route_seen_at_ms: None,
        last_hops: None,
        circle_tier: CircleTier::Inner {},
    });
    record.destination_hex = destination_hex;
    record.saved_at_ms = now;
    if let Some(peer) = current_peer {
        record.identity_hex = peer.identity_hex.or(record.identity_hex);
        record.lxmf_destination_hex = peer
            .lxmf_destination_hex
            .or(record.lxmf_destination_hex)
            .or(Some(normalized_selected));
        record.app_data = peer.app_data.or(record.app_data);
        record.display_name = peer.display_name.or(record.display_name);
        record.last_route_seen_at_ms = peer
            .lxmf_last_seen_at_ms
            .or(peer.announce_last_seen_at_ms)
            .or(record.last_route_seen_at_ms);
    } else if record.lxmf_destination_hex.is_none() {
        record.lxmf_destination_hex = Some(normalized_selected);
    }
    Ok(record)
}

async fn persist_selected_peer_destination(
    state: &NodeRuntimeState,
    bus: &EventBus,
    selected_destination_hex: &str,
) -> Result<SavedPeerRecord, NodeError> {
    let peer = selected_saved_peer_record(state, selected_destination_hex).await?;
    let invalidation = state.app_state.upsert_saved_peer(&peer)?;
    bus.emit(NodeEvent::ProjectionInvalidated { invalidation });
    {
        let mut messaging = state.messaging.lock().await;
        messaging.mark_peer_saved(peer.destination_hex.as_str(), true);
        record_saved_peer_profile(&mut messaging, &peer);
    }
    Ok(peer)
}

fn restore_saved_peer_management(
    messaging: &mut sdkmsg::MessagingStore,
    saved_peers: &[SavedPeerRecord],
) -> RestoredSavedPeerManagement {
    let mut restored_destinations = Vec::new();
    let mut seen_destinations = HashSet::new();
    for peer in saved_peers {
        let Some(destination_hex) = normalize_hex_32(peer.destination_hex.as_str()) else {
            continue;
        };
        if !seen_destinations.insert(destination_hex.clone()) {
            continue;
        }
        messaging.mark_peer_saved(destination_hex.as_str(), true);
        record_saved_peer_profile(messaging, peer);
        restored_destinations.push(destination_hex);
    }
    let pruned_destinations = messaging.prune_saved_destinations_with_non_rem_announce_evidence();
    if !pruned_destinations.is_empty() {
        let pruned_set = pruned_destinations.iter().collect::<HashSet<_>>();
        restored_destinations.retain(|destination| !pruned_set.contains(destination));
    }
    let mut link_targets = Vec::new();
    let mut seen_link_targets = HashSet::new();
    for destination_hex in &restored_destinations {
        if let Some(target) = messaging
            .peer_by_destination(destination_hex.as_str())
            .and_then(|peer| managed_peer_link_target(&peer))
            .filter(|target| seen_link_targets.insert(target.destination_hex.clone()))
        {
            link_targets.push(target);
        }
    }
    RestoredSavedPeerManagement {
        route_request_destinations: restored_destinations,
        link_targets,
        pruned_destinations,
    }
}

fn normalized_saved_peer_destinations(saved_peers: &[SavedPeerRecord]) -> Vec<String> {
    let mut destinations = saved_peers
        .iter()
        .filter_map(|peer| normalize_hex_32(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    destinations.sort();
    destinations.dedup();
    destinations
}

fn normalized_unique_destinations(destinations: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut normalized = destinations
        .into_iter()
        .filter_map(|destination| normalize_hex_32(destination.as_str()))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

async fn mark_peer_destinations_ignored(state: &NodeRuntimeState, destinations: &[String]) {
    let destinations = normalized_unique_destinations(destinations.iter().cloned());
    if destinations.is_empty() {
        return;
    }
    {
        let mut ignored = state.ignored_peer_destinations.lock().await;
        ignored.extend(destinations.iter().cloned());
    }
    if let Err(err) = state.app_state.add_ignored_peer_destinations(&destinations) {
        debug!(
            "[peers] failed to persist ignored destinations={} reason={}",
            destinations.join(","),
            err,
        );
    }
}

async fn clear_ignored_peer_destinations(state: &NodeRuntimeState, destinations: &[String]) {
    let destinations = normalized_unique_destinations(destinations.iter().cloned());
    if destinations.is_empty() {
        return;
    }
    {
        let mut ignored = state.ignored_peer_destinations.lock().await;
        for destination in &destinations {
            ignored.remove(destination);
        }
    }
    if let Err(err) = state
        .app_state
        .remove_ignored_peer_destinations(&destinations)
    {
        debug!(
            "[peers] failed to clear ignored destinations={} reason={}",
            destinations.join(","),
            err,
        );
    }
}

async fn peer_destinations_are_ignored(
    state: &NodeRuntimeState,
    destinations: impl IntoIterator<Item = String>,
) -> bool {
    let destinations = normalized_unique_destinations(destinations);
    if destinations.is_empty() {
        return false;
    }
    let ignored = state.ignored_peer_destinations.lock().await;
    destinations
        .iter()
        .any(|destination| ignored.contains(destination))
}

async fn apply_saved_peer_management_projection(
    state: &NodeRuntimeState,
    bus: &EventBus,
    saved_peers: &[SavedPeerRecord],
) -> Result<(), NodeError> {
    let desired_destinations = normalized_saved_peer_destinations(saved_peers);
    let desired_set = desired_destinations.iter().cloned().collect::<HashSet<_>>();
    clear_ignored_peer_destinations(state, desired_destinations.as_slice()).await;

    let (cleanup_destinations, changed_destinations, desired_targets) = {
        let mut messaging = state.messaging.lock().await;
        let previous_saved = messaging.saved_destination_hexes();
        let previous_saved_set = previous_saved.iter().cloned().collect::<HashSet<_>>();
        let removed_saved = previous_saved_set
            .difference(&desired_set)
            .cloned()
            .collect::<HashSet<_>>();
        let previous_peers = messaging.list_peers();
        let mut cleanup_destinations = removed_saved.clone();

        for peer in &previous_peers {
            let equivalents = equivalent_peer_destinations(peer)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let removed_match = equivalents
                .iter()
                .any(|destination| removed_saved.contains(destination));
            let still_desired_match = equivalents
                .iter()
                .any(|destination| desired_set.contains(destination));
            if removed_match && !still_desired_match {
                cleanup_destinations.extend(equivalents);
            }
        }

        let (added, removed) =
            messaging.replace_saved_destinations(desired_destinations.iter().map(String::as_str));
        for peer in saved_peers {
            record_saved_peer_profile(&mut messaging, peer);
        }
        let now = now_ms();
        for destination in &cleanup_destinations {
            if desired_set.contains(destination) {
                continue;
            }
            messaging.mark_peer_saved(destination, false);
            messaging.set_peer_active_link(destination, false, now);
        }

        let mut changed_destinations = cleanup_destinations.iter().cloned().collect::<Vec<_>>();
        changed_destinations.extend(added);
        changed_destinations.extend(removed);
        changed_destinations.sort();
        changed_destinations.dedup();

        let mut seen_targets = HashSet::<String>::new();
        let desired_targets = messaging
            .list_peers()
            .into_iter()
            .filter(|peer| peer.saved)
            .filter_map(|peer| managed_peer_link_target(&peer))
            .filter(|target| seen_targets.insert(target.destination_hex.clone()))
            .collect::<Vec<_>>();

        let mut cleanup_destinations = cleanup_destinations.into_iter().collect::<Vec<_>>();
        cleanup_destinations.sort();
        cleanup_destinations.dedup();

        (cleanup_destinations, changed_destinations, desired_targets)
    };

    cleanup_removed_saved_destinations(state, cleanup_destinations.as_slice()).await;
    mark_peer_destinations_ignored(state, cleanup_destinations.as_slice()).await;

    for target in desired_targets {
        add_desired_managed_peer_link_and_schedule(state, bus, target, "saved-peers-updated").await;
    }

    for destination in &changed_destinations {
        emit_peer_changed(state, bus, destination).await;
    }
    sync_auto_propagation_node(state, bus).await;
    Ok(())
}

async fn cleanup_removed_saved_destinations(state: &NodeRuntimeState, destinations: &[String]) {
    if destinations.is_empty() {
        return;
    }
    state
        .direct_delivery_health
        .clear(destinations.iter().map(String::as_str));
    state
        .managed_peer_links
        .remove_desired(destinations.iter().map(String::as_str))
        .await;
    let links_to_close = {
        let mut connected = state.connected_peers.lock().await;
        let mut out_links = state.out_links.lock().await;
        let mut links_to_close = Vec::new();
        for destination in destinations {
            let Ok(address_hash) = parse_address_hash(destination.as_str()) else {
                continue;
            };
            connected.remove(&address_hash);
            if let Some(link) = out_links.remove(&address_hash) {
                links_to_close.push(link);
            }
        }
        links_to_close
    };
    for link in links_to_close {
        link.lock().await.close();
    }
}

async fn seed_runtime_projection_snapshot(
    state: &NodeRuntimeState,
    snapshot: &runtime_projection::RuntimeProjectionSnapshot,
) {
    let sync_status = snapshot.sync_status();
    *state.active_propagation_node_hex.lock().await =
        sync_status.active_propagation_node_hex.clone();
    let mut messaging = state.messaging.lock().await;
    messaging.update_sync_status(|current| {
        if let Some(sdk_sync_status) = to_sdk_sync_status(sync_status.clone()) {
            *current = sdk_sync_status;
        }
    });
    // Saved peer management survives restart, but availability and "seen"
    // timestamps must be rebuilt from fresh announces after startup.
    for peer in snapshot.restored_peers() {
        messaging.mark_peer_saved(peer.destination_hex.as_str(), peer.saved);
        messaging.record_resolution_error(
            peer.destination_hex.as_str(),
            peer.last_resolution_error.clone(),
        );
    }
    for message in snapshot.messages() {
        messaging.upsert_message(to_sdk_message_record(message));
    }
}

fn sdk_peer_is_directly_reachable(peer: &sdkmsg::PeerRecord) -> bool {
    peer.active_link && matches!(peer.state, sdkmsg::PeerState::Connected)
}

fn sdk_peer_has_known_delivery_route(peer: &sdkmsg::PeerRecord) -> bool {
    peer.identity_hex
        .as_deref()
        .and_then(normalize_hex_32)
        .is_some()
        && sdk_peer_has_known_lxmf_route(peer)
}

fn mark_peer_link_state(
    messaging: &mut sdkmsg::MessagingStore,
    link_destination_hex: &str,
    canonical_destination_hex: &str,
    active: bool,
    changed_at_ms: u64,
) {
    messaging.set_peer_active_link(link_destination_hex, active, changed_at_ms);
    if link_destination_hex != canonical_destination_hex {
        messaging.set_peer_active_link(canonical_destination_hex, active, changed_at_ms);
    }
}

fn mark_peer_active_after_successful_link(
    messaging: &mut sdkmsg::MessagingStore,
    link_destination_hex: &str,
    canonical_destination_hex: &str,
    changed_at_ms: u64,
) {
    mark_peer_link_state(
        messaging,
        link_destination_hex,
        canonical_destination_hex,
        true,
        changed_at_ms,
    );
}

async fn record_peer_link_state(
    state: &NodeRuntimeState,
    bus: &EventBus,
    link_destination_hex: &str,
    active: bool,
) {
    let canonical_destination_hex =
        canonical_app_destination_hex(state, link_destination_hex).await;
    if active {
        clear_peer_direct_delivery_unhealthy(
            state,
            link_destination_hex,
            Some(canonical_destination_hex.as_str()),
        )
        .await;
    }
    let change = {
        let mut messaging = state.messaging.lock().await;
        if active {
            mark_peer_active_after_successful_link(
                &mut messaging,
                link_destination_hex,
                canonical_destination_hex.as_str(),
                now_ms(),
            );
        } else {
            mark_peer_link_state(
                &mut messaging,
                link_destination_hex,
                canonical_destination_hex.as_str(),
                false,
                now_ms(),
            );
        }
        messaging
            .peer_change_for_destination(canonical_destination_hex.as_str())
            .map(from_sdk_peer_change)
    };
    if let Some(change) = change.as_ref() {
        debug!(
            "[peers][link-state] link_destination={} canonical_destination={} active={} projected_destination={} state={:?} saved={} stale={} active_link={} identity={} lxmf={} announce_seen={} lxmf_seen={} last_error={}",
            link_destination_hex,
            canonical_destination_hex,
            active,
            change.destination_hex,
            change.state,
            change.saved,
            change.stale,
            change.active_link,
            change.identity_hex.as_deref().unwrap_or("-"),
            change.lxmf_destination_hex.as_deref().unwrap_or("-"),
            change
                .announce_last_seen_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            change
                .lxmf_last_seen_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            change.last_error.as_deref().unwrap_or("-"),
        );
    } else {
        debug!(
            "[peers][link-state] link_destination={link_destination_hex} canonical_destination={canonical_destination_hex} active={active} projected_destination=- state=missing",
        );
    }
    if let Some(change) = change {
        state.sdk.record_peer_changed(
            &change.destination_hex,
            change.state,
            change.last_error.as_deref(),
        );
    }
    emit_peer_changed(state, bus, canonical_destination_hex.as_str()).await;
    sync_auto_propagation_node(state, bus).await;
}
