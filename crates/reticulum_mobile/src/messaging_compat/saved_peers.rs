impl MessagingStore {
    pub fn mark_peer_saved(&mut self, destination_hex: &str, saved: bool) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if saved {
            self.saved_destinations.insert(normalized.clone());
            self.last_resolution_errors.remove(&normalized);
        } else {
            self.saved_destinations.remove(&normalized);
            self.saved_peer_profiles.remove(&normalized);
            self.active_link_destinations.remove(&normalized);
            self.last_resolution_errors.remove(&normalized);
            self.last_resolution_attempt_at_ms.remove(&normalized);
        }
    }

    pub fn record_saved_peer_profile(
        &mut self,
        destination_hex: &str,
        identity_hex: Option<&str>,
        lxmf_destination_hex: Option<&str>,
        app_data: Option<&str>,
        display_name: Option<&str>,
        last_route_seen_at_ms: Option<u64>,
        _last_hops: Option<u8>,
    ) {
        let destination_hex = normalize_hex(destination_hex);
        if destination_hex.is_empty() {
            return;
        }
        let identity_hex = identity_hex
            .map(normalize_hex)
            .filter(|value| !value.is_empty());
        let lxmf_destination_hex = lxmf_destination_hex
            .map(normalize_hex)
            .filter(|value| !value.is_empty());
        if let (Some(identity_hex), Some(lxmf_destination_hex)) =
            (identity_hex.as_ref(), lxmf_destination_hex.as_ref())
        {
            self.resolved_lxmf_by_identity
                .insert(identity_hex.clone(), lxmf_destination_hex.clone());
        }
        if let Some(identity_hex) = identity_hex.as_ref() {
            self.update_app_destination_mapping(destination_hex.clone(), identity_hex.clone());
        }
        self.saved_peer_profiles.insert(
            destination_hex.clone(),
            SavedPeerProfile {
                destination_hex,
                identity_hex,
                lxmf_destination_hex,
                app_data: app_data
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                display_name: display_name
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                last_route_seen_at_ms,
            },
        );
    }

    pub fn saved_destination_hexes(&self) -> Vec<String> {
        let mut destinations = self.saved_destinations.iter().cloned().collect::<Vec<_>>();
        destinations.sort();
        destinations
    }

    pub fn replace_saved_destinations<I, S>(
        &mut self,
        destinations: I,
    ) -> (Vec<String>, Vec<String>)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let next = destinations
            .into_iter()
            .map(|destination| normalize_hex(destination.as_ref()))
            .filter(|destination| !destination.is_empty())
            .collect::<HashSet<_>>();
        let current = self.saved_destinations.clone();
        let mut added = next.difference(&current).cloned().collect::<Vec<_>>();
        let mut removed = current.difference(&next).cloned().collect::<Vec<_>>();
        added.sort();
        removed.sort();

        for destination in &removed {
            self.mark_peer_saved(destination, false);
        }
        for destination in &added {
            self.mark_peer_saved(destination, true);
        }

        (added, removed)
    }

    pub fn prune_saved_destinations_with_non_rem_announce_evidence(&mut self) -> Vec<String> {
        let mut app_dest_by_identity = HashMap::<String, String>::new();
        let mut lxmf_dest_by_identity = HashMap::<String, String>::new();
        let mut app_records = HashMap::<String, AnnounceRecord>::new();
        let mut lxmf_records = HashMap::<String, AnnounceRecord>::new();

        for record in self.announce_records.values() {
            if record.destination_kind == "app" {
                app_dest_by_identity
                    .insert(record.identity_hex.clone(), record.destination_hex.clone());
                app_records.insert(record.destination_hex.clone(), record.clone());
            } else if record.destination_kind == "lxmf_delivery" {
                lxmf_dest_by_identity
                    .insert(record.identity_hex.clone(), record.destination_hex.clone());
                lxmf_records.insert(record.destination_hex.clone(), record.clone());
            }
        }
        for (identity_hex, destination_hex) in &self.resolved_app_destination_by_identity {
            app_dest_by_identity
                .entry(identity_hex.clone())
                .or_insert_with(|| destination_hex.clone());
        }
        for (identity_hex, lxmf_destination_hex) in &self.resolved_lxmf_by_identity {
            lxmf_dest_by_identity
                .entry(identity_hex.clone())
                .or_insert_with(|| lxmf_destination_hex.clone());
        }
        for profile in self.saved_peer_profiles.values() {
            if let (Some(identity_hex), Some(lxmf_destination_hex)) = (
                profile.identity_hex.as_ref(),
                profile.lxmf_destination_hex.as_ref(),
            ) {
                lxmf_dest_by_identity
                    .entry(identity_hex.clone())
                    .or_insert_with(|| lxmf_destination_hex.clone());
            }
            if let Some(identity_hex) = profile.identity_hex.as_ref() {
                app_dest_by_identity
                    .entry(identity_hex.clone())
                    .or_insert_with(|| profile.destination_hex.clone());
            }
        }

        let mut removed = Vec::new();
        for saved_destination in self.saved_destination_hexes() {
            let lxmf_record = lxmf_records.get(saved_destination.as_str()).or_else(|| {
                app_records
                    .get(saved_destination.as_str())
                    .map(|record| record.identity_hex.as_str())
                    .or_else(|| {
                        self.resolved_app_identity_by_destination
                            .get(saved_destination.as_str())
                            .map(String::as_str)
                    })
                    .and_then(|identity| lxmf_dest_by_identity.get(identity))
                    .and_then(|lxmf_destination| lxmf_records.get(lxmf_destination.as_str()))
            });
            let has_non_rem_lxmf_evidence = lxmf_record.is_some_and(|record| {
                let app_data = record.app_data.trim();
                !app_data.is_empty() && !supports_mission_traffic(Some(app_data))
            });
            if has_non_rem_lxmf_evidence {
                self.mark_peer_saved(saved_destination.as_str(), false);
                removed.push(saved_destination);
            }
        }
        removed
    }

    pub fn is_peer_saved(&self, destination_hex: &str) -> bool {
        let normalized = normalize_hex(destination_hex);
        !normalized.is_empty() && self.saved_destinations.contains(normalized.as_str())
    }

    pub fn current_lxmf_announce_destination(&self, destination_hex: &str) -> Option<String> {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return None;
        }

        let record = self.announce_records.get(normalized.as_str())?;
        if record.destination_kind != "lxmf_delivery" {
            return None;
        }

        let current =
            current_time_ms().saturating_sub(record.received_at_ms) <= self.peer_stale_after_ms;
        current.then_some(record.destination_hex.clone())
    }

    fn saved_profile_for_destination(&self, destination_hex: &str) -> Option<&SavedPeerProfile> {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return None;
        }
        self.saved_peer_profiles
            .get(normalized.as_str())
            .or_else(|| {
                self.saved_peer_profiles.values().find(|profile| {
                    profile.lxmf_destination_hex.as_deref() == Some(normalized.as_str())
                        || profile.identity_hex.as_deref() == Some(normalized.as_str())
                })
            })
    }

    pub fn record_resolution_attempt(&mut self, destination_hex: &str, attempted_at_ms: u64) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        self.last_resolution_attempt_at_ms
            .insert(normalized, attempted_at_ms);
    }

    pub fn record_resolution_result(
        &mut self,
        destination_hex: &str,
        identity_hex: &str,
        lxmf_destination_hex: &str,
        _resolved_at_ms: u64,
    ) {
        let normalized_destination = normalize_hex(destination_hex);
        let normalized_identity = normalize_hex(identity_hex);
        let normalized_lxmf_destination = normalize_hex(lxmf_destination_hex);
        if normalized_destination.is_empty() || normalized_identity.is_empty() {
            return;
        }
        self.update_app_destination_mapping(
            normalized_destination.clone(),
            normalized_identity.clone(),
        );
        if !normalized_lxmf_destination.is_empty() {
            self.resolved_lxmf_by_identity
                .insert(normalized_identity, normalized_lxmf_destination);
        }
        self.last_resolution_errors.remove(&normalized_destination);
    }

    pub fn record_resolution_error(&mut self, destination_hex: &str, error: Option<String>) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if let Some(error) = error.filter(|value| !value.trim().is_empty()) {
            self.last_resolution_errors.insert(normalized, error);
        } else {
            self.last_resolution_errors.remove(&normalized);
        }
    }

    pub fn set_peer_active_link(
        &mut self,
        destination_hex: &str,
        active: bool,
        changed_at_ms: u64,
    ) {
        let normalized = normalize_hex(destination_hex);
        if normalized.is_empty() {
            return;
        }
        if active {
            self.active_link_destinations.insert(normalized);
        } else {
            self.active_link_destinations.remove(&normalized);
        }

        let _ = changed_at_ms;
    }

}
