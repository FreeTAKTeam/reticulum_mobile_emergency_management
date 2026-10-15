// Plugin map sessions (plugin API 1.3). The operator explicitly selects map
// objects and starts a session with one plugin; that plugin may then read
// only those objects and receives their changes as host events. Sessions
// live in the native runtime, never survive a restart, and end when the
// plugin loses eligibility, every selected object is gone, or the operator,
// the plugin or the host stops them. Events wait in a bounded outbox the
// Android host drains and delivers across the plugin Binder boundary.

pub(crate) const PLUGIN_MAP_SESSION_API_MINOR: u16 = 3;
pub(crate) const PLUGIN_MAP_SESSION_MAX_OBJECTS: usize = 16;
pub(crate) const PLUGIN_MAP_SESSION_MAX_PER_PLUGIN: usize = 4;
const PLUGIN_MAP_SESSION_EVENT_BACKLOG: i64 = 256;
const PLUGIN_MAP_OBJECT_NAME_MAX_CHARS: usize = 128;
const PLUGIN_MAP_OBJECT_TEXT_MAX_CHARS: usize = 64;
const PLUGIN_MAP_OBJECT_NOTES_MAX_CHARS: usize = 1024;

/// Whether a plugin may use a map operation that needs API `min_minor` and
/// the given capability, both declared in its manifest and granted by REM.
pub(crate) fn plugin_map_capability_allowed(
    plugin: &InstalledPluginRecord,
    min_minor: u16,
    capability: fn(&PluginCapabilityRecord) -> bool,
) -> bool {
    plugin.trusted
        && plugin.enabled
        && !matches!(plugin.state.as_str(), "Missing" | "Incompatible")
        && plugin.discovered.api_major == 1
        && plugin.discovered.api_minor >= min_minor
        && capability(&plugin.discovered.declared_capabilities)
        && capability(&plugin.granted_capabilities)
}

fn plugin_map_session_capability(capabilities: &PluginCapabilityRecord) -> bool {
    capabilities.map_selection_read
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    match value.char_indices().nth(max_chars) {
        Some((end, _)) => (value[..end].to_string(), true),
        None => (value.to_string(), false),
    }
}

/// Plugin-facing marker JSON with every free-text field bounded, so a
/// session of the maximum size always fits the 64 KiB Binder limit even
/// when replicas carry long text.
pub(crate) fn plugin_map_object_json(marker: &r3akt_map_core::MarkerRecord) -> serde_json::Value {
    let mut value = marker.api_value();
    let mut truncated = false;
    for (key, max_chars) in [
        ("name", PLUGIN_MAP_OBJECT_NAME_MAX_CHARS),
        ("type", PLUGIN_MAP_OBJECT_TEXT_MAX_CHARS),
        ("category", PLUGIN_MAP_OBJECT_TEXT_MAX_CHARS),
        ("symbol", PLUGIN_MAP_OBJECT_TEXT_MAX_CHARS),
        ("origin_rch", PLUGIN_MAP_OBJECT_TEXT_MAX_CHARS),
        ("notes", PLUGIN_MAP_OBJECT_NOTES_MAX_CHARS),
    ] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
            let (bounded, cut) = truncate_chars(text, max_chars);
            truncated |= cut;
            value[key] = serde_json::Value::String(bounded);
        }
    }
    value["textTruncated"] = serde_json::Value::Bool(truncated);
    value
}

pub(crate) fn plugin_host_event_json(event: &str, payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "protocolVersion": 1, "event": event, "payload": payload })
}

#[track_caller]
fn plugin_map_session_io(error: impl std::fmt::Debug) -> NodeError {
    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
}

/// Uses the marker service's reason wrapper so the bridge can surface the
/// plain message to the plugin or UI.
#[track_caller]
fn plugin_map_session_invalid(reason: &str) -> NodeError {
    map_marker_invalid(reason)
}

