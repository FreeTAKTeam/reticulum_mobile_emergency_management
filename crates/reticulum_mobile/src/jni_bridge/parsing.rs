fn bridge_state() -> &'static Mutex<BridgeState> {
    static STATE: OnceLock<Mutex<BridgeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BridgeState::default()))
}

fn last_error() -> &'static Mutex<Option<LastError>> {
    static LAST_ERROR: OnceLock<Mutex<Option<LastError>>> = OnceLock::new();
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

fn set_last_error(code: impl Into<String>, message: impl Into<String>) {
    if let Ok(mut guard) = last_error().lock() {
        *guard = Some(LastError {
            code: code.into(),
            message: message.into(),
        });
    }
}

fn clear_last_error() {
    if let Ok(mut guard) = last_error().lock() {
        *guard = None;
    }
}

fn set_last_node_error(err: NodeError) {
    let code = node_error_code(&err).to_string();
    let message = err.to_string();
    set_last_error(code, message);
}

fn node_error_code(err: &NodeError) -> &'static str {
    match err {
        NodeError::InvalidConfig {} => "InvalidConfig",
        NodeError::IoError {} => "IoError",
        NodeError::NetworkError {} => "NetworkError",
        NodeError::ReticulumError {} => "ReticulumError",
        NodeError::AlreadyRunning {} => "AlreadyRunning",
        NodeError::NotRunning {} => "NotRunning",
        NodeError::Timeout {} => "Timeout",
        NodeError::LxmfWireEncodeError {} => "LxmfWireEncodeError",
        NodeError::LxmfMessageIdParseError {} => "LxmfMessageIdParseError",
        NodeError::LxmfPacketTooLarge {} => "LxmfPacketTooLarge",
        NodeError::LxmfPacketBuildError {} => "LxmfPacketBuildError",
        NodeError::EventStreamClosed {} => "EventStreamClosed",
        NodeError::InternalError {} => "InternalError",
    }
}

fn jstring_to_rust(env: &mut JNIEnv, value: JString) -> Result<String, String> {
    env.get_string(&value)
        .map_err(|e| format!("jni string conversion failed: {e}"))
        .map(|s| s.into())
}

fn make_jstring_or_null(env: &mut JNIEnv, value: String) -> jstring {
    match env.new_string(value) {
        Ok(output) => output.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn parse_hub_mode(value: Option<&str>) -> HubMode {
    match value
        .unwrap_or("Autonomous")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "connected" => HubMode::Connected {},
        "semiautonomous" | "semi_autonomous" | "semi-autonomous" | "rchlxmf" | "rch_lxmf"
        | "rchhttp" | "rch_http" => HubMode::SemiAutonomous {},
        _ => HubMode::Autonomous {},
    }
}

fn parse_log_level(value: Option<&str>) -> LogLevel {
    match value.unwrap_or("Info").trim().to_ascii_lowercase().as_str() {
        "trace" => LogLevel::Trace {},
        "debug" => LogLevel::Debug {},
        "warn" => LogLevel::Warn {},
        "error" => LogLevel::Error {},
        _ => LogLevel::Info {},
    }
}

fn normalize_rnode_region(value: Option<String>) -> String {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "EU868" => "EU868".to_string(),
        _ => "US915".to_string(),
    }
}

fn normalize_rnode_profile(value: Option<String>) -> String {
    match value.unwrap_or_default().trim() {
        "REM-MF-URBAN-v1" => "REM-MF-URBAN-v1".to_string(),
        "REM-LM-EXTREME-v1" => "REM-LM-EXTREME-v1".to_string(),
        _ => "REM-LF-RURAL-v1".to_string(),
    }
}

fn normalize_rnode_connection_mode(value: Option<String>) -> Result<String, NodeError> {
    RnodeConnectionMode::parse(value.as_deref()).map(|mode| mode.as_str().to_string())
}

