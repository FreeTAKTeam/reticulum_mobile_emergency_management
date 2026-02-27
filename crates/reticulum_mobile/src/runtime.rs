use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel as cb;
use fs_err as fs;
use lxmf::message::Message as LxmfMessage;
use rand_core::OsRng;
use regex::Regex;
use reticulum::destination::link::{LinkEvent, LinkStatus};
use reticulum::destination::{DestinationDesc, DestinationName};
use reticulum::hash::AddressHash;
use reticulum::identity::PrivateIdentity;
use reticulum::iface::tcp_client::TcpClient;
use reticulum::packet::{Packet, PacketDataBuffer, PropagationType};
use reticulum::transport::{SendPacketOutcome as RnsSendOutcome, Transport, TransportConfig};
use tokio::sync::{mpsc, Mutex as TokioMutex};

use crate::event_bus::EventBus;
use crate::types::{
    HubMode, NodeConfig, NodeError, NodeEvent, NodeStatus, PeerChange, PeerState, SendOutcome,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");

const DEFAULT_LINK_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_IDENTITY_WAIT_TIMEOUT: Duration = Duration::from_secs(12);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn normalize_hex_32(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.len() != 32 {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn parse_address_hash(hex_32: &str) -> Result<AddressHash, NodeError> {
    let normalized = normalize_hex_32(hex_32).ok_or(NodeError::InvalidConfig {})?;
    AddressHash::new_from_hex_string(&normalized).map_err(|_| NodeError::InvalidConfig {})
}

fn address_hash_to_hex(hash: &AddressHash) -> String {
    hash.to_hex_string()
}

fn join_url(base: &str, path: &str) -> Result<String, NodeError> {
    let base = base.trim();
    if base.is_empty() {
        return Err(NodeError::InvalidConfig {});
    }
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    Ok(format!("{base}/{path}"))
}

fn extract_hex_destinations(text: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[^0-9a-f])([0-9a-f]{32})(?:$|[^0-9a-f])").expect("regex")
    });

    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for caps in re.captures_iter(text) {
        let Some(m) = caps.get(1) else {
            continue;
        };
        let value = m.as_str().to_ascii_lowercase();
        if seen.insert(value.clone()) {
            out.push(value);
        }
    }
    out
}

fn send_outcome_to_udl(outcome: RnsSendOutcome) -> SendOutcome {
    match outcome {
        RnsSendOutcome::SentDirect => SendOutcome::SentDirect {},
        RnsSendOutcome::SentBroadcast => SendOutcome::SentBroadcast {},
        RnsSendOutcome::DroppedMissingDestinationIdentity => {
            SendOutcome::DroppedMissingDestinationIdentity {}
        }
        RnsSendOutcome::DroppedCiphertextTooLarge => SendOutcome::DroppedCiphertextTooLarge {},
        RnsSendOutcome::DroppedEncryptFailed => SendOutcome::DroppedEncryptFailed {},
        RnsSendOutcome::DroppedNoRoute => SendOutcome::DroppedNoRoute {},
    }
}

fn create_transport_data_packet(destination: AddressHash, bytes: &[u8]) -> Packet {
    let mut packet = Packet::default();
    packet.header.propagation_type = PropagationType::Transport;
    packet.destination = destination;
    packet.data = PacketDataBuffer::new_from_slice(bytes);
    packet
}

async fn send_transport_packet_with_path_retry(
    transport: &Arc<Transport>,
    destination: AddressHash,
    bytes: &[u8],
) -> RnsSendOutcome {
    const MAX_ATTEMPTS: usize = 6;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    let mut last_outcome = RnsSendOutcome::DroppedNoRoute;

    for _ in 0..MAX_ATTEMPTS {
        let packet = create_transport_data_packet(destination, bytes);
        let outcome = transport.send_packet_with_outcome(packet).await;
        if matches!(outcome, RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast) {
            return outcome;
        }

        last_outcome = outcome;
        if matches!(
            outcome,
            RnsSendOutcome::DroppedNoRoute | RnsSendOutcome::DroppedMissingDestinationIdentity
        ) {
            transport.request_path(&destination, None, None).await;
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }
        break;
    }

    last_outcome
}

