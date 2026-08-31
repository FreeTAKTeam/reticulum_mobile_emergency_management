fn spawn_payload_receivers(state: &NodeRuntimeState, bus: &EventBus) {
    // Data receiver.
    {
        let transport = state.transport.clone();
        let bus = bus.clone();
        let sdk = state.sdk.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.received_data_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.destination);
                        let expected_lxmf = {
                            let local_lxmf_destination = state
                                .lxmf_destination
                                .lock()
                                .await
                                .desc
                                .address_hash
                                .to_hex_string();
                            destination_hex == local_lxmf_destination
                        };
                        info!(
                            "[lxmf][rx] data_event destination={} bytes={}",
                            destination_hex,
                            event.data.as_slice().len(),
                        );
                        emit_received_payload(
                            &state,
                            &bus,
                            &sdk,
                            destination_hex,
                            event.data.as_slice().to_vec(),
                            None,
                            expected_lxmf,
                        )
                        .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Resource receiver.
    {
        let transport = state.transport.clone();
        let bus = bus.clone();
        let sdk = state.sdk.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut rx = transport.resource_events();
            loop {
                match rx.recv().await {
                    Ok(event) => match event.kind {
                        ResourceEventKind::Complete(complete) => {
                            let destination_hex =
                                if let Some(link) = transport.find_in_link(&event.link_id).await {
                                    address_hash_to_hex(
                                        &link.lock().await.destination().address_hash,
                                    )
                                } else if let Some(link) =
                                    transport.find_out_link(&event.link_id).await
                                {
                                    address_hash_to_hex(
                                        &link.lock().await.destination().address_hash,
                                    )
                                } else {
                                    address_hash_to_hex(&event.link_id)
                                };
                            info!(
                                "[lxmf][events] resource complete link_id={} destination={} bytes={} metadata_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                destination_hex,
                                complete.data.len(),
                                complete.metadata.as_ref().map(Vec::len).unwrap_or(0),
                            );
                            emit_received_payload(
                                &state,
                                &bus,
                                &sdk,
                                destination_hex,
                                complete.data,
                                complete.metadata,
                                true,
                            )
                            .await;
                        }
                        ResourceEventKind::Progress(progress) => {
                            debug!(
                                "[lxmf][debug] resource progress link_id={} received_bytes={} total_bytes={} received_parts={} total_parts={}",
                                address_hash_to_hex(&event.link_id),
                                progress.received_bytes,
                                progress.total_bytes,
                                progress.received_parts,
                                progress.total_parts,
                            );
                        }
                        ResourceEventKind::SegmentComplete(progress) => {
                            debug!(
                                "[lxmf][debug] resource segment complete link_id={} original_hash={} segment={} total_segments={} total_data_size={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(progress.original_hash.as_slice()),
                                progress.segment_index,
                                progress.total_segments,
                                progress.total_data_size,
                            );
                        }
                        ResourceEventKind::OutboundComplete => {
                            info!(
                                "[lxmf][events] resource outbound complete link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::OutboundFailed => {
                            info!(
                                "[lxmf][events] resource outbound failed link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::OutboundCancelled => {
                            info!(
                                "[lxmf][events] resource outbound cancelled link_id={} hash={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                            );
                        }
                        ResourceEventKind::InboundFailed(failure) => {
                            warn!(
                                "[lxmf][events] resource inbound failed link_id={} hash={} reason={} received_parts={} total_parts={} received_bytes={} total_bytes={}",
                                address_hash_to_hex(&event.link_id),
                                hex::encode(event.hash.as_slice()),
                                failure.reason,
                                failure.progress.received_parts,
                                failure.progress.total_parts,
                                failure.progress.received_bytes,
                                failure.progress.total_bytes,
                            );
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }
}

fn spawn_delivery_tracking_tasks(
    state: &NodeRuntimeState,
    bus: &EventBus,
    receipt_message_ids: &Arc<Mutex<HashMap<String, ReceiptMessageTracking>>>,
) {
    // Pending LXMF acknowledgement timeout watcher.
    {
        let bus = bus.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = now_ms();
                let mut expired = Vec::<PendingLxmfDelivery>::new();
                {
                    let mut guard = state.pending_lxmf_deliveries.lock().await;
                    let saver_active = *state.power_saver_rx.borrow();
                    let expired_keys = guard
                        .iter()
                        .filter(|(_, pending)| {
                            pending_ack_timeout_elapsed(pending, now)
                                && (!saver_active
                                    || pending
                                        .resend
                                        .as_ref()
                                        .is_some_and(|resend| {
                                            resend.traffic_class.allowed_in_power_saver()
                                        }))
                        })
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for key in expired_keys {
                        if let Some(pending) = guard.remove(&key) {
                            expired.push(pending);
                        }
                    }
                }
                for pending in expired {
                    match retry_pending_ack_timeout_via_propagation(&state, &bus, &pending).await {
                        Ok(true) => continue,
                        Ok(false) => {
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                "ack timeout",
                            );
                        }
                        Err(err) => {
                            let detail = format!("ack timeout; propagation retry failed: {err}");
                            record_pending_delivery_timed_out(
                                state.sdk.as_ref(),
                                &bus,
                                &pending,
                                detail.as_str(),
                            );
                        }
                    }
                }
            }
        });
    }

    // Cleanup stale buffered acknowledgements and receipt tracking.
    {
        let pending_lxmf_acknowledgements = state.pending_lxmf_acknowledgements.clone();
        let receipt_message_ids = receipt_message_ids.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let now = now_ms();
                let pruned_acks = {
                    let mut guard = pending_lxmf_acknowledgements.lock().await;
                    prune_expired_buffered_acknowledgements(&mut guard, now)
                };
                let pruned_receipts = if let Ok(mut guard) = receipt_message_ids.lock() {
                    prune_expired_receipt_tracking(&mut guard, now)
                } else {
                    0
                };
                if pruned_acks > 0 || pruned_receipts > 0 {
                    debug!(
                        "[runtime] pruned stale state buffered_acks={pruned_acks} receipt_tracking={pruned_receipts}",
                    );
                }
            }
        });
    }
}
