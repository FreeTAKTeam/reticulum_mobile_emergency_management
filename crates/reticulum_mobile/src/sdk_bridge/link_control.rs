fn build_link_identify_payload(identity: &PrivateIdentity, link_id: &AddressHash) -> Vec<u8> {
    let identity_value = identity.as_identity();
    let mut public_key = Vec::with_capacity(64);
    public_key.extend_from_slice(identity_value.public_key.as_bytes());
    public_key.extend_from_slice(identity_value.verifying_key.as_bytes());

    let mut signed_data = Vec::with_capacity(16 + public_key.len());
    signed_data.extend_from_slice(link_id.as_slice());
    signed_data.extend_from_slice(public_key.as_slice());
    let signature = identity.sign(signed_data.as_slice());

    let mut payload = Vec::with_capacity(public_key.len() + signature.to_bytes().len());
    payload.extend_from_slice(public_key.as_slice());
    payload.extend_from_slice(signature.to_bytes().as_slice());
    payload
}

fn build_link_request_payload(path: &str, data: rmpv::Value) -> Result<Vec<u8>, NodeError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let path_hash = address_hash(path.as_bytes());
    rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::F64(timestamp),
        rmpv::Value::Binary(path_hash.to_vec()),
        data,
    ]))
    .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))
}

async fn send_link_context_packet(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
    context: PacketContext,
    payload: &[u8],
    direct_iface: Option<AddressHash>,
) -> Result<Option<[u8; 16]>, NodeError> {
    let packet = {
        let guard = link.lock().await;
        if guard.status() != LinkStatus::Active {
            return Err(NodeError::Timeout {});
        }

        let mut packet_data = PacketDataBuffer::new();
        let cipher_len = {
            let ciphertext = guard
                .encrypt(payload, packet_data.accuire_buf_max())
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
            ciphertext.len()
        };
        packet_data.resize(cipher_len);

        Packet {
            header: Header {
                ifac_flag: IfacFlag::Open,
                header_type: HeaderType::Type1,
                context_flag: ContextFlag::Unset,
                propagation_type: PropagationType::Broadcast,
                destination_type: DestinationType::Link,
                packet_type: PacketType::Data,
                hops: 0,
            },
            ifac: None,
            destination: *guard.id(),
            transport: None,
            context,
            data: packet_data,
        }
    };

    let request_id = if context == PacketContext::Request {
        let hash = packet.hash().to_bytes();
        let mut request_id = [0u8; 16];
        request_id.copy_from_slice(&hash[..16]);
        Some(request_id)
    } else {
        None
    };

    if let Some(iface) = direct_iface {
        transport.send_direct(iface, packet).await;
        return Ok(request_id);
    }

    let outcome = transport.send_packet_with_outcome(packet).await;
    if !matches!(
        outcome,
        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
    ) {
        return Err(NodeError::NetworkError {});
    }
    Ok(request_id)
}

