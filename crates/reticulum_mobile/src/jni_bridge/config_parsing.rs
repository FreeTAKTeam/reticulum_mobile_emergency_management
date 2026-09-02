const RNODE_FREQUENCY_MIN_HZ: u64 = 137_000_000;
const RNODE_FREQUENCY_MAX_HZ: u64 = 3_000_000_000;

fn normalize_rnode_region(value: Option<String>) -> Result<String, NodeError> {
    let normalized = value.unwrap_or_default().trim().to_ascii_uppercase();
    match normalized.as_str() {
        "" | "US915" => Ok("US915".to_string()),
        "EU868" => Ok("EU868".to_string()),
        "AU915" => Ok("AU915".to_string()),
        "AS923" => Ok("AS923".to_string()),
        "IN865" => Ok("IN865".to_string()),
        "KR920" => Ok("KR920".to_string()),
        "RU864" => Ok("RU864".to_string()),
        _ => Err(crate::error_context::contextual_node_error(
            NodeError::InvalidConfig {},
            format!("unsupported RNode LoRa region: {normalized}"),
        )),
    }
}

pub(crate) fn rnode_region_default_frequency_hz(region: &str) -> u64 {
    match region.trim().to_ascii_uppercase().as_str() {
        "EU868" => 868_000_000,
        "AU915" => 915_000_000,
        "AS923" => 923_000_000,
        "IN865" => 865_000_000,
        "KR920" => 920_000_000,
        "RU864" => 864_000_000,
        _ => 915_000_000,
    }
}

fn normalize_rnode_profile(value: Option<String>) -> Result<String, NodeError> {
    let normalized = value.unwrap_or_default().trim().to_string();
    match normalized.as_str() {
        "" | "REM-LF-RURAL-v1" => Ok("REM-LF-RURAL-v1".to_string()),
        "REM-MF-URBAN-v1" => Ok("REM-MF-URBAN-v1".to_string()),
        "REM-LM-EXTREME-v1" => Ok("REM-LM-EXTREME-v1".to_string()),
        _ => Err(crate::error_context::contextual_node_error(
            NodeError::InvalidConfig {},
            format!("unsupported RNode LoRa profile: {normalized}"),
        )),
    }
}

fn normalize_rnode_connection_mode(value: Option<String>) -> Result<String, NodeError> {
    RnodeConnectionMode::parse(value.as_deref()).map(|mode| mode.as_str().to_string())
}

fn to_rnode_settings_record(
    input: Option<RnodeSettingsInput>,
) -> Result<RnodeSettingsRecord, NodeError> {
    let input = input.unwrap_or_default();
    let region = normalize_rnode_region(input.region)?;
    let frequency_hz = match input.frequency_hz {
        Some(value) if (RNODE_FREQUENCY_MIN_HZ..=RNODE_FREQUENCY_MAX_HZ).contains(&value) => {
            value
        }
        Some(0) | None => rnode_region_default_frequency_hz(&region),
        Some(value) => {
            return Err(crate::error_context::contextual_node_error(
                NodeError::InvalidConfig {},
                format!(
                    "RNode LoRa frequency must be between {RNODE_FREQUENCY_MIN_HZ} and {RNODE_FREQUENCY_MAX_HZ} Hz; got {value}"
                ),
            ));
        }
    };
    Ok(RnodeSettingsRecord {
        enabled: input.enabled.unwrap_or(false),
        connection_mode: normalize_rnode_connection_mode(input.connection_mode)?,
        peripheral_id: input.peripheral_id.unwrap_or_default().trim().to_string(),
        display_name: input.display_name.unwrap_or_default().trim().to_string(),
        region,
        profile: normalize_rnode_profile(input.profile)?,
        frequency_hz,
    })
}

fn parse_node_config(input: NodeConfigInput) -> Result<NodeConfig, NodeError> {
    Ok(NodeConfig {
        name: input
            .name
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "emergency-ops-mobile".to_string()),
        storage_dir: input.storage_dir.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        tcp_clients: input
            .tcp_clients
            .unwrap_or_default()
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect(),
        broadcast: input.broadcast.unwrap_or(true),
        transport_node_enabled: input.transport_node_enabled.unwrap_or(true),
        announce_interval_seconds: input.announce_interval_seconds.unwrap_or(1800).max(1),
        stale_after_minutes: input.stale_after_minutes.unwrap_or(30).max(1),
        announce_capabilities: input
            .announce_capabilities
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "R3AKT,EMergencyMessages".to_string()),
        hub_mode: parse_hub_mode(input.hub_mode.as_deref()),
        hub_identity_hash: trimmed_non_empty(input.hub_identity_hash),
        hub_api_base_url: trimmed_non_empty(input.hub_api_base_url),
        hub_api_key: trimmed_non_empty(input.hub_api_key),
        hub_refresh_interval_seconds: input.hub_refresh_interval_seconds.unwrap_or(3600).max(1),
        rnode: to_rnode_settings_record(input.rnode)?,
    })
}
