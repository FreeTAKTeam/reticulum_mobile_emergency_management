fn parse_address_hash(hex_32: &str) -> Result<AddressHash, NodeError> {
    let normalized = normalize_hex_32(hex_32).ok_or(NodeError::InvalidConfig {})?;
    AddressHash::new_from_hex_string(&normalized).map_err(|_| NodeError::InvalidConfig {})
}

async fn ensure_destination_desc(
    state: &SdkTransportState,
    dest: AddressHash,
    expected_name: Option<DestinationName>,
) -> Result<DestinationDesc, NodeError> {
    if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
        return Ok(desc);
    }

    state.transport.request_path(&dest, None, None).await;

    let deadline = tokio::time::Instant::now() + DEFAULT_IDENTITY_WAIT_TIMEOUT;
    loop {
        if let Some(desc) = state.known_destinations.lock().await.get(&dest).copied() {
            return Ok(desc);
        }

        if let Some(identity) = state.transport.destination_identity(&dest).await {
            let name = expected_name.unwrap_or_else(|| {
                DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1)
            });
            return Ok(DestinationDesc {
                identity,
                address_hash: dest,
                name,
            });
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn resolve_propagation_destination_desc(
    state: &SdkTransportState,
    destination: AddressHash,
) -> Result<DestinationDesc, NodeError> {
    ensure_destination_desc(
        state,
        destination,
        Some(DestinationName::new(
            LXMF_PROPAGATION_NAME.0,
            LXMF_PROPAGATION_NAME.1,
        )),
    )
    .await
}

async fn ensure_lxmf_output_link(
    state: &SdkTransportState,
    desc: DestinationDesc,
    requested_destination_hex: Option<&str>,
    resolved_destination_hex: Option<&str>,
    connect_timeout: Duration,
    max_attempts: usize,
) -> Result<Arc<TokioMutex<Link>>, NodeError> {
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    for attempt in 0..max_attempts.max(1) {
        state
            .transport
            .request_path(&desc.address_hash, None, None)
            .await;

        let link = {
            let mut links = state.out_links.lock().await;
            if let Some(existing) = links.get(&desc.address_hash).cloned() {
                existing
            } else {
                let created = state.transport.link(desc).await;
                links.insert(desc.address_hash, created.clone());
                created
            }
        };

        match wait_for_link_active(&state.transport, &link, connect_timeout).await {
            Ok(()) => return Ok(link),
            Err(err) => {
                let stale = state.out_links.lock().await.remove(&desc.address_hash);
                if let Some(stale) = stale {
                    stale.lock().await.close();
                }
                if attempt + 1 == max_attempts.max(1) {
                    log_lxmf_link_activation_failure(
                        "failed",
                        &desc,
                        requested_destination_hex,
                        resolved_destination_hex,
                        attempt + 1,
                        &err,
                    );
                    return Err(err);
                }
                log_lxmf_link_activation_failure(
                    "retry",
                    &desc,
                    requested_destination_hex,
                    resolved_destination_hex,
                    attempt + 1,
                    &err,
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(NodeError::Timeout {})
}

fn log_lxmf_link_activation_failure(
    status: &str,
    desc: &DestinationDesc,
    requested_destination_hex: Option<&str>,
    resolved_destination_hex: Option<&str>,
    attempt: usize,
    err: &NodeError,
) {
    if let (Some(requested_destination_hex), Some(resolved_destination_hex)) =
        (requested_destination_hex, resolved_destination_hex)
    {
        info!(
            "[lxmf][events][sdk] link activation {status} requested_destination={} resolved_destination={} link_destination={} attempt={} reason={}",
            requested_destination_hex,
            resolved_destination_hex,
            desc.address_hash.to_hex_string(),
            attempt,
            err,
        );
        return;
    }

    info!(
        "[lxmf][events][sdk] link activation {status} destination={} attempt={} reason={}",
        desc.address_hash.to_hex_string(),
        attempt,
        err,
    );
}

async fn clear_lxmf_output_link(state: &SdkTransportState, destination: &AddressHash) {
    let stale = state.out_links.lock().await.remove(destination);
    if let Some(stale) = stale {
        stale.lock().await.close();
    }
}

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<Link>>,
    timeout: Duration,
) -> Result<(), NodeError> {
    if link.lock().await.status() == LinkStatus::Active {
        return Ok(());
    }

    let link_id = *link.lock().await.id();
    let mut events = transport.out_link_events();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if link.lock().await.status() == LinkStatus::Active {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        match tokio::time::timeout(Duration::from_millis(250), events.recv()).await {
            Ok(Ok(event)) => {
                if event.id == link_id && matches!(event.event, LinkEvent::Activated) {
                    return Ok(());
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        }
    }
}
