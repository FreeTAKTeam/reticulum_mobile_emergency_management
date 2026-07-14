fn write_plugin_tx(
    transaction: &Transaction<'_>,
    plugin: &InstalledPluginRecord,
) -> Result<(), NodeError> {
    transaction
        .execute(
            "INSERT INTO plugins (
                plugin_id, package_name, publisher_fingerprint, json, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(plugin_id) DO UPDATE SET
                package_name = excluded.package_name,
                publisher_fingerprint = excluded.publisher_fingerprint,
                json = excluded.json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                plugin.discovered.plugin_id,
                plugin.discovered.package_name,
                plugin.discovered.publisher_fingerprint,
                serialize_json(plugin)?,
                plugin.updated_at_ms as i64
            ],
        )
        .map_err(|_| NodeError::IoError {})?;
    Ok(())
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_discovered_plugin(plugin: &mut DiscoveredPluginRecord) -> Result<(), NodeError> {
    plugin.plugin_id = plugin.plugin_id.trim().to_ascii_lowercase();
    plugin.display_name = plugin.display_name.trim().to_string();
    plugin.version = plugin.version.trim().to_string();
    plugin.package_name = plugin.package_name.trim().to_string();
    plugin.service_class_name = plugin.service_class_name.trim().to_string();
    plugin.publisher_fingerprint = normalize_fingerprint(plugin.publisher_fingerprint.as_str());
    plugin.publisher_history = plugin
        .publisher_history
        .iter()
        .map(|value| normalize_fingerprint(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect();
    plugin.publisher_history.sort();
    plugin.publisher_history.dedup();
    if !crate::plugin_runtime::validate_plugin_id(plugin.plugin_id.as_str())
        || plugin.display_name.is_empty()
        || plugin.version.is_empty()
        || plugin.package_name.is_empty()
        || plugin.service_class_name.is_empty()
        || plugin.publisher_fingerprint.len() != 64
    {
        return Err(NodeError::InvalidConfig {});
    }
    let validation_record = InstalledPluginRecord {
        discovered: plugin.clone(),
        state: "Discovered".to_string(),
        trusted: false,
        enabled: false,
        granted_capabilities: PluginCapabilityRecord::default(),
        diagnostic: None,
        updated_at_ms: 0,
    };
    crate::plugin_runtime::validate_discovered_plugin(&validation_record)
}

fn intersect_capabilities(granted: &mut PluginCapabilityRecord, declared: &PluginCapabilityRecord) {
    granted.events_publish &= declared.events_publish;
    granted.sensors_publish &= declared.sensors_publish;
    granted.lxmf_send &= declared.lxmf_send;
    granted.lxmf_receive &= declared.lxmf_receive;
    granted.notifications_raise &= declared.notifications_raise;
}

fn validate_sensor_sample(sample: &PluginSensorSampleRequest) -> Result<(), NodeError> {
    if sample.device_id.trim().is_empty()
        || sample.sensor_type.trim().is_empty()
        || sample.display_name.trim().is_empty()
        || sample.timestamp_ms == 0
        || sample.stale_after_ms == 0
        || !matches!(
            sample.origin.trim().to_ascii_lowercase().as_str(),
            "local" | "remote"
        )
        || sample
            .confidence
            .is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
    {
        return Err(NodeError::InvalidConfig {});
    }
    if sample.sensor_type == "heart_rate_bpm"
        && sample
            .value
            .as_i64()
            .is_none_or(|value| !(1..=240).contains(&value))
    {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(())
}

fn sensor_status(
    connection_state: Option<&str>,
    sample_at_ms: u64,
    stale_after_ms: u64,
    current_ms: u64,
) -> &'static str {
    if connection_state.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_uppercase().as_str(),
            "DISCONNECTED" | "ERROR" | "UNSUPPORTED"
        )
    }) {
        return "Offline";
    }
    let age_ms = current_ms.saturating_sub(sample_at_ms);
    if age_ms > stale_after_ms.saturating_mul(2) {
        "Offline"
    } else if age_ms > stale_after_ms {
        "Stale"
    } else {
        "Active"
    }
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, NodeError> {
    serde_json::to_string(value).map_err(|_| NodeError::InternalError {})
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, NodeError> {
    serde_json::from_str(value).map_err(|_| NodeError::InternalError {})
}

fn announce_class_name(class: AnnounceClass) -> &'static str {
    match class {
        AnnounceClass::PeerApp {} => "PeerApp",
        AnnounceClass::RchHubServer {} => "RchHubServer",
        AnnounceClass::PropagationNode {} => "PropagationNode",
        AnnounceClass::LxmfDelivery {} => "LxmfDelivery",
        AnnounceClass::Other {} => "Other",
    }
}

fn announce_class_from_name(value: &str) -> AnnounceClass {
    match value.trim().to_ascii_lowercase().as_str() {
        "peerapp" | "peer_app" | "peer-app" => AnnounceClass::PeerApp {},
        "rchhubserver" | "rch_hub_server" | "rch-hub-server" => AnnounceClass::RchHubServer {},
        "propagationnode" | "propagation_node" | "propagation-node" => {
            AnnounceClass::PropagationNode {}
        }
        "lxmfdelivery" | "lxmf_delivery" | "lxmf-delivery" => AnnounceClass::LxmfDelivery {},
        _ => AnnounceClass::Other {},
    }
}