pub enum Command {
    Stop { resp: cb::Sender<Result<(), NodeError>> },
    ConnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    DisconnectPeer {
        destination_hex: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SendBytes {
        destination_hex: String,
        bytes: Vec<u8>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    BroadcastBytes {
        bytes: Vec<u8>,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetAnnounceCapabilities {
        capability_string: String,
        resp: cb::Sender<Result<(), NodeError>>,
    },
    SetLogLevel { level: crate::types::LogLevel },
    RefreshHubDirectory { resp: cb::Sender<Result<(), NodeError>> },
}

#[derive(Clone)]
struct NodeRuntimeState {
    identity: PrivateIdentity,
    transport: Arc<Transport>,
    lxmf_destination: Arc<TokioMutex<reticulum::destination::SingleInputDestination>>,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
    out_links:
        Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>>,
}

async fn ensure_destination_desc(
    state: &NodeRuntimeState,
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

async fn wait_for_link_active(
    transport: &Arc<Transport>,
    link: &Arc<TokioMutex<reticulum::destination::link::Link>>,
) -> Result<(), NodeError> {
    if link.lock().await.status() == LinkStatus::Active {
        return Ok(());
    }

    let link_id = *link.lock().await.id();
    let mut events = transport.out_link_events();
    let deadline = tokio::time::Instant::now() + DEFAULT_LINK_CONNECT_TIMEOUT;

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

async fn refresh_hub_directory_http(config: &NodeConfig) -> Result<Vec<String>, NodeError> {
    let base = config
        .hub_api_base_url
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let url = join_url(base, "/Client")?;

    let mut req = reqwest::Client::new().get(url);
    if let Some(key) = config
        .hub_api_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        req = req
            .header("X-API-Key", key)
            .header("Authorization", format!("Bearer {}", key));
    }

    let body = req
        .send()
        .await
        .map_err(|_| NodeError::NetworkError {})?
        .text()
        .await
        .map_err(|_| NodeError::NetworkError {})?;

    Ok(extract_hex_destinations(&body))
}

async fn refresh_hub_directory_lxmf(
    config: &NodeConfig,
    state: &NodeRuntimeState,
) -> Result<Vec<String>, NodeError> {
    let hub_hex = config
        .hub_identity_hash
        .as_deref()
        .ok_or(NodeError::InvalidConfig {})?;
    let hub = parse_address_hash(hub_hex)?;

    let hub_name = DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1);
    let hub_desc = ensure_destination_desc(state, hub, Some(hub_name)).await?;

    let link = {
        let mut links = state.out_links.lock().await;
        if let Some(existing) = links.get(&hub).cloned() {
            existing
        } else {
            let created = state.transport.link(hub_desc).await;
            links.insert(hub, created.clone());
            created
        }
    };

    wait_for_link_active(&state.transport, &link).await?;

    let mut source = [0u8; 16];
    source.copy_from_slice(
        state
            .lxmf_destination
            .lock()
            .await
            .desc
            .address_hash
            .as_slice(),
    );
    let mut destination = [0u8; 16];
    destination.copy_from_slice(hub.as_slice());

    let content = r#"\\\{"Command":"ListClients"}"#;

    let mut message = LxmfMessage::new();
    message.source_hash = Some(source);
    message.destination_hash = Some(destination);
    message.set_title_from_string("ListClients");
    message.set_content_from_string(content);

    let wire = message
        .to_wire(Some(&state.identity))
        .map_err(|_| NodeError::InternalError {})?;

    let packet = link
        .lock()
        .await
        .data_packet(&wire)
        .map_err(|_| NodeError::InternalError {})?;
    let outcome = state.transport.send_packet_with_outcome(packet).await;
    if !matches!(outcome, RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast) {
        return Err(NodeError::NetworkError {});
    }

    let mut rx = state.transport.received_data_events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(NodeError::Timeout {});
        }

        let received = match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(event)) => event,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(NodeError::InternalError {})
            }
            Err(_) => continue,
        };

        if received.destination != hub {
            continue;
        }

        let Ok(reply) = LxmfMessage::from_wire(received.data.as_slice()) else {
            continue;
        };

        let mut text = String::new();
        if !reply.title.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.title));
            text.push('\n');
        }
        if !reply.content.is_empty() {
            text.push_str(&String::from_utf8_lossy(&reply.content));
            text.push('\n');
        }
        if let Some(fields) = &reply.fields {
            text.push_str(&format!("{fields:?}"));
        }

        let destinations = extract_hex_destinations(&text);
        if !destinations.is_empty() {
            return Ok(destinations);
        }
    }
}