async fn wait_for_link_request_response(
    data_rx: &mut tokio::sync::broadcast::Receiver<ReceivedData>,
    resource_rx: &mut tokio::sync::broadcast::Receiver<ResourceEvent>,
    expected_destination: AddressHash,
    expected_link_id: AddressHash,
    request_id: [u8; 16],
    timeout: Duration,
) -> Result<rmpv::Value, NodeError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(NodeError::Timeout {});
        }
        let remaining = deadline.saturating_duration_since(now);

        tokio::select! {
            _ = tokio::time::sleep(remaining) => {
                return Err(NodeError::Timeout {});
            }
            result = data_rx.recv() => {
                match result {
                    Ok(event) => {
                        if !link_response_destination_matches(
                            event.destination,
                            expected_destination,
                            expected_link_id,
                        ) {
                            continue;
                        }
                        if let Some((response_id, payload)) =
                            parse_link_response_frame(event.data.as_slice())
                        {
                            if response_id == request_id {
                                return Ok(payload);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(NodeError::InternalError {});
                    }
                }
            }
            result = resource_rx.recv() => {
                match result {
                    Ok(event) => {
                        let ResourceEventKind::Complete(complete) = event.kind else {
                            continue;
                        };
                        if event.link_id != expected_link_id {
                            continue;
                        }
                        if let Some((response_id, payload)) =
                            parse_link_response_frame(complete.data.as_slice())
                        {
                            if response_id == request_id {
                                return Ok(payload);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(NodeError::InternalError {});
                    }
                }
            }
        }
    }
}

fn link_response_destination_matches(
    actual: AddressHash,
    expected_destination: AddressHash,
    expected_link_id: AddressHash,
) -> bool {
    actual == expected_link_id || actual == expected_destination
}

fn parse_link_response_frame(bytes: &[u8]) -> Option<([u8; 16], rmpv::Value)> {
    let value = rmp_serde::from_slice::<rmpv::Value>(bytes).ok()?;
    let rmpv::Value::Array(entries) = value else {
        return None;
    };
    if entries.len() != 2 {
        return None;
    }
    let request_bytes = value_to_bytes(entries.first()?)?;
    if request_bytes.len() != 16 {
        return None;
    }
    let mut request_id = [0u8; 16];
    request_id.copy_from_slice(request_bytes.as_slice());
    Some((request_id, entries.get(1)?.clone()))
}

fn value_to_bytes(value: &rmpv::Value) -> Option<Vec<u8>> {
    match value {
        rmpv::Value::Binary(bytes) => Some(bytes.clone()),
        rmpv::Value::String(text) => {
            let value = text.as_str()?;
            if let Ok(decoded) = hex::decode(value) {
                return Some(decoded);
            }
            Some(value.as_bytes().to_vec())
        }
        _ => None,
    }
}

fn rmpv_propagation_envelope_payloads(value: &rmpv::Value) -> Option<Vec<Vec<u8>>> {
    let rmpv::Value::Array(entries) = value else {
        return None;
    };
    if entries.len() < 2 {
        return None;
    }
    let timestamp_like = matches!(
        entries.first(),
        Some(rmpv::Value::F32(_)) | Some(rmpv::Value::F64(_)) | Some(rmpv::Value::Integer(_))
    );
    if !timestamp_like {
        return None;
    }
    let rmpv::Value::Array(payloads) = &entries[1] else {
        return None;
    };
    let decoded = payloads
        .iter()
        .map(value_to_bytes)
        .collect::<Option<Vec<_>>>()?;
    (!decoded.is_empty()).then_some(decoded)
}

fn propagation_payloads_from_bytes(bytes: &[u8]) -> Vec<Vec<u8>> {
    if let Ok(value) = rmp_serde::from_slice::<rmpv::Value>(bytes) {
        if let Some(payloads) = rmpv_propagation_envelope_payloads(&value) {
            return payloads;
        }
    }
    vec![bytes.to_vec()]
}

fn propagation_payloads_from_fetch_entry(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    if let Some(payloads) = rmpv_propagation_envelope_payloads(value) {
        return Ok(payloads);
    }
    match value {
        rmpv::Value::Binary(bytes) => Ok(propagation_payloads_from_bytes(bytes)),
        rmpv::Value::String(text) => {
            let value = text.as_str().ok_or(NodeError::InternalError {})?;
            let bytes = hex::decode(value).unwrap_or_else(|_| value.as_bytes().to_vec());
            Ok(propagation_payloads_from_bytes(bytes.as_slice()))
        }
        rmpv::Value::Array(entries) => {
            if entries.len() >= 2 {
                if let Some(payloads) = rmpv_propagation_envelope_payloads(&entries[1]) {
                    return Ok(payloads);
                }
                if let Some(bytes) = entries.get(1).and_then(value_to_bytes) {
                    return Ok(propagation_payloads_from_bytes(bytes.as_slice()));
                }
            }
            Err(NodeError::InternalError {})
        }
        _ => Err(NodeError::InternalError {}),
    }
}

fn rmpv_binary_array(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    let rmpv::Value::Array(values) = value else {
        return Err(NodeError::InternalError {});
    };
    values
        .iter()
        .map(|value| match value {
            rmpv::Value::Binary(bytes) => Ok(bytes.clone()),
            _ => Err(NodeError::InternalError {}),
        })
        .collect()
}

fn rmpv_propagation_payload_array(value: &rmpv::Value) -> Result<Vec<Vec<u8>>, NodeError> {
    if let Some(payloads) = rmpv_propagation_envelope_payloads(value) {
        return Ok(payloads);
    }
    let rmpv::Value::Array(values) = value else {
        return Err(NodeError::InternalError {});
    };
    let mut payloads = Vec::new();
    for value in values {
        payloads.extend(propagation_payloads_from_fetch_entry(value)?);
    }
    Ok(payloads)
}

fn rmpv_shape(value: &rmpv::Value) -> String {
    match value {
        rmpv::Value::Nil => "nil".to_string(),
        rmpv::Value::Boolean(_) => "bool".to_string(),
        rmpv::Value::Integer(_) => "int".to_string(),
        rmpv::Value::F32(_) | rmpv::Value::F64(_) => "float".to_string(),
        rmpv::Value::String(_) => "string".to_string(),
        rmpv::Value::Binary(bytes) => format!("bin({})", bytes.len()),
        rmpv::Value::Array(values) => {
            let preview = values
                .iter()
                .take(4)
                .map(rmpv_shape)
                .collect::<Vec<_>>()
                .join(",");
            if values.len() > 4 {
                format!("array({})[{preview},...]", values.len())
            } else {
                format!("array({})[{preview}]", values.len())
            }
        }
        rmpv::Value::Map(values) => format!("map({})", values.len()),
        rmpv::Value::Ext(_, bytes) => format!("ext({})", bytes.len()),
    }
}

fn apply_fetch_limit(transient_ids: &mut Vec<Vec<u8>>, limit: Option<u32>) {
    if let Some(limit) = limit {
        transient_ids.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }
}

fn propagation_fetch_batches(transient_ids: &[Vec<u8>]) -> Vec<Vec<Vec<u8>>> {
    transient_ids
        .chunks(PROPAGATION_FETCH_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn propagation_purge_batches(transient_ids: &[Vec<u8>]) -> Vec<Vec<Vec<u8>>> {
    transient_ids
        .chunks(PROPAGATION_PURGE_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn queue_fetched_transient_id_for_purge(
    purge_queue: &mut Vec<Vec<u8>>,
    transient_id: Option<Vec<u8>>,
) -> bool {
    if let Some(transient_id) = transient_id {
        purge_queue.push(transient_id);
        false
    } else {
        true
    }
}
