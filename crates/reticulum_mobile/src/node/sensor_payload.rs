use r3akt_map_core::sensor_store::SensorOrigin;
use r3akt_map_core::sensor_wire::{self as sensor_wire, OriginRemovedEvent};

struct SensorSnapshot {
    origin: SensorOrigin,
    events: Vec<JsonValue>,
    provenance: Vec<crate::app_state::SensorProducer>,
}
fn sensor_snapshot(
    store: &AppStateStore,
    mut origin: SensorOrigin,
) -> Result<Option<SensorSnapshot>, NodeError> {
    let mut permitted = std::collections::BTreeSet::new();
    for id in &store.sensor_producer_ids(&origin.origin_id)? {
        if let Some(producer) =
            store.sensor_component_producer(&origin.origin_id, &format!("sensor:{id}"))?
        {
            if store.sensor_producer_allowed(&producer, false)?.is_some() {
                permitted.insert(id.clone());
            }
        }
    }
    if permitted.is_empty() {
        return Ok(None);
    }
    let ttl = store.sensor_ttl_ms()?;
    origin
        .sensors
        .retain(|id, s| permitted.contains(id) && now_ms().saturating_sub(s.sample_at_ms) <= ttl);
    origin.manifests.retain(|id, _| permitted.contains(id));
    origin
        .sensor_tombstones
        .retain(|id, _| permitted.contains(id));
    if origin.origin_kind != "device" {
        let allowed = match store.sensor_component_producer(&origin.origin_id, "position")? {
            Some(p) => store.sensor_producer_allowed(&p, true)?.is_some(),
            None => false,
        };
        if !allowed && origin.deleted_at_ms.is_none() {
            return Ok(None);
        }
    }
    let mut events = Vec::new();
    if let Some(deleted_at_ms) = origin.deleted_at_ms {
        events.push(json!(SensorEvent::OriginRemoved(OriginRemovedEvent {
            origin_id: origin.origin_id.clone(),
            deleted_at_ms
        })));
    } else {
        if let Some(marker) = store.map_marker_core_record(&origin.origin_id)? {
            let mut payload = marker.api_value();
            payload["origin_rch"] = json!(origin.writer_id);
            events.push(json!({"event_type":"mission.marker.created","payload":payload}));
        }
        for manifest in origin.manifests.values_mut() {
            let producer = store.sensor_component_producer(
                &origin.origin_id,
                &format!("sensor:{}", manifest.sensor_id),
            )?;
            let video_allowed = match producer {
                Some(p) => {
                    store.sensor_producer_allowed(&p, false)?.is_some_and(|p| {
                        crate::app_state::plugin_map_capability_allowed(&p, 4, |c| {
                            c.sensor_video_share
                        })
                    }) && store.sensor_video_approved(&origin.origin_id, &manifest.sensor_id)?
                }
                None => false,
            };
            if !video_allowed {
                manifest.video = None;
            }
            events.push(json!(SensorEvent::Manifest(
                store.published_sensor_manifest(
                    &origin.origin_id,
                    manifest,
                    *origin
                        .manifest_times
                        .get(&manifest.sensor_id)
                        .ok_or(NodeError::InvalidConfig {})?
                )?
            )));
        }
        for (sensor_id, deleted_at_ms) in &origin.sensor_tombstones {
            events.push(json!(SensorEvent::SensorRemoved(SensorRemovedEvent {
                origin_id: origin.origin_id.clone(),
                sensor_id: sensor_id.clone(),
                deleted_at_ms: *deleted_at_ms
            })));
        }
    }
    // Permission/certificate changes alter pending snapshot identity even when readings are unchanged.
    let mut provenance = Vec::new();
    for id in &permitted {
        if let Some(p) =
            store.sensor_component_producer(&origin.origin_id, &format!("sensor:{id}"))?
        {
            provenance.push(p);
        }
    }
    if origin.sensors.is_empty() && events.is_empty() {
        return Ok(None);
    }
    Ok(Some(SensorSnapshot {
        origin,
        events,
        provenance,
    }))
}
fn sensor_hash(value: &impl serde::Serialize) -> Result<String, NodeError> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(value).map_err(|_| NodeError::InvalidConfig {})?,
    )))
}
fn sensor_metadata_hash(snapshot: &SensorSnapshot) -> Result<String, NodeError> {
    let mut events = snapshot.events.clone();
    for event in &mut events {
        if event["event_type"] == "mission.marker.created" {
            if let Some(payload) = event["payload"].as_object_mut() {
                for key in ["position", "time", "stale_at", "updated_at"] {
                    payload.remove(key);
                }
            }
        }
    }
    sensor_hash(&events)
}
fn sensor_route(status: &NodeStatus, peers: &[PeerRecord]) -> Result<String, NodeError> {
    sensor_hash(&(
        status
            .interfaces
            .iter()
            .map(|i| (&i.interface_hex, &i.state))
            .collect::<Vec<_>>(),
        peers
            .iter()
            .map(|p| (&p.destination_hex, p.active_link))
            .collect::<Vec<_>>(),
    ))
}
fn sensor_command(
    snapshot: &SensorSnapshot,
    destination: &str,
    scope: &str,
    route: &str,
) -> Result<String, NodeError> {
    let hash = sensor_hash(&(
        &snapshot.origin,
        &snapshot.events,
        &snapshot.provenance,
        destination,
        scope,
        route,
    ))?;
    Ok(format!(
        "r3s:{}:{}:{}:{}",
        snapshot.origin.origin_id,
        &hash[..32],
        &sensor_metadata_hash(snapshot)?[..32],
        &route[..32]
    ))
}
fn sensor_scope(
    config: Option<&NodeConfigFingerprint>,
    snapshot: Option<&HubDirectorySnapshot>,
) -> String {
    format!(
        "{:?}:{:?}:{}:{:?}",
        config.map(|c| c.hub_mode),
        config.and_then(|c| c.hub_identity_hash.as_ref()),
        active_team_uid(snapshot),
        snapshot.map(active_local_team_destinations)
    )
}
fn sensor_fields(
    snapshot: &SensorSnapshot,
    command: &str,
    metadata: bool,
    team: &str,
) -> Result<Vec<u8>, NodeError> {
    let origin = &snapshot.origin;
    let mut events = if metadata {
        snapshot.events.clone()
    } else {
        Vec::new()
    };
    if let Some(position) = origin
        .position
        .as_ref()
        .filter(|_| origin.deleted_at_ms.is_none())
    {
        events.push(json!(SensorEvent::OriginFix(OriginFixEvent {
            origin_id: origin.origin_id.clone(),
            position: position.clone()
        })));
    }
    let mut entries = vec![
        (MsgPackValue::from(11), MsgPackValue::from(team)),
        (
            MsgPackValue::from(crate::lxmf_fields::FIELD_EVENT),
            rmp_serde::from_slice::<MsgPackValue>(
                &rmp_serde::to_vec(&events).map_err(|_| NodeError::InvalidConfig {})?,
            )
            .map_err(|_| NodeError::InvalidConfig {})?,
        ),
        (
            MsgPackValue::from(FIELD_COMMANDS),
            MsgPackValue::Array(vec![MsgPackValue::Map(vec![
                (
                    MsgPackValue::from("command_id"),
                    MsgPackValue::from(command),
                ),
                (
                    MsgPackValue::from("command_type"),
                    MsgPackValue::from("r3s.telemetry"),
                ),
            ])]),
        ),
    ];
    let sensors = origin.sensors.values().cloned().collect::<Vec<_>>();
    if !sensors.is_empty() && origin.deleted_at_ms.is_none() {
        if origin.origin_id == origin.writer_id {
            let packed = sensor_wire::encode_packed(
                origin.latest_source_time() / 1000,
                origin.position.as_ref(),
                &sensors,
            )
            .map_err(|_| NodeError::InvalidConfig {})?;
            entries.push((MsgPackValue::from(2), MsgPackValue::Binary(packed)));
        } else {
            let stream = sensor_wire::encode_origin_sensor_stream(
                &origin.origin_id,
                origin.position.as_ref(),
                &sensors,
            )
            .map_err(|_| NodeError::InvalidConfig {})?;
            entries.push((
                MsgPackValue::from(3),
                rmp_serde::from_slice::<MsgPackValue>(&stream)
                    .map_err(|_| NodeError::InvalidConfig {})?,
            ));
        }
    }
    rmp_serde::to_vec(&MsgPackValue::Map(entries)).map_err(|_| NodeError::InvalidConfig {})
}
