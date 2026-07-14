async fn compat_fetch_propagated_lxmf(
    state: &SdkTransportState,
    relay_hex: &str,
    limit: Option<u32>,
    direct_iface_hex: Option<&str>,
) -> Result<PropagationFetchResult, NodeError> {
    let relay_hash = parse_address_hash(relay_hex)?;
    let relay_desc = resolve_propagation_destination_desc(state, relay_hash).await?;
    let direct_iface = direct_iface_hex.and_then(|value| parse_address_hash(value).ok());
    let (destination_hex, destination_hash, local_identity) = {
        let destination = state.lxmf_destination.lock().await;
        (
            destination.desc.address_hash.to_hex_string(),
            destination.desc.address_hash,
            destination.identity.clone(),
        )
    };

    let available_value = propagation_remote_control_request(
        state,
        relay_desc,
        "/get",
        rmpv::Value::Array(vec![rmpv::Value::Nil, rmpv::Value::Nil]),
        PROPAGATION_CONTROL_TIMEOUT,
        direct_iface,
        1,
    )
    .await?;
    let mut transient_ids = rmpv_binary_array(&available_value)?;
    let available_count = transient_ids.len();
    apply_fetch_limit(&mut transient_ids, limit);
    info!(
        "[sync] propagation sync available relay={} destination={} available={} requested={}",
        relay_hex,
        destination_hex,
        available_count,
        transient_ids.len()
    );
    if transient_ids.is_empty() {
        clear_lxmf_output_link(state, &relay_desc.address_hash).await;
        return Ok(PropagationFetchResult {
            destination_hex,
            available_count,
            fetched_count: 0,
            fetched_entry_count: 0,
            extracted_payload_count: 0,
            imported_wires: Vec::new(),
            failed_count: 0,
            malformed_count: 0,
            decrypt_failed_count: 0,
        });
    }

    let mut payloads = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut fetched_entry_count = 0usize;
    let mut malformed_count = 0usize;
    let mut failed_count = 0usize;
    let mut decrypt_failed_count = 0usize;
    let mut last_fetch_error: Option<NodeError> = None;
    let mut fetch_queue = propagation_fetch_batches(transient_ids.as_slice())
        .into_iter()
        .enumerate()
        .collect::<VecDeque<_>>();
    while let Some((batch_index, batch)) = fetch_queue.pop_front() {
        let batch_len = batch.len();
        let fetch_ids =
            rmpv::Value::Array(batch.clone().into_iter().map(rmpv::Value::Binary).collect());
        let fetched_value = match propagation_remote_control_request(
            state,
            relay_desc,
            "/get",
            rmpv::Value::Array(vec![
                fetch_ids,
                rmpv::Value::Nil,
                rmpv::Value::from(PROPAGATION_FETCH_TRANSFER_LIMIT_KB),
            ]),
            PROPAGATION_FETCH_CONTROL_TIMEOUT,
            direct_iface,
            1,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                if batch_len > 1 {
                    info!(
                        "[sync] propagation sync fetch batch split relay={} destination={} batch={} size={} reason={}",
                        relay_hex, destination_hex, batch_index, batch_len, err
                    );
                    for transient_id in batch.into_iter().rev() {
                        fetch_queue.push_front((batch_index, vec![transient_id]));
                    }
                    continue;
                }
                failed_count = failed_count.saturating_add(batch_len);
                info!(
                    "[sync] propagation sync fetch batch failed relay={} destination={} batch={} reason={}",
                    relay_hex, destination_hex, batch_index, err
                );
                last_fetch_error = Some(err);
                continue;
            }
        };
        match rmpv_propagation_payload_array(&fetched_value) {
            Ok(batch_payloads) => {
                fetched_entry_count = fetched_entry_count.saturating_add(batch_len);
                if batch_payloads.len() == batch_len {
                    payloads.extend(
                        batch
                            .into_iter()
                            .zip(batch_payloads)
                            .map(|(transient_id, payload)| (Some(transient_id), payload)),
                    );
                } else {
                    payloads.extend(batch_payloads.into_iter().map(|payload| (None, payload)));
                }
            }
            Err(err) => {
                if batch_len > 1 {
                    info!(
                        "[sync] propagation sync malformed fetch batch split relay={} destination={} batch={} size={} shape={}",
                        relay_hex,
                        destination_hex,
                        batch_index,
                        batch_len,
                        rmpv_shape(&fetched_value)
                    );
                    for transient_id in batch.into_iter().rev() {
                        fetch_queue.push_front((batch_index, vec![transient_id]));
                    }
                    continue;
                }
                failed_count = failed_count.saturating_add(batch_len);
                malformed_count = malformed_count.saturating_add(batch_len);
                info!(
                    "[sync] propagation sync malformed fetch response relay={} destination={} batch={} shape={}",
                    relay_hex,
                    destination_hex,
                    batch_index,
                    rmpv_shape(&fetched_value)
                );
                last_fetch_error = Some(err);
            }
        }
    }
    clear_lxmf_output_link(state, &relay_desc.address_hash).await;
    if payloads.is_empty() {
        if let Some(err) = last_fetch_error {
            return Err(err);
        }
    }
    let extracted_payload_count = payloads.len();
    let fetched_count = fetched_entry_count;
    let mut imported_wires = Vec::with_capacity(extracted_payload_count);
    let mut fetched_transient_ids_to_purge = Vec::<Vec<u8>>::new();
    for (index, (transient_id, payload)) in payloads.into_iter().enumerate() {
        match decrypt_local_propagated_wire(&local_identity, &destination_hash, payload.as_slice())
        {
            Ok(wire) => {
                if let Some(transient_id) = transient_id {
                    fetched_transient_ids_to_purge.push(transient_id);
                }
                imported_wires.push(wire);
            }
            Err(err) => {
                failed_count = failed_count.saturating_add(1);
                decrypt_failed_count = decrypt_failed_count.saturating_add(1);
                let transient_destination_hex = payload
                    .get(..16)
                    .map(hex::encode)
                    .unwrap_or_else(|| "-".to_string());
                let retained = queue_fetched_transient_id_for_purge(
                    &mut fetched_transient_ids_to_purge,
                    transient_id,
                );
                info!(
                    "[sync] propagated payload import failed relay={} destination={} local_identity={} transient_destination={} index={} reason={} retained={}",
                    relay_hex,
                    destination_hex,
                    local_identity.address_hash().to_hex_string(),
                    transient_destination_hex,
                    index,
                    err,
                    retained
                );
            }
        }
    }
    fetched_transient_ids_to_purge.sort();
    fetched_transient_ids_to_purge.dedup();
    if !fetched_transient_ids_to_purge.is_empty() {
        let purge_count = fetched_transient_ids_to_purge.len();
        let mut purged_count = 0usize;
        let mut purge_failed_count = 0usize;
        for batch in propagation_purge_batches(&fetched_transient_ids_to_purge) {
            let batch_count = batch.len();
            let haves = rmpv::Value::Array(batch.into_iter().map(rmpv::Value::Binary).collect());
            match propagation_remote_control_fire_and_forget(
                state,
                relay_desc,
                "/get",
                rmpv::Value::Array(vec![rmpv::Value::Nil, haves]),
                direct_iface,
            )
            .await
            {
                Ok(_) => {
                    purged_count = purged_count.saturating_add(batch_count);
                }
                Err(err) => {
                    purge_failed_count = purge_failed_count.saturating_add(batch_count);
                    info!(
                        "[sync] propagation sync purge batch failed relay={} destination={} purged={} reason={}",
                        relay_hex, destination_hex, batch_count, err
                    );
                }
            }
        }
        if purged_count > 0 {
            info!(
                "[sync] propagation sync queued purge for fetched entries relay={} destination={} purged={} failed={}",
                relay_hex, destination_hex, purged_count, purge_failed_count
            );
        } else if purge_failed_count > 0 {
            info!(
                "[sync] propagation sync purge failed relay={} destination={} purged={} reason=all_batches_failed",
                relay_hex, destination_hex, purge_count
            );
        }
    }

    Ok(PropagationFetchResult {
        destination_hex,
        available_count,
        fetched_count,
        fetched_entry_count,
        extracted_payload_count,
        imported_wires,
        failed_count,
        malformed_count,
        decrypt_failed_count,
    })
}

