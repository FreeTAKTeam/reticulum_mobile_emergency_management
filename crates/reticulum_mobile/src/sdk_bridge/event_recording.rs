impl RuntimeLxmfSdk {
    pub(crate) async fn fetch_propagated_lxmf_from_relay(
        &self,
        relay_hex: &str,
        limit: Option<u32>,
        direct_iface_hex: Option<&str>,
    ) -> Result<PropagationFetchResult, NodeError> {
        let relay_hex = relay_hex.trim();
        if relay_hex.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        compat_fetch_propagated_lxmf(&self.transport, relay_hex, limit, direct_iface_hex).await
    }

    fn record_event(&self, event_type: &str, severity: Severity, payload: JsonValue) {
        if let Err(err) = self
            .client
            .backend()
            .record_event(event_type, severity, payload)
        {
            warn!("failed to record {event_type} SDK event: {err}");
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery events preserve routing and mission correlation fields"
    )]
    fn record_delivery_update(
        &self,
        message_id_hex: &str,
        delivery_state: DeliveryState,
        destination_hex: &str,
        source_hex: Option<&str>,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        let message_id = MessageId(message_id_hex.to_owned());
        if let Err(err) = self.client.backend().record_delivery(
            &message_id,
            delivery_state.clone(),
            detail.map(ToOwned::to_owned),
        ) {
            warn!("failed to update SDK delivery {message_id_hex}: {err}");
            return;
        }
        self.record_event(
            EVENT_DELIVERY_UPDATED,
            if matches!(
                delivery_state,
                DeliveryState::Failed | DeliveryState::Rejected | DeliveryState::Expired
            ) {
                Severity::Warn
            } else {
                Severity::Info
            },
            json!({
                "message_id": message_id_hex,
                "destination_hex": destination_hex,
                "source_hex": source_hex,
                "correlation_id": correlation_id,
                "command_id": command_id,
                "command_type": command_type,
                "event_uid": event_uid,
                "mission_uid": mission_uid,
                "status": format!("{delivery_state:?}"),
                "detail": detail,
            }),
        );
    }

    pub(crate) fn record_packet_received(
        &self,
        destination_hex: &str,
        source_hex: Option<&str>,
        bytes: &[u8],
        fields_bytes: Option<&[u8]>,
    ) {
        self.record_event(
            EVENT_PACKET_RECEIVED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "source_hex": source_hex,
                "bytes_base64": BASE64_STANDARD.encode(bytes),
                "fields_base64": fields_bytes.map(|value| BASE64_STANDARD.encode(value)),
            }),
        );
    }

    pub(crate) fn record_announce_received(
        &self,
        destination_hex: &str,
        identity_hex: &str,
        destination_kind: &str,
        app_data: &str,
        hops: u8,
        interface_hex: &str,
    ) {
        self.record_event(
            EVENT_ANNOUNCE_RECEIVED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "identity_hex": identity_hex,
                "destination_kind": destination_kind,
                "app_data": app_data,
                "hops": hops,
                "interface_hex": interface_hex,
            }),
        );
    }

    pub(crate) fn record_peer_changed(
        &self,
        destination_hex: &str,
        state: PeerState,
        last_error: Option<&str>,
    ) {
        let state_name = match state {
            PeerState::Connecting {} => "connecting",
            PeerState::Connected {} => "connected",
            PeerState::Disconnected {} => "disconnected",
        };
        self.record_event(
            EVENT_PEER_CHANGED,
            Severity::Info,
            json!({
                "destination_hex": destination_hex,
                "state": state_name,
                "last_error": last_error,
            }),
        );
    }

    pub(crate) fn record_hub_directory_updated(&self, snapshot: &HubDirectorySnapshot) {
        self.record_event(
            EVENT_HUB_DIRECTORY_UPDATED,
            Severity::Info,
            json!({
                "effective_connected_mode": snapshot.effective_connected_mode,
                "items": snapshot.items.iter().map(|item| json!({
                    "identity": item.identity,
                    "destination_hash": item.destination_hash,
                    "display_name": item.display_name,
                    "announce_capabilities": item.announce_capabilities,
                    "client_type": item.client_type,
                    "registered_mode": item.registered_mode,
                    "last_seen": item.last_seen,
                    "status": item.status,
                })).collect::<Vec<_>>(),
                "received_at_ms": snapshot.received_at_ms,
            }),
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_sent(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
    ) {
        self.record_delivery_update(
            message_id_hex,
            DeliveryState::Sent,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            None,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps source and mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_acknowledged(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        source_hex: Option<&str>,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.record_delivery_update(
            message_id_hex,
            DeliveryState::Delivered,
            destination_hex,
            source_hex,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_failed(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.record_delivery_update(
            message_id_hex,
            DeliveryState::Failed,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "delivery event wrapper keeps mission correlation fields explicit"
    )]
    pub(crate) fn record_delivery_timed_out(
        &self,
        message_id_hex: &str,
        destination_hex: &str,
        correlation_id: Option<&str>,
        command_id: Option<&str>,
        command_type: Option<&str>,
        event_uid: Option<&str>,
        mission_uid: Option<&str>,
        detail: Option<&str>,
    ) {
        self.record_delivery_update(
            message_id_hex,
            DeliveryState::Expired,
            destination_hex,
            None,
            correlation_id,
            command_id,
            command_type,
            event_uid,
            mission_uid,
            detail,
        );
    }
}
