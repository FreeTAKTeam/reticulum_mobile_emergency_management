async fn wait_for_result_receipt(
    tracker: &ReceiptTracker,
    receipt_hash: &str,
    timeout: Duration,
) -> Result<(), NodeError> {
    let signal = Arc::new(tokio::sync::Notify::new());
    {
        let mut tracking = tracker
            .receipt_message_ids
            .lock()
            .map_err(|_| NodeError::InternalError {})?;
        match tracking.remove(receipt_hash) {
            Some(ReceiptMessageTracking::Observed { .. }) => signal.notify_one(),
            Some(existing) => {
                tracking.insert(receipt_hash.to_string(), existing);
                return Err(NodeError::InternalError {});
            }
            None => {
                tracking.insert(
                    receipt_hash.to_string(),
                    ReceiptMessageTracking::Awaited {
                        signal: signal.clone(),
                        recorded_at_ms: now_ms(),
                    },
                );
            }
        }
    }
    let observed = tokio::time::timeout(timeout, signal.notified())
        .await
        .is_ok();
    let mut tracking = tracker
        .receipt_message_ids
        .lock()
        .map_err(|_| NodeError::InternalError {})?;
    if matches!(tracking.get(receipt_hash), Some(ReceiptMessageTracking::Awaited { signal: current, .. }) if Arc::ptr_eq(current, &signal))
    {
        tracking.remove(receipt_hash);
    }
    if observed {
        Ok(())
    } else {
        Err(NodeError::Timeout {})
    }
}

async fn send_operational_result_with_recovery(
    state: &NodeRuntimeState,
    bus: &EventBus,
    ack: &OperationalAck,
    fields: Vec<u8>,
    metadata: Option<MissionSyncMetadata>,
) -> Result<LxmfSendReport, NodeError> {
    let destination = parse_address_hash(&ack.destination_hex)?;
    for attempt in 0..2 {
        let existing_link = state.transport.delivery_link(&destination).await;
        let report = send_lxmf_with_delivery_policy(
            state,
            bus,
            &ack.destination_hex,
            &[],
            None,
            Some(fields.clone()),
            metadata.clone(),
            SendMode::Auto {},
            SendTaskClass::MissionAck,
            false,
            OutboundTrafficClass::Control {},
        )
        .await?;
        if matches!(report.method, LxmfDeliveryMethod::Direct {})
            && matches!(report.representation, LxmfDeliveryRepresentation::Packet {})
        {
            let hash = report
                .receipt_hash_hex
                .as_deref()
                .ok_or(NodeError::InternalError {})?;
            match wait_for_result_receipt(&state.receipt_tracker, hash, DEFAULT_LXMF_ACK_TIMEOUT)
                .await
            {
                Ok(()) => return Ok(report),
                Err(NodeError::Timeout {}) if attempt == 0 => {
                    // Close only the captured existing handle, never a concurrent replacement.
                    if let Some(link) = existing_link {
                        link.lock().await.close();
                    }
                    info!(
                        "[lxmf][mission] application result transport proof timed out destination={} command={}; retrying once",
                        ack.destination_hex, ack.command_id
                    );
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        return Ok(report);
    }
    Err(NodeError::Timeout {})
}
