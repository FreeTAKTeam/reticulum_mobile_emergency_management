impl AppStateStore {
    pub fn list_plugins(&self) -> Result<Vec<InstalledPluginRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT json FROM plugins ORDER BY plugin_id ASC")
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(deserialize_json(&row.map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?)?);
        }
        Ok(records)
    }

    pub fn get_plugin(&self, plugin_id: &str) -> Result<Option<InstalledPluginRecord>, NodeError> {
        let connection = self.connect()?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT json FROM plugins WHERE plugin_id = ?1",
                params![plugin_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        raw.map(|value| deserialize_json(value.as_str()))
            .transpose()
    }

    pub fn list_trusted_plugin_publishers(
        &self,
    ) -> Result<Vec<TrustedPluginPublisherRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT json FROM plugin_publishers ORDER BY fingerprint ASC")
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(deserialize_json(&row.map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?)?);
        }
        Ok(records)
    }

    pub fn approve_plugin_publisher(
        &self,
        plugin_id: &str,
        display_name: Option<&str>,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let plugin = self
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if plugin.state == "Missing" || plugin.state == "Incompatible" {
            return Err(NodeError::InvalidConfig {});
        }
        let fingerprint = normalize_fingerprint(plugin.discovered.publisher_fingerprint.as_str());
        let approved_at_ms = now_ms();
        let publisher = TrustedPluginPublisherRecord {
            fingerprint: fingerprint.clone(),
            display_name: display_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(plugin.discovered.display_name.as_str())
                .to_string(),
            approved_at_ms,
        };
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        transaction
            .execute(
                "INSERT INTO plugin_publishers (fingerprint, json, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(fingerprint) DO UPDATE SET
                    json = excluded.json,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    fingerprint,
                    serialize_json(&publisher)?,
                    crate::numeric::u64_to_i64_saturating(approved_at_ms)
                ],
            )
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        for mut signed_plugin in self.list_plugins()? {
            let signed_by_publisher =
                normalize_fingerprint(signed_plugin.discovered.publisher_fingerprint.as_str())
                    == fingerprint
                    || signed_plugin
                        .discovered
                        .publisher_history
                        .iter()
                        .any(|value| normalize_fingerprint(value.as_str()) == fingerprint);
            if !signed_by_publisher {
                continue;
            }
            signed_plugin.trusted = true;
            signed_plugin.enabled = false;
            signed_plugin.granted_capabilities = PluginCapabilityRecord::default();
            signed_plugin.state = match signed_plugin.state.as_str() {
                "Missing" => "Missing",
                "Incompatible" => "Incompatible",
                _ => "Disabled",
            }
            .to_string();
            signed_plugin.diagnostic = None;
            signed_plugin.updated_at_ms = approved_at_ms;
            write_plugin_tx(&transaction, &signed_plugin)?;
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Plugins {},
            Some(plugin_id.to_string()),
            Some("plugin-publisher-approved".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(invalidation)
    }

    pub fn revoke_plugin_publisher(
        &self,
        fingerprint: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let fingerprint = normalize_fingerprint(fingerprint);
        let plugins = self.list_plugins()?;
        let mut revoked_fingerprints = std::collections::HashSet::from([fingerprint.clone()]);
        for plugin in &plugins {
            let signer_set = std::iter::once(plugin.discovered.publisher_fingerprint.as_str())
                .chain(
                    plugin
                        .discovered
                        .publisher_history
                        .iter()
                        .map(String::as_str),
                )
                .map(normalize_fingerprint)
                .collect::<std::collections::HashSet<_>>();
            if signer_set.contains(fingerprint.as_str()) {
                revoked_fingerprints.extend(signer_set);
            }
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        for revoked in &revoked_fingerprints {
            transaction
                .execute(
                    "DELETE FROM plugin_publishers WHERE fingerprint = ?1",
                    params![revoked],
                )
                .map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::IoError {}, error)
                })?;
        }
        for mut plugin in plugins {
            let matches = std::iter::once(plugin.discovered.publisher_fingerprint.as_str())
                .chain(
                    plugin
                        .discovered
                        .publisher_history
                        .iter()
                        .map(String::as_str),
                )
                .map(normalize_fingerprint)
                .any(|value| revoked_fingerprints.contains(value.as_str()));
            if matches {
                plugin.trusted = false;
                plugin.enabled = false;
                plugin.granted_capabilities = PluginCapabilityRecord::default();
                plugin.state = "Untrusted".to_string();
                plugin.diagnostic = Some("Publisher trust was revoked".to_string());
                plugin.updated_at_ms = now_ms();
                write_plugin_tx(&transaction, &plugin)?;
            }
        }
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Plugins {},
            None,
            Some("plugin-publisher-revoked".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(invalidation)
    }

    pub fn set_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut plugin = self
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if enabled
            && (!plugin.trusted || plugin.state == "Incompatible" || plugin.state == "Missing")
        {
            return Err(NodeError::InvalidConfig {});
        }
        plugin.enabled = enabled;
        plugin.state = if enabled { "Stopped" } else { "Disabled" }.to_string();
        plugin.diagnostic = None;
        plugin.updated_at_ms = now_ms();
        self.write_plugin_with_invalidation(plugin, "plugin-enabled-updated")
    }

    pub fn grant_plugin_capabilities(
        &self,
        plugin_id: &str,
        granted: PluginCapabilityRecord,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut plugin = self
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin.trusted || !granted.is_subset_of(&plugin.discovered.declared_capabilities) {
            return Err(NodeError::InvalidConfig {});
        }
        plugin.granted_capabilities = granted;
        plugin.updated_at_ms = now_ms();
        self.write_plugin_with_invalidation(plugin, "plugin-capabilities-updated")
    }

    pub fn set_plugin_runtime_state(
        &self,
        plugin_id: &str,
        state: &str,
        diagnostic: Option<String>,
    ) -> Result<ProjectionInvalidation, NodeError> {
        if !matches!(state, "Binding" | "Running" | "Stopped" | "Failed") {
            return Err(NodeError::InvalidConfig {});
        }
        let mut plugin = self
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin.trusted || !plugin.enabled {
            return Err(NodeError::InvalidConfig {});
        }
        plugin.state = state.to_string();
        plugin.diagnostic = diagnostic;
        plugin.updated_at_ms = now_ms();
        self.write_plugin_with_invalidation(plugin, "plugin-runtime-state-updated")
    }

    pub fn list_plugin_sensors(&self) -> Result<Vec<PluginSensorRecord>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT json FROM plugin_sensors
                 ORDER BY sample_at_ms DESC, plugin_id ASC, device_id ASC, sensor_type ASC",
            )
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let current = now_ms();
        let mut records = Vec::new();
        for row in rows {
            let mut record: PluginSensorRecord = deserialize_json(&row.map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?)?;
            record.status = sensor_status(
                record.connection_state.as_deref(),
                record.sample_at_ms,
                record.stale_after_ms,
                current,
            )
            .to_string();
            records.push(record);
        }
        Ok(records)
    }

    pub fn record_plugin_sensor(
        &self,
        plugin_id: &str,
        sample: PluginSensorSampleRequest,
    ) -> Result<(PluginSensorRecord, ProjectionInvalidation), NodeError> {
        let plugin = self
            .get_plugin(plugin_id)?
            .ok_or(NodeError::InvalidConfig {})?;
        if !plugin.trusted
            || !plugin.enabled
            || !plugin.discovered.declared_capabilities.sensors_publish
            || !plugin.granted_capabilities.sensors_publish
        {
            return Err(NodeError::InvalidConfig {});
        }
        validate_sensor_sample(&sample)?;
        let current = now_ms();
        let status = sensor_status(
            sample.connection_state.as_deref(),
            sample.timestamp_ms,
            sample.stale_after_ms,
            current,
        );
        let record = PluginSensorRecord {
            plugin_id: plugin_id.to_string(),
            device_id: sample.device_id.trim().to_string(),
            sensor_type: sample.sensor_type.trim().to_string(),
            display_name: sample.display_name.trim().to_string(),
            value: sample.value,
            unit: normalize_optional_string(sample.unit.as_deref()),
            operator_rns_identity: normalize_optional_string(
                sample.operator_rns_identity.as_deref(),
            ),
            confidence: sample.confidence,
            connection_state: normalize_optional_string(sample.connection_state.as_deref()),
            sample_at_ms: sample.timestamp_ms,
            stale_after_ms: sample.stale_after_ms,
            status: status.to_string(),
            origin: sample.origin.trim().to_ascii_lowercase(),
        };
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        transaction
            .execute(
                "INSERT INTO plugin_sensors (
                    plugin_id, device_id, sensor_type, sample_at_ms, json
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(plugin_id, device_id, sensor_type) DO UPDATE SET
                    sample_at_ms = excluded.sample_at_ms,
                    json = excluded.json",
                params![
                    record.plugin_id,
                    record.device_id,
                    record.sensor_type,
                    crate::numeric::u64_to_i64_saturating(record.sample_at_ms),
                    serialize_json(&record)?
                ],
            )
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::IoError {}, error)
            })?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::PluginSensors {},
            Some(format!(
                "{}:{}:{}",
                record.plugin_id, record.device_id, record.sensor_type
            )),
            Some("plugin-sensor-updated".to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok((record, invalidation))
    }

    fn write_plugin_with_invalidation(
        &self,
        plugin: InstalledPluginRecord,
        reason: &str,
    ) -> Result<ProjectionInvalidation, NodeError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        write_plugin_tx(&transaction, &plugin)?;
        let invalidation = self.bump_projection_revision_tx(
            &transaction,
            ProjectionScope::Plugins {},
            Some(plugin.discovered.plugin_id.clone()),
            Some(reason.to_string()),
        )?;
        transaction.commit().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::IoError {}, error)
        })?;
        Ok(invalidation)
    }
}