pub async fn run_node(
    config: NodeConfig,
    identity: PrivateIdentity,
    status: Arc<Mutex<NodeStatus>>,
    bus: EventBus,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
) {
    let mut transport_cfg = TransportConfig::new(config.name.clone(), &identity, config.broadcast);
    transport_cfg.set_retransmit(false);

    if let Some(dir) = config
        .storage_dir
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let mut path = PathBuf::from(dir);
        path.push("ratchets.dat");
        transport_cfg.set_ratchet_store_path(path);
    }

    let mut transport = Transport::new(transport_cfg);

    for endpoint in &config.tcp_clients {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            continue;
        }
        transport
            .iface_manager()
            .lock()
            .await
            .spawn(TcpClient::new(endpoint), TcpClient::spawn);
    }

    let app_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .await;
    let lxmf_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .await;

    let transport = Arc::new(transport);

    let announce_capabilities = Arc::new(TokioMutex::new(config.announce_capabilities.clone()));
    let known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let out_links: Arc<TokioMutex<HashMap<AddressHash, Arc<TokioMutex<reticulum::destination::link::Link>>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let connected_peers: Arc<TokioMutex<HashSet<AddressHash>>> =
        Arc::new(TokioMutex::new(HashSet::new()));

    let state = NodeRuntimeState {
        identity: identity.clone(),
        transport: transport.clone(),
        lxmf_destination: lxmf_destination.clone(),
        known_destinations: known_destinations.clone(),
        out_links: out_links.clone(),
    };

    if let Ok(mut guard) = status.lock() {
        guard.running = true;
        bus.emit(NodeEvent::StatusChanged { status: guard.clone() });
    }

    // Announces.
    {
        let transport = transport.clone();
        let app_destination = app_destination.clone();
        let lxmf_destination = lxmf_destination.clone();
        let announce_capabilities = announce_capabilities.clone();
        let interval_secs = config.announce_interval_seconds.max(1);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                let caps = announce_capabilities.lock().await.clone();
                transport
                    .send_announce(&app_destination, Some(caps.as_bytes()))
                    .await;
                transport.send_announce(&lxmf_destination, None).await;
            }
        });
    }

    // Announce receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        let known_destinations = known_destinations.clone();
        tokio::spawn(async move {
            let mut rx = transport.recv_announces().await;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let desc = event.destination.lock().await.desc;
                        known_destinations.lock().await.insert(desc.address_hash, desc);
                        let destination_hex = address_hash_to_hex(&desc.address_hash);
                        let app_data = String::from_utf8(event.app_data.as_slice().to_vec())
                            .unwrap_or_else(|_| hex::encode(event.app_data.as_slice()));
                        let interface_hex = hex::encode(event.interface);
                        bus.emit(NodeEvent::AnnounceReceived {
                            destination_hex,
                            app_data,
                            hops: event.hops,
                            interface_hex,
                            received_at_ms: now_ms(),
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Data receiver.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        tokio::spawn(async move {
            let mut rx = transport.received_data_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        bus.emit(NodeEvent::PacketReceived {
                            destination_hex: address_hash_to_hex(&event.destination),
                            bytes: event.data.as_slice().to_vec(),
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Link events.
    {
        let transport = transport.clone();
        let bus = bus.clone();
        tokio::spawn(async move {
            let mut rx = transport.out_link_events();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let destination_hex = address_hash_to_hex(&event.address_hash);
                        match event.event {
                            LinkEvent::Activated => bus.emit(NodeEvent::PeerChanged {
                                change: PeerChange {
                                    destination_hex,
                                    state: PeerState::Connected {},
                                    last_error: None,
                                },
                            }),
                            LinkEvent::Closed => bus.emit(NodeEvent::PeerChanged {
                                change: PeerChange {
                                    destination_hex,
                                    state: PeerState::Disconnected {},
                                    last_error: None,
                                },
                            }),
                            LinkEvent::Data(_) => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    // Optional periodic hub refresh.
    if !matches!(config.hub_mode, HubMode::Disabled {}) && config.hub_refresh_interval_seconds > 0 {
        let bus = bus.clone();
        let config = config.clone();
        let state = state.clone();
        let interval_secs = config.hub_refresh_interval_seconds;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs as u64));
            loop {
                interval.tick().await;
                let destinations = match config.hub_mode {
                    HubMode::RchHttp {} => refresh_hub_directory_http(&config).await.ok(),
                    HubMode::RchLxmf {} => refresh_hub_directory_lxmf(&config, &state).await.ok(),
                    HubMode::Disabled {} => None,
                };
                if let Some(destinations) = destinations {
                    bus.emit(NodeEvent::HubDirectoryUpdated {
                        destinations,
                        received_at_ms: now_ms(),
                    });
                }
            }
        });
    }

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Command::Stop { resp } => {
                if let Ok(mut guard) = status.lock() {
                    guard.running = false;
                    bus.emit(NodeEvent::StatusChanged { status: guard.clone() });
                }
                let _ = resp.send(Ok(()));
                break;
            }
            Command::SetLogLevel { level } => {
                crate::logger::NodeLogger::global().set_level(level);
            }
            Command::SetAnnounceCapabilities {
                capability_string,
                resp,
            } => {
                *announce_capabilities.lock().await = capability_string;
                let caps = announce_capabilities.lock().await.clone();
                transport
                    .send_announce(&app_destination, Some(caps.as_bytes()))
                    .await;
                let _ = resp.send(Ok(()));
            }
            Command::ConnectPeer {
                destination_hex,
                resp,
            } => {
                let destination_hex_copy = destination_hex.clone();
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex.clone(),
                            state: PeerState::Connecting {},
                            last_error: None,
                        },
                    });
                    connected_peers.lock().await.insert(dest);
                    // Fire a path request in the background; direct send will resolve identity on demand.
                    transport.request_path(&dest, None, None).await;
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex.clone(),
                            state: PeerState::Connected {},
                            last_error: None,
                        },
                    });
                    Ok::<(), NodeError>(())
                }
                .await;
                if let Err(err) = &result {
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex: destination_hex_copy,
                            state: PeerState::Disconnected {},
                            last_error: Some(err.to_string()),
                        },
                    });
                }
                let _ = resp.send(result);
            }
            Command::DisconnectPeer {
                destination_hex,
                resp,
            } => {
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    connected_peers.lock().await.remove(&dest);
                    // Clean up any stale link from older builds if present.
                    if let Some(link) = out_links.lock().await.remove(&dest) {
                        link.lock().await.close();
                    }
                    bus.emit(NodeEvent::PeerChanged {
                        change: PeerChange {
                            destination_hex,
                            state: PeerState::Disconnected {},
                            last_error: None,
                        },
                    });
                    Ok::<(), NodeError>(())
                }
                .await;
                let _ = resp.send(result);
            }
            Command::SendBytes {
                destination_hex,
                bytes,
                resp,
            } => {
                let result = async {
                    let dest = parse_address_hash(&destination_hex)?;
                    let outcome =
                        send_transport_packet_with_path_retry(&transport, dest, &bytes).await;
                    let mapped = send_outcome_to_udl(outcome);
                    bus.emit(NodeEvent::PacketSent {
                        destination_hex: destination_hex.clone(),
                        bytes: bytes.clone(),
                        outcome: mapped,
                    });

                    if matches!(
                        outcome,
                        RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast
                    ) {
                        Ok(())
                    } else {
                        Err(NodeError::NetworkError {})
                    }
                }
                .await;
                let _ = resp.send(result);
            }
            Command::BroadcastBytes { bytes, resp } => {
                let result = async {
                    let peers = connected_peers.lock().await.iter().copied().collect::<Vec<_>>();
                    let mut sent_any = false;
                    for dest in peers {
                        let outcome =
                            send_transport_packet_with_path_retry(&transport, dest, &bytes).await;
                        bus.emit(NodeEvent::PacketSent {
                            destination_hex: address_hash_to_hex(&dest),
                            bytes: bytes.clone(),
                            outcome: send_outcome_to_udl(outcome),
                        });
                        if matches!(outcome, RnsSendOutcome::SentDirect | RnsSendOutcome::SentBroadcast)
                        {
                            sent_any = true;
                        }
                    }

                    if sent_any {
                        Ok::<(), NodeError>(())
                    } else {
                        Err(NodeError::NetworkError {})
                    }
                }
                .await;
                let _ = resp.send(result);
            }
            Command::RefreshHubDirectory { resp } => {
                let result = match config.hub_mode {
                    HubMode::Disabled {} => Err(NodeError::InvalidConfig {}),
                    HubMode::RchHttp {} => refresh_hub_directory_http(&config).await,
                    HubMode::RchLxmf {} => refresh_hub_directory_lxmf(&config, &state).await,
                }
                .map(|destinations| {
                    bus.emit(NodeEvent::HubDirectoryUpdated {
                        destinations,
                        received_at_ms: now_ms(),
                    });
                });
                let _ = resp.send(result.map(|_| ()));
            }
        }
    }

    if let Ok(mut guard) = status.lock() {
        guard.running = false;
        bus.emit(NodeEvent::StatusChanged { status: guard.clone() });
    }
}

fn identity_path(storage_dir: &Path) -> PathBuf {
    storage_dir.join("identity.hex")
}

pub fn load_or_create_identity(
    storage_dir: Option<&str>,
    name: &str,
) -> Result<PrivateIdentity, NodeError> {
    let Some(dir) = storage_dir.map(str::trim).filter(|v| !v.is_empty()) else {
        // Deterministic fallback for dev.
        return Ok(PrivateIdentity::new_from_name(name));
    };

    let dir = PathBuf::from(dir);
    fs::create_dir_all(&dir).map_err(|_| NodeError::IoError {})?;
    let path = identity_path(&dir);

    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(|_| NodeError::IoError {})?;
        let hex = raw.trim();
        return PrivateIdentity::new_from_hex_string(hex).map_err(|_| NodeError::IoError {});
    }

    let identity = PrivateIdentity::new_from_rand(OsRng);
    fs::write(&path, identity.to_hex_string()).map_err(|_| NodeError::IoError {})?;
    Ok(identity)
}
