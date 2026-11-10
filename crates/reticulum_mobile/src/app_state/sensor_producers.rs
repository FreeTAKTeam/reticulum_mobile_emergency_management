#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub(crate) struct SensorProducer {
    pub plugin_id: String,
    pub fingerprint: String,
}
pub(crate) enum PluginSensorUpdate {
    Sample(r3akt_map_core::sensor::SensorState),
    Event(SensorEvent),
}

impl AppStateStore {
    pub(crate) fn sensor_plugin_owns_origin(
        &self,
        id: &str,
        plugin: &InstalledPluginRecord,
    ) -> Result<bool, NodeError> {
        self.connect()?.prepare("SELECT 1 FROM r3s_plugin_origin_owners WHERE origin_id=?1 AND plugin_id=?2 AND fingerprint=?3")
            .and_then(|mut s|s.exists(params![id,plugin.discovered.plugin_id,plugin.discovered.publisher_fingerprint]))
            .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))
    }

    pub(crate) fn remember_local_sensor_writer(&self, writer: &str) -> Result<(), NodeError> {
        if crate::delivery_policy::normalize_hex_32(writer).as_deref() != Some(writer) {
            return Err(NodeError::InvalidConfig {});
        }
        self.connect()?.execute("INSERT INTO metadata(key,value) VALUES ('r3s_local_writer',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[writer])
            .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))?;
        Ok(())
    }

    pub(crate) fn local_sensor_writer(&self) -> Result<Option<String>, NodeError> {
        self.connect()?
            .query_row(
                "SELECT value FROM metadata WHERE key='r3s_local_writer'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))
    }
    pub(crate) fn sensor_plugin(
        &self,
        id: &str,
        capability: fn(&PluginCapabilityRecord) -> bool,
    ) -> Result<InstalledPluginRecord, NodeError> {
        self.get_plugin(id)?
            .filter(|p| plugin_map_capability_allowed(p, 4, capability))
            .ok_or(NodeError::InvalidConfig {})
    }

    fn claim_sensor_component_sql(
        connection: &Connection,
        origin: &str,
        component: &str,
        plugin: &InstalledPluginRecord,
    ) -> Result<(), NodeError> {
        let count=connection.execute("INSERT INTO r3s_producers(origin_id,component,plugin_id,fingerprint) VALUES (?1,?2,?3,?4)
            ON CONFLICT(origin_id,component) DO UPDATE SET plugin_id=excluded.plugin_id
            WHERE r3s_producers.plugin_id=excluded.plugin_id AND r3s_producers.fingerprint=excluded.fingerprint",
            params![origin,component,plugin.discovered.plugin_id,plugin.discovered.publisher_fingerprint])
            .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))?;
        if count != 1 {
            return Err(NodeError::InvalidConfig {});
        }
        Ok(())
    }

    pub(crate) fn write_plugin_sensor(
        &self,
        id: &str,
        writer: &str,
        plugin: &InstalledPluginRecord,
        update: PluginSensorUpdate,
    ) -> Result<bool, NodeError> {
        let connection = self.connect()?;
        let transaction = rusqlite::Transaction::new_unchecked(
            &connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        let raw: String = transaction
            .query_row(
                "SELECT json FROM plugins WHERE plugin_id=?1",
                [&plugin.discovered.plugin_id],
                |r| r.get(0),
            )
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        let current: InstalledPluginRecord = deserialize_json(&raw)?;
        let position = matches!(
            &update,
            PluginSensorUpdate::Event(SensorEvent::OriginFix(_))
        );
        let capability: fn(&PluginCapabilityRecord) -> bool = if position {
            |c| c.origin_telemetry_publish
        } else {
            |c| c.sensor_geometry_publish
        };
        if current.discovered.publisher_fingerprint != plugin.discovered.publisher_fingerprint
            || !plugin_map_capability_allowed(&current, 4, capability)
        {
            return Err(NodeError::InvalidConfig {});
        }
        let component = match &update {
            PluginSensorUpdate::Sample(s) => format!("sensor:{}", s.sensor_id),
            PluginSensorUpdate::Event(SensorEvent::OriginFix(_)) => "position".into(),
            PluginSensorUpdate::Event(SensorEvent::Manifest(e)) => {
                if e.manifest.video.is_some()
                    && !plugin_map_capability_allowed(&current, 4, |c| c.sensor_video_share)
                {
                    return Err(NodeError::InvalidConfig {});
                }
                format!("sensor:{}", e.manifest.sensor_id)
            }
            PluginSensorUpdate::Event(SensorEvent::SensorRemoved(e)) => {
                format!("sensor:{}", e.sensor_id)
            }
            _ => return Err(NodeError::InvalidConfig {}),
        };
        Self::claim_sensor_component_sql(&transaction, id, &component, &current)?;
        let changed = match &update {
            PluginSensorUpdate::Sample(s) => r3s_store::upsert_sensors(
                &transaction,
                id,
                writer,
                std::slice::from_ref(s),
                now_ms(),
            )
            .map_err(r3s_store_error)?,
            PluginSensorUpdate::Event(event) => {
                self.apply_sensor_event_sql(&transaction, writer, event)?
            }
        };
        transaction
            .commit()
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        Ok(changed)
    }

    pub(crate) fn sensor_producer_ids(&self, origin: &str) -> Result<Vec<String>, NodeError> {
        let connection = self.connect()?;
        let mut statement=connection.prepare("SELECT component FROM r3s_producers WHERE origin_id=?1 AND component LIKE 'sensor:%' ORDER BY component").map_err(|_|NodeError::IoError {})?;
        let components = statement
            .query_map([origin], |r| r.get::<_, String>(0))
            .map_err(|_| NodeError::IoError {})?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| NodeError::IoError {})?;
        Ok(components
            .into_iter()
            .filter_map(|s| s.strip_prefix("sensor:").map(str::to_string))
            .collect())
    }

    pub(crate) fn sensor_component_producer(
        &self,
        origin: &str,
        component: &str,
    ) -> Result<Option<SensorProducer>, NodeError> {
        self.connect()?.query_row("SELECT plugin_id,fingerprint FROM r3s_producers WHERE origin_id=?1 AND component=?2",
            params![origin,component],|r|Ok(SensorProducer{plugin_id:r.get(0)?,fingerprint:r.get(1)?})).optional()
            .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))
    }

    pub(crate) fn sensor_producer_allowed(
        &self,
        producer: &SensorProducer,
        position: bool,
    ) -> Result<Option<InstalledPluginRecord>, NodeError> {
        let capability = if position {
            (|c: &PluginCapabilityRecord| c.origin_telemetry_publish)
                as fn(&PluginCapabilityRecord) -> bool
        } else {
            (|c: &PluginCapabilityRecord| c.sensor_geometry_publish)
                as fn(&PluginCapabilityRecord) -> bool
        };
        Ok(self.get_plugin(&producer.plugin_id)?.filter(|p| {
            p.discovered.publisher_fingerprint == producer.fingerprint
                && plugin_map_capability_allowed(p, 4, capability)
                && plugin_map_capability_allowed(p, 4, |c| c.sensor_geometry_share)
                && plugin_map_capability_allowed(p, 4, |c| c.lxmf_send)
        }))
    }

    fn sensor_video_approval(&self, id: &str, sensor: &str) -> Result<Option<String>, NodeError> {
        let Some(origin) = self.sensor_origin(id)? else {
            return Ok(None);
        };
        let Some(video) = origin.manifests.get(sensor).and_then(|m| m.video.as_ref()) else {
            return Ok(None);
        };
        let Some(producer) = self.sensor_component_producer(id, &format!("sensor:{sensor}"))?
        else {
            return Ok(None);
        };
        let value = serde_json::to_vec(&(video, producer.plugin_id, producer.fingerprint))
            .map_err(|e| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, e)
            })?;
        Ok(Some(hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
            value,
        ))))
    }

    pub(crate) fn sensor_video_approved(&self, id: &str, sensor: &str) -> Result<bool, NodeError> {
        let current = self.sensor_video_approval(id, sensor)?;
        let saved: Option<String> = self
            .connect()?
            .query_row(
                "SELECT approval FROM r3s_video_approvals WHERE origin_id=?1 AND sensor_id=?2",
                params![id, sensor],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        Ok(current.is_some() && current == saved)
    }

    /// Called only by the operator bridge, never the plugin host dispatch.
    pub(crate) fn set_sensor_video_sharing(
        &self,
        id: &str,
        sensor: &str,
        approved: bool,
    ) -> Result<(), NodeError> {
        let connection = self.connect()?;
        if approved {
            let approval = self
                .sensor_video_approval(id, sensor)?
                .ok_or(NodeError::InvalidConfig {})?;
            connection.execute("INSERT INTO r3s_video_approvals(origin_id,sensor_id,approval) VALUES (?1,?2,?3)
                ON CONFLICT(origin_id,sensor_id) DO UPDATE SET approval=excluded.approval",params![id,sensor,approval])
                .map_err(|e|crate::error_context::contextual_node_error(NodeError::IoError {},e))?;
        } else {
            connection
                .execute(
                    "DELETE FROM r3s_video_approvals WHERE origin_id=?1 AND sensor_id=?2",
                    params![id, sensor],
                )
                .map_err(|e| {
                    crate::error_context::contextual_node_error(NodeError::IoError {}, e)
                })?;
        }
        Ok(())
    }
}
