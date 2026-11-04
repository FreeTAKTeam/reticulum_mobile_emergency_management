impl MessagingStore {
    pub fn new(stale_after_minutes: u32) -> Self {
        Self {
            announce_records: HashMap::new(),
            announced_destination_by_identity: HashMap::new(),
            mission_announce_destinations: HashSet::new(),
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
        // Resolution mappings can change before an announce; index received records separately.
        if let Some(previous) = self.announced_destination_by_identity.insert(
            (destination_kind.to_owned(), identity_hex.to_owned()),
            destination_hex.to_owned(),
        ) {
            if previous != destination_hex {
                self.announce_records.remove(&previous);
                self.mission_announce_destinations.remove(&previous);
            }
        }
    }

    fn announced_or_resolved_destination(&self, kind: &str, identity: &str) -> Option<&String> {
        let identity = normalize_hex(identity);
        self.announced_destination_by_identity
            .get(&(kind.to_owned(), identity.clone()))
            .or_else(|| match kind {
                "app" => self.resolved_app_destination_by_identity.get(&identity),
                "lxmf_delivery" => self.resolved_lxmf_by_identity.get(&identity),
                _ => None,
            })
    }

    pub fn conversation_id_for(destination_hex: &str) -> String {
        destination_hex.trim().to_ascii_lowercase()
    }

    pub fn record_announce(&mut self, mut record: AnnounceRecord) {
        let destination_hex = normalize_hex(record.destination_hex.as_str());
        let identity_hex = normalize_hex(record.identity_hex.as_str());
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
            } else {
                self.announced_destination_by_identity.remove(&(
                    existing.destination_kind.clone(),
                    normalize_hex(existing.identity_hex.as_str()),
                ));
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
                self.resolved_lxmf_by_identity
                    .insert(identity_hex.clone(), destination_hex.clone());
                self.replace_announce_for_identity(
                    "lxmf_delivery",
                    identity_hex.as_str(),
                    destination_hex.as_str(),
                );
            }
        }
        if record.destination_kind == "lxmf_delivery"
            && supports_mission_traffic(Some(record.app_data.as_str()))
        {
            self.mission_announce_destinations
                .insert(destination_hex.clone());
        } else {
            self.mission_announce_destinations.remove(&destination_hex);
        }
        self.announce_records.insert(destination_hex, record);
    }

    pub fn list_announces(&self) -> Vec<AnnounceRecord> {
        let mut records = self.iter_announces().cloned().collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.received_at_ms));
        records
    }

    pub fn iter_announces(&self) -> impl Iterator<Item = &AnnounceRecord> {
        self.announce_records.values()
    }
}
