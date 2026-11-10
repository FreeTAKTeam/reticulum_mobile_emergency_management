#[derive(Debug, Default)]
pub(crate) struct SensorPublicationAttempt {
    pub command_id: String,
    pub route: String,
    pub metadata_hash: String,
    pub attempted_at_ms: u64,
}
impl AppStateStore {
    pub(crate) fn reset_sensor_attempts(&self) -> Result<(), NodeError> {
        self.connect()?.execute("UPDATE r3s_publication SET command_id='',metadata_hash='',route='',attempted_at_ms=0",[]).map_err(|_|NodeError::IoError {})?;
        Ok(())
    }

    pub(crate) fn sensor_last_metadata_time(&self, id: &str) -> Result<u64, NodeError> {
        let time: Option<i64> = self
            .connect()?
            .query_row(
                "SELECT MAX(updated_at_ms) FROM r3s_manifest_publications WHERE origin_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(time.map_or(0, crate::numeric::i64_to_u64_saturating))
    }
    /// The operator's disclosure choice is metadata with its own monotonic host revision.
    pub(crate) fn published_sensor_manifest(
        &self,
        origin: &str,
        manifest: &r3akt_map_core::sensor::SensorManifest,
        source_time: u64,
    ) -> Result<r3akt_map_core::sensor_wire::ManifestEvent, NodeError> {
        let connection = self.connect()?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(|_| NodeError::IoError {})?;
        let json = serialize_json(manifest)?;
        let prior:Option<(String,i64)>=transaction.query_row("SELECT json,updated_at_ms FROM r3s_manifest_publications WHERE origin_id=?1 AND sensor_id=?2",params![origin,manifest.sensor_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|NodeError::IoError {})?;
        let time = match prior {
            Some((old, time)) if old == json => {
                crate::numeric::i64_to_u64_saturating(time).max(source_time)
            }
            Some((_, time)) => now_ms()
                .max(source_time)
                .max(crate::numeric::i64_to_u64_saturating(time).saturating_add(1)),
            None => source_time,
        };
        transaction.execute("INSERT INTO r3s_manifest_publications VALUES (?1,?2,?3,?4) ON CONFLICT(origin_id,sensor_id) DO UPDATE SET json=excluded.json,updated_at_ms=excluded.updated_at_ms",params![origin,manifest.sensor_id,json,crate::numeric::u64_to_i64_saturating(time)]).map_err(|_|NodeError::IoError {})?;
        transaction.commit().map_err(|_| NodeError::IoError {})?;
        Ok(r3akt_map_core::sensor_wire::ManifestEvent {
            origin_id: origin.into(),
            manifest: manifest.clone(),
            updated_at_ms: time,
        })
    }
    pub(crate) fn local_sensor_origins(&self) -> Result<Vec<r3s_store::SensorOrigin>, NodeError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT DISTINCT origin_id FROM r3s_producers ORDER BY origin_id")
            .map_err(|e| crate::error_context::contextual_node_error(NodeError::IoError {}, e))?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|_| NodeError::IoError {})?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| NodeError::IoError {})?;
        ids.iter()
            .filter_map(|id| r3s_store::get(&connection, id).transpose())
            .collect::<Result<Vec<_>, _>>()
            .map_err(r3s_store_error)
    }
    pub(crate) fn sensor_publication_attempt(
        &self,
        id: &str,
        destination: &str,
    ) -> Result<SensorPublicationAttempt, NodeError> {
        self.connect()?.query_row("SELECT command_id,route,metadata_hash,attempted_at_ms FROM r3s_publication WHERE origin_id=?1 AND destination=?2",
            params![id,destination],|r|Ok(SensorPublicationAttempt{command_id:r.get(0)?,route:r.get(1)?,metadata_hash:r.get(2)?,attempted_at_ms:crate::numeric::i64_to_u64_saturating(r.get(3)?)}))
            .optional().map(|r|r.unwrap_or_default()).map_err(|_|NodeError::IoError {})
    }
    pub(crate) fn record_sensor_attempt(
        &self,
        id: &str,
        destination: &str,
        command: &str,
        route: &str,
        metadata_hash: &str,
    ) -> Result<(), NodeError> {
        self.connect()?.execute("INSERT INTO r3s_publication(origin_id,destination,command_id,snapshot_hash,route,metadata_hash,attempted_at_ms)
            VALUES (?1,?2,?3,?6,?4,'',?5) ON CONFLICT(origin_id,destination) DO UPDATE SET command_id=excluded.command_id,snapshot_hash=excluded.snapshot_hash,metadata_hash=CASE WHEN r3s_publication.route<>excluded.route THEN '' ELSE r3s_publication.metadata_hash END,route=excluded.route,attempted_at_ms=excluded.attempted_at_ms",
            params![id,destination,command,route,crate::numeric::u64_to_i64_saturating(now_ms()),metadata_hash]).map_err(|_|NodeError::IoError {})?;
        Ok(())
    }
    pub(crate) fn retry_sensor_attempt(
        &self,
        id: &str,
        destination: &str,
    ) -> Result<(), NodeError> {
        self.connect()?
            .execute(
                "UPDATE r3s_publication SET command_id='' WHERE origin_id=?1 AND destination=?2",
                params![id, destination],
            )
            .map_err(|_| NodeError::IoError {})?;
        Ok(())
    }
    pub(crate) fn record_sensor_delivery(
        &self,
        update: &crate::types::LxmfDeliveryUpdate,
    ) -> Result<(), NodeError> {
        use crate::types::{ApplicationAckState as Ack, LxmfDeliveryStatus as Delivery};
        let Some(command) = update.command_id.as_deref() else {
            return Ok(());
        };
        let mut parts = command.split(':');
        let (Some("r3s"), Some(origin), Some(_snapshot), Some(metadata), Some(route), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Ok(());
        };
        if [metadata, route]
            .iter()
            .any(|v| v.len() != 32 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Ok(());
        }
        let accepted = !matches!(
            update.application_ack_state,
            Ack::Rejected {} | Ack::Failed {}
        ) && matches!(
            update.status,
            Delivery::Delivered {} | Delivery::Acknowledged {}
        );
        let failed = matches!(
            update.status,
            Delivery::Failed {}
                | Delivery::Expired {}
                | Delivery::Evicted {}
                | Delivery::TimedOut {}
        ) || matches!(
            update.application_ack_state,
            Ack::Rejected {} | Ack::Failed {}
        );
        let connection = self.connect()?;
        if accepted {
            connection.execute("UPDATE r3s_publication SET metadata_hash=snapshot_hash WHERE origin_id=?1 AND destination=?2 AND substr(snapshot_hash,1,32)=?3 AND substr(route,1,32)=?4",params![origin,update.destination_hex,metadata,route]).map_err(|_|NodeError::IoError {})?;
        } else if failed {
            connection.execute("UPDATE r3s_publication SET command_id='' WHERE origin_id=?1 AND destination=?2 AND command_id=?3",params![origin,update.destination_hex,command]).map_err(|_|NodeError::IoError {})?;
        }
        Ok(())
    }
}
