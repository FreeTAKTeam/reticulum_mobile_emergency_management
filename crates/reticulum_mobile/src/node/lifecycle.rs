impl Node {
    pub fn new() -> Result<Self, NodeError> {
        Self::with_storage_dir(None)
    }

    pub(crate) fn with_storage_dir(storage_dir: Option<&str>) -> Result<Self, NodeError> {
        NodeLogger::install();
        let (power_saver_tx, _) = watch::channel(false);

        let initial = NodeStatus {
            running: false,
            name: "reticulum-mobile".to_string(),
            identity_hex: String::new(),
            app_destination_hex: String::new(),
            lxmf_destination_hex: String::new(),
            readiness: RuntimeReadinessSnapshot::default(),
            interfaces: Vec::new(),
        };

        Ok(Self {
            inner: Mutex::new(NodeInner {
                app_state: create_app_state_store(storage_dir)?,
                bus: EventBus::new(),
                status: Arc::new(Mutex::new(initial)),
                peers_snapshot: Arc::new(Mutex::new(Vec::new())),
                sync_status_snapshot: Arc::new(Mutex::new(SyncStatus {
                    phase: crate::types::SyncPhase::Idle {},
                    active_propagation_node_hex: None,
                    requested_at_ms: None,
                    completed_at_ms: None,
                    messages_received: 0,
                    detail: None,
                })),
                hub_directory_snapshot: Arc::new(Mutex::new(None)),
                sos_device_telemetry: Arc::new(Mutex::new(None)),
                sos_detector: Arc::new(Mutex::new(SosTriggerDetector::new())),
                power_state: PowerStateRecord::default(),
                power_saver_tx,
                next_telemetry_publish_at_ms: None,
                deferred_announce_capabilities: None,
                community_status_sent_in_saver: false,
                active_config: None,
                runtime: None,
                cmd_tx: None,
                priority_cmd_tx: None,
            }),
        })
    }

    pub(crate) fn initialize_storage(&self, storage_dir: Option<&str>) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        if inner.runtime.is_some() {
            return Ok(());
        }
        inner.app_state = create_app_state_store(storage_dir)?;
        Ok(())
    }

    fn start_fresh(
        &self,
        config: NodeConfig,
        config_fingerprint: NodeConfigFingerprint,
    ) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        if inner.runtime.is_some() {
            return Err(NodeError::AlreadyRunning {});
        }

        let identity = load_or_create_identity(config.storage_dir.as_deref(), &config.name)?;

        let app_hash = SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .desc
        .address_hash;
        let lxmf_hash = SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .desc
        .address_hash;

        if let Ok(mut guard) = inner.status.lock() {
            *guard = NodeStatus {
                running: false,
                name: config.name.clone(),
                identity_hex: identity.address_hash().to_hex_string(),
                app_destination_hex: app_hash.to_hex_string(),
                lxmf_destination_hex: lxmf_hash.to_hex_string(),
                readiness: RuntimeReadinessSnapshot::for_config(&config)?,
                interfaces: Vec::new(),
            };
        }

        let prestart_state = {
            let legacy_import_completed = inner.app_state.legacy_import_completed()?;
            let app_settings = inner.app_state.get_app_settings()?;
            let saved_peers = inner.app_state.get_saved_peers()?;
            let eams = inner.app_state.get_eams()?;
            let events = inner.app_state.get_events()?;
            let messages = inner.app_state.list_messages(None)?;
            let telemetry_positions = inner.app_state.get_telemetry_positions()?;

            if legacy_import_completed
                || app_settings.is_some()
                || !saved_peers.is_empty()
                || !eams.is_empty()
                || !events.is_empty()
                || !messages.is_empty()
                || !telemetry_positions.is_empty()
            {
                Some(LegacyImportPayload {
                    settings: app_settings,
                    saved_peers,
                    eams,
                    events,
                    messages,
                    telemetry_positions,
                })
            } else {
                None
            }
        };

        inner.app_state = create_app_state_store(config.storage_dir.as_deref())?;
        if let Some(prestart_state) = prestart_state {
            inner.app_state.import_legacy_state(&prestart_state)?;
        }
        let restored_hub_directory = restored_hub_directory_for_config(&inner.app_state, &config)?;
        if let Ok(mut snapshot) = inner.hub_directory_snapshot.lock() {
            *snapshot = restored_hub_directory;
        }

        // Forward Rust logs to the UI event bus.
        NodeLogger::global().set_bus(Some(inner.bus.clone()));

        if let Ok(guard) = inner.status.lock() {
            inner.bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }

        let runtime = build_node_runtime()?;
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let (priority_cmd_tx, priority_cmd_rx) = mpsc::channel(PRIORITY_COMMAND_QUEUE_CAPACITY);

        runtime.spawn(run_node(
            config,
            identity,
            inner.app_state.clone(),
            inner.status.clone(),
            inner.peers_snapshot.clone(),
            inner.sync_status_snapshot.clone(),
            inner.hub_directory_snapshot.clone(),
            inner.bus.clone(),
            inner.power_saver_tx.subscribe(),
            cmd_rx,
            priority_cmd_rx,
        ));

        inner.runtime = Some(runtime);
        inner.cmd_tx = Some(cmd_tx);
        inner.priority_cmd_tx = Some(priority_cmd_tx);
        inner.active_config = Some(config_fingerprint);

        Ok(())
    }

    pub fn start(&self, config: NodeConfig) -> Result<(), NodeError> {
        let config_fingerprint = NodeConfigFingerprint::from_config(&config)?;
        let should_restart = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            match (&inner.runtime, &inner.active_config) {
                (Some(_), Some(active_config)) if active_config == &config_fingerprint => {
                    return Ok(());
                }
                (Some(_), _) => true,
                _ => false,
            }
        };

        if should_restart {
            self.stop()?;
        }

        self.start_fresh(config, config_fingerprint)?;
        let _ = self.publish_community_status();
        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        let (
            runtime,
            cmd_tx,
            priority_cmd_tx,
            bus,
            status,
            peers_snapshot,
            sync_status_snapshot,
            hub_directory_snapshot,
        ) = {
            let mut inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.active_config = None;
            (
                inner.runtime.take(),
                inner.cmd_tx.take(),
                inner.priority_cmd_tx.take(),
                inner.bus.clone(),
                inner.status.clone(),
                inner.peers_snapshot.clone(),
                inner.sync_status_snapshot.clone(),
                inner.hub_directory_snapshot.clone(),
            )
        };

        let Some(runtime) = runtime else {
            return Ok(());
        };

        if let Some(cmd_tx) = priority_cmd_tx.or(cmd_tx) {
            let (tx, rx) = cb::bounded(1);
            let _ = dispatch_command(&cmd_tx, Command::Stop { resp: tx });
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }

        drop(runtime);
        NodeLogger::global().set_bus(None);

        if let Ok(mut guard) = status.lock() {
            guard.running = false;
            guard.refresh_readiness();
            bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }
        if let Ok(mut guard) = peers_snapshot.lock() {
            guard.clear();
        }
        if let Ok(mut guard) = sync_status_snapshot.lock() {
            *guard = SyncStatus {
                phase: crate::types::SyncPhase::Idle {},
                active_propagation_node_hex: None,
                requested_at_ms: None,
                completed_at_ms: None,
                messages_received: 0,
                detail: None,
            };
        }
        if let Ok(mut guard) = hub_directory_snapshot.lock() {
            *guard = None;
        }

        Ok(())
    }

    pub fn restart(&self, config: NodeConfig) -> Result<(), NodeError> {
        self.stop()?;
        self.start(config)
    }

    pub fn connect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Control {})?;
            inner
                .priority_cmd_tx
                .clone()
                .or_else(|| inner.cmd_tx.clone())
                .ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ConnectPeer {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn disconnect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner
                .priority_cmd_tx
                .clone()
                .or_else(|| inner.cmd_tx.clone())
                .ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::DisconnectPeer {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_bytes(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
    ) -> Result<(), NodeError> {
        self.send_bytes_with_class(
            destination_hex,
            bytes,
            fields_bytes,
            send_mode,
            OutboundTrafficClass::Raw {},
        )
    }

    fn send_bytes_with_class(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        send_mode: SendMode,
        traffic_class: OutboundTrafficClass,
    ) -> Result<(), NodeError> {
        let (tx, active_config, hub_directory_snapshot) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(inner.power_state.saver_active, traffic_class)?;
            if !matches!(
                traffic_class,
                OutboundTrafficClass::Sos {} | OutboundTrafficClass::CommunityStatus {}
            ) && peer_is_outer_saved(&inner.app_state.get_saved_peers()?, &destination_hex)
            {
                return Err(NodeError::InvalidConfig {});
            }
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
            )
        };
        let destination_hex = routed_destination_hex(
            destination_hex,
            active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?;
        let fields_bytes = fields_bytes
            .map(|fields| {
                fields_with_active_team(
                    Some(fields),
                    active_team_uid(hub_directory_snapshot.as_ref()),
                )
            })
            .transpose()?;

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SendBytes {
                destination_hex,
                bytes,
                fields_bytes,
                send_mode,
                traffic_class,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn broadcast_bytes(&self, bytes: Vec<u8>) -> Result<(), NodeError> {
        let (
            tx,
            active_config,
            hub_directory_snapshot,
            status,
            peers,
            active_propagation_node_hex,
        ) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Raw {})?;
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let peers = inner
                .peers_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let active_propagation_node_hex = inner
                .sync_status_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .active_propagation_node_hex
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                hub_directory_snapshot,
                status,
                peers,
                active_propagation_node_hex,
            )
        };
        if let Some(config) = active_config.as_ref() {
            match effective_hub_mode(config.hub_mode, hub_directory_snapshot.as_ref()) {
                HubMode::Connected {} => {
                    return self.send_bytes(
                        configured_hub_destination(config)?,
                        bytes,
                        None,
                        SendMode::Auto {},
                    );
                }
                HubMode::SemiAutonomous {} => {
                    let hub_is_configured = config
                        .hub_identity_hash
                        .as_deref()
                        .and_then(normalize_hex_32)
                        .is_some();
                    let Some(snapshot) = hub_directory_snapshot.as_ref() else {
                        return Ok(());
                    };
                    if !hub_is_configured {
                        return Ok(());
                    }
                    let directory_destinations = snapshot
                        .items
                        .iter()
                        .map(|item| item.destination_hash.clone())
                        .collect::<Vec<_>>();
                    let targets = build_transient_replication_targets(
                        &status,
                        peers.as_slice(),
                        directory_destinations.as_slice(),
                        active_propagation_node_hex.as_deref(),
                    );
                    for target in targets {
                        self.send_bytes(
                            target.app_destination_hex,
                            bytes.clone(),
                            None,
                            target.send_mode,
                        )?;
                    }
                    return Ok(());
                }
                HubMode::Autonomous {} => {}
            }
        }

        let (resp_tx, _resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::BroadcastBytes {
                bytes,
                resp: resp_tx,
            },
        )
    }

}
