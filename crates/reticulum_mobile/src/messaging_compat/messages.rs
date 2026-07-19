impl MessagingStore {
    pub fn upsert_message(&mut self, message: MessageRecord) -> bool {
        let is_new = !self
            .message_records
            .contains_key(message.message_id_hex.as_str());
        self.message_records
            .insert(message.message_id_hex.clone(), message.clone());
        if is_new {
            self.message_order.push(message.message_id_hex);
        }
        is_new
    }

    pub fn update_message_delivery_state(
        &mut self,
        update: MessageDeliveryUpdate<'_>,
    ) -> Option<MessageRecord> {
        let MessageDeliveryUpdate {
            message_id_hex,
            state,
            transport_state,
            application_ack_state,
            detail,
            last_wire_message_id_hex,
            updated_at_ms,
        } = update;
        let resolved_message_id_hex = if self.message_records.contains_key(message_id_hex) {
            message_id_hex.to_string()
        } else {
            self.message_records
                .iter()
                .find_map(|(stored_message_id_hex, record)| {
                    record
                        .last_wire_message_id_hex
                        .as_deref()
                        .is_some_and(|wire_message_id_hex| {
                            wire_message_id_hex.eq_ignore_ascii_case(message_id_hex)
                        })
                        .then(|| stored_message_id_hex.clone())
                })?
        };
        let record = self
            .message_records
            .get_mut(resolved_message_id_hex.as_str())?;
        if let Some(state) = state {
            record.state = state;
        }
        if let Some(transport_state) = transport_state {
            record.transport_state = transport_state;
        }
        if let Some(application_ack_state) = application_ack_state {
            record.application_ack_state = application_ack_state;
        }
        if let Some(last_wire_message_id_hex) = last_wire_message_id_hex {
            record.last_wire_message_id_hex = Some(last_wire_message_id_hex);
        }
        record.detail = detail;
        record.updated_at_ms = updated_at_ms;
        Some(record.clone())
    }

    pub fn delete_conversation_messages<'a, I>(&mut self, conversation_keys: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        let keys = conversation_keys
            .into_iter()
            .map(normalize_hex)
            .filter(|key| !key.is_empty())
            .collect::<HashSet<_>>();
        if keys.is_empty() {
            return false;
        }

        let removed_ids = self
            .message_records
            .iter()
            .filter_map(|(message_id_hex, record)| {
                let conversation_id = normalize_hex(record.conversation_id.as_str());
                let destination_hex = normalize_hex(record.destination_hex.as_str());
                let source_hex = record.source_hex.as_deref().map(normalize_hex);
                (keys.contains(conversation_id.as_str())
                    || keys.contains(destination_hex.as_str())
                    || source_hex
                        .as_deref()
                        .is_some_and(|value| keys.contains(value)))
                .then_some(message_id_hex.clone())
            })
            .collect::<HashSet<_>>();
        if removed_ids.is_empty() {
            return false;
        }

        self.message_records
            .retain(|message_id_hex, _| !removed_ids.contains(message_id_hex));
        self.message_order
            .retain(|message_id_hex| !removed_ids.contains(message_id_hex));
        true
    }

    pub fn list_messages(&self, conversation_id: Option<&str>) -> Vec<MessageRecord> {
        let mut out = Vec::<MessageRecord>::new();
        for message_id_hex in &self.message_order {
            let Some(record) = self.message_records.get(message_id_hex).cloned() else {
                continue;
            };
            if conversation_id.is_some_and(|value| record.conversation_id != value) {
                continue;
            }
            out.push(record);
        }
        out.sort_by(|left, right| {
            let left_time = left
                .received_at_ms
                .or(left.sent_at_ms)
                .unwrap_or(left.updated_at_ms);
            let right_time = right
                .received_at_ms
                .or(right.sent_at_ms)
                .unwrap_or(right.updated_at_ms);
            left_time.cmp(&right_time)
        });
        out
    }

    pub fn list_conversations(&self) -> Vec<ConversationRecord> {
        let peers = self.list_peers();
        let mut peer_map = HashMap::<String, PeerRecord>::new();
        for peer in peers {
            peer_map.insert(peer.destination_hex.clone(), peer.clone());
            if let Some(lxmf_destination_hex) = peer.lxmf_destination_hex.clone() {
                peer_map.insert(lxmf_destination_hex, peer);
            }
        }

        let records = self.list_messages(None);
        let mut by_conversation = HashMap::<String, ConversationRecord>::new();
        for record in records {
            let entry = by_conversation
                .entry(record.conversation_id.clone())
                .or_insert_with(|| ConversationRecord {
                    conversation_id: record.conversation_id.clone(),
                    peer_destination_hex: record.destination_hex.clone(),
                    peer_display_name: peer_map
                        .get(&record.destination_hex)
                        .and_then(peer_display_name_for),
                    last_message_preview: None,
                    last_message_at_ms: 0,
                    unread_count: 0,
                    last_message_state: None,
                });

            let event_time = record
                .received_at_ms
                .or(record.sent_at_ms)
                .unwrap_or(record.updated_at_ms);
            if event_time >= entry.last_message_at_ms {
                entry.peer_destination_hex = record.destination_hex.clone();
                entry.peer_display_name = peer_map
                    .get(&record.destination_hex)
                    .and_then(peer_display_name_for);
                entry.last_message_preview = message_preview(record.body_utf8.as_str());
                entry.last_message_at_ms = event_time;
                entry.last_message_state = Some(record.state);
            }
            if matches!(record.direction, MessageDirection::Inbound) {
                entry.unread_count = entry.unread_count.saturating_add(1);
            }
        }

        let mut out = by_conversation.into_values().collect::<Vec<_>>();
        out.sort_by(|left, right| right.last_message_at_ms.cmp(&left.last_message_at_ms));
        out
    }

}
