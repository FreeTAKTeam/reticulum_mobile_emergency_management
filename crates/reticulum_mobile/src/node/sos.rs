impl Node {
    pub fn get_sos_settings(&self) -> Result<SosSettingsRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        Ok(inner
            .app_state
            .get_sos_settings()?
            .map(normalize_sos_settings)
            .unwrap_or_else(default_sos_settings))
    }

    pub fn set_sos_settings(&self, settings: SosSettingsRecord) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let normalized = normalize_sos_settings(settings);
        let invalidation = inner.app_state.set_sos_settings(&normalized)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn set_sos_pin(&self, pin: Option<String>) -> Result<(), NodeError> {
        let mut settings = self.get_sos_settings()?;
        set_pin(&mut settings, pin.as_deref().unwrap_or_default())?;
        self.set_sos_settings(settings)
    }

    pub fn get_sos_status(&self) -> Result<SosStatusRecord, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        Ok(inner
            .app_state
            .get_sos_status()?
            .unwrap_or_else(idle_status))
    }

    pub fn list_sos_alerts(&self) -> Result<Vec<SosAlertRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_sos_alerts()
    }

    pub fn list_sos_locations(&self) -> Result<Vec<SosLocationRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_sos_locations()
    }

    pub fn list_sos_audio(&self) -> Result<Vec<SosAudioRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.list_sos_audio()
    }

    pub fn record_sos_audio(&self, audio: SosAudioRecord) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.upsert_sos_audio(&audio)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        Ok(())
    }

    pub fn submit_sos_device_telemetry(
        &self,
        telemetry: SosDeviceTelemetryRecord,
    ) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        *inner
            .sos_device_telemetry
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))? = Some(telemetry);
        Ok(())
    }

    pub fn submit_sos_accelerometer_sample(
        &self,
        x: f64,
        y: f64,
        z: f64,
        at_ms: u64,
    ) -> Result<Option<SosStatusRecord>, NodeError> {
        let (settings, detector) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            (settings, inner.sos_detector.clone())
        };
        let trigger = detector
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .accelerometer_sample(&settings, x, y, z, at_ms);
        match trigger {
            Some(source) => self.trigger_sos(source).map(Some),
            None => Ok(None),
        }
    }

    pub fn submit_sos_screen_event(
        &self,
        at_ms: u64,
    ) -> Result<Option<SosStatusRecord>, NodeError> {
        let (settings, detector) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            (settings, inner.sos_detector.clone())
        };
        let trigger = detector
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .screen_event(&settings, at_ms);
        match trigger {
            Some(source) => self.trigger_sos(source).map(Some),
            None => Ok(None),
        }
    }

    pub fn trigger_sos(&self, source: SosTriggerSource) -> Result<SosStatusRecord, NodeError> {
        let (
            app_state,
            bus,
            tx,
            status,
            settings,
            saved_peers,
            peers,
            active_propagation_node_hex,
            active_config,
            hub_directory_snapshot,
            telemetry_store,
            current,
        ) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            if !settings.enabled {
                return Err(NodeError::InvalidConfig {});
            }
            let current = inner
                .app_state
                .get_sos_status()?
                .unwrap_or_else(idle_status);
            if !matches!(current.state, SosState::Idle {} | SosState::Active {}) {
                return Ok(current);
            }
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
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            (
                inner.app_state.clone(),
                inner.bus.clone(),
                inner
                    .priority_cmd_tx
                    .clone()
                    .or_else(|| inner.cmd_tx.clone())
                    .ok_or(NodeError::NotRunning {})?,
                status,
                settings,
                inner.app_state.get_saved_peers()?,
                peers,
                active_propagation_node_hex,
                inner.active_config.clone(),
                hub_directory_snapshot,
                inner.sos_device_telemetry.clone(),
                current,
            )
        };

        let rebroadcast_active = matches!(current.state, SosState::Active {});
        let incident_id = if rebroadcast_active {
            current
                .incident_id
                .clone()
                .unwrap_or_else(|| new_incident_id(status.identity_hex.as_str()))
        } else {
            new_incident_id(status.identity_hex.as_str())
        };
        let countdown = if rebroadcast_active {
            0
        } else {
            settings.countdown_seconds
        };
        if countdown > 0 {
            let deadline = now_ms().saturating_add(u64::from(countdown) * 1000);
            let countdown_record = countdown_status(incident_id.clone(), source, deadline);
            emit_sos_status(&app_state, &bus, &countdown_record, "sos-countdown")?;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(u64::from(countdown)));
                if !app_state_has_pending_sos_countdown(&app_state, incident_id.as_str()) {
                    return;
                }
                let telemetry = latest_sos_telemetry(&telemetry_store);
                run_sos_fanout(
                    app_state,
                    bus,
                    tx,
                    status,
                    settings,
                    saved_peers,
                    peers,
                    active_propagation_node_hex,
                    active_config,
                    hub_directory_snapshot,
                    telemetry,
                    incident_id,
                    source,
                    SosMessageKind::Active {},
                );
            });
            return Ok(countdown_record);
        }

        let telemetry = latest_sos_telemetry(&telemetry_store);
        let kind = if rebroadcast_active {
            SosMessageKind::Update {}
        } else {
            SosMessageKind::Active {}
        };
        let active = run_sos_fanout(
            app_state,
            bus,
            tx,
            status,
            settings,
            saved_peers,
            peers,
            active_propagation_node_hex,
            active_config,
            hub_directory_snapshot,
            telemetry,
            incident_id,
            source,
            kind,
        )
        .unwrap_or_else(idle_status);
        Ok(active)
    }

    pub fn deactivate_sos(&self, pin: Option<String>) -> Result<SosStatusRecord, NodeError> {
        let (
            app_state,
            bus,
            tx,
            status,
            settings,
            saved_peers,
            peers,
            active_propagation_node_hex,
            active_config,
            hub_directory_snapshot,
            telemetry,
            current,
        ) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let settings = inner
                .app_state
                .get_sos_settings()?
                .map(normalize_sos_settings)
                .unwrap_or_else(default_sos_settings);
            if !verify_pin(&settings, pin.as_deref()) {
                return Err(NodeError::InvalidConfig {});
            }
            let status = inner
                .status
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let telemetry = inner
                .sos_device_telemetry
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            let current = inner
                .app_state
                .get_sos_status()?
                .unwrap_or_else(idle_status);
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
            let hub_directory_snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            (
                inner.app_state.clone(),
                inner.bus.clone(),
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                status,
                settings,
                inner.app_state.get_saved_peers()?,
                peers,
                active_propagation_node_hex,
                inner.active_config.clone(),
                hub_directory_snapshot,
                telemetry,
                current,
            )
        };
        let incident_id = current
            .incident_id
            .clone()
            .unwrap_or_else(|| new_incident_id(status.identity_hex.as_str()));
        run_sos_fanout(
            app_state.clone(),
            bus.clone(),
            tx,
            status,
            settings,
            saved_peers,
            peers,
            active_propagation_node_hex,
            active_config,
            hub_directory_snapshot,
            telemetry,
            incident_id,
            SosTriggerSource::Manual {},
            SosMessageKind::Cancelled {},
        );
        let idle = idle_status();
        emit_sos_status(&app_state, &bus, &idle, "sos-deactivated")?;
        Ok(idle)
    }

    pub fn get_operational_summary(&self) -> Result<OperationalSummary, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let peers = inner
            .peers_snapshot
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .clone();
        let sync = inner
            .sync_status_snapshot
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .clone();
        let status = inner
            .status
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .clone();
        let persisted_messages = inner.app_state.list_messages(None)?;
        let conversation_count = crate::numeric::usize_to_u32_saturating(
            persisted_messages
                .iter()
                .map(|message| message.conversation_id.clone())
                .collect::<std::collections::HashSet<String>>()
                .len(),
        );
        Ok(OperationalSummary {
            running: status.running,
            peer_count_total: crate::numeric::usize_to_u32_saturating(peers.len()),
            saved_peer_count: crate::numeric::usize_to_u32_saturating(
                inner.app_state.get_saved_peers()?.len(),
            ),
            connected_peer_count: crate::numeric::usize_to_u32_saturating(
                peers.iter().filter(|peer| peer.active_link).count(),
            ),
            conversation_count,
            message_count: crate::numeric::usize_to_u32_saturating(persisted_messages.len()),
            eam_count: crate::numeric::usize_to_u32_saturating(
                inner.app_state.get_eams()?.len(),
            ),
            event_count: crate::numeric::usize_to_u32_saturating(
                inner.app_state.get_events()?.len(),
            ),
            telemetry_count: crate::numeric::usize_to_u32_saturating(
                inner.app_state.get_telemetry_positions()?.len(),
            ),
            active_propagation_node_hex: sync.active_propagation_node_hex,
            updated_at_ms: crate::runtime::now_ms(),
        })
    }

    pub fn subscribe_events(&self) -> Arc<EventSubscription> {
        let rx = self
            .inner
            .lock()
            .map(|inner| inner.bus.subscribe())
            .unwrap_or_else(|_| {
                let (_tx, rx) = cb::unbounded();
                rx
            });
        Arc::new(EventSubscription::new(rx))
    }

}
