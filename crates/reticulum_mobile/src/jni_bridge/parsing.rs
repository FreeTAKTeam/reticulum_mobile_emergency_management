fn bridge_state() -> &'static Mutex<BridgeState> {
    static STATE: OnceLock<Mutex<BridgeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BridgeState::default()))
}

fn set_last_error(code: impl Into<String>, message: impl Into<String>) {
    set_last_error_with_context(code, message, None, None);
}

fn set_last_error_with_context(
    code: impl Into<String>,
    message: impl Into<String>,
    operation: Option<&str>,
    cause: Option<String>,
) {
    let code = code.into();
    LAST_JNI_ERROR.with(|slot| {
        slot.replace(Some(LastError {
            retryable: node_error_code_is_retryable(code.as_str()),
            code,
            message: message.into(),
            operation: operation.map(ToOwned::to_owned),
            cause,
        }));
    });
}

fn clear_last_error() {
    LAST_JNI_ERROR.with(|slot| slot.replace(None));
}

fn take_last_error() -> Option<LastError> {
    LAST_JNI_ERROR.with(|slot| slot.borrow_mut().take())
}
#[cfg(test)]
fn current_last_error() -> Option<LastError> {
    LAST_JNI_ERROR.with(|slot| slot.borrow().clone())
}
fn set_last_node_error(err: NodeError) {
    set_last_node_error_with_operation(None, err);
}
fn set_last_node_error_for(operation: &'static str, err: NodeError) {
    set_last_node_error_with_operation(Some(operation), err);
}
fn set_last_node_error_with_operation(operation: Option<&str>, err: NodeError) {
    let code = crate::error_context::node_error_code(&err);
    if let Some(context) = crate::error_context::take_internal_failure(code) {
        let cause = format!("{}: {}", context.operation, context.cause);
        set_last_error_with_context(
            context.code,
            context.message,
            operation.or(Some(context.operation.as_str())),
            Some(cause),
        );
    } else {
        set_last_error_with_context(code, err.to_string(), operation, None);
    }
}

fn node_error_code_is_retryable(code: &str) -> bool {
    crate::error_context::node_error_code_is_retryable(code)
}
fn contain_jni_panic<T>(operation: &'static str, action: impl FnOnce() -> T) -> T
where
    T: JniNodeFailure,
{
    crate::error_context::clear_internal_failure();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(result) => result,
        Err(payload) => {
            let cause = payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_string());
            set_last_error_with_context(
                "InternalError",
                "native operation failed unexpectedly",
                Some(operation),
                Some(cause),
            );
            T::node_failure()
        }
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

#[jni_boundary]
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
    drop(guard);
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

#[cfg(test)]
mod panic_boundary_tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn panic_is_contained_and_returns_compatible_failure_values() {
        clear_last_error();

        let result: jint = contain_jni_panic("testInt", || panic!("boundary failure"));

        assert_eq!(result, RESULT_ERR);
        let error = current_last_error().expect("panic boundary should set last error");
        assert_eq!(error.code, "InternalError");
        assert_eq!(error.operation.as_deref(), Some("testInt"));
        assert!(!error.retryable);
        assert_eq!(error.cause.as_deref(), Some("boundary failure"));

        let result: jstring = contain_jni_panic("testObject", || panic!("object failure"));

        assert!(result.is_null());
        let error = current_last_error().expect("panic boundary should set last error");
        assert_eq!(error.operation.as_deref(), Some("testObject"));
    }

    #[test]
    fn last_error_envelopes_are_scoped_to_the_calling_thread() {
        clear_last_error();
        let barrier = Arc::new(Barrier::new(2));
        let handles = ["first", "second"].map(|code| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                set_last_error(code, format!("{code} failure"));
                barrier.wait();
                take_last_error().expect("thread should retrieve its own error")
            })
        });

        let errors = handles.map(|handle| handle.join().expect("error thread should finish"));

        assert_eq!(errors[0].code, "first");
        assert_eq!(errors[1].code, "second");
        assert!(take_last_error().is_none());
    }
}
