impl AppStateStore {
    pub fn list_messages(
        &self,
        conversation_id: Option<&str>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        self.list_messages_resolved(conversation_id, &ConversationPeerResolver::default())
    }

    pub(crate) fn list_messages_resolved(
        &self,
        conversation_id: Option<&str>,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let connection = self.connect()?;
        self.repair_message_conversations(&connection, resolver)?;
        let mut records = Vec::new();
        if let Some(conversation_id) = conversation_id {
            let conversation_id = resolver.canonical_for(conversation_id);
            let mut statement = connection
                .prepare(
                    "SELECT json FROM messages WHERE conversation_id = ?1
                     ORDER BY updated_at_ms ASC, message_id_hex ASC",
                )
                .map_err(|_| NodeError::IoError {})?;
            let rows = statement
                .query_map(params![conversation_id], |row| row.get::<_, String>(0))
                .map_err(|_| NodeError::IoError {})?;
            for row in rows {
                let raw: String = row.map_err(|_| NodeError::IoError {})?;
                records.push(deserialize_json(&raw)?);
            }
        } else {
            let mut statement = connection
                .prepare("SELECT json FROM messages ORDER BY updated_at_ms ASC, message_id_hex ASC")
                .map_err(|_| NodeError::IoError {})?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|_| NodeError::IoError {})?;
            for row in rows {
                let raw: String = row.map_err(|_| NodeError::IoError {})?;
                records.push(deserialize_json(&raw)?);
            }
        }
        Ok(records)
    }

    #[cfg(test)]
    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, NodeError> {
        self.list_conversations_resolved(&ConversationPeerResolver::default())
    }

    pub(crate) fn list_conversations_resolved(
        &self,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<ConversationRecord>, NodeError> {
        let messages = self.list_messages_resolved(None, resolver)?;
        let labels = self
            .get_saved_peers()?
            .into_iter()
            .map(|peer| {
                (
                    normalize_message_peer_key(peer.destination_hex.as_str()),
                    peer.label,
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut conversations = std::collections::HashMap::<String, ConversationRecord>::new();

        for message in messages {
            let updated_at_ms = message
                .received_at_ms
                .or(message.sent_at_ms)
                .unwrap_or(message.updated_at_ms);
            let resolved_peer = resolver.peer_for_canonical(message.conversation_id.as_str());
            let peer_destination_hex = resolved_peer
                .map(|peer| peer.peer_destination_hex.clone())
                .unwrap_or_else(|| message.conversation_id.clone());
            let preview = truncate_preview(message.body_utf8.as_str());
            let peer_display_name = resolved_peer
                .and_then(|peer| peer.display_name.clone())
                .or_else(|| labels.get(&peer_destination_hex).cloned().flatten());
            let next = ConversationRecord {
                conversation_id: message.conversation_id.clone(),
                peer_destination_hex,
                peer_display_name,
                last_message_preview: preview,
                last_message_at_ms: updated_at_ms,
                unread_count: 0,
                last_message_state: Some(message.state),
            };

            match conversations.get(&message.conversation_id) {
                Some(existing) if existing.last_message_at_ms > next.last_message_at_ms => {}
                _ => {
                    conversations.insert(message.conversation_id.clone(), next);
                }
            }
        }

        let mut records = conversations.into_values().collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .last_message_at_ms
                .cmp(&left.last_message_at_ms)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        Ok(records)
    }

    pub fn upsert_message(
        &self,
        message: &MessageRecord,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let canonical_message = canonicalize_chat_message(message);
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        self.write_message_tx(&transaction, &canonical_message)?;
        let messages = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Messages {},
            Some(canonical_message.conversation_id.clone()),
            Some("message-upserted".to_string()),
        )?;
        let conversations = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Conversations {},
            None,
            Some("message-upserted".to_string()),
        )?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(vec![messages, conversations])
    }

    pub(crate) fn delete_conversation_resolved(
        &self,
        conversation_id: &str,
        resolver: &ConversationPeerResolver,
    ) -> Result<Vec<ProjectionInvalidation>, NodeError> {
        let canonical_id = resolver.canonical_for(conversation_id);
        if canonical_id.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let mut ids = resolver.aliases_for_canonical(canonical_id.as_str());
        ids.push(canonical_id.clone());
        ids.sort();
        ids.dedup();
        let normalized_ids = ids
            .iter()
            .map(|id| normalize_message_peer_key(id.as_str()))
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();

        let mut connection = self.connect()?;
        self.repair_message_conversations(&connection, resolver)?;
        let transaction = connection
            .transaction()
            .map_err(|_| NodeError::IoError {})?;
        let removed_sos =
            self.delete_sos_records_for_conversations_tx(&transaction, normalized_ids.as_slice())?;
        for id in &ids {
            transaction
                .execute(
                    "DELETE FROM messages WHERE conversation_id = ?1",
                    params![id],
                )
                .map_err(|_| NodeError::IoError {})?;
        }
        let messages = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Messages {},
            Some(canonical_id),
            Some("conversation-deleted".to_string()),
        )?;
        let conversations = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Conversations {},
            None,
            Some("conversation-deleted".to_string()),
        )?;
        let sos = if removed_sos {
            Some(self.bump_projection_revision_tx(
                &transaction,
                ProjectionScope::Sos {},
                None,
                Some("conversation-deleted".to_string()),
            )?)
        } else {
            None
        };
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        let mut invalidations = vec![messages, conversations];
        if let Some(invalidation) = sos {
            invalidations.push(invalidation);
        }
        Ok(invalidations)
    }

}
