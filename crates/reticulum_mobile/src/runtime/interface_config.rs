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
fn normalize_rnode_region(region: &str) -> &'static str {
    if region.trim().eq_ignore_ascii_case("EU868") {
        "EU868"
    } else {
        "US915"
    }
}

#[cfg(target_os = "android")]
fn rnode_lora_config(settings: &RnodeSettingsRecord) -> Result<LoraConfig, String> {
    let mut config = LoraConfig::for_region(normalize_rnode_region(&settings.region))
        .into_lora_config_option()
        .unwrap_or_else(LoraConfig::us915_default);
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
        "REM-LF-RURAL-v1" | _ => {
            config.bandwidth_hz = 250_000;
            config.spreading_factor = 11;
            config.coding_rate = 5;
        }
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

            active.store(true, Ordering::Release);
            let context = transport
                .iface_manager()
                .lock()
                .await
                .new_context_with_role_and_mode(adapter, IfaceRole::Unicast, InterfaceMode::Full);
            let iface = *context.channel.address();
            let status_update = new_interface_status(iface, label.clone(), "connected");
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
                NativeRnodeBleKissInterface::spawn(context).await;
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
    if matches!(connection_mode, RnodeConnectionMode::Tcp) {
        return;
    }
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
            RnodeConnectionMode::Tcp => unreachable!(),
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
