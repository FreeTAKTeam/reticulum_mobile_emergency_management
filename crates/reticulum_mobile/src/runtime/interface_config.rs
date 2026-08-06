fn tcp_endpoint_connect_addr(endpoint: &str) -> &str {
    endpoint
        .trim()
        .strip_prefix("tcp://")
        .unwrap_or_else(|| endpoint.trim())
}

fn configured_tcp_client_endpoints(endpoints: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for endpoint in endpoints {
        let connect_addr = tcp_endpoint_connect_addr(endpoint).trim();
        if connect_addr.is_empty() || normalized.iter().any(|value| value == connect_addr) {
            continue;
        }
        normalized.push(connect_addr.to_string());
    }
    normalized
}

fn tcp_endpoint_host(connect_addr: &str) -> &str {
    connect_addr
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(connect_addr)
        .trim_matches(['[', ']'])
        .trim()
}

fn tcp_endpoint_is_loopback(connect_addr: &str) -> bool {
    let host = tcp_endpoint_host(connect_addr).to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

fn tcp_readiness_monitor_endpoints(endpoints: &[String]) -> Vec<String> {
    endpoints
        .iter()
        .filter(|endpoint| !tcp_endpoint_is_loopback(endpoint))
        .cloned()
        .collect()
}

fn tcp_data_path_unavailable_message(endpoints: &[String]) -> String {
    format!(
        "transport startup failed: no reachable Reticulum TCP interface endpoints={}",
        endpoints.join(",")
    )
}

#[cfg(target_os = "android")]
fn normalized_rnode_region(region: &str) -> Result<&'static str, String> {
    match region.trim().to_ascii_uppercase().as_str() {
        "US915" => Ok("US915"),
        "EU868" => Ok("EU868"),
        "AU915" => Ok("AU915"),
        "AS923" => Ok("AS923"),
        "IN865" => Ok("IN865"),
        "KR920" => Ok("KR920"),
        "RU864" => Ok("RU864"),
        value => Err(format!("unsupported RNode LoRa region: {value}")),
    }
}

#[cfg(target_os = "android")]
fn rnode_lora_config(settings: &RnodeSettingsRecord) -> Result<LoraConfig, String> {
    let region = normalized_rnode_region(&settings.region)?;
    let mut config = LoraConfig::for_region(region)
        .into_lora_config_option()
        .ok_or_else(|| format!("unsupported RNode LoRa region: {region}"))?;
    if settings.frequency_hz > 0 {
        config.frequency_hz = settings.frequency_hz;
    }
    match settings.profile.trim() {
        "REM-MF-URBAN-v1" => {
            config.bandwidth_hz = 250_000;
            config.spreading_factor = 9;
            config.coding_rate = 5;
        }
        "REM-LM-EXTREME-v1" => {
            config.bandwidth_hz = 125_000;
            config.spreading_factor = 11;
            config.coding_rate = 8;
        }
        "REM-LF-RURAL-v1" => {
            config.bandwidth_hz = 250_000;
            config.spreading_factor = 11;
            config.coding_rate = 5;
        }
        value => return Err(format!("unsupported RNode LoRa profile: {value}")),
    }
    config.validate()?;
    Ok(config)
}

#[cfg(target_os = "android")]
trait IntoLoraConfigOption {
    fn into_lora_config_option(self) -> Option<LoraConfig>;
}

#[cfg(target_os = "android")]
impl IntoLoraConfigOption for Option<LoraConfig> {
    fn into_lora_config_option(self) -> Option<LoraConfig> {
        self
    }
}

#[cfg(target_os = "android")]
impl<E> IntoLoraConfigOption for Result<Option<LoraConfig>, E> {
    fn into_lora_config_option(self) -> Option<LoraConfig> {
        self.unwrap_or(None)
    }
}

#[cfg(target_os = "android")]
struct RnodeBleWiring {
    label: String,
    lora: LoraConfig,
    native: NativeRnodeBleSettings,
    kiss: RnodeBleKissConfig,
}

#[cfg(target_os = "android")]
fn rnode_ble_wiring_from_settings(
    settings: &RnodeSettingsRecord,
) -> Result<RnodeBleWiring, String> {
    let peripheral_id = settings.peripheral_id.trim().to_string();
    if peripheral_id.is_empty() {
        return Err("RNode Bluetooth is enabled but no paired device is selected.".to_string());
    }

    let lora = rnode_lora_config(settings)?;
    let label = if settings.display_name.trim().is_empty() {
        format!("rnode-ble:{peripheral_id}")
    } else {
        format!("rnode-ble:{}", settings.display_name.trim())
    };
    let native = NativeRnodeBleSettings::for_peripheral(peripheral_id)
        .with_peripheral_alias(settings.display_name.trim());
    let kiss = RnodeBleKissConfig {
        mtu: usize::from(lora.max_payload_bytes),
        max_write_len: 20,
        read_frame_timeout: RNODE_BLE_READ_FRAME_TIMEOUT,
        initial_frames: lora.probe_frames(),
        deferred_frames: lora.radio_config_frames(),
        shutdown_frames: lora.shutdown_frames(),
        ..RnodeBleKissConfig::default()
    };

    Ok(RnodeBleWiring {
        label,
        lora,
        native,
        kiss,
    })
}

