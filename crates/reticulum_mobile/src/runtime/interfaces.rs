pub(crate) fn lxmf_private_identity(
    identity: &PrivateIdentity,
) -> Result<lxmf::identity::PrivateIdentity, NodeError> {
    lxmf::identity::PrivateIdentity::from_private_key_bytes(&identity.to_private_key_bytes())
        .map_err(|_| NodeError::InternalError {})
}

fn parse_address_hash(hex_32: &str) -> Result<AddressHash, NodeError> {
    let normalized = normalize_hex_32(hex_32).ok_or(NodeError::InvalidConfig {})?;
    AddressHash::new_from_hex_string(&normalized).map_err(|_| NodeError::InvalidConfig {})
}

fn address_hash_to_hex(hash: &AddressHash) -> String {
    hash.to_hex_string()
}

#[derive(Default)]
struct InterfaceTrafficSample {
    packets: u64,
    bytes: u64,
    announces: u64,
    data: u64,
    proofs: u64,
    link_requests: u64,
}

impl InterfaceTrafficSample {
    fn record(&mut self, packet: &Packet) {
        self.packets = self.packets.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(packet.data.as_slice().len() as u64);
        match packet.header.packet_type {
            PacketType::Announce => {
                self.announces = self.announces.saturating_add(1);
            }
            PacketType::Data => {
                self.data = self.data.saturating_add(1);
            }
            PacketType::Proof => {
                self.proofs = self.proofs.saturating_add(1);
            }
            PacketType::LinkRequest => {
                self.link_requests = self.link_requests.saturating_add(1);
            }
        }
    }
}

type ActiveInterfaceRegistry = Arc<TokioMutex<HashMap<AddressHash, InterfaceStatusRecord>>>;

fn interface_status_kind(label: &str) -> &'static str {
    if interface_label_is_rnode_ble(label) {
        "rnode_ble"
    } else {
        "tcp_client"
    }
}

fn new_interface_status(
    interface: AddressHash,
    label: String,
    state: &'static str,
) -> InterfaceStatusRecord {
    let kind = interface_status_kind(&label).to_string();
    InterfaceStatusRecord {
        interface_hex: interface.to_hex_string(),
        label,
        kind,
        state: state.to_string(),
        last_error: None,
        rx_packets: 0,
        rx_bytes: 0,
        last_activity_ms: 0,
    }
}

async fn publish_interface_registry_snapshot(
    active_interface_registry: &ActiveInterfaceRegistry,
    status: &Arc<Mutex<NodeStatus>>,
    bus: &EventBus,
    changed: Option<InterfaceStatusRecord>,
) {
    let mut interfaces = active_interface_registry
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    interfaces.sort_by(|left, right| left.label.cmp(&right.label));
    if let Ok(mut guard) = status.lock() {
        guard.interfaces = interfaces;
        guard.refresh_readiness();
        bus.emit(NodeEvent::StatusChanged {
            status: guard.clone(),
        });
    }
    if let Some(status) = changed {
        bus.emit(NodeEvent::InterfaceStatusChanged { status });
    }
}

fn effective_announce_interval_seconds(configured_seconds: u32) -> u32 {
    configured_seconds.max(MIN_EFFECTIVE_ANNOUNCE_INTERVAL_SECONDS)
}

fn spawn_interface_traffic_monitor(
    transport: Arc<Transport>,
    active_interface_registry: ActiveInterfaceRegistry,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
) {
    tokio::spawn(async move {
        let mut rx = transport.iface_rx();
        let mut interval = tokio::time::interval(INTERFACE_TRAFFIC_LOG_INTERVAL);
        let mut samples = HashMap::<AddressHash, InterfaceTrafficSample>::new();
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if samples.is_empty() {
                        continue;
                    }
                    let mut changed = Vec::new();
                    let mut rows = samples.drain().collect::<Vec<_>>();
                    rows.sort_by_key(|(_, sample)| std::cmp::Reverse(sample.bytes));
                    {
                        let mut endpoints = active_interface_registry.lock().await;
                        for (interface, sample) in rows.iter() {
                            if let Some(record) = endpoints.get_mut(interface) {
                                record.rx_packets = record.rx_packets.saturating_add(sample.packets);
                                record.rx_bytes = record.rx_bytes.saturating_add(sample.bytes);
                                record.last_activity_ms = now_ms();
                                changed.push(record.clone());
                            }
                        }
                    }
                    for (interface, sample) in rows {
                        let endpoints = active_interface_registry.lock().await;
                        let endpoint = endpoints
                            .get(&interface)
                            .map(|record| record.label.as_str())
                            .unwrap_or("unknown");
                        info!(
                            "[iface][rx] endpoint=<{}> iface={} packets={} bytes={} announces={} data={} proofs={} link_requests={}",
                            endpoint,
                            interface,
                            sample.packets,
                            sample.bytes,
                            sample.announces,
                            sample.data,
                            sample.proofs,
                            sample.link_requests,
                        );
                    }
                    for status_update in changed {
                        publish_interface_registry_snapshot(
                            &active_interface_registry,
                            &status,
                            &bus,
                            Some(status_update),
                        )
                        .await;
                    }
                }
                message = rx.recv() => {
                    match message {
                        Ok(message) => {
                            samples
                                .entry(message.address)
                                .or_default()
                                .record(&message.packet);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("[iface][rx] monitor lagged skipped={}", skipped);
                        }
                    }
                }
            }
        }
    });
}

