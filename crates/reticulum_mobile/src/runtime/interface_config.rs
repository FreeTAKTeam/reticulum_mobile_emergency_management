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

pub(crate) fn validate_shared_radio_settings(
    region: &str,
    profile: &str,
    frequency_hz: u64,
    allow_region_default_frequency: bool,
) -> Result<&'static str, String> {
    let region = normalized_rnode_region(region)?;
    if !matches!(
        profile.trim(),
        "REM-MF-URBAN-v1" | "REM-LF-RURAL-v1" | "REM-LM-EXTREME-v1"
    ) {
        return Err(format!("unsupported RNode LoRa profile: {}", profile.trim()));
    }
    if frequency_hz == 0 && allow_region_default_frequency {
        return Ok(region);
    }
    if !(137_000_000..=3_000_000_000).contains(&frequency_hz) {
        return Err(format!(
            "RNode frequency is outside the supported 137 MHz-3 GHz range: {frequency_hz}"
        ));
    }
    Ok(region)
}

#[cfg(target_os = "android")]
fn rnode_lora_config(settings: &RnodeSettingsRecord) -> Result<LoraConfig, String> {
    let region = validate_shared_radio_settings(
        &settings.region,
        &settings.profile,
        u64::from(settings.frequency_hz),
        true,
    )?;
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
    // RNode interfaces use the hardware transport MTU. Keeping the generic
    // LoRa default (220) rejects normal signed LXMF announces before they ever
    // reach the radio.
    config.max_payload_bytes = 508;
    config.validate_rnode()?;
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
    endpoint: String,
    mode: AndroidRnodeMode,
    device_id: String,
    lora: LoraConfig,
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
    let connection_mode = RnodeConnectionMode::parse(Some(&settings.connection_mode))
        .map_err(|error| error.to_string())?;
    let (mode, scheme, max_write_len) = match connection_mode {
        RnodeConnectionMode::Ble => (AndroidRnodeMode::Ble, "ble", 20),
        RnodeConnectionMode::BluetoothClassic => {
            (AndroidRnodeMode::BluetoothClassic, "bluetooth-classic", 4 * 1024)
        }
        RnodeConnectionMode::Usb | RnodeConnectionMode::Tcp => {
            return Err(format!(
                "RNode {} is not an Android Bluetooth bearer",
                connection_mode.as_str()
            ));
        }
    };
    let label = if settings.display_name.trim().is_empty() {
        format!("rnode-{scheme}:{peripheral_id}")
    } else {
        format!("rnode-{scheme}:{}", settings.display_name.trim())
    };
    let endpoint = format!("{scheme}://{peripheral_id}");
    let kiss = RnodeBleKissConfig {
        mtu: usize::from(lora.max_payload_bytes),
        max_write_len,
        read_frame_timeout: RNODE_BLE_READ_FRAME_TIMEOUT,
        initial_frames: lora.probe_frames(),
        deferred_frames: lora.radio_config_frames(),
        shutdown_frames: lora.shutdown_frames(),
        ..RnodeBleKissConfig::default()
    };

    Ok(RnodeBleWiring {
        label,
        endpoint,
        mode,
        device_id: peripheral_id,
        lora,
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
                "RNode Bluetooth/KISS startup did not report a detected online radio within 30 seconds"
                    .to_string(),
            ),
        )
    } else {
        ("connecting", None)
    }
}

#[cfg(target_os = "android")]
async fn monitor_rnode_runtime_status(
    runtime_status: RnodeBearerRuntimeStatusHandle,
    iface: AddressHash,
    label: String,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    let startup_deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        publish_rnode_runtime_status(
            &runtime_status,
            iface,
            &label,
            &active_interface_registry,
            &status,
            &bus,
            tokio::time::Instant::now() >= startup_deadline,
        )
        .await;
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
        RnodeConnectionMode::Ble | RnodeConnectionMode::BluetoothClassic => {}
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
            let mode = wiring.mode;
            let endpoint = wiring.endpoint;
            let backend = AndroidRnodeBackend::new(mode, wiring.device_id);
            let generation = backend.generation();
            let adapter = RnodeBearerKissInterface::new(
                label.clone(),
                endpoint,
                backend,
                wiring.kiss,
                wiring.lora,
            );
            let runtime_status = adapter.runtime_status_handle();

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
            {
                let mut registry = active_interface_registry.lock().await;
                registry.retain(|_, record| record.label != label);
                registry.insert(iface, status_update.clone());
            }
            publish_interface_registry_snapshot(
                &active_interface_registry,
                &status,
                &bus,
                Some(status_update),
            )
            .await;
            info!(
                "rnode_bluetooth: configured label={} peripheral={} mode={} generation={} region={} profile={} iface={}",
                label,
                peripheral_id,
                settings.connection_mode,
                generation,
                settings.region,
                settings.profile,
                iface
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
                let runtime_status_for_monitor = runtime_status.clone();
                let runtime_monitor = tokio::spawn(monitor_rnode_runtime_status(
                    runtime_status_for_monitor,
                    iface,
                    label_for_task.clone(),
                    registry_for_task.clone(),
                    status_for_task.clone(),
                    bus_for_task.clone(),
                ));
                RnodeBearerKissInterface::spawn(context).await;
                runtime_monitor.abort();
                if let Err(error) = runtime_monitor.await {
                    if !error.is_cancelled() {
                        warn!(
                            "rnode_ble: runtime status monitor failed label={} iface={} error={}",
                            label_for_task, iface, error
                        );
                    }
                }
                // The interface can fail between monitor polls. Publish one final sample and
                // retain it until the next retry starts so the actionable startup error is not
                // replaced by a generic disconnected/pending state.
                publish_rnode_runtime_status(
                    &runtime_status,
                    iface,
                    &label_for_task,
                    &registry_for_task,
                    &status_for_task,
                    &bus_for_task,
                    true,
                )
                .await;
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