#[cfg(any(target_os = "android", test))]
fn rnode_runtime_interface_state(
    snapshot: &serde_json::Value,
    startup_timed_out: bool,
) -> (&'static str, Option<String>) {
    let detected = snapshot
        .pointer("/probe_status/detected")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let online = snapshot
        .get("online")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let last_error = snapshot
        .get("last_command_error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if detected && online && last_error.is_none() {
        ("connected", None)
    } else if let Some(error) = last_error {
        ("failed", Some(error))
    } else if startup_timed_out {
        (
            "failed",
            Some(
                "RNode BLE/KISS startup did not report a detected online radio within 30 seconds"
                    .to_string(),
            ),
        )
    } else {
        ("connecting", None)
    }
}

#[cfg(target_os = "android")]
async fn monitor_rnode_runtime_status(
    runtime_status: rns_transport::iface::rnode_ble::RnodeBleRuntimeStatusHandle,
    iface: AddressHash,
    label: String,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    let startup_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = runtime_status.to_json();
        let (next_state, next_error) = rnode_runtime_interface_state(
            &snapshot,
            tokio::time::Instant::now() >= startup_deadline,
        );
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
                "rnode_ble: runtime state changed label={} iface={} state={}",
                label, iface, next_state
            );
            publish_interface_registry_snapshot(
                &active_interface_registry,
                &status,
                &bus,
                Some(update),
            )
            .await;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(target_os = "android")]
