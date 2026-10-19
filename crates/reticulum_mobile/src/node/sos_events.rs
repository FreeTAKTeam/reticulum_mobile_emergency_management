/// MECP beacon events published to the team event timeline when SOS starts and ends.
/// The LXMF SOS fan-out stays the emergency channel; these events record the beacon
/// in every peer's Events view through the normal event replication path.
const SOS_EVENT_MISSION_UID: &str = "r3akt-default-mission";
const SOS_EVENT_TYPE_KEYWORD: &str = "r3akt:event-type:B";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SosBeaconEvent {
    /// MECP B01, severity 0 (Mayday): automated distress beacon active.
    Activated,
    /// MECP B03, severity 2 (Safety): cancel beacon, I am OK.
    Cancelled,
}

impl SosBeaconEvent {
    fn code(self) -> &'static str {
        match self {
            Self::Activated => "B01",
            Self::Cancelled => "B03",
        }
    }

    fn mecp_prefix(self) -> &'static str {
        match self {
            Self::Activated => "MECP/0/B01",
            Self::Cancelled => "MECP/2/B03",
        }
    }
}

/// `#SOS_<incident>` tag that ties the B01 and B03 events of one incident together.
fn sos_incident_tag(incident_id: &str) -> String {
    let tag = incident_id
        .trim()
        .trim_start_matches("sos-")
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
        .collect::<String>();
    format!("#SOS_{tag}")
}

fn sos_beacon_event_for(
    source_identity: &str,
    source_name: Option<String>,
    incident_id: &str,
    kind: SosBeaconEvent,
    at_ms: u64,
) -> Result<EventProjectionRecord, NodeError> {
    let source_identity = normalize_hex_32(source_identity).ok_or(NodeError::InvalidConfig {})?;
    let incident_id = incident_id.trim();
    if incident_id.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let code = kind.code();
    Ok(EventProjectionRecord {
        uid: format!("rem-sos-{}-v1:{source_identity}:{incident_id}", code.to_ascii_lowercase()),
        command_id: format!("sos-{}-{at_ms}", code.to_ascii_lowercase()),
        source_identity,
        callsign: source_name.clone().unwrap_or_default(),
        source_display_name: source_name,
        timestamp: at_ms.to_string(),
        // Same command as operator-created events, so every peer stores it as a normal event.
        command_type: "mission.registry.log_entry.upsert".to_string(),
        mission_uid: SOS_EVENT_MISSION_UID.to_string(),
        content: format!("{} {}", kind.mecp_prefix(), sos_incident_tag(incident_id)),
        server_time: None,
        client_time: None,
        keywords: vec![SOS_EVENT_TYPE_KEYWORD.to_string(), code.to_string()],
        content_hashes: Vec::new(),
        updated_at_ms: at_ms,
        deleted_at_ms: None,
        correlation_id: Some(incident_id.to_string()),
        topics: vec![SOS_EVENT_MISSION_UID.to_string()],
    })
}

impl Node {
    /// Publishes the B01/B03 beacon event: stored now, replication queued without
    /// waiting for delivery. A failure is reported on the event bus and never
    /// blocks the SOS itself.
    fn publish_sos_beacon_event(&self, incident_id: &str, kind: SosBeaconEvent) {
        let result = self.local_identity_and_name().and_then(|(identity_hex, name)| {
            let event = sos_beacon_event_for(&identity_hex, Some(name), incident_id, kind, now_ms())?;
            self.upsert_event_queued(event, OutboundTrafficClass::Sos {})
        });
        if let Err(error) = result {
            if let Ok(inner) = self.inner.lock() {
                inner.bus.emit(NodeEvent::Error {
                    code: "SosEventFailed".to_string(),
                    message: format!(
                        "SOS {} event was not published incident={incident_id} reason={error}",
                        kind.code()
                    ),
                });
            }
        }
    }

    /// Stores the event and queues its replication without waiting for delivery,
    /// so the SOS call returns to the user immediately.
    fn upsert_event_queued(
        &self,
        record: EventProjectionRecord,
        traffic_class: OutboundTrafficClass,
    ) -> Result<(), NodeError> {
        self.upsert_event_with_delivery(record, None, traffic_class, false)
    }

    fn local_identity_and_name(&self) -> Result<(String, String), NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let status = inner.status.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        Ok((status.identity_hex.clone(), status.name.clone()))
    }
}
