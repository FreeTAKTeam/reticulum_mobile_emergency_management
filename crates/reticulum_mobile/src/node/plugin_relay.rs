#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RelayChatInput {
    pub id: String,
    pub source_hex: String,
    pub destination_hex: String,
    pub text: String,
    pub radio_gid: String,
}

fn validate_relay_input(input: &RelayChatInput, event: bool) -> Result<(), NodeError> {
    let valid_hex = |value: &str, size: usize| {
        value.len() == size
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    };
    if !valid_hex(&input.id, 32)
        || !valid_hex(&input.source_hex, 32)
        || !valid_hex(&input.radio_gid, 12)
        || input.text.is_empty()
        || input.text.len() > 4096
        || if event {
            input.destination_hex != "-"
        } else {
            !valid_hex(&input.destination_hex, 32)
        }
    {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(())
}

fn relay_chat_id(plugin_id: &str, input: &RelayChatInput) -> String {
    hex::encode(Sha256::digest(
        format!(
            "relay-chat:{plugin_id}:{}:{}:{}",
            input.source_hex, input.destination_hex, input.id
        )
        .as_bytes(),
    ))
}

impl Node {
    fn require_communications_relay(
        &self,
        plugin_id: &str,
    ) -> Result<InstalledPluginRecord, NodeError> {
        self.list_plugins()?
            .into_iter()
            .find(|plugin| {
                plugin.discovered.plugin_id == plugin_id && crate::app_state::relay_allowed(plugin)
            })
            .ok_or(NodeError::InvalidConfig {})
    }

    pub(crate) fn relay_identity(&self, plugin_id: &str) -> Result<JsonValue, NodeError> {
        self.require_communications_relay(plugin_id)?;
        let status = self.get_status();
        if !status.running {
            return Err(NodeError::NotRunning {});
        }
        Ok(json!({"destinationHex":status.lxmf_destination_hex}))
    }

    pub(crate) fn accept_relay_chat(
        &self,
        plugin_id: &str,
        mut input: RelayChatInput,
        sent: bool,
        radio_accepted: bool,
    ) -> Result<JsonValue, NodeError> {
        let plugin = self.require_communications_relay(plugin_id)?;
        let status = self.get_status();
        if !status.running {
            return Err(NodeError::NotRunning {});
        }
        if sent {
            input.source_hex = status.lxmf_destination_hex.clone();
        }
        validate_relay_input(&input, false)?;
        let id = relay_chat_id(plugin_id, &input);
        if let Some(existing) = self.with_app_state(|store, _| store.message_for_wire(&id))? {
            if existing.body_utf8 != input.text {
                return Err(NodeError::InvalidConfig {});
            }
            if !sent
                || !radio_accepted
                || existing
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.ends_with(":radio-accepted-unconfirmed"))
            {
                return Ok(json!({"accepted":true,"duplicate":true,"messageId":id}));
            }
        }
        let local = input.destination_hex == status.lxmf_destination_hex
            || input.destination_hex == status.app_destination_hex;
        if !sent && !local {
            if !plugin.discovered.declared_capabilities.lxmf_send
                || !plugin.granted_capabilities.lxmf_send
            {
                return Err(NodeError::InvalidConfig {});
            }
            let claimed =
                self.with_app_state(|store, _| store.claim_relay_delivery(plugin_id, &id))?;
            if !claimed {
                let previous = self
                    .with_app_state(|store, _| store.relay_forward_result(plugin_id, &id))?
                    .ok_or(NodeError::Timeout {})?;
                let original = self
                    .with_app_state(|store, _| store.message_for_wire(&previous))?
                    .ok_or(NodeError::Timeout {})?;
                if original.body_utf8.split_once("] ").map(|(_, text)| text)
                    != Some(input.text.as_str())
                {
                    return Err(NodeError::InvalidConfig {});
                }
                return Ok(json!({"accepted":true,"duplicate":true,"forwardedMessageId":previous}));
            }
            let forwarded = self.send_lxmf(SendLxmfRequest {
                destination_hex: input.destination_hex,
                body_utf8: format!(
                    "[goTenna:{}:{}; reported source {}] {}",
                    input.radio_gid, input.id, input.source_hex, input.text
                ),
                title: None,
                send_mode: SendMode::Auto {},
            })?;
            self.with_app_state(|store, _| {
                store.confirm_relay_forward(plugin_id, &id, &forwarded)
            })?;
            return Ok(json!({"accepted":true,"forwardedMessageId":forwarded}));
        }
        let now = now_ms();
        let record = MessageRecord {
            message_id_hex: id.clone(),
            conversation_id: if sent {
                input.destination_hex.clone()
            } else {
                input.source_hex.clone()
            },
            direction: if sent {
                MessageDirection::Outbound {}
            } else {
                MessageDirection::Inbound {}
            },
            destination_hex: input.destination_hex.clone(),
            source_hex: Some(input.source_hex),
            requested_destination_hex: Some(input.destination_hex.clone()),
            delivery_destination_hex: Some(input.destination_hex),
            recipient_identity_hex: None,
            last_wire_message_id_hex: None,
            title: None,
            body_utf8: input.text,
            traffic_class: OutboundTrafficClass::Chat {},
            method: crate::types::MessageMethod::Direct {},
            state: if sent && !radio_accepted {
                crate::types::MessageState::Queued {}
            } else if sent {
                crate::types::MessageState::SentDirect {}
            } else {
                crate::types::MessageState::Received {}
            },
            transport_state: if sent && !radio_accepted {
                crate::types::TransportDeliveryState::Queued {}
            } else if sent {
                crate::types::TransportDeliveryState::SentDirect {}
            } else {
                crate::types::TransportDeliveryState::TransportDelivered {}
            },
            application_ack_state: crate::types::ApplicationAckState::NotRequired {},
            detail: Some(format!(
                "external-relay:gotenna:{}:{}",
                input.radio_gid,
                if sent && !radio_accepted {
                    "radio-attempt-unconfirmed"
                } else if sent {
                    "radio-accepted-unconfirmed"
                } else {
                    "unverified"
                }
            )),
            sent_at_ms: sent.then_some(now),
            received_at_ms: (!sent).then_some(now),
            updated_at_ms: now,
        };
        let tx = self
            .inner
            .lock()
            .map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?
            .cmd_tx
            .clone()
            .ok_or(NodeError::NotRunning {})?;
        let (resp, receiver) = cb::bounded(1);
        dispatch_command(
            &tx,
            Command::ImportRelayChat {
                plugin_id: plugin_id.to_string(),
                record: Box::new(record),
                resp,
            },
        )?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))?;
        Ok(json!({"accepted":true,"messageId":id,"local":true}))
    }

    pub(crate) fn accept_relay_event(
        &self,
        plugin_id: &str,
        input: RelayChatInput,
    ) -> Result<JsonValue, NodeError> {
        let plugin = self.require_communications_relay(plugin_id)?;
        validate_relay_input(&input, true)?;
        if !plugin.discovered.declared_capabilities.events_publish
            || !plugin.granted_capabilities.events_publish
        {
            return Err(NodeError::InvalidConfig {});
        }
        let code =
            crate::app_state::relay_event_code(&input.text).ok_or(NodeError::InvalidConfig {})?;
        let status = self.get_status();
        if !status.running {
            return Err(NodeError::NotRunning {});
        }
        let digest = hex::encode(Sha256::digest(
            format!("relay-event:{plugin_id}:{}:{}", input.source_hex, input.id).as_bytes(),
        ));
        let uid = format!("gotenna-{}", &digest[..32]);
        if let Some(existing) = self.get_events()?.iter().find(|event| event.uid == uid) {
            if existing.content != code {
                return Err(NodeError::InvalidConfig {});
            }
            return Ok(json!({"accepted":true,"duplicate":true}));
        }
        let now = now_ms();
        let timestamp = current_timestamp_rfc3339();
        self.upsert_event(EventProjectionRecord {
            uid: uid.clone(),
            command_id: uid.clone(),
            source_identity: status.identity_hex,
            source_display_name: Some(format!(
                "goTenna {} (unverified report {})",
                input.radio_gid, input.source_hex
            )),
            timestamp,
            command_type: "mission.registry.log_entry.upsert".to_string(),
            mission_uid: "r3akt-default-mission".to_string(),
            content: code,
            callsign: format!("goTenna {}", input.radio_gid),
            server_time: None,
            client_time: None,
            keywords: vec![],
            content_hashes: vec![],
            updated_at_ms: now,
            deleted_at_ms: None,
            correlation_id: Some(uid),
            topics: vec!["gotenna-relay".to_string()],
        })?;
        Ok(json!({"accepted":true}))
    }
}
