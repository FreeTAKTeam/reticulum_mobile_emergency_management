#[expect(
    clippy::too_many_arguments,
    reason = "retain relay policy with a captured offline failure"
)]
async fn send_lxmf_via_propagation_candidates(
    state: &NodeRuntimeState,
    destination: AddressHash,
    request: (&str, OutboundTrafficClass),
    body: &[u8],
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: Option<MissionSyncMetadata>,
    failed_attempt: &mut Option<crate::sdk_bridge::FailedLxmfAttempt>,
) -> Result<LxmfSendReport, NodeError> {
    let (requested_destination_hex, traffic_class) = request;
    let active_relay = state.active_propagation_node_hex.lock().await.clone();
    let active_relay_hex = active_relay.as_deref().unwrap_or("");
    let announces = state.messaging.lock().await.list_announces();
    let mut relay_candidates = propagation_sync_candidate_relays(
        announces.as_slice(),
        active_relay_hex,
        state.preferred_propagation_node_hex.as_deref(),
    );
    if relay_candidates.is_empty() {
        return Err(delivery_route_unavailable_error());
    }

    let mut last_error = None;
    for (index, relay_candidate) in relay_candidates.drain(..).enumerate() {
        ensure_current_telemetry_delivery(
            state,
            traffic_class,
            requested_destination_hex,
            metadata.as_ref(),
        )?;
        let attempt_number = index + 1;
        info!(
            "[lxmf][mission] propagation send relay attempt relay={relay_candidate} attempt={attempt_number}/{PROPAGATION_SYNC_MAX_RELAY_ATTEMPTS} destination={requested_destination_hex}",
        );

        match state
            .sdk
            .send_lxmf_via_propagation_relay(
                destination,
                body,
                title.clone(),
                fields_bytes.clone(),
                metadata.clone(),
                relay_candidate.clone(),
                failed_attempt,
            )
            .await
        {
            Ok(report) if lxmf_send_succeeded(report.outcome) => {
                return Ok(report);
            }
            Ok(report) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={} destination={} outcome={:?}",
                    relay_candidate, requested_destination_hex, report.outcome,
                );
                last_error = Some(NodeError::NetworkError {});
            }
            Err(err) => {
                info!(
                    "[lxmf][mission] propagation send relay attempt failed relay={relay_candidate} destination={requested_destination_hex} reason={err}",
                );
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or(NodeError::NetworkError {}))
}