async fn propagation_remote_control_request(
    state: &SdkTransportState,
    relay_desc: DestinationDesc,
    path: &str,
    data: rmpv::Value,
    timeout: Duration,
    direct_iface: Option<AddressHash>,
    max_attempts: usize,
) -> Result<rmpv::Value, NodeError> {
    let mut last_error = None;
    for attempt in 0..max_attempts.max(1) {
        let relay_destination_hex = relay_desc.address_hash.to_hex_string();
        let link = ensure_lxmf_output_link(
            state,
            relay_desc,
            Some(path),
            Some(relay_destination_hex.as_str()),
            DEFAULT_LINK_CONNECT_TIMEOUT,
            DEFAULT_LINK_CONNECT_ATTEMPTS,
        )
        .await?;
        let link_id = *link.lock().await.id();
        let identify_payload = build_link_identify_payload(&state.identity, &link_id);
        if let Err(err) = send_link_context_packet(
            &state.transport,
            &link,
            PacketContext::LinkIdentify,
            identify_payload.as_slice(),
            direct_iface,
        )
        .await
        {
            clear_lxmf_output_link(state, &relay_desc.address_hash).await;
            info!(
                "[sync] propagation control identify failed relay={} path={} attempt={} reason={}",
                relay_desc.address_hash.to_hex_string(),
                path,
                attempt + 1,
                err
            );
            last_error = Some(err);
            continue;
        }

        let mut data_rx = state.transport.received_data_events();
        let mut resource_rx = state.transport.resource_events();
        let request_payload = build_link_request_payload(path, data.clone())?;
        let request_id = match send_link_context_packet(
            &state.transport,
            &link,
            PacketContext::Request,
            request_payload.as_slice(),
            direct_iface,
        )
        .await
        {
            Ok(Some(request_id)) => request_id,
            Ok(None) => return Err(NodeError::InternalError {}),
            Err(err) => {
                clear_lxmf_output_link(state, &relay_desc.address_hash).await;
                info!(
                    "[sync] propagation control request failed relay={} path={} attempt={} reason={}",
                    relay_desc.address_hash.to_hex_string(),
                    path,
                    attempt + 1,
                    err
                );
                last_error = Some(err);
                continue;
            }
        };

        match wait_for_link_request_response(
            &mut data_rx,
            &mut resource_rx,
            relay_desc.address_hash,
            link_id,
            request_id,
            timeout,
        )
        .await
        {
            Ok(value) => return Ok(value),
            Err(err) => {
                clear_lxmf_output_link(state, &relay_desc.address_hash).await;
                debug!(
                    "[sync] propagation control response unavailable relay={} path={} attempt={} reason={}",
                    relay_desc.address_hash.to_hex_string(),
                    path,
                    attempt + 1,
                    err
                );
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or(NodeError::Timeout {}))
}

async fn propagation_remote_control_fire_and_forget(
    state: &SdkTransportState,
    relay_desc: DestinationDesc,
    path: &str,
    data: rmpv::Value,
    direct_iface: Option<AddressHash>,
) -> Result<(), NodeError> {
    let relay_destination_hex = relay_desc.address_hash.to_hex_string();
    let link = ensure_lxmf_output_link(
        state,
        relay_desc,
        Some(path),
        Some(relay_destination_hex.as_str()),
        DEFAULT_LINK_CONNECT_TIMEOUT,
        DEFAULT_LINK_CONNECT_ATTEMPTS,
    )
    .await?;
    let link_id = *link.lock().await.id();
    let identify_payload = build_link_identify_payload(&state.identity, &link_id);
    if let Err(err) = send_link_context_packet(
        &state.transport,
        &link,
        PacketContext::LinkIdentify,
        identify_payload.as_slice(),
        direct_iface,
    )
    .await
    {
        clear_lxmf_output_link(state, &relay_desc.address_hash).await;
        info!(
            "[sync] propagation control identify failed relay={} path={} reason={}",
            relay_desc.address_hash.to_hex_string(),
            path,
            err
        );
        return Err(err);
    }

    let request_payload = build_link_request_payload(path, data)?;
    match send_link_context_packet(
        &state.transport,
        &link,
        PacketContext::Request,
        request_payload.as_slice(),
        direct_iface,
    )
    .await
    {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(NodeError::InternalError {}),
        Err(err) => {
            clear_lxmf_output_link(state, &relay_desc.address_hash).await;
            info!(
                "[sync] propagation control request failed relay={} path={} reason={}",
                relay_desc.address_hash.to_hex_string(),
                path,
                err
            );
            Err(err)
        }
    }
}
