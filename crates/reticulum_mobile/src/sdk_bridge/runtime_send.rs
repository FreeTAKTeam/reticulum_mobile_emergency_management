impl RuntimeLxmfSdk {
    pub(crate) async fn new(runtime_id: String, transport: SdkTransportState) -> Self {
        let source_destination = transport.lxmf_destination.lock().await.desc.address_hash;
        let backend = InProcessBackend::new(InProcessBackendConfig::new(
            runtime_id,
            Handle::current(),
            transport.transport.clone(),
            transport.identity.clone(),
            source_destination,
        ));
        Self {
            client: Arc::new(Client::new(backend)),
            transport,
        }
    }

    pub(crate) async fn start(&self) -> Result<(), NodeError> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            let mut config = SdkConfig::desktop_local_default();
            config.rpc_backend = None;
            client
                .start(
                    StartRequest::new(config)
                        .with_requested_capability("reticulum.capability.raw_bytes")
                        .with_requested_capability("reticulum.capability.msgpack_fields"),
                )
                .map(|_| ())
        })
        .await
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
    }

    pub(crate) async fn shutdown(&self) -> Result<(), NodeError> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || client.shutdown(ShutdownMode::Graceful).map(|_| ()))
            .await
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
    }

    pub(crate) async fn send_lxmf_via_propagation_relay(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<String>,
        fields_bytes: Option<Vec<u8>>,
        metadata: Option<MissionSyncMetadata>,
        propagation_relay_hex: String,
    ) -> Result<LxmfSendReport, NodeError> {
        self.send_lxmf_with_direct_attempt(
            destination,
            content,
            title,
            fields_bytes,
            metadata,
            SendMode::PropagationOnly {},
            None,
            None,
            None,
            Some(propagation_relay_hex),
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "send boundary keeps payload, metadata, mode, and retry identity explicit"
    )]
    pub(crate) async fn send_lxmf_with_direct_attempt(
        &self,
        destination: AddressHash,
        content: &[u8],
        title: Option<String>,
        fields_bytes: Option<Vec<u8>>,
        metadata: Option<MissionSyncMetadata>,
        send_mode: SendMode,
        direct_attempt: Option<usize>,
        link_connect_timeout: Option<Duration>,
        direct_packet_max_wire_bytes: Option<usize>,
        propagation_relay_hex: Option<String>,
    ) -> Result<LxmfSendReport, NodeError> {
        let source = self
            .transport
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .to_hex_string();
        let requested_destination_hex = destination.to_hex_string();
        let mut request = SendRequest::new(
            source,
            requested_destination_hex.clone(),
            json!({
                "encoding": "base64",
                "title": title.clone().unwrap_or_default(),
                "content_base64": BASE64_STANDARD.encode(content),
            }),
        )
        .with_extension(EXT_RAW_BYTES_BASE64, json!(BASE64_STANDARD.encode(content)));
        if let Some(fields_bytes) = fields_bytes.as_ref() {
            request = request.with_extension(
                EXT_FIELDS_BASE64,
                json!(BASE64_STANDARD.encode(fields_bytes)),
            );
        }
        request = request.with_extension(
            EXT_SEND_MODE,
            json!(match send_mode {
                SendMode::Auto {} => "Auto",
                SendMode::DirectOnly {} => "DirectOnly",
                SendMode::PropagationOnly {} => "PropagationOnly",
            }),
        );
        if matches!(send_mode, SendMode::PropagationOnly {}) {
            request = request.with_extension(EXT_USE_PROPAGATION_NODE, json!(true));
        }
        if let Some(propagation_relay_hex) = propagation_relay_hex
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request =
                request.with_extension(EXT_PROPAGATION_RELAY_HEX, json!(propagation_relay_hex));
        }
        if metadata_is_accepted_result(metadata.as_ref()) {
            request = request.with_extension(EXT_ACCEPTED_RESULT_ACK, json!(true));
        }
        if let Some(link_connect_timeout) = link_connect_timeout {
            let timeout_ms =
                crate::numeric::u128_to_u64_saturating(link_connect_timeout.as_millis());
            request = request.with_extension(EXT_LINK_CONNECT_TIMEOUT_MS, json!(timeout_ms));
        }
        if let Some(direct_packet_max_wire_bytes) = direct_packet_max_wire_bytes {
            request = request.with_extension(
                EXT_DIRECT_PACKET_MAX_WIRE_BYTES,
                json!(direct_packet_max_wire_bytes),
            );
        }
        if let Some(correlation_id) = metadata
            .as_ref()
            .and_then(|value| value.correlation_id.clone())
        {
            request = request.with_correlation_id(correlation_id);
        }
        if let Some(idempotency_key) = metadata.as_ref().and_then(|value| {
            (!metadata_uses_compact_eam_tracking_marker(value))
                .then(|| value.tracking_key().map(ToOwned::to_owned))
                .flatten()
        }) {
            request = request.with_idempotency_key(idempotency_key_for_send_attempt(
                &idempotency_key,
                send_mode,
                direct_attempt,
            ));
        }

        let active_relay = self
            .transport
            .active_propagation_node_hex
            .lock()
            .await
            .clone()
            .map(|value| parse_address_hash(value.trim()))
            .transpose()?;
        self.client
            .backend()
            .set_propagation_relay(active_relay)
            .map_err(map_sdk_error_to_node_error)?;

        let client = self.client.clone();
        let message_id = tokio::task::spawn_blocking(move || client.send(request))
            .await
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .map_err(|err| {
                warn!("in-process LXMF send failed destination={requested_destination_hex}: {err}");
                map_sdk_error_to_node_error(err)
            })?;
        let report = self
            .client
            .backend()
            .send_report(&message_id)
            .map_err(map_sdk_error_to_node_error)?
            .ok_or(NodeError::InternalError {})?;

        let outcome = match report.outcome {
            RuntimeDeliveryOutcome::SentDirect => RnsSendOutcome::SentDirect,
            RuntimeDeliveryOutcome::SentBroadcast => RnsSendOutcome::SentBroadcast,
        };
        let method = match report.method {
            RuntimeDeliveryMethod::Opportunistic => LxmfDeliveryMethod::Opportunistic {},
            RuntimeDeliveryMethod::Direct => LxmfDeliveryMethod::Direct {},
            RuntimeDeliveryMethod::Propagated => LxmfDeliveryMethod::Propagated {},
        };
        let representation = match report.representation {
            RuntimeDeliveryRepresentation::Packet => LxmfDeliveryRepresentation::Packet {},
            RuntimeDeliveryRepresentation::Resource => LxmfDeliveryRepresentation::Resource {},
        };

        if let Some(metadata) = metadata.as_ref().filter(|value| value.is_event_related()) {
            info!(
                "[lxmf][events][sdk] attempting send requested_destination={} resolved_destination={} kind={} name={} message_id={} event_uid={} mission_uid={} correlation={}",
                requested_destination_hex,
                report.resolved_destination,
                metadata.primary_kind(),
                metadata.primary_name().unwrap_or("-"),
                report.message_id.0,
                metadata.event_uid.as_deref().unwrap_or("-"),
                metadata.mission_uid.as_deref().unwrap_or("-"),
                metadata.correlation_id.as_deref().unwrap_or("-"),
            );
        }

        let track_delivery_timeout = metadata
            .as_ref()
            .is_some_and(|value| value.command_present && value.tracking_key().is_some());

        Ok(LxmfSendReport {
            outcome,
            message_id_hex: report.message_id.0,
            resolved_destination_hex: report.resolved_destination,
            metadata,
            track_delivery_timeout,
            used_propagation_node: matches!(method, LxmfDeliveryMethod::Propagated {}),
            method,
            representation,
            relay_destination_hex: report.relay_destination,
            fallback_stage: None,
            receipt_hash_hex: report.receipt_hash,
        })
    }

}
