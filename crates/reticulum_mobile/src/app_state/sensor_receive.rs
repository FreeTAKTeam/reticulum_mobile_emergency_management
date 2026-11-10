impl AppStateStore {
    /// The runtime authenticates the LXMF source and current mission policy first.
    pub(crate) fn receive_sensor_frame(
        &self,
        source: &str,
        relay: bool,
        frame: &r3akt_map_core::sensor_frame::SensorFrame,
    ) -> Result<Vec<String>, NodeError> {
        if relay {
            for assertion in &frame.assertions {
                if let Some(existing) = self.sensor_origin(&assertion.origin_id)? {
                    if existing.writer_id != assertion.writer_id
                        || existing.origin_kind != assertion.origin_kind
                    {
                        return Err(NodeError::InvalidConfig {});
                    }
                    continue;
                }
                if assertion.origin_kind != "device" {
                    self.remember_sensor_marker_writer(&assertion.origin_id, &assertion.writer_id)?;
                }
                r3s_store::bind_origin(
                    &self.connect()?,
                    &assertion.origin_id,
                    &assertion.writer_id,
                    &assertion.origin_kind,
                    &assertion.label,
                )
                .map_err(r3s_store_error)?;
            }
        }
        let mut writers = std::collections::BTreeMap::new();
        for id in frame
            .events
            .iter()
            .map(SensorEvent::origin_id)
            .chain(frame.samples.iter().map(|s| s.origin_id.as_str()))
            .chain(frame.expiries.iter().map(|s| s.origin_id.as_str()))
        {
            if writers.contains_key(id) {
                continue;
            }
            let writer = if id == source {
                source.to_string()
            } else {
                let mut binding = self.sensor_marker_writer(id)?;
                if binding.is_none() {
                    binding = self
                        .sensor_origin(id)?
                        .filter(|o| o.origin_kind == "device" && o.origin_id == o.writer_id)
                        .map(|o| o.writer_id);
                }
                let Some(writer) = binding else {
                    continue;
                };
                if writer != source && !relay {
                    continue;
                }
                writer
            };
            if self.sensor_origin(id)?.is_none() {
                self.bind_sensor_origin(id, &writer, id == writer)?;
            }
            writers.insert(id.to_string(), writer);
        }
        let connection = self.connect()?;
        let transaction = rusqlite::Transaction::new_unchecked(
            &connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        let mut changed = std::collections::BTreeSet::new();
        for event in &frame.events {
            if let Some(writer) = writers.get(event.origin_id()) {
                if self.apply_sensor_event_sql(&transaction, writer, event)? {
                    if matches!(event, SensorEvent::OriginRemoved(_)) && event.origin_id() != source
                    {
                        r3akt_map_core::sqlite::delete(&transaction, event.origin_id())
                            .map_err(map_marker_store_error)?;
                    }
                    changed.insert(event.origin_id().to_string());
                }
            }
        }
        for sample in &frame.samples {
            let Some(writer) = writers.get(&sample.origin_id) else {
                continue;
            };
            if sample.sensors.is_empty() {
                continue;
            }
            if r3s_store::upsert_sensors(
                &transaction,
                &sample.origin_id,
                writer,
                &sample.sensors,
                now_ms(),
            )
            .map_err(r3s_store_error)?
            {
                changed.insert(sample.origin_id.clone());
            }
            let time = sample
                .sensors
                .iter()
                .map(|s| s.sample_at_ms)
                .max()
                .ok_or(NodeError::InvalidConfig {})?;
            r3s_store::retain_packed(
                &transaction,
                &sample.origin_id,
                writer,
                &sample.packed_telemeter,
                time,
                now_ms(),
            )
            .map_err(r3s_store_error)?;
        }
        for expiry in &frame.expiries {
            if let Some(writer) = writers.get(&expiry.origin_id) {
                if r3akt_map_core::sensor_expiry::store(&transaction, writer, expiry)
                    .map_err(r3s_store_error)?
                {
                    changed.insert(expiry.origin_id.clone());
                }
            }
        }
        transaction
            .commit()
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        Ok(changed.into_iter().collect())
    }
}
