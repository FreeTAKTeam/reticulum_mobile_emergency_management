use r3akt_map_core::sensor::{OriginPosition, SensorManifest, SensorState};
use r3akt_map_core::sensor_wire::{ManifestEvent, OriginFixEvent, SensorEvent, SensorRemovedEvent};

pub(crate) type SensorGeometryResult = Result<JsonValue, (&'static str, String)>;
fn sensor_operation_error(code: &'static str, message: impl ToString) -> (&'static str, String) {
    (code, message.to_string())
}
fn sensor_operation_host(error: NodeError) -> (&'static str, String) {
    let reason = crate::error_context::take_internal_failure("InvalidConfig")
        .map_or_else(|| error.to_string(), |f| f.cause);
    sensor_operation_error("InvalidRequest", reason)
}
fn sensor_payload<T: serde::de::DeserializeOwned>(
    value: JsonValue,
) -> Result<T, (&'static str, String)> {
    serde_json::from_value(value)
        .map_err(|_| sensor_operation_error("InvalidRequest", "invalid sensor payload schema"))
}

impl Node {
    pub fn get_sensor_origins_json(&self) -> Result<String, NodeError> {
        self.with_app_state(|store, _| {
            serde_json::to_string(&store.sensor_origins_json()?).map_err(|e| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, e)
            })
        })
    }

    pub fn set_sensor_video_sharing(
        &self,
        origin_id: String,
        sensor_id: String,
        approved: bool,
    ) -> Result<(), NodeError> {
        self.with_app_state(|store, bus| {
            store.set_sensor_video_sharing(&origin_id, &sensor_id, approved)?;
            emit_projection_invalidation(bus, store.sensor_geometry_invalidation(&origin_id)?);
            Ok(())
        })
    }

    pub(crate) fn sensor_geometry_operation(
        &self,
        plugin_id: &str,
        operation: &str,
        payload: &JsonValue,
    ) -> SensorGeometryResult {
        let inner = self
            .inner
            .lock()
            .map_err(|_| sensor_operation_error("HostError", "node state unavailable"))?;
        let store = &inner.app_state;
        let capability: fn(&PluginCapabilityRecord) -> bool = match operation {
            "map.origins.telemetry.upsert" => |c| c.origin_telemetry_publish,
            "map.sensors.upsert" | "map.sensors.remove" | "map.sensors.manifest.upsert" => {
                |c| c.sensor_geometry_publish
            }
            _ => {
                return Err(sensor_operation_error(
                    "UnsupportedOperation",
                    "unknown sensor operation",
                ))
            }
        };
        let plugin = store.sensor_plugin(plugin_id, capability).map_err(|_| {
            sensor_operation_error(
                "PermissionDenied",
                "trusted API 1.4 plugin and explicit capability grant required",
            )
        })?;
        let status = inner
            .status
            .lock()
            .map_err(|_| sensor_operation_error("HostError", "node status unavailable"))?
            .clone();
        let writer = normalize_hex_32(&status.lxmf_destination_hex)
            .or(store.local_sensor_writer().map_err(sensor_operation_host)?)
            .ok_or_else(|| {
                sensor_operation_error(
                    "HostError",
                    "start this node once to establish its origin identity",
                )
            })?;
        store
            .remember_local_sensor_writer(&writer)
            .map_err(sensor_operation_host)?;
        let raw_id = payload
            .get("originId")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| sensor_operation_error("InvalidRequest", "originId is required"))?;
        let id = if raw_id == "self" {
            writer.clone()
        } else {
            normalize_hex_32(raw_id).ok_or_else(|| {
                sensor_operation_error("InvalidRequest", "originId must be a canonical object hash")
            })?
        };
        let is_self = id == writer;
        if !is_self
            && (!store
                .plugin_owns_map_marker(plugin_id, &id)
                .map_err(sensor_operation_host)?
                || !store
                    .sensor_plugin_owns_origin(&id, &plugin)
                    .map_err(sensor_operation_host)?)
        {
            return Err(sensor_operation_error(
                "PermissionDenied",
                "origin was not created by this plugin and publisher",
            ));
        }
        if !is_self {
            store
                .remember_sensor_marker_writer(&id, &writer)
                .map_err(sensor_operation_host)?;
        }
        store
            .bind_sensor_origin(&id, &writer, is_self)
            .map_err(sensor_operation_host)?;
        let now = now_ms();
        let event = match operation {
            "map.sensors.upsert" => {
                let sensor: SensorState =
                    sensor_payload(payload.get("sensor").cloned().unwrap_or(JsonValue::Null))?;
                r3akt_map_core::sensor_wire::encode_value(std::slice::from_ref(&sensor))
                    .and_then(|_| r3akt_map_core::sensor::source_time(sensor.sample_at_ms, now))
                    .map_err(|e| sensor_operation_error("InvalidRequest", e))?;
                let changed = store
                    .write_plugin_sensor(
                        &id,
                        &writer,
                        &plugin,
                        crate::app_state::PluginSensorUpdate::Sample(sensor.clone()),
                    )
                    .map_err(|_| {
                        sensor_operation_error(
                            "PermissionDenied",
                            "sensor ownership, grant or state update denied",
                        )
                    })?;
                if is_self {
                    sync_sensor_observer(store, &id)?;
                }
                if changed {
                    emit_projection_invalidation(
                        &inner.bus,
                        store
                            .sensor_geometry_invalidation(&id)
                            .map_err(sensor_operation_host)?,
                    );
                }
                return Ok(
                    json!({"accepted":true,"originId":id,"sensorId":sensor.sensor_id,"sampleAtMs":sensor.sample_at_ms}),
                );
            }
            "map.origins.telemetry.upsert" => {
                if is_self {
                    return Err(sensor_operation_error(
                        "PermissionDenied",
                        "plugins cannot replace the controller observer fix",
                    ));
                }
                let position: OriginPosition =
                    sensor_payload(payload.get("position").cloned().unwrap_or(JsonValue::Null))?;
                position
                    .validate()
                    .and_then(|_| r3akt_map_core::sensor::source_time(position.fix_at_ms, now))
                    .map_err(|e| sensor_operation_error("InvalidRequest", e))?;
                SensorEvent::OriginFix(OriginFixEvent {
                    origin_id: id.clone(),
                    position,
                })
            }
            "map.sensors.manifest.upsert" => {
                let manifest: SensorManifest =
                    sensor_payload(payload.get("manifest").cloned().unwrap_or(JsonValue::Null))?;
                manifest
                    .validate()
                    .map_err(|e| sensor_operation_error("InvalidRequest", e))?;
                if manifest.video.is_some()
                    && !crate::app_state::plugin_map_capability_allowed(&plugin, 4, |c| {
                        c.sensor_video_share
                    })
                {
                    return Err(sensor_operation_error(
                        "PermissionDenied",
                        "video metadata requires its separate operator grant",
                    ));
                }
                SensorEvent::Manifest(ManifestEvent {
                    origin_id: id.clone(),
                    manifest,
                    updated_at_ms: now,
                })
            }
            "map.sensors.remove" => {
                let sensor_id = payload
                    .get("sensorId")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        sensor_operation_error("InvalidRequest", "sensorId is required")
                    })?
                    .to_string();
                let owner = store
                    .sensor_component_producer(&id, &format!("sensor:{sensor_id}"))
                    .map_err(sensor_operation_host)?;
                if owner.is_none_or(|p| {
                    p.plugin_id != plugin_id
                        || p.fingerprint != plugin.discovered.publisher_fingerprint
                }) {
                    return Err(sensor_operation_error(
                        "PermissionDenied",
                        "sensor belongs to a different plugin or publisher",
                    ));
                }
                let latest = store
                    .sensor_origin(&id)
                    .map_err(sensor_operation_host)?
                    .map_or(0, |o| o.latest_source_time())
                    .max(
                        store
                            .sensor_last_metadata_time(&id)
                            .map_err(sensor_operation_host)?,
                    );
                SensorEvent::SensorRemoved(SensorRemovedEvent {
                    origin_id: id.clone(),
                    sensor_id,
                    deleted_at_ms: now.max(latest.saturating_add(1)),
                })
            }
            _ => {
                return Err(sensor_operation_error(
                    "UnsupportedOperation",
                    "unknown sensor operation",
                ))
            }
        };
        if store
            .write_plugin_sensor(
                &id,
                &writer,
                &plugin,
                crate::app_state::PluginSensorUpdate::Event(event),
            )
            .map_err(sensor_operation_host)?
        {
            emit_projection_invalidation(
                &inner.bus,
                store
                    .sensor_geometry_invalidation(&id)
                    .map_err(sensor_operation_host)?,
            );
        }
        Ok(json!({"accepted":true,"originId":id}))
    }
}

fn sync_sensor_observer(store: &AppStateStore, id: &str) -> Result<(), (&'static str, String)> {
    if let Some(position) = store.local_observer_fix().map_err(sensor_operation_host)? {
        if position.updated_at_ms > 0 {
            let position = OriginPosition {
                lat: position.lat,
                lon: position.lon,
                alt_m: position.alt,
                altitude_datum: r3akt_map_core::sensor::AltitudeDatum::Unknown,
                fix_at_ms: position.updated_at_ms,
                speed_mps: position.speed,
                course_deg: position.course,
                accuracy_m: position.accuracy,
            };
            store
                .apply_sensor_event(
                    id,
                    &SensorEvent::OriginFix(OriginFixEvent {
                        origin_id: id.to_string(),
                        position,
                    }),
                )
                .map_err(sensor_operation_host)?;
        }
    }
    Ok(())
}