fn spawn_rnode_ble_interface(
    transport: Arc<Transport>,
    bus: EventBus,
    settings: RnodeSettingsRecord,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
) {
    if !settings.enabled {
        return;
    }
    let connection_mode = match RnodeConnectionMode::parse(Some(&settings.connection_mode)) {
        Ok(mode) => mode,
        Err(error) => {
            set_runtime_interface_readiness(
                &status,
                &bus,
                "rnode",
                RuntimeReadinessState::Failed,
                "RNode configuration is invalid".to_string(),
                Some(error.to_string()),
            );
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: format!("Invalid RNode connection mode: {error}"),
            });
            return;
        }
    };
    match connection_mode {
        RnodeConnectionMode::Ble => {}
        RnodeConnectionMode::BluetoothClassic => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: "RNode Bluetooth Classic/SPP is selected, but the Android SPP backend is not wired into REM yet.".to_string(),
            });
            return;
        }
        RnodeConnectionMode::Usb => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: "RNode USB is selected, but the Android USB serial transport backend is not wired into REM yet.".to_string(),
            });
            return;
        }
        RnodeConnectionMode::Tcp => {
            info!("rnode_ble: RNode TCP mode selected; skipping Android BLE interface spawn");
            return;
        }
    }
    if let Err(error) = rnode_ble_wiring_from_settings(&settings) {
        set_runtime_interface_readiness(
            &status,
            &bus,
            "rnode",
            RuntimeReadinessState::Failed,
            "RNode interface configuration failed".to_string(),
            Some(error.clone()),
        );
        bus.emit(NodeEvent::Error {
            code: "InvalidConfig".to_string(),
            message: if error.starts_with("RNode Bluetooth") {
                error
            } else {
                format!("RNode LoRa profile is invalid: {error}")
            },
        });
        return;
    }
    let peripheral_id = settings.peripheral_id.trim().to_string();

    tokio::spawn(async move {
        let active = Arc::new(AtomicBool::new(false));
        loop {
            if active.load(Ordering::Acquire) {
                tokio::time::sleep(RNODE_BLE_INTERFACE_RETRY_INTERVAL).await;
                continue;
            }

            let wiring = match rnode_ble_wiring_from_settings(&settings) {
                Ok(wiring) => wiring,
                Err(error) => {
                    set_runtime_interface_readiness(
                        &status,
                        &bus,
                        "rnode",
                        RuntimeReadinessState::Failed,
                        "RNode interface configuration failed".to_string(),
                        Some(error.clone()),
                    );
                    bus.emit(NodeEvent::Error {
                        code: "InvalidConfig".to_string(),
                        message: if error.starts_with("RNode Bluetooth") {
                            error
                        } else {
                            format!("RNode LoRa profile is invalid: {error}")
                        },
                    });
                    return;
                }
            };
            let label = wiring.label;
            let adapter =
                NativeRnodeBleKissInterface::new(label.clone(), wiring.native, wiring.kiss)
                    .with_rnode_validation(wiring.lora, Duration::from_millis(15_000))
                    .with_detection_fallback_timeout(Duration::from_millis(5_000));
            let Some(runtime_status) = adapter.runtime_status_handle() else {
                let error = "RNode runtime status monitor was not initialized".to_string();
                set_runtime_interface_readiness(
                    &status,
                    &bus,
                    "rnode",
                    RuntimeReadinessState::Failed,
                    "RNode interface startup failed".to_string(),
                    Some(error.clone()),
                );
                bus.emit(NodeEvent::Error {
                    code: "Internal".to_string(),
                    message: error,
                });
                tokio::time::sleep(RNODE_BLE_INTERFACE_RETRY_INTERVAL).await;
                continue;
            };

            active.store(true, Ordering::Release);
            let context = transport
                .iface_manager()
                .lock()
                .await
                .new_context_with_role_and_mode(adapter, IfaceRole::Unicast, InterfaceMode::Full);
            let iface = *context.channel.address();
            // Creating the context does not mean that Bluetooth or the KISS session is
            // usable. Keep readiness pending until the RNode is detected and online.
            let status_update = new_interface_status(iface, label.clone(), "connecting");
            active_interface_registry
                .lock()
                .await
                .insert(iface, status_update.clone());
            publish_interface_registry_snapshot(
                &active_interface_registry,
                &status,
                &bus,
                Some(status_update),
            )
            .await;
            info!(
                "rnode_ble: configured label={} peripheral={} region={} profile={} iface={}",
                label, peripheral_id, settings.region, settings.profile, iface
            );
            emit_operational_notice(
                &bus,
                LogLevel::Info {},
                format!(
                    "RNode Bluetooth LoRa interface enabled: {} ({}, {})",
                    label, settings.region, settings.profile
                ),
            );

            let active_for_task = active.clone();
            let registry_for_task = active_interface_registry.clone();
            let status_for_task = status.clone();
            let bus_for_task = bus.clone();
            let label_for_task = label.clone();
            tokio::spawn(async move {
                let runtime_monitor = tokio::spawn(monitor_rnode_runtime_status(
                    runtime_status,
                    iface,
                    label_for_task.clone(),
                    registry_for_task.clone(),
                    status_for_task.clone(),
                    bus_for_task.clone(),
                ));
                NativeRnodeBleKissInterface::spawn(context).await;
                runtime_monitor.abort();
                if let Err(error) = runtime_monitor.await {
                    if !error.is_cancelled() {
                        warn!(
                            "rnode_ble: runtime status monitor failed label={} iface={} error={}",
                            label_for_task, iface, error
                        );
                    }
                }
                let removed = registry_for_task.lock().await.remove(&iface);
                if let Some(mut removed) = removed {
                    removed.state = "disconnected".to_string();
                    publish_interface_registry_snapshot(
                        &registry_for_task,
                        &status_for_task,
                        &bus_for_task,
                        Some(removed),
                    )
                    .await;
                }
                active_for_task.store(false, Ordering::Release);
                warn!(
                    "rnode_ble: stopped interface label={} iface={}; retrying",
                    label_for_task, iface
                );
            });

            tokio::time::sleep(RNODE_BLE_INTERFACE_RETRY_INTERVAL).await;
        }
    });
}

#[cfg(not(target_os = "android"))]
fn spawn_rnode_ble_interface(
    _transport: Arc<Transport>,
    bus: EventBus,
    settings: RnodeSettingsRecord,
    _active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
) {
    if !settings.enabled {
        return;
    }
    let connection_mode = match RnodeConnectionMode::parse(Some(&settings.connection_mode)) {
        Ok(mode) => mode,
        Err(error) => {
            bus.emit(NodeEvent::Error {
                code: "InvalidConfig".to_string(),
                message: format!("Invalid RNode connection mode: {error}"),
            });
            return;
        }
    };
    let message = match connection_mode {
            RnodeConnectionMode::Ble => {
                "RNode BLE LoRa is only available on Android builds.".to_string()
            }
            RnodeConnectionMode::BluetoothClassic => {
                "RNode Bluetooth Classic/SPP is only available after a platform SPP backend is configured.".to_string()
            }
            RnodeConnectionMode::Usb => {
                "RNode USB serial is only available after a platform USB backend is configured.".to_string()
            }
            RnodeConnectionMode::Tcp => return,
        };
    set_runtime_interface_readiness(
        &status,
        &bus,
        "rnode",
        RuntimeReadinessState::Unsupported,
        message.clone(),
        Some(message.clone()),
    );
    bus.emit(NodeEvent::Error {
        code: "InvalidConfig".to_string(),
        message,
    });
}