async fn announce_destinations(
    transport: &Arc<Transport>,
    _app_destination: &Arc<TokioMutex<SingleInputDestination>>,
    lxmf_destination: &Arc<TokioMutex<SingleInputDestination>>,
    announce_capabilities: &Arc<TokioMutex<String>>,
    reason: &str,
) {
    let caps = announce_capabilities.lock().await.clone();
    let lxmf_hex = lxmf_destination
        .lock()
        .await
        .desc
        .address_hash
        .to_hex_string();
    info!(
        "[announce] sending reason={} kind={} destination={}",
        reason, DESTINATION_KIND_LXMF_DELIVERY, lxmf_hex,
    );
    transport
        .set_destination_announce_app_data(lxmf_destination, Some(caps.as_bytes().to_vec()))
        .await;
    send_announce_with_trace(
        transport,
        lxmf_destination,
        Some(caps.as_bytes()),
        reason,
        DESTINATION_KIND_LXMF_DELIVERY,
    )
    .await;
}

async fn send_announce_with_trace(
    transport: &Arc<Transport>,
    destination: &Arc<TokioMutex<SingleInputDestination>>,
    app_data: Option<&[u8]>,
    reason: &str,
    destination_kind: &str,
) {
    let (destination_hex, app_data_len, packet) = {
        let mut destination = destination.lock().await;
        let destination_hex = destination.desc.address_hash.to_hex_string();
        let app_data_len = app_data.map(|value| value.len()).unwrap_or(0);
        let packet = destination
            .announce(OsRng, app_data)
            .expect("valid announce packet");
        (destination_hex, app_data_len, packet)
    };
    let trace = transport.send_packet_with_trace(packet).await;
    info!(
        "[announce][tx] reason={} kind={} destination={} app_data_len={} outcome={:?} broadcast={} direct_iface={} matched={} sent={} failed={}",
        reason,
        destination_kind,
        destination_hex,
        app_data_len,
        trace.outcome,
        trace.broadcast,
        trace
            .direct_iface
            .map(|iface| iface.to_hex_string())
            .unwrap_or_else(|| "none".to_string()),
        trace.dispatch.matched_ifaces,
        trace.dispatch.sent_ifaces,
        trace.dispatch.failed_ifaces,
    );
}

fn announce_destination_kind_from_name_hash(name_hash: &[u8]) -> &'static str {
    let app_name = DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1);
    if name_hash == app_name.as_name_hash_slice() {
        return DESTINATION_KIND_APP;
    }

    let lxmf_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    if name_hash == lxmf_name.as_name_hash_slice() {
        return DESTINATION_KIND_LXMF_DELIVERY;
    }

    let propagation_name = DestinationName::new(LXMF_PROPAGATION_NAME.0, LXMF_PROPAGATION_NAME.1);
    if name_hash == propagation_name.as_name_hash_slice() {
        return DESTINATION_KIND_LXMF_PROPAGATION;
    }

    DESTINATION_KIND_OTHER
}

fn app_data_has_rem_peer_capabilities(app_data: &str) -> bool {
    supports_mission_traffic(Some(app_data))
}

fn classify_announce(destination_kind: &str, app_data: &str) -> AnnounceClass {
    let tokens = parse_announce_metadata(app_data).capability_tokens;
    if tokens.iter().any(|token| token == "r3akt")
        && RCH_SERVER_FEATURE_CAPABILITIES
            .iter()
            .any(|capability| tokens.iter().any(|token| token == capability))
    {
        return AnnounceClass::RchHubServer {};
    }

    match destination_kind {
        DESTINATION_KIND_LXMF_PROPAGATION => AnnounceClass::PropagationNode {},
        DESTINATION_KIND_LXMF_DELIVERY => AnnounceClass::LxmfDelivery {},
        _ => {
            if supports_mission_traffic(Some(app_data)) {
                return AnnounceClass::PeerApp {};
            }
            AnnounceClass::Other {}
        }
    }
}

fn announce_class_is_operator_relevant(class: AnnounceClass) -> bool {
    matches!(class, AnnounceClass::RchHubServer {})
}

fn announce_is_operator_relevant(class: AnnounceClass, is_rem_capable_lxmf_delivery: bool) -> bool {
    announce_class_is_operator_relevant(class)
        || (matches!(class, AnnounceClass::LxmfDelivery {}) && is_rem_capable_lxmf_delivery)
}

fn operator_label(display_name: Option<&str>, fallback_hex: &str) -> String {
    display_name
        .and_then(normalize_rem_display_name)
        .unwrap_or_else(|| fallback_hex.to_ascii_lowercase())
}

fn short_destination_hex(value: &str) -> String {
    let prefix = value
        .chars()
        .take(5)
        .collect::<String>()
        .to_ascii_lowercase();
    if prefix.len() < value.len() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn operator_announce_message(
    announce_class: AnnounceClass,
    is_rem_capable_lxmf_delivery: bool,
    display_name: Option<&str>,
    destination_hex: &str,
    _identity_hex: &str,
    hops: u8,
) -> Option<String> {
    if !announce_is_operator_relevant(announce_class, is_rem_capable_lxmf_delivery) {
        return None;
    }

    let subject = operator_label(display_name, destination_hex);
    let prefix = match announce_class {
        AnnounceClass::RchHubServer {} => "RCH hub",
        AnnounceClass::LxmfDelivery {} if is_rem_capable_lxmf_delivery => "",
        _ => return None,
    };
    let label = if prefix.is_empty() {
        subject
    } else {
        format!("{prefix} {subject}")
    };
    Some(format!(
        "[announce] {label} dest={} hops={hops}.",
        short_destination_hex(destination_hex),
    ))
}
