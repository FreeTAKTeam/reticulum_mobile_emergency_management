use sha2::{Digest, Sha256};

const RELAY_RETAIN: i64 = 2048;

fn relay_error(error: impl std::fmt::Debug) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
}

pub(crate) fn relay_allowed(plugin: &InstalledPluginRecord) -> bool {
    plugin.discovered.api_major == 1
        && plugin.discovered.api_minor >= 5
        && plugin.trusted
        && plugin.enabled
        && !matches!(
            plugin.state.as_str(),
            "Missing" | "Incompatible" | "Untrusted" | "Disabled"
        )
        && plugin.discovered.declared_capabilities.communications_relay
        && plugin.granted_capabilities.communications_relay
}

impl AppStateStore {
    pub(crate) fn record_relay_ingress(
        &self,
        kind: &str,
        source: &str,
        key: &str,
        payload: &serde_json::Value,
    ) -> Result<(), NodeError> {
        if crate::delivery_policy::normalize_hex_32(source).is_none()
            || !matches!(kind, "text" | "position" | "event")
        {
            return Err(NodeError::InvalidConfig {});
        }
        // Keep no copy of private ingress until a currently authorized consumer exists.
        if !self.list_plugins()?.iter().any(relay_allowed) {
            return Ok(());
        }
        let json = serialize_json(payload)?;
        if json.len() > 8192 {
            return Err(NodeError::InvalidConfig {});
        }
        let id = hex::encode(Sha256::digest(format!("{kind}:{source}:{key}").as_bytes()));
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(relay_error)?;
        transaction.execute(
            "INSERT INTO plugin_relay_ingress(id,kind,source,received_at_ms,payload) SELECT ?1,?2,?3,?4,?5 WHERE NOT EXISTS(SELECT 1 FROM plugin_relay_ingress WHERE id=?1)",
            params![id, kind, source.to_ascii_lowercase(), crate::numeric::u64_to_i64_saturating(now_ms()), json],
        ).map_err(relay_error)?;
        transaction.execute(
            "DELETE FROM plugin_relay_ingress WHERE sequence IN (SELECT sequence FROM plugin_relay_ingress ORDER BY sequence DESC LIMIT -1 OFFSET ?1)",
            [RELAY_RETAIN],
        ).map_err(relay_error)?;
        transaction.commit().map_err(relay_error)
    }

    pub(crate) fn poll_plugin_relay(
        &self,
        plugin_id: &str,
        cursor: Option<u64>,
        limit: usize,
    ) -> Result<serde_json::Value, NodeError> {
        if !(1..=16).contains(&limit)
            || !self
                .get_plugin(plugin_id)?
                .as_ref()
                .is_some_and(relay_allowed)
        {
            return Err(NodeError::InvalidConfig {});
        }
        let connection = self.connect()?;
        let (oldest, newest): (i64, i64) = connection.query_row(
            "SELECT coalesce(min(sequence),0),coalesce(max(sequence),0) FROM plugin_relay_ingress", [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(relay_error)?;
        let oldest = crate::numeric::i64_to_u64_saturating(oldest);
        let newest = crate::numeric::i64_to_u64_saturating(newest);
        let Some(cursor) = cursor else {
            return Ok(serde_json::json!({"items":[],"cursor":newest,"gap":false}));
        };
        if cursor > newest {
            return Ok(serde_json::json!({"items":[],"cursor":newest,"gap":true}));
        }
        let mut statement = connection.prepare(
            "SELECT sequence,id,kind,source,received_at_ms,payload FROM plugin_relay_ingress WHERE sequence > ?1 ORDER BY sequence LIMIT ?2",
        ).map_err(relay_error)?;
        let rows = statement
            .query_map(
                params![
                    crate::numeric::u64_to_i64_saturating(cursor),
                    i64::try_from(limit).map_err(relay_error)?
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(relay_error)?;
        let mut items = Vec::new();
        let mut next = cursor;
        let mut bytes = 0;
        for row in rows {
            let (sequence, id, kind, source, received_at_ms, payload) = row.map_err(relay_error)?;
            let sequence = u64::try_from(sequence).map_err(relay_error)?;
            let item = serde_json::json!({"sequence":sequence,"id":id,"kind":kind,"source":source,
                "receivedAtMs":received_at_ms,"payload":deserialize_json::<serde_json::Value>(&payload)?});
            let size = item.to_string().len();
            if bytes + size > 48_000 {
                break;
            }
            bytes += size;
            items.push(item);
            next = sequence;
        }
        Ok(serde_json::json!({"items":items,"cursor":next,"gap":oldest > cursor.saturating_add(1)}))
    }
}

pub(crate) fn relay_event_code(content: &str) -> Option<String> {
    let expanded = crate::runtime::expand_event_wire_content(content);
    let token = expanded.split_whitespace().next()?;
    let rest = token.strip_prefix("MECP/")?;
    let (severity, codes) = rest.split_once('/')?;
    if !matches!(severity, "0" | "1" | "2" | "3")
        || !codes.split('+').all(|code| {
            let bytes = code.as_bytes();
            bytes.len() == 3
                && bytes[0].is_ascii_uppercase()
                && bytes[1].is_ascii_digit()
                && bytes[2].is_ascii_digit()
        })
    {
        return None;
    }
    Some(token.to_string())
}

impl AppStateStore {
    pub(crate) fn claim_relay_delivery(
        &self,
        plugin_id: &str,
        id: &str,
    ) -> Result<bool, NodeError> {
        self.connect()?.execute("INSERT OR IGNORE INTO plugin_relay_deliveries(plugin_id,delivery_id) VALUES(?1,?2)",
            params![plugin_id,id]).map(|changed| changed > 0).map_err(relay_error)
    }

    pub(crate) fn relay_forward_result(
        &self,
        plugin_id: &str,
        id: &str,
    ) -> Result<Option<String>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key=?1",
                [format!("relay-forward:{plugin_id}:{id}")],
                |row| row.get(0),
            )
            .optional()
            .map_err(relay_error)
    }

    pub(crate) fn confirm_relay_forward(
        &self,
        plugin_id: &str,
        id: &str,
        message_id: &str,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "INSERT OR REPLACE INTO metadata(key,value) VALUES(?1,?2)",
                params![format!("relay-forward:{plugin_id}:{id}"), message_id],
            )
            .map(|_| ())
            .map_err(relay_error)
    }

    fn record_relay_event(&self, record: &EventProjectionRecord) -> Result<(), NodeError> {
        if record.deleted_at_ms.is_some()
            || record.uid.starts_with("gotenna-")
            || record.topics.iter().any(|topic| topic == "gotenna-relay")
        {
            return Ok(());
        }
        let Some(code) = relay_event_code(&record.content) else {
            return Ok(());
        };
        let key = format!("{}:{}", record.uid, record.updated_at_ms);
        self.record_relay_ingress(
            "event",
            &record.source_identity,
            &key,
            &serde_json::json!({"uid":record.uid,"code":code}),
        )
    }
}