fn to_rnode_settings_record(
    input: Option<RnodeSettingsInput>,
) -> Result<RnodeSettingsRecord, NodeError> {
    let input = input.unwrap_or_default();
    Ok(RnodeSettingsRecord {
        enabled: input.enabled.unwrap_or(false),
        connection_mode: normalize_rnode_connection_mode(input.connection_mode)?,
        peripheral_id: input.peripheral_id.unwrap_or_default().trim().to_string(),
        display_name: input.display_name.unwrap_or_default().trim().to_string(),
        region: normalize_rnode_region(input.region),
        profile: normalize_rnode_profile(input.profile),
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
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
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
        hub_identity_hash: input.hub_identity_hash.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_api_base_url: input.hub_api_base_url.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_api_key: input.hub_api_key.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        hub_refresh_interval_seconds: input.hub_refresh_interval_seconds.unwrap_or(3600).max(1),
        rnode: to_rnode_settings_record(input.rnode)?,
    })
}

#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_initializeStorage(
    mut env: JNIEnv,
    _class: JClass,
    storage_dir: JString,
) -> jint {
    clear_last_error();
    let raw = match jstring_to_rust(&mut env, storage_dir) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return RESULT_ERR;
        }
    };

    let storage_dir = {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };

    let mut guard = match bridge_state().lock() {
        Ok(guard) => guard,
        Err(_) => {
            set_last_error("InternalError", "bridge lock poisoned");
            return RESULT_ERR;
        }
    };

    let node = match ensure_node_with_storage(&mut guard, storage_dir.as_deref()) {
        Ok(node) => node,
        Err(error) => {
            set_last_node_error(error);
            return RESULT_ERR;
        }
    };
    match node.initialize_storage(storage_dir.as_deref()) {
        Ok(()) => RESULT_OK,
        Err(err) => {
            set_last_node_error(err);
            RESULT_ERR
        }
    }
}