fn plugin_map_session_stop_reason(plugin: Option<&InstalledPluginRecord>) -> Option<&'static str> {
    let Some(plugin) = plugin else {
        return Some("plugin-unavailable");
    };
    if matches!(plugin.state.as_str(), "Missing" | "Incompatible") {
        Some("plugin-unavailable")
    } else if !plugin.trusted || !plugin.enabled {
        Some("plugin-disabled")
    } else if !plugin_map_capability_allowed(
        plugin,
        PLUGIN_MAP_SESSION_API_MINOR,
        plugin_map_session_capability,
    ) {
        Some("capability-revoked")
    } else {
        None
    }
}

impl AppStateStore {
    pub(crate) fn clear_plugin_map_sessions(connection: &Connection) -> Result<(), NodeError> {
        connection
            .execute_batch(
                "DELETE FROM plugin_map_sessions; DELETE FROM plugin_map_session_events;",
            )
            .map_err(plugin_map_session_io)
    }

    pub(crate) fn list_plugin_map_sessions(
        &self,
        plugin_id: Option<&str>,
    ) -> Result<Vec<crate::types::PluginMapSessionRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT json FROM plugin_map_sessions
                 WHERE ?1 IS NULL OR plugin_id = ?1 ORDER BY started_at_ms ASC, session_id ASC",
            )
            .map_err(plugin_map_session_io)?;
        let rows = statement
            .query_map(params![plugin_id], |row| row.get::<_, String>(0))
            .map_err(plugin_map_session_io)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(deserialize_json(&row.map_err(plugin_map_session_io)?)?);
        }
        Ok(sessions)
    }

    /// The session, only when it belongs to `plugin_id`.
    pub(crate) fn plugin_map_session(
        &self,
        plugin_id: &str,
        session_id: &str,
    ) -> Result<Option<crate::types::PluginMapSessionRecord>, NodeError> {
        Ok(self
            .list_plugin_map_sessions(Some(plugin_id))?
            .into_iter()
            .find(|session| session.session_id == session_id.trim()))
    }

    /// Current snapshots of the session's selected objects, nothing else.
    pub(crate) fn plugin_map_session_objects(
        &self,
        session: &crate::types::PluginMapSessionRecord,
    ) -> Result<Vec<serde_json::Value>, NodeError> {
        let connection = self.connect()?;
        let mut objects = Vec::new();
        for id in &session.object_destination_hashes {
            if let Some(marker) =
                r3akt_map_core::sqlite::get(&connection, id).map_err(plugin_map_session_io)?
            {
                objects.push(plugin_map_object_json(&marker));
            }
        }
        Ok(objects)
    }

    /// Starts an operator-selected session and queues `map.session.started`.
    pub(crate) fn start_plugin_map_session(
        &self,
        plugin_id: &str,
        object_destination_hashes: &[String],
    ) -> Result<(crate::types::PluginMapSessionRecord, ProjectionInvalidation), NodeError> {
        let plugin = self.get_plugin(plugin_id)?;
        if plugin_map_session_stop_reason(plugin.as_ref()).is_some() {
            return Err(plugin_map_session_invalid(
                "plugin is not enabled for map selection sessions",
            ));
        }
        let mut ids = Vec::new();
        for id in object_destination_hashes {
            let id = id.trim().to_ascii_lowercase();
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() || ids.len() > PLUGIN_MAP_SESSION_MAX_OBJECTS {
            return Err(plugin_map_session_invalid(
                "select between 1 and 16 map objects",
            ));
        }
        if self.list_plugin_map_sessions(Some(plugin_id))?.len()
            >= PLUGIN_MAP_SESSION_MAX_PER_PLUGIN
        {
            return Err(plugin_map_session_invalid(
                "plugin already has 4 active map sessions",
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(plugin_map_session_io)?;
        let mut objects = Vec::new();
        for id in &ids {
            let marker = r3akt_map_core::sqlite::get(&transaction, id)
                .map_err(plugin_map_session_io)?
                .ok_or_else(|| plugin_map_session_invalid("selected map object not found"))?;
            objects.push(plugin_map_object_json(&marker));
        }
        let session = crate::types::PluginMapSessionRecord {
            session_id: uuid_local_id(),
            plugin_id: plugin_id.to_string(),
            object_destination_hashes: ids,
            started_at_ms: now_ms(),
            started_by: "operator".to_string(),
        };
        transaction
            .execute(
                "INSERT INTO plugin_map_sessions (session_id, plugin_id, json, started_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    session.session_id,
                    session.plugin_id,
                    serialize_json(&session)?,
                    crate::numeric::u64_to_i64_saturating(session.started_at_ms)
                ],
            )
            .map_err(plugin_map_session_io)?;
        let mut payload = serde_json::to_value(&session).map_err(plugin_map_session_io)?;
        payload["objects"] = serde_json::Value::Array(objects);
        queue_plugin_map_event_tx(
            &transaction,
            plugin_id,
            &plugin_host_event_json("map.session.started", payload),
        )?;
        let invalidation = self.bump_plugin_map_session_revision_tx(
            &transaction,
            plugin_id,
            "plugin-map-session-started",
        )?;
        transaction.commit().map_err(plugin_map_session_io)?;
        Ok((session, invalidation))
    }

    /// Stops sessions matching `session_id` and/or `plugin_id`. With
    /// `notify`, the plugin receives `map.session.stopped`; it is skipped
    /// when the plugin's connection is already gone.
    pub(crate) fn stop_plugin_map_sessions(
        &self,
        plugin_id: Option<&str>,
        session_id: Option<&str>,
        reason: &str,
        notify: bool,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let targets = self
            .list_plugin_map_sessions(plugin_id)?
            .into_iter()
            .filter(|session| session_id.is_none_or(|id| session.session_id == id.trim()))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return Ok(None);
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(plugin_map_session_io)?;
        for session in &targets {
            end_plugin_map_session_tx(&transaction, session, reason, notify)?;
        }
        let key = plugin_id.unwrap_or(targets[0].plugin_id.as_str());
        let invalidation = self.bump_plugin_map_session_revision_tx(
            &transaction,
            key,
            "plugin-map-session-stopped",
        )?;
        transaction.commit().map_err(plugin_map_session_io)?;
        Ok(Some(invalidation))
    }

    /// Ends every session whose plugin is no longer trusted, enabled,
    /// installed, compatible, or granted `mapSelectionRead`.
    pub(crate) fn reconcile_plugin_map_sessions(
        &self,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let sessions = self.list_plugin_map_sessions(None)?;
        if sessions.is_empty() {
            return Ok(None);
        }
        let plugins = self.list_plugins()?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(plugin_map_session_io)?;
        let mut changed = None;
        for session in &sessions {
            let plugin = plugins
                .iter()
                .find(|plugin| plugin.discovered.plugin_id == session.plugin_id);
            if let Some(reason) = plugin_map_session_stop_reason(plugin) {
                end_plugin_map_session_tx(&transaction, session, reason, true)?;
                changed = Some(session.plugin_id.clone());
            }
        }
        let Some(plugin_id) = changed else {
            return Ok(None);
        };
        let invalidation = self.bump_plugin_map_session_revision_tx(
            &transaction,
            &plugin_id,
            "plugin-map-session-stopped",
        )?;
        transaction.commit().map_err(plugin_map_session_io)?;
        Ok(Some(invalidation))
    }

    /// Queues events for sessions that selected `object_destination_hash`
    /// after it changed. A deleted object leaves its sessions; a session
    /// with no objects left ends.
    pub(crate) fn queue_plugin_map_object_change(
        &self,
        object_destination_hash: &str,
        reason: &str,
    ) -> Result<Option<ProjectionInvalidation>, NodeError> {
        let id = object_destination_hash.trim().to_ascii_lowercase();
        let sessions = self
            .list_plugin_map_sessions(None)?
            .into_iter()
            .filter(|session| session.object_destination_hashes.contains(&id))
            .collect::<Vec<_>>();
        if sessions.is_empty() {
            return Ok(None);
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(plugin_map_session_io)?;
        let marker =
            r3akt_map_core::sqlite::get(&transaction, &id).map_err(plugin_map_session_io)?;
        let mut membership_changed = None;
        for mut session in sessions {
            let event = match &marker {
                Some(marker) => plugin_host_event_json(
                    "map.session.object.updated",
                    serde_json::json!({
                        "sessionId": session.session_id,
                        "reason": reason,
                        "object": plugin_map_object_json(marker),
                    }),
                ),
                None => plugin_host_event_json(
                    "map.session.object.removed",
                    serde_json::json!({ "sessionId": session.session_id, "objectDestinationHash": id }),
                ),
            };
            queue_plugin_map_event_tx(&transaction, &session.plugin_id, &event)?;
            if marker.is_some() {
                continue;
            }
            session
                .object_destination_hashes
                .retain(|selected| selected != &id);
            if session.object_destination_hashes.is_empty() {
                end_plugin_map_session_tx(&transaction, &session, "objects-removed", true)?;
            } else {
                transaction
                    .execute(
                        "UPDATE plugin_map_sessions SET json = ?2 WHERE session_id = ?1",
                        params![session.session_id, serialize_json(&session)?],
                    )
                    .map_err(plugin_map_session_io)?;
            }
            membership_changed = Some(session.plugin_id.clone());
        }
        let invalidation = match membership_changed {
            Some(plugin_id) => Some(self.bump_plugin_map_session_revision_tx(
                &transaction,
                &plugin_id,
                "plugin-map-session-updated",
            )?),
            None => None,
        };
        transaction.commit().map_err(plugin_map_session_io)?;
        Ok(invalidation)
    }

    /// Removes and returns up to `limit` queued `(plugin_id, event)` pairs, oldest first.
    pub(crate) fn drain_plugin_map_session_events(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, serde_json::Value)>, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(plugin_map_session_io)?;
        let rows = transaction
            .prepare("SELECT seq, plugin_id, event_json FROM plugin_map_session_events ORDER BY seq ASC LIMIT ?1")
            .and_then(|mut statement| {
                statement
                    .query_map(params![i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(plugin_map_session_io)?;
        if let Some((last, _, _)) = rows.last() {
            transaction
                .execute(
                    "DELETE FROM plugin_map_session_events WHERE seq <= ?1",
                    params![last],
                )
                .map_err(plugin_map_session_io)?;
        }
        transaction.commit().map_err(plugin_map_session_io)?;
        rows.into_iter()
            .map(|(_, plugin_id, event)| Ok((plugin_id, deserialize_json(&event)?)))
            .collect()
    }

    fn bump_plugin_map_session_revision_tx(
        &self,
        transaction: &Transaction<'_>,
        plugin_id: &str,
        reason: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        self.bump_projection_revision_tx(
            transaction,
            ProjectionScope::Plugins {},
            Some(plugin_id.to_string()),
            Some(reason.to_string()),
        )
    }
}

fn end_plugin_map_session_tx(
    transaction: &Transaction<'_>,
    session: &crate::types::PluginMapSessionRecord,
    reason: &str,
    notify: bool,
) -> Result<(), NodeError> {
    transaction
        .execute(
            "DELETE FROM plugin_map_sessions WHERE session_id = ?1",
            params![session.session_id],
        )
        .map_err(plugin_map_session_io)?;
    if notify {
        queue_plugin_map_event_tx(
            transaction,
            &session.plugin_id,
            &plugin_host_event_json(
                "map.session.stopped",
                serde_json::json!({ "sessionId": session.session_id, "reason": reason }),
            ),
        )?;
    }
    Ok(())
}

/// Appends an event, keeping only the newest backlog so a host that stops
/// draining cannot grow the database without bound.
fn queue_plugin_map_event_tx(
    transaction: &Transaction<'_>,
    plugin_id: &str,
    event: &serde_json::Value,
) -> Result<(), NodeError> {
    transaction
        .execute(
            "INSERT INTO plugin_map_session_events (plugin_id, event_json) VALUES (?1, ?2)",
            params![plugin_id, serialize_json(event)?],
        )
        .map_err(plugin_map_session_io)?;
    transaction
        .execute(
            "DELETE FROM plugin_map_session_events
             WHERE seq <= (SELECT MAX(seq) FROM plugin_map_session_events) - ?1",
            params![PLUGIN_MAP_SESSION_EVENT_BACKLOG],
        )
        .map_err(plugin_map_session_io)?;
    Ok(())
}
