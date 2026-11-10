use r3akt_map_core::{sensor_store as r3s_store, sensor_wire::SensorEvent};

fn r3s_store_error(error: r3s_store::StoreError) -> NodeError {
    let category = if matches!(error, r3s_store::StoreError::Invalid(_)) {
        NodeError::InvalidConfig {}
    } else {
        NodeError::IoError {}
    };
    crate::error_context::contextual_node_error(category, error)
}

impl AppStateStore {
    fn initialize_sensor_geometry(connection: &Connection) -> Result<(), NodeError> {
        r3s_store::ensure_schema(connection).map_err(r3s_store_error)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS r3s_marker_writers (
            origin_id TEXT PRIMARY KEY, writer_id TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS r3s_producers (
            origin_id TEXT NOT NULL, component TEXT NOT NULL, plugin_id TEXT NOT NULL,
            fingerprint TEXT NOT NULL, PRIMARY KEY(origin_id,component));
            CREATE TABLE IF NOT EXISTS r3s_plugin_origin_owners (
            origin_id TEXT PRIMARY KEY, plugin_id TEXT NOT NULL, fingerprint TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS r3s_video_approvals (
            origin_id TEXT NOT NULL, sensor_id TEXT NOT NULL, approval TEXT NOT NULL,
            PRIMARY KEY(origin_id,sensor_id));
            CREATE TABLE IF NOT EXISTS r3s_manifest_publications (
            origin_id TEXT NOT NULL,sensor_id TEXT NOT NULL,json TEXT NOT NULL,updated_at_ms INTEGER NOT NULL,
            PRIMARY KEY(origin_id,sensor_id));
            CREATE TABLE IF NOT EXISTS r3s_publication (
            origin_id TEXT NOT NULL, destination TEXT NOT NULL, command_id TEXT NOT NULL,
            snapshot_hash TEXT NOT NULL, route TEXT NOT NULL, metadata_hash TEXT NOT NULL,
            attempted_at_ms INTEGER NOT NULL, PRIMARY KEY(origin_id,destination));")
            .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))?;
        Ok(())
    }

    pub(crate) fn sensor_origin(
        &self,
        id: &str,
    ) -> Result<Option<r3s_store::SensorOrigin>, NodeError> {
        r3s_store::get(&self.connect()?, id).map_err(r3s_store_error)
    }

    pub(crate) fn sensor_origins(&self) -> Result<Vec<r3s_store::SensorOrigin>, NodeError> {
        r3s_store::list(&self.connect()?).map_err(r3s_store_error)
    }

    pub(crate) fn sensor_ttl_ms(&self) -> Result<u64, NodeError> {
        Ok(u64::from(
            self.get_app_settings()?
                .map_or(5, |s| s.telemetry.stale_after_minutes)
                .max(1),
        ) * 60_000)
    }

    pub(crate) fn sensor_origins_json(&self) -> Result<serde_json::Value, NodeError> {
        let ttl = self.sensor_ttl_ms()?;
        let now = now_ms();
        let mut items = Vec::new();
        for origin in self.sensor_origins()? {
            if origin.origin_kind != "device"
                && self.map_marker_core_record(&origin.origin_id)?.is_none()
            {
                continue;
            }
            let mut item = origin.projection(now, ttl).map_err(r3s_store_error)?;
            if let Some(sensors) = item["sensors"].as_array_mut() {
                for sensor in sensors {
                    if let Some(id) = sensor["state"]["sensorId"].as_str() {
                        let producer = self.sensor_component_producer(
                            &origin.origin_id,
                            &format!("sensor:{id}"),
                        )?;
                        if producer.is_some() {
                            sensor["videoSharingApproved"] = serde_json::json!(
                                self.sensor_video_approved(&origin.origin_id, id)?
                            );
                        }
                    }
                }
            }
            items.push(item);
        }
        Ok(serde_json::json!({"items":items,"observedAtMs":now,"ttlMs":ttl}))
    }

    pub(crate) fn remember_sensor_marker_writer(
        &self,
        id: &str,
        writer: &str,
    ) -> Result<(), NodeError> {
        for value in [id, writer] {
            if crate::delivery_policy::normalize_hex_32(value).as_deref() != Some(value) {
                return Err(NodeError::InvalidConfig {});
            }
        }
        let connection = self.connect()?;
        connection
            .execute(
                "INSERT OR IGNORE INTO r3s_marker_writers(origin_id,writer_id) VALUES (?1,?2)",
                params![id, writer],
            )
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        if self.sensor_marker_writer(id)?.as_deref() != Some(writer) {
            return Err(NodeError::InvalidConfig {});
        }
        Ok(())
    }

