impl Node {
    pub fn announce_now(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner
                .priority_cmd_tx
                .clone()
                .or_else(|| inner.cmd_tx.clone())
                .ok_or(NodeError::NotRunning {})?
        };

        dispatch_command(&tx, Command::AnnounceNow {})
    }

    pub fn request_peer_identity(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RequestPeerIdentity {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_lxmf(&self, request: SendLxmfRequest) -> Result<String, NodeError> {
        let (tx, active_config, hub_directory_snapshot, app_state, peers_snapshot) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let snapshot = inner
                .hub_directory_snapshot
                .lock()
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                .clone();
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                inner.active_config.clone(),
                snapshot,
                inner.app_state.clone(),
                inner.peers_snapshot.clone(),
            )
        };
        let requested_destination = normalize_hex_32(request.destination_hex.as_str())
            .ok_or(NodeError::InvalidConfig {})?;
        let request = SendLxmfRequest {
            destination_hex: routed_chat_destination_hex(
                requested_destination,
                active_config.as_ref(),
                hub_directory_snapshot.as_ref(),
                || {
                    let messages = app_state.list_messages(None)?;
                    let peers = peers_snapshot
                        .lock()
                        .map(|guard| guard.clone())
                        .map_err(|error| {
                            crate::error_context::contextual_node_error(
                                NodeError::InternalError {},
                                error,
                            )
                        })?;
                    Ok((messages, peers))
                },
            )?,
            ..request
        };
        let fields_bytes = fields_with_active_team(
            None,
            active_team_uid(hub_directory_snapshot.as_ref()),
        )?;

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SendLxmf {
                request,
                fields_bytes,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn retry_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RetryLxmf {
                message_id_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn cancel_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
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
            Command::CancelLxmf {
                message_id_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_active_propagation_node(
        &self,
        destination_hex: Option<String>,
    ) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SetActivePropagationNode {
                destination_hex,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn request_lxmf_sync(&self, limit: Option<u32>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RequestLxmfSync {
                limit,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(LXMF_SYNC_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_announces(&self) -> Result<Vec<AnnounceRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                tx
            } else {
                return inner.app_state.list_announces();
            }
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ListAnnounces { resp: resp_tx },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_peers(&self) -> Result<Vec<PeerRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                tx
            } else {
                return inner
                    .peers_snapshot
                    .lock()
                    .map(|guard| guard.clone())
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error));
            }
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ListPeers { resp: resp_tx },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                tx
            } else {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let resolver = conversation_peer_resolver(&peers);
                return inner.app_state.list_conversations_resolved(&resolver);
            }
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ListConversations { resp: resp_tx },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<String>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                tx
            } else {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let resolver = conversation_peer_resolver(&peers);
                return inner
                    .app_state
                    .list_messages_resolved(conversation_id.as_deref(), &resolver);
            }
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ListMessages {
                conversation_id,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn delete_conversation(&self, conversation_id: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                Some(tx)
            } else {
                let peers = inner
                    .peers_snapshot
                    .lock()
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
                    .clone();
                let resolver = conversation_peer_resolver(&peers);
                for invalidation in inner
                    .app_state
                    .delete_conversation_resolved(conversation_id.as_str(), &resolver)?
                {
                    emit_projection_invalidation(&inner.bus, invalidation);
                }
                None
            }
        };

        if let Some(tx) = tx {
            let (resp_tx, resp_rx) = cb::bounded(1);
            dispatch_command(
                &tx,
                Command::DeleteConversation {
                    conversation_id,
                    resp: resp_tx,
                },
            )?;
            resp_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or(Err(NodeError::Timeout {}))
        } else {
            Ok(())
        }
    }

    pub fn get_lxmf_sync_status(&self) -> Result<SyncStatus, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            if let Some(tx) = inner.cmd_tx.clone() {
                tx
            } else {
                return inner
                    .sync_status_snapshot
                    .lock()
                    .map(|guard| guard.clone())
                    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error));
            }
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::GetLxmfSyncStatus { resp: resp_tx },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_telemetry_destinations(&self) -> Result<Vec<String>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
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
        let hub_directory_snapshot = inner
            .hub_directory_snapshot
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .clone();
        let sync_status = inner
            .sync_status_snapshot
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .clone();
        Ok(build_runtime_telemetry_destinations(
            &status,
            peers.as_slice(),
            sync_status.active_propagation_node_hex.as_deref(),
            inner.active_config.as_ref(),
            hub_directory_snapshot.as_ref(),
        )?
        .into_iter()
        .map(|target| target.app_destination_hex)
        .collect())
    }

    pub fn set_announce_capabilities(&self, capability_string: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::SetAnnounceCapabilities {
                capability_string,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_log_level(&self, level: LogLevel) {
        NodeLogger::global().set_level(level);
        if let Ok(inner) = self.inner.lock() {
            if let Some(tx) = inner.cmd_tx.clone() {
                let _ = tx.try_send(Command::SetLogLevel { level });
            }
        }
    }

    pub fn legacy_import_completed(&self) -> Result<bool, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.legacy_import_completed()
    }

    pub fn import_legacy_state(&self, payload: LegacyImportPayload) -> Result<(), NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidations = inner.app_state.import_legacy_state(&payload)?;
        for invalidation in invalidations {
            emit_projection_invalidation(&inner.bus, invalidation);
        }
        Ok(())
    }

    pub fn get_app_settings(&self) -> Result<Option<AppSettingsRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_app_settings()
    }

    pub fn set_app_settings(&self, mut settings: AppSettingsRecord) -> Result<(), NodeError> {
        normalize_team_settings(&mut settings.teams)?;
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        let invalidation = inner.app_state.set_app_settings(&settings)?;
        emit_projection_invalidation(&inner.bus, invalidation);
        let mut snapshot = inner.hub_directory_snapshot.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let mut next_snapshot = snapshot
            .clone()
            .unwrap_or_else(|| HubDirectorySnapshot::yellow_only(crate::runtime::now_ms()));
        apply_local_team_settings(&mut next_snapshot, &settings.teams);
        *snapshot = Some(next_snapshot.clone());
        drop(snapshot);
        inner
            .bus
            .emit(NodeEvent::HubDirectoryUpdated { snapshot: next_snapshot });
        let summary = inner.app_state.bump_projection_revision(
            ProjectionScope::OperationalSummary {},
            None,
            Some("settings-updated".to_string()),
        )?;
        emit_projection_invalidation(&inner.bus, summary);
        Ok(())
    }

    pub fn get_saved_peers(&self) -> Result<Vec<SavedPeerRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
        inner.app_state.get_saved_peers()
    }

    pub fn set_saved_peers(&self, peers: Vec<SavedPeerRecord>) -> Result<(), NodeError> {
        let cmd_tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let invalidation = inner.app_state.set_saved_peers(&peers)?;
            emit_projection_invalidation(&inner.bus, invalidation);
            let summary = inner.app_state.bump_projection_revision(
                ProjectionScope::OperationalSummary {},
                None,
                Some("saved-peers-updated".to_string()),
            )?;
            emit_projection_invalidation(&inner.bus, summary);
            inner.cmd_tx.clone()
        };

        if let Some(tx) = cmd_tx {
            let (resp_tx, resp_rx) = cb::bounded(1);
            dispatch_command(
                &tx,
                Command::SetSavedPeers {
                    peers,
                    resp: resp_tx,
                },
            )?;
            resp_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or(Err(NodeError::Timeout {}))?;
        }
        Ok(())
    }

}
