impl MessagingStore {
    pub fn new(stale_after_minutes: u32) -> Self {
        Self {
            announce_records: HashMap::new(),
            resolved_app_destination_by_identity: HashMap::new(),
            resolved_app_identity_by_destination: HashMap::new(),
            resolved_lxmf_by_identity: HashMap::new(),
            saved_destinations: HashSet::new(),
            saved_peer_profiles: HashMap::new(),
            active_link_destinations: HashSet::new(),
            last_resolution_errors: HashMap::new(),
            last_resolution_attempt_at_ms: HashMap::new(),
            message_records: HashMap::new(),
            message_order: Vec::new(),
            outbound_messages: HashMap::new(),
            sync_status: SyncStatus::default(),
            peer_stale_after_ms: u64::from(stale_after_minutes.max(1)) * 60_000,
        }
    }

    fn update_app_destination_mapping(&mut self, destination_hex: String, identity_hex: String) {
        if let Some(previous_destination_hex) = self
            .resolved_app_destination_by_identity
            .insert(identity_hex.clone(), destination_hex.clone())
        {
            if previous_destination_hex != destination_hex {
                self.resolved_app_identity_by_destination
                    .remove(previous_destination_hex.as_str());
            }
        }
        self.resolved_app_identity_by_destination
            .insert(destination_hex, identity_hex);
    }

    fn replace_announce_for_identity(
        &mut self,
        destination_kind: &str,
        identity_hex: &str,
        destination_hex: &str,
    ) {
        let identity_hex = normalize_hex(identity_hex);
        if identity_hex.is_empty() {
            return;
        }

        let destination_hex = normalize_hex(destination_hex);
        let replaced_destinations = self
            .announce_records
            .iter()
            .filter(|(candidate_destination_hex, record)| {
                record.destination_kind == destination_kind
                    && normalize_hex(record.identity_hex.as_str()) == identity_hex
                    && *candidate_destination_hex != &destination_hex
            })
            .map(|(candidate_destination_hex, _)| candidate_destination_hex.clone())
            .collect::<Vec<_>>();

        for replaced_destination_hex in replaced_destinations {
            self.announce_records
                .remove(replaced_destination_hex.as_str());
        }
    }

    pub fn conversation_id_for(destination_hex: &str) -> String {
        destination_hex.trim().to_ascii_lowercase()
    }

    pub fn record_announce(&mut self, mut record: AnnounceRecord) {
        let destination_hex = normalize_hex(record.destination_hex.as_str());
        let identity_hex = normalize_hex(record.identity_hex.as_str());
        let destination_kind = record.destination_kind.clone();
        let received_at_ms = record.received_at_ms;
        if let Some(existing) = self.announce_records.get(destination_hex.as_str()) {
            if existing.received_at_ms > record.received_at_ms {
                return;
            }
            if existing.destination_kind == record.destination_kind
                && normalize_hex(existing.identity_hex.as_str()) == identity_hex
            {
                if record.app_data.trim().is_empty() && !existing.app_data.trim().is_empty() {
                    record.app_data = existing.app_data.clone();
                }
                if record.display_name.is_none() {
                    record.display_name = existing.display_name.clone();
                }
            }
        }
        if !destination_hex.is_empty() && !identity_hex.is_empty() {
            if record.destination_kind == "app" {
                self.update_app_destination_mapping(destination_hex.clone(), identity_hex.clone());
                self.replace_announce_for_identity(
                    "app",
                    identity_hex.as_str(),
                    destination_hex.as_str(),
                );
            } else if record.destination_kind == "lxmf_delivery" {
                if let Some(previous_destination_hex) = self
                    .resolved_lxmf_by_identity
                    .insert(identity_hex.clone(), destination_hex.clone())
                {
                    if previous_destination_hex != destination_hex {
                        self.announce_records
                            .remove(previous_destination_hex.as_str());
                    }
                }
                self.replace_announce_for_identity(
                    "lxmf_delivery",
                    identity_hex.as_str(),
                    destination_hex.as_str(),
                );
            }
        }
        self.announce_records.insert(destination_hex, record);

        let _ = destination_kind;
        let _ = received_at_ms;
    }

    pub fn list_announces(&self) -> Vec<AnnounceRecord> {
        let mut records = self.announce_records.values().cloned().collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.received_at_ms));
        records
    }

}