    pub(crate) fn sensor_marker_writer(&self, id: &str) -> Result<Option<String>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT writer_id FROM r3s_marker_writers WHERE origin_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))
    }

    pub(crate) fn bind_sensor_origin(
        &self,
        id: &str,
        writer: &str,
        self_origin: bool,
    ) -> Result<(), NodeError> {
        let (kind, label) = if self_origin {
            if id != writer {
                return Err(NodeError::InvalidConfig {});
            }
            (
                "device".to_string(),
                if self.local_sensor_writer()?.as_deref() == Some(id) {
                    self.get_app_settings()?
                        .map_or_else(|| id.to_string(), |s| s.display_name)
                } else {
                    id.to_string()
                },
            )
        } else {
            if self.sensor_marker_writer(id)?.as_deref() != Some(writer) {
                return Err(NodeError::InvalidConfig {});
            }
            let marker = self
                .map_marker_core_record(id)?
                .ok_or(NodeError::InvalidConfig {})?;
            let kind = match marker.category.as_str() {
                "uav" | "camera" | "fixed" | "vehicle" => marker.category.clone(),
                _ => "mobile".into(),
            };
            (kind, marker.name)
        };
        r3s_store::bind_origin(&self.connect()?, id, writer, &kind, &label).map_err(r3s_store_error)
    }

    pub(crate) fn apply_sensor_event(
        &self,
        writer: &str,
        event: &SensorEvent,
    ) -> Result<bool, NodeError> {
        self.apply_sensor_event_sql(&self.connect()?, writer, event)
    }

    fn apply_sensor_event_sql(
        &self,
        connection: &Connection,
        writer: &str,
        event: &SensorEvent,
    ) -> Result<bool, NodeError> {
        event.validate().map_err(|e| {
            crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, e)
        })?;
        let now = now_ms();
        match event {
            SensorEvent::OriginFix(e) => {
                let position = r3akt_map_core::Position::new(e.position.lat, e.position.lon)
                    .map_err(|e| {
                        crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, e)
                    })?;
                let changed =
                    r3s_store::upsert_position(connection, &e.origin_id, writer, &e.position, now)
                        .map_err(r3s_store_error)?;
                if changed {
                    if let Some(mut marker) = r3akt_map_core::sqlite::get(connection, &e.origin_id)
                        .map_err(map_marker_store_error)?
                    {
                        marker.move_to(
                            position,
                            crate::numeric::u64_to_i64_saturating(e.position.fix_at_ms),
                        );
                        r3akt_map_core::sqlite::upsert(connection, &marker)
                            .map_err(map_marker_store_error)?;
                    }
                }
                Ok(changed)
            }
            SensorEvent::Manifest(e) => r3s_store::upsert_manifest(
                connection,
                &e.origin_id,
                writer,
                &e.manifest,
                e.updated_at_ms,
                now,
            )
            .map_err(r3s_store_error),
            SensorEvent::SensorRemoved(e) => r3s_store::remove_sensor(
                connection,
                &e.origin_id,
                writer,
                &e.sensor_id,
                e.deleted_at_ms,
                now,
            )
            .map_err(r3s_store_error),
            SensorEvent::OriginRemoved(e) => {
                r3s_store::remove_origin(connection, &e.origin_id, writer, e.deleted_at_ms, now)
                    .map_err(r3s_store_error)
            }
        }
    }

    pub(crate) fn sensor_geometry_invalidation(
        &self,
        id: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        self.bump_projection_revision(
            ProjectionScope::Telemetry {},
            Some(id.to_string()),
            Some("sensor-geometry-updated".into()),
        )
    }

    pub(crate) fn remove_sensor_origin_for_marker(&self, id: &str) -> Result<(), NodeError> {
        if let Some(origin) = self
            .sensor_origin(id)?
            .filter(|o| o.deleted_at_ms.is_none())
        {
            let time = now_ms().max(
                origin
                    .latest_source_time()
                    .max(self.sensor_last_metadata_time(id)?)
                    .saturating_add(1),
            );
            r3s_store::remove_origin(&self.connect()?, id, &origin.writer_id, time, now_ms())
                .map_err(r3s_store_error)?;
        }
        Ok(())
    }
}