fn parse_message_direction(value: &str) -> Result<MessageDirection, NodeError> {
    match value.trim() {
        "Inbound" => Ok(MessageDirection::Inbound {}),
        "Outbound" => Ok(MessageDirection::Outbound {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_message_method(value: &str) -> Result<MessageMethod, NodeError> {
    match value.trim() {
        "Direct" => Ok(MessageMethod::Direct {}),
        "Opportunistic" => Ok(MessageMethod::Opportunistic {}),
        "Propagated" => Ok(MessageMethod::Propagated {}),
        "Resource" => Ok(MessageMethod::Resource {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_message_state(value: &str) -> Result<MessageState, NodeError> {
    match value.trim() {
        "Queued" => Ok(MessageState::Queued {}),
        "PathRequested" => Ok(MessageState::PathRequested {}),
        "LinkEstablishing" => Ok(MessageState::LinkEstablishing {}),
        "Sending" => Ok(MessageState::Sending {}),
        "SentDirect" => Ok(MessageState::SentDirect {}),
        "SentToPropagation" => Ok(MessageState::SentToPropagation {}),
        "Delivered" => Ok(MessageState::Delivered {}),
        "Failed" => Ok(MessageState::Failed {}),
        "TimedOut" => Ok(MessageState::TimedOut {}),
        "Cancelled" => Ok(MessageState::Cancelled {}),
        "Received" => Ok(MessageState::Received {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_transport_delivery_state(
    value: Option<&str>,
) -> Result<TransportDeliveryState, NodeError> {
    match value.unwrap_or("Queued").trim() {
        "Queued" => Ok(TransportDeliveryState::Queued {}),
        "Sending" => Ok(TransportDeliveryState::Sending {}),
        "SentDirect" => Ok(TransportDeliveryState::SentDirect {}),
        "SentToPropagation" => Ok(TransportDeliveryState::SentToPropagation {}),
        "TransportDelivered" => Ok(TransportDeliveryState::TransportDelivered {}),
        "Failed" => Ok(TransportDeliveryState::Failed {}),
        "TimedOut" => Ok(TransportDeliveryState::TimedOut {}),
        "Cancelled" => Ok(TransportDeliveryState::Cancelled {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_application_ack_state(value: Option<&str>) -> Result<ApplicationAckState, NodeError> {
    match value.unwrap_or("NotRequired").trim() {
        "NotRequired" => Ok(ApplicationAckState::NotRequired {}),
        "Waiting" => Ok(ApplicationAckState::Waiting {}),
        "Accepted" => Ok(ApplicationAckState::Accepted {}),
        "Completed" => Ok(ApplicationAckState::Completed {}),
        "Rejected" => Ok(ApplicationAckState::Rejected {}),
        "Failed" => Ok(ApplicationAckState::Failed {}),
        _ => Err(NodeError::InvalidConfig {}),
    }
}

fn parse_sos_trigger_source(value: Option<&str>) -> SosTriggerSource {
    match value
        .unwrap_or("Manual")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "floatingbutton" | "floating_button" | "floating-button" => {
            SosTriggerSource::FloatingButton {}
        }
        "shake" => SosTriggerSource::Shake {},
        "tappattern" | "tap_pattern" | "tap-pattern" => SosTriggerSource::TapPattern {},
        "powerbutton" | "power_button" | "power-button" => SosTriggerSource::PowerButton {},
        "restore" => SosTriggerSource::Restore {},
        "remote" => SosTriggerSource::Remote {},
        _ => SosTriggerSource::Manual {},
    }
}

fn trimmed_non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn to_saved_peer_record(input: SavedPeerInput) -> SavedPeerRecord {
    SavedPeerRecord {
        destination_hex: input.destination.trim().to_ascii_lowercase(),
        label: trimmed_non_empty(input.label),
        saved_at_ms: input.saved_at,
        identity_hex: trimmed_non_empty(input.identity_hex).map(|value| value.to_ascii_lowercase()),
        lxmf_destination_hex: trimmed_non_empty(input.lxmf_destination_hex)
            .map(|value| value.to_ascii_lowercase()),
        app_data: trimmed_non_empty(input.app_data),
        display_name: trimmed_non_empty(input.display_name),
        last_route_seen_at_ms: input.last_route_seen_at_ms,
        last_hops: input.last_hops,
    }
}

fn to_sos_settings_record(input: SosSettingsInput) -> SosSettingsRecord {
    SosSettingsRecord {
        enabled: input.enabled,
        message_template: input.message_template,
        cancel_message_template: input.cancel_message_template,
        countdown_seconds: input.countdown_seconds,
        include_location: input.include_location,
        trigger_shake: input.trigger_shake,
        trigger_tap_pattern: input.trigger_tap_pattern,
        trigger_power_button: input.trigger_power_button,
        shake_sensitivity: input.shake_sensitivity,
        audio_recording: input.audio_recording,
        audio_duration_seconds: input.audio_duration_seconds,
        periodic_updates: input.periodic_updates,
        update_interval_seconds: input.update_interval_seconds,
        floating_button: input.floating_button,
        silent_auto_answer: input.silent_auto_answer,
        deactivation_pin_hash: input.deactivation_pin_hash,
        deactivation_pin_salt: input.deactivation_pin_salt,
        floating_button_x: input.floating_button_x,
        floating_button_y: input.floating_button_y,
        active_pill_x: input.active_pill_x,
        active_pill_y: input.active_pill_y,
    }
}

fn to_sos_telemetry_record(input: SosTelemetryInput) -> SosDeviceTelemetryRecord {
    SosDeviceTelemetryRecord {
        lat: input.lat,
        lon: input.lon,
        alt: input.alt,
        speed: input.speed,
        course: input.course,
        accuracy: input.accuracy,
        battery_percent: input.battery_percent,
        battery_charging: input.battery_charging,
        updated_at_ms: input.updated_at_ms.unwrap_or_else(crate::runtime::now_ms),
    }
}
