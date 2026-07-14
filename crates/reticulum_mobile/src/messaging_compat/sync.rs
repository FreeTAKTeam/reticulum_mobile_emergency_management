impl MessagingStore {
    pub fn store_outbound(&mut self, outbound: StoredOutboundMessage) {
        self.outbound_messages
            .insert(outbound.message_id_hex.clone(), outbound);
    }

    pub fn outbound(&self, message_id_hex: &str) -> Option<StoredOutboundMessage> {
        self.outbound_messages.get(message_id_hex).cloned()
    }

    pub fn set_active_propagation_node(&mut self, destination_hex: Option<String>) -> SyncStatus {
        self.sync_status.active_propagation_node_hex =
            destination_hex.map(|value| normalize_hex(value.as_str()));
        self.sync_status.clone()
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.sync_status.clone()
    }

    pub fn update_sync_status<F>(&mut self, apply: F) -> SyncStatus
    where
        F: FnOnce(&mut SyncStatus),
    {
        apply(&mut self.sync_status);
        self.sync_status.clone()
    }
}
