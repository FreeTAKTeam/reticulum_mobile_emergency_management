pub(crate) fn canonicalize_chat_message(message: &MessageRecord) -> MessageRecord {
    canonicalize_chat_message_with_resolver(message, &ConversationPeerResolver::default())
}

fn canonicalize_chat_message_with_resolver(
    message: &MessageRecord,
    resolver: &ConversationPeerResolver,
) -> MessageRecord {
    let peer_key = canonical_message_peer_key(message);
    let conversation_key = normalize_message_peer_key(message.conversation_id.as_str());
    let canonical_id = resolver
        .resolve(peer_key.as_str())
        .or_else(|| resolver.resolve(conversation_key.as_str()))
        .map(|record| record.canonical_id.clone())
        .unwrap_or(peer_key);
    MessageRecord {
        conversation_id: canonical_id,
        ..message.clone()
    }
}

fn canonical_message_peer_key(message: &MessageRecord) -> String {
    let preferred = match message.direction {
        MessageDirection::Inbound {} => message
            .source_hex
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(message.destination_hex.as_str()),
        MessageDirection::Outbound {} => message.destination_hex.as_str(),
    };
    let normalized = normalize_message_peer_key(preferred);
    if !normalized.is_empty() {
        return normalized;
    }
    normalize_message_peer_key(message.conversation_id.as_str())
}

fn normalize_message_peer_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn conversation_has_messages(
    connection: &Connection,
    conversation_id: &str,
) -> Result<bool, NodeError> {
    let normalized = normalize_message_peer_key(conversation_id);
    if normalized.is_empty() {
        return Ok(false);
    }
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM messages WHERE conversation_id = ?1 LIMIT 1",
            params![normalized],
            |row| row.get(0),
        )
        .map_err(|_| NodeError::IoError {})?;
    Ok(count > 0)
}

fn conversation_has_sos_cancellation(
    connection: &Connection,
    conversation_id: &str,
    since_ms: u64,
) -> Result<bool, NodeError> {
    let normalized = normalize_message_peer_key(conversation_id);
    if normalized.is_empty() {
        return Ok(false);
    }
    let mut statement = connection
        .prepare(
            "SELECT json FROM messages
             WHERE conversation_id = ?1 AND updated_at_ms >= ?2
             ORDER BY updated_at_ms DESC, message_id_hex ASC",
        )
        .map_err(|_| NodeError::IoError {})?;
    let rows = statement
        .query_map(params![normalized, since_ms as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|_| NodeError::IoError {})?;
    for row in rows {
        let message: MessageRecord = deserialize_json(&row.map_err(|_| NodeError::IoError {})?)?;
        let detail = message.detail.as_deref().unwrap_or("").to_ascii_lowercase();
        if detail.contains("sos:cancelled")
            || matches!(
                sos_kind_from_text(message.body_utf8.as_str()),
                Some(crate::types::SosMessageKind::Cancelled {})
            )
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn truncate_preview(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(120).collect())
}

fn query_json_records<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<T>, NodeError> {
    let mut statement = connection.prepare(sql).map_err(|_| NodeError::IoError {})?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| NodeError::IoError {})?;
    let mut records = Vec::new();
    for row in rows {
        records.push(deserialize_json(&row.map_err(|_| NodeError::IoError {})?)?);
    }
    Ok(records)
}
