impl Node {
    pub fn announce_now(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Control {})?;
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
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Control {})?;
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
        let (tx, active_config, hub_directory_snapshot, app_state, peers_snapshot, saver_active) = {
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
                inner.power_state.saver_active,
            )
        };
        let requested_destination = normalize_hex_32(request.destination_hex.as_str())
            .ok_or(NodeError::InvalidConfig {})?;
        let logical_destination_hex = requested_destination.clone();
        ensure_outbound_admitted(saver_active, OutboundTrafficClass::Chat {})?;
        let saved_peers = app_state.get_saved_peers()?;
        let runtime_peers = peers_snapshot.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        if !inner_saved_peer_authorizes_destination(
            &saved_peers,
            runtime_peers.as_slice(),
            &requested_destination,
        ) {
            return Err(NodeError::InvalidConfig {});
        }
        drop(runtime_peers);
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
                requested_destination_hex: logical_destination_hex,
                fields_bytes,
                resp: resp_tx,
            },
        )?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn retry_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let (tx, delivery_destination_hex) = {
            let inner = self.inner.lock().map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            let message = inner
                .app_state
                .list_messages(None)?
                .into_iter()
                .find(|message| message.message_id_hex == message_id_hex)
                .ok_or(NodeError::InvalidConfig {})?;
            ensure_outbound_admitted(inner.power_state.saver_active, message.traffic_class)?;
            let logical_destination_hex = message
                .requested_destination_hex
                .clone()
                .unwrap_or_else(|| message.destination_hex.clone());
            let delivery_destination_hex =
                if matches!(message.traffic_class, OutboundTrafficClass::Chat {}) {
                let runtime_peers = inner.peers_snapshot.lock().map_err(|error| {
                    crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
                })?;
                if !inner_saved_peer_authorizes_destination(
                    &inner.app_state.get_saved_peers()?,
                    runtime_peers.as_slice(),
                    message
                        .requested_destination_hex
                        .as_deref()
                        .unwrap_or(&message.destination_hex),
                ) {
                    return Err(NodeError::InvalidConfig {});
                }
                let hub_directory_snapshot = inner
                    .hub_directory_snapshot
                    .lock()
                    .map_err(|error| {
                        crate::error_context::contextual_node_error(
                            NodeError::InternalError {},
                            error,
                        )
                    })?
                    .clone();
                routed_chat_destination_hex(
                    logical_destination_hex,
                    inner.active_config.as_ref(),
                    hub_directory_snapshot.as_ref(),
                    || Ok((inner.app_state.list_messages(None)?, runtime_peers.clone())),
                )?
            } else {
                message
                    .delivery_destination_hex
                    .clone()
                    .unwrap_or(logical_destination_hex)
            };
            (
                inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?,
                delivery_destination_hex,
            )
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::RetryLxmf {
                message_id_hex,
                delivery_destination_hex,
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
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Control {})?;
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
            ensure_outbound_admitted(inner.power_state.saver_active, OutboundTrafficClass::Control {})?;
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

}
