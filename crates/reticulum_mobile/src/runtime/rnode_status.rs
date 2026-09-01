#[cfg(any(target_os = "android", test))]
fn rnode_startup_evidence(snapshot: &serde_json::Value) -> serde_json::Value {
    let field = |pointer: &str| {
        snapshot
            .pointer(pointer)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };
    serde_json::json!({
        "startup_validated": field("/startup_validated"),
        "startup_compatibility_warning": field("/startup_compatibility_warning"),
        "probe": {
            "detected": field("/probe_status/detected"),
            "firmware_version": field("/probe_status/firmware_version/label"),
            "platform": field("/probe_status/platform"),
            "mcu": field("/probe_status/mcu"),
        },
        "configured": {
            "frequency_hz": field("/configured/frequency_hz"),
            "bandwidth_hz": field("/configured/bandwidth_hz"),
            "spreading_factor": field("/configured/spreading_factor"),
            "coding_rate": field("/configured/coding_rate"),
            "tx_power_dbm": field("/configured/tx_power_dbm"),
            "max_payload_bytes": field("/configured/max_payload_bytes"),
        },
        "reported": {
            "frequency_hz": field("/radio_status/frequency_hz"),
            "bandwidth_hz": field("/radio_status/bandwidth_hz"),
            "spreading_factor": field("/radio_status/spreading_factor"),
            "coding_rate": field("/radio_status/coding_rate"),
            "tx_power_dbm": field("/radio_status/tx_power_dbm"),
            "radio_state": field("/radio_status/radio_state"),
        },
        "reported_bitrate_bps": field("/reported_bitrate_bps"),
    })
}

#[cfg(target_os = "android")]
async fn publish_rnode_runtime_status(
    runtime_status: &RnodeBearerRuntimeStatusHandle,
    iface: AddressHash,
    label: &str,
    active_interface_registry: &ActiveInterfaceRegistry,
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    startup_timed_out: bool,
) {
    let snapshot = runtime_status.to_json();
    let (next_state, next_error) =
        rnode_runtime_interface_state(&snapshot, startup_timed_out);
    let update = {
        let mut registry = active_interface_registry.lock().await;
        registry.get_mut(&iface).and_then(|record| {
            if record.state == next_state && record.last_error == next_error {
                return None;
            }
            record.state = next_state.to_string();
            record.last_error = next_error.clone();
            Some(record.clone())
        })
    };
    if let Some(update) = update {
        if next_state == "connected" {
            info!(
                "rnode_ble: startup evidence label={} iface={} evidence={}",
                label,
                iface,
                rnode_startup_evidence(&snapshot)
            );
        }
        info!(
            "rnode_ble: runtime state changed label={} iface={} state={} error={}",
            label,
            iface,
            next_state,
            next_error.as_deref().unwrap_or("none")
        );
        publish_interface_registry_snapshot(active_interface_registry, status, bus, Some(update))
            .await;
    }
}
