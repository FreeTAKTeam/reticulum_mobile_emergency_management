impl MessagingStore {
    pub fn list_peers(&self) -> Vec<PeerRecord> {
        let now_ms = current_time_ms();
        let mut candidate_destinations = self.mission_announce_destinations.clone();
        for saved_destination in &self.saved_destinations {
            let saved_profile = self.saved_profile_for_destination(saved_destination);
            let canonical_destination = self
                .announce_records
                .get(saved_destination)
                .filter(|record| record.destination_kind == "app")
                .map(|record| record.identity_hex.as_str())
                .or_else(|| {
                    self.resolved_app_identity_by_destination
                        .get(saved_destination)
                        .map(String::as_str)
                })
                .and_then(|identity| {
                    self.announced_or_resolved_destination("lxmf_delivery", identity)
                })
                .filter(|destination| {
                    self.mission_announce_destinations
                        .contains(destination.as_str())
                })
                .cloned()
                .or_else(|| {
                    saved_profile
                        .filter(|profile| supports_mission_traffic(profile.app_data.as_deref()))
                        .and_then(|profile| profile.lxmf_destination_hex.clone())
                })
                .unwrap_or_else(|| saved_destination.clone());
            candidate_destinations.insert(canonical_destination);
        }

        let mut peers = Vec::<PeerRecord>::new();
        let mut projected_destinations = HashSet::<String>::new();
        for destination_hex in candidate_destinations {
            if !projected_destinations.insert(destination_hex.clone()) {
                continue;
            }
            let lxmf_record = self.announce_records.get(&destination_hex).filter(|_| {
                self.mission_announce_destinations
                    .contains(&destination_hex)
            });
            let saved_profile = self.saved_profile_for_destination(destination_hex.as_str());
            let identity_hex = lxmf_record
                .map(|record| record.identity_hex.clone())
                .or_else(|| saved_profile.and_then(|profile| profile.identity_hex.clone()))
                .or_else(|| {
                    self.announce_records
                        .get(&destination_hex)
                        .filter(|record| record.destination_kind == "app")
                        .map(|record| record.identity_hex.clone())
                })
                .or_else(|| {
                    self.resolved_app_identity_by_destination
                        .get(&destination_hex)
                        .cloned()
                });
            let app_record = identity_hex
                .as_ref()
                .and_then(|identity| self.announced_or_resolved_destination("app", identity))
                .and_then(|value| self.announce_records.get(value))
                .filter(|record| record.destination_kind == "app");
            let lxmf_destination_hex = lxmf_record
                .map(|record| record.destination_hex.clone())
                .or_else(|| {
                    identity_hex.as_ref().and_then(|identity| {
                        self.announced_or_resolved_destination("lxmf_delivery", identity)
                            .cloned()
                    })
                })
                .or_else(|| saved_profile.and_then(|profile| profile.lxmf_destination_hex.clone()));
            let app_alias_destination = identity_hex
                .as_ref()
                .and_then(|identity| self.announced_or_resolved_destination("app", identity))
                .or_else(|| saved_profile.map(|profile| &profile.destination_hex));
            let saved_alias = app_alias_destination.is_some_and(|app_destination| {
                self.saved_destinations.contains(app_destination.as_str())
            });
            let saved = self.saved_destinations.contains(destination_hex.as_str()) || saved_alias;
            let last_resolution_error = self
                .last_resolution_errors
                .get(&destination_hex)
                .cloned()
                .or_else(|| {
                    identity_hex
                        .as_ref()
                        .and_then(|identity| {
                            self.announced_or_resolved_destination("app", identity)
                        })
                        .and_then(|app_destination| {
                            self.last_resolution_errors.get(app_destination).cloned()
                        })
                });
            let announce_last_seen_at_ms = lxmf_record
                .map(|record| record.received_at_ms)
                .or_else(|| saved_profile.and_then(|profile| profile.last_route_seen_at_ms));
            let lxmf_last_seen_at_ms = announce_last_seen_at_ms;
            let has_transport_link = self
                .active_link_destinations
                .contains(destination_hex.as_str())
                || app_alias_destination
                    .is_some_and(|value| self.active_link_destinations.contains(value.as_str()))
                || lxmf_destination_hex
                    .as_ref()
                    .is_some_and(|value| self.active_link_destinations.contains(value.as_str()));
            let active_link = has_transport_link;
            let latest_route_seen_at_ms = announce_last_seen_at_ms;
            let peer_app_data = lxmf_record
                .map(|record| record.app_data.as_str())
                .or_else(|| saved_profile.and_then(|profile| profile.app_data.as_deref()));
            let mission_capable = supports_mission_traffic(peer_app_data);
            let unsaved_recent = announce_last_seen_at_ms.is_some_and(|seen_at_ms| {
                now_ms.saturating_sub(seen_at_ms) <= self.peer_stale_after_ms
            });
            if !saved && (!mission_capable || !unsaved_recent) {
                continue;
            }
            let latest_seen_at_ms = latest_route_seen_at_ms.unwrap_or(0);
            let stale = peer_is_stale(
                saved,
                active_link,
                latest_route_seen_at_ms,
                now_ms,
                self.peer_stale_after_ms,
            );
            let availability_state = peer_availability_state(
                lxmf_record.is_some(),
                identity_hex.as_ref(),
                lxmf_destination_hex.as_ref(),
                stale,
            );
            peers.push(PeerRecord {
                destination_hex: destination_hex.clone(),
                identity_hex,
                lxmf_destination_hex: lxmf_destination_hex.clone(),
                display_name: lxmf_record
                    .and_then(|record| record.display_name.clone())
                    .or_else(|| app_record.and_then(|record| record.display_name.clone()))
                    .or_else(|| saved_profile.and_then(|profile| profile.display_name.clone())),
                app_data: peer_app_data.map(ToOwned::to_owned),
                state: compatibility_peer_state(saved, availability_state, active_link),
                saved,
                stale,
                active_link,
                last_resolution_error,
                last_resolution_attempt_at_ms: self
                    .last_resolution_attempt_at_ms
                    .get(&destination_hex)
                    .copied()
                    .or_else(|| {
                        app_alias_destination.and_then(|app_destination| {
                            self.last_resolution_attempt_at_ms
                                .get(app_destination)
                                .copied()
                        })
                    }),
                last_seen_at_ms: latest_seen_at_ms,
                announce_last_seen_at_ms,
                lxmf_last_seen_at_ms,
            });
        }

        peers.sort_by_key(|peer| std::cmp::Reverse(peer.last_seen_at_ms));
        peers
    }

    pub fn app_destination_for_identity(&self, identity_hex: &str) -> Option<String> {
        let normalized = normalize_hex(identity_hex);
        if normalized.is_empty() {
            return None;
        }

        self.resolved_app_destination_by_identity
            .get(normalized.as_str())
            .cloned()
            .or_else(|| {
                self.resolved_app_identity_by_destination.iter().find_map(
                    |(destination_hex, resolved_identity_hex)| {
                        if normalize_hex(resolved_identity_hex.as_str()) == normalized {
                            Some(destination_hex.clone())
                        } else {
                            None
                        }
                    },
                )
            })
    }

    pub fn peer_by_destination(&self, destination_hex: &str) -> Option<PeerRecord> {
        let normalized = normalize_hex(destination_hex);
        self.list_peers().into_iter().find(|peer| {
            peer.destination_hex == normalized
                || peer.lxmf_destination_hex.as_deref() == Some(normalized.as_str())
                || peer
                    .identity_hex
                    .as_ref()
                    .and_then(|identity_hex| self.app_destination_for_identity(identity_hex))
                    .as_deref()
                    == Some(normalized.as_str())
        })
    }

    pub fn peer_change_for_destination(&self, destination_hex: &str) -> Option<PeerChange> {
        self.peer_by_destination(destination_hex)
            .map(peer_change_from_record)
    }
}
