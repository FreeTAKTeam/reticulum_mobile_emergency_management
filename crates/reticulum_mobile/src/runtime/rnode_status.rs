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
