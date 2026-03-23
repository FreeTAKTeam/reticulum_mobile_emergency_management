use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel as cb;
use reticulum::destination::DestinationName;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::event_bus::EventBus;
use crate::logger::NodeLogger;
use crate::runtime::{load_or_create_identity, run_node, Command};
use crate::types::{
    AnnounceRecord, ConversationRecord, LogLevel, MessageRecord, NodeConfig, NodeError, NodeEvent,
    NodeStatus, PeerRecord, SendLxmfRequest, SyncStatus,
};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");
const SEND_COMMAND_TIMEOUT: Duration = Duration::from_secs(45);

struct NodeInner {
    bus: EventBus,
    status: Arc<Mutex<NodeStatus>>,
    peers_snapshot: Arc<Mutex<Vec<PeerRecord>>>,
    sync_status_snapshot: Arc<Mutex<SyncStatus>>,
    runtime: Option<Runtime>,
    cmd_tx: Option<mpsc::UnboundedSender<Command>>,
}

pub struct Node {
    inner: Mutex<NodeInner>,
}

impl Node {
    pub fn new() -> Self {
        NodeLogger::install();

        let initial = NodeStatus {
            running: false,
            name: "reticulum-mobile".to_string(),
            identity_hex: String::new(),
            app_destination_hex: String::new(),
            lxmf_destination_hex: String::new(),
        };

        Self {
            inner: Mutex::new(NodeInner {
                bus: EventBus::new(),
                status: Arc::new(Mutex::new(initial)),
                peers_snapshot: Arc::new(Mutex::new(Vec::new())),
                sync_status_snapshot: Arc::new(Mutex::new(SyncStatus {
                    phase: crate::types::SyncPhase::Idle {},
                    active_propagation_node_hex: None,
                    requested_at_ms: None,
                    completed_at_ms: None,
                    messages_received: 0,
                    detail: None,
                })),
                runtime: None,
                cmd_tx: None,
            }),
        }
    }

    pub fn start(&self, config: NodeConfig) -> Result<(), NodeError> {
        let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        if inner.runtime.is_some() {
            return Err(NodeError::AlreadyRunning {});
        }

        if config.name.trim().is_empty() {
            return Err(NodeError::InvalidConfig {});
        }

        let identity = load_or_create_identity(config.storage_dir.as_deref(), &config.name)?;

        let app_hash = reticulum::destination::SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .desc
        .address_hash;
        let lxmf_hash = reticulum::destination::SingleInputDestination::new(
            identity.clone(),
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .desc
        .address_hash;

        if let Ok(mut guard) = inner.status.lock() {
            *guard = NodeStatus {
                running: false,
                name: config.name.clone(),
                identity_hex: identity.address_hash().to_hex_string(),
                app_destination_hex: app_hash.to_hex_string(),
                lxmf_destination_hex: lxmf_hash.to_hex_string(),
            };
        }

        // Forward Rust logs to the UI event bus.
        NodeLogger::global().set_bus(Some(inner.bus.clone()));

        if let Ok(guard) = inner.status.lock() {
            inner.bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }

        let runtime = Runtime::new().map_err(|_| NodeError::InternalError {})?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        runtime.spawn(run_node(
            config,
            identity,
            inner.status.clone(),
            inner.peers_snapshot.clone(),
            inner.sync_status_snapshot.clone(),
            inner.bus.clone(),
            cmd_rx,
        ));

        inner.runtime = Some(runtime);
        inner.cmd_tx = Some(cmd_tx);

        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        let (runtime, cmd_tx, bus, status, peers_snapshot, sync_status_snapshot) = {
            let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            (
                inner.runtime.take(),
                inner.cmd_tx.take(),
                inner.bus.clone(),
                inner.status.clone(),
                inner.peers_snapshot.clone(),
                inner.sync_status_snapshot.clone(),
            )
        };

        let Some(runtime) = runtime else {
            return Ok(());
        };

        if let Some(cmd_tx) = cmd_tx {
            let (tx, rx) = cb::bounded(1);
            let _ = cmd_tx.send(Command::Stop { resp: tx });
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }

        drop(runtime);
        NodeLogger::global().set_bus(None);

        if let Ok(mut guard) = status.lock() {
            guard.running = false;
            bus.emit(NodeEvent::StatusChanged {
                status: guard.clone(),
            });
        }
        if let Ok(mut guard) = peers_snapshot.lock() {
            guard.clear();
        }
        if let Ok(mut guard) = sync_status_snapshot.lock() {
            *guard = SyncStatus {
                phase: crate::types::SyncPhase::Idle {},
                active_propagation_node_hex: None,
                requested_at_ms: None,
                completed_at_ms: None,
                messages_received: 0,
                detail: None,
            };
        }

        Ok(())
    }

    pub fn restart(&self, config: NodeConfig) -> Result<(), NodeError> {
        self.stop()?;
        self.start(config)
    }

    pub fn get_status(&self) -> NodeStatus {
        let inner = self.inner.lock().ok();
        let Some(inner) = inner else {
            return NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
            };
        };

        inner
            .status
            .lock()
            .map(|v| v.clone())
            .unwrap_or(NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
            })
    }

    pub fn connect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::ConnectPeer {
            destination_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn disconnect_peer(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::DisconnectPeer {
            destination_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_bytes(
        &self,
        destination_hex: String,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
        use_propagation_node: bool,
    ) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::SendBytes {
            destination_hex,
            bytes,
            fields_bytes,
            use_propagation_node,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn broadcast_bytes(&self, bytes: Vec<u8>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::BroadcastBytes {
            bytes,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn announce_now(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        tx.send(Command::AnnounceNow {})
            .map_err(|_| NodeError::NotRunning {})
    }

    pub fn request_peer_identity(&self, destination_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::RequestPeerIdentity {
            destination_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn send_lxmf(&self, request: SendLxmfRequest) -> Result<String, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::SendLxmf {
            request,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn retry_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::RetryLxmf {
            message_id_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(SEND_COMMAND_TIMEOUT)
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn cancel_lxmf(&self, message_id_hex: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::CancelLxmf {
            message_id_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_active_propagation_node(
        &self,
        destination_hex: Option<String>,
    ) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::SetActivePropagationNode {
            destination_hex,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn request_lxmf_sync(&self, limit: Option<u32>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::RequestLxmfSync {
            limit,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_announces(&self) -> Result<Vec<AnnounceRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::ListAnnounces { resp: resp_tx })
            .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_peers(&self) -> Result<Vec<PeerRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .peers_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| NodeError::InternalError {})
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::ListConversations { resp: resp_tx })
            .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn list_messages(
        &self,
        conversation_id: Option<String>,
    ) -> Result<Vec<MessageRecord>, NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::ListMessages {
            conversation_id,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn get_lxmf_sync_status(&self) -> Result<SyncStatus, NodeError> {
        let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
        inner
            .sync_status_snapshot
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| NodeError::InternalError {})
    }

    pub fn set_announce_capabilities(&self, capability_string: String) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::SetAnnounceCapabilities {
            capability_string,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn set_log_level(&self, level: LogLevel) {
        NodeLogger::global().set_level(level);
        if let Ok(inner) = self.inner.lock() {
            if let Some(tx) = inner.cmd_tx.clone() {
                let _ = tx.send(Command::SetLogLevel { level });
            }
        }
    }

    pub fn subscribe_events(&self) -> Arc<EventSubscription> {
        let rx = self
            .inner
            .lock()
            .map(|inner| inner.bus.subscribe())
            .unwrap_or_else(|_| {
                let (_tx, rx) = cb::unbounded();
                rx
            });
        Arc::new(EventSubscription::new(rx))
    }

    pub fn refresh_hub_directory(&self) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::RefreshHubDirectory { resp: resp_tx })
            .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(Err(NodeError::Timeout {}))
    }
}

pub struct EventSubscription {
    rx: cb::Receiver<NodeEvent>,
    closed: AtomicBool,
}

impl EventSubscription {
    fn new(rx: cb::Receiver<NodeEvent>) -> Self {
        Self {
            rx,
            closed: AtomicBool::new(false),
        }
    }

    pub fn next(&self, timeout_ms: u32) -> Option<NodeEvent> {
        if self.closed.load(Ordering::Relaxed) {
            return None;
        }

        if timeout_ms == 0 {
            return self.rx.try_recv().ok();
        }

        self.rx
            .recv_timeout(Duration::from_millis(timeout_ms as u64))
            .ok()
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mission_sync::parse_mission_sync_metadata;
    use crate::HubMode;
    use rmpv::Value as MsgPackValue;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, OnceLock};
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};

    static TEST_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

    const TEST_TIMEOUT: Duration = Duration::from_secs(30);

    struct TcpRelayHandle {
        addr: SocketAddr,
        shutdown: Arc<Notify>,
        task: tokio::task::JoinHandle<()>,
    }

    impl TcpRelayHandle {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind relay listener");
            let addr = listener.local_addr().expect("relay local addr");
            let shutdown = Arc::new(Notify::new());
            let clients: Arc<AsyncMutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>> =
                Arc::new(AsyncMutex::new(HashMap::new()));
            let next_client_id = Arc::new(AtomicUsize::new(1));

            let task = {
                let shutdown = shutdown.clone();
                let clients = clients.clone();
                let next_client_id = next_client_id.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = shutdown.notified() => break,
                            accepted = listener.accept() => {
                                let Ok((stream, _peer)) = accepted else {
                                    break;
                                };
                                let client_id = next_client_id.fetch_add(1, AtomicOrdering::Relaxed);
                                let (mut read_half, mut write_half) = stream.into_split();
                                let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
                                clients.lock().await.insert(client_id, tx);

                                let writer_clients = clients.clone();
                                tokio::spawn(async move {
                                    while let Some(chunk) = rx.recv().await {
                                        if write_half.write_all(chunk.as_slice()).await.is_err() {
                                            break;
                                        }
                                    }
                                    writer_clients.lock().await.remove(&client_id);
                                });

                                let reader_clients = clients.clone();
                                tokio::spawn(async move {
                                    let mut buf = vec![0u8; 4096];
                                    loop {
                                        let read = match read_half.read(&mut buf).await {
                                            Ok(0) => break,
                                            Ok(n) => n,
                                            Err(_) => break,
                                        };
                                        let chunk = buf[..read].to_vec();
                                        let mut guard = reader_clients.lock().await;
                                        let mut dead_clients = Vec::new();
                                        for (peer_id, sender) in guard.iter() {
                                            if *peer_id == client_id {
                                                continue;
                                            }
                                            if sender.send(chunk.clone()).is_err() {
                                                dead_clients.push(*peer_id);
                                            }
                                        }
                                        for peer_id in dead_clients {
                                            guard.remove(&peer_id);
                                        }
                                    }
                                    reader_clients.lock().await.remove(&client_id);
                                });
                            }
                        }
                    }
                })
            };

            Self {
                addr,
                shutdown,
                task,
            }
        }

        fn address(&self) -> String {
            self.addr.to_string()
        }

        async fn shutdown(self) {
            self.shutdown.notify_waiters();
            let _ = self.task.await;
        }
    }

    fn test_lock() -> &'static AsyncMutex<()> {
        TEST_LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "reticulum_mobile_e2e_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    fn prepare_storage_dir(name: &str) -> PathBuf {
        let dir = unique_test_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create storage dir");
        dir
    }

    fn build_config(name: &str, storage_dir: &Path, relay_addr: &str) -> NodeConfig {
        NodeConfig {
            name: name.to_string(),
            storage_dir: Some(storage_dir.to_string_lossy().to_string()),
            tcp_clients: vec![relay_addr.to_string()],
            broadcast: true,
            announce_interval_seconds: 1,
            announce_capabilities: "e2e-test".to_string(),
            hub_mode: HubMode::Disabled {},
            hub_identity_hash: None,
            hub_api_base_url: None,
            hub_api_key: None,
            hub_refresh_interval_seconds: 0,
        }
    }

    fn wait_for_event<F>(
        subscription: &Arc<EventSubscription>,
        timeout: Duration,
        mut predicate: F,
    ) -> Option<NodeEvent>
    where
        F: FnMut(&NodeEvent) -> bool,
    {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout_ms = remaining
                .as_millis()
                .min(u32::MAX as u128)
                .max(1) as u32;
            if let Some(event) = subscription.next(timeout_ms.min(250)) {
                if predicate(&event) {
                    return Some(event);
                }
            }
        }
    }

    fn msgpack_map(entries: Vec<(&str, MsgPackValue)>) -> MsgPackValue {
        MsgPackValue::Map(
            entries
                .into_iter()
                .map(|(key, value)| (MsgPackValue::from(key), value))
                .collect(),
        )
    }

    fn mission_command_fields(
        command_id: &str,
        correlation_id: &str,
        command_type: &str,
        args: Vec<(&str, MsgPackValue)>,
    ) -> Vec<u8> {
        let fields = msgpack_map(vec![(
            "9",
            MsgPackValue::Array(vec![msgpack_map(vec![
                ("command_id", MsgPackValue::from(command_id)),
                ("correlation_id", MsgPackValue::from(correlation_id)),
                ("command_type", MsgPackValue::from(command_type)),
                ("args", msgpack_map(args)),
            ])]),
        )]);
        rmp_serde::to_vec(&fields).expect("msgpack command fields")
    }

    fn mission_event_fields(
        event_type: &str,
        event_uid: &str,
        payload: Vec<(&str, MsgPackValue)>,
    ) -> Vec<u8> {
        let fields = msgpack_map(vec![(
            "13",
            MsgPackValue::Map(vec![
                (MsgPackValue::from("event_type"), MsgPackValue::from(event_type)),
                (MsgPackValue::from("event_id"), MsgPackValue::from(event_uid)),
                (MsgPackValue::from("payload"), msgpack_map(payload)),
            ]),
        )]);
        rmp_serde::to_vec(&fields).expect("msgpack event fields")
    }

    async fn start_node_pair(test_name: &str) -> (TcpRelayHandle, Node, Node) {
        let relay = TcpRelayHandle::start().await;

        let node_a_storage = prepare_storage_dir(&format!("{test_name}_a"));
        let node_b_storage = prepare_storage_dir(&format!("{test_name}_b"));

        let node_a = Node::new();
        node_a
            .start(build_config(
                &format!("{test_name}-a"),
                node_a_storage.as_path(),
                relay.address().as_str(),
            ))
            .expect("start node a");

        let node_b = Node::new();
        node_b
            .start(build_config(
                &format!("{test_name}-b"),
                node_b_storage.as_path(),
                relay.address().as_str(),
            ))
            .expect("start node b");

        node_a.announce_now().expect("announce node a");
        node_b.announce_now().expect("announce node b");
        tokio::time::sleep(Duration::from_millis(500)).await;

        let node_b_lxmf_destination_hex = node_b.get_status().lxmf_destination_hex;
        node_a
            .request_peer_identity(node_b_lxmf_destination_hex.clone())
            .expect("resolve node b");

        (relay, node_a, node_b)
    }

    async fn stop_node(node: Node) {
        let _ = tokio::task::spawn_blocking(move || node.stop()).await;
    }

    fn assert_packet_received(
        event: NodeEvent,
        expected_source_hex: &str,
        expected_body: &str,
        expected_fields: Option<&[u8]>,
    ) {
        match event {
            NodeEvent::MessageReceived { message } => {
                assert_eq!(message.source_hex.as_deref(), Some(expected_source_hex));
                assert_eq!(message.body_utf8, expected_body);
            }
            NodeEvent::PacketReceived {
                source_hex,
                bytes,
                fields_bytes,
                ..
            } => {
                assert_eq!(source_hex.as_deref(), Some(expected_source_hex));
                assert_eq!(bytes.as_slice(), expected_body.as_bytes());
                match (expected_fields, fields_bytes.as_deref()) {
                    (None, None) => {}
                    (Some(expected), Some(actual)) => {
                        assert_eq!(actual, expected);
                    }
                    (None, Some(_)) => panic!("unexpected mission fields"),
                    (Some(_), None) => panic!("expected mission fields"),
                }
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_chat_message_is_received_by_peer() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("chat").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "chat: hello from node a";
        let subscription = node_b.subscribe_events();
        let message_id = node_a
            .send_lxmf(SendLxmfRequest {
                destination_hex: node_b_status.lxmf_destination_hex.clone(),
                body_utf8: body.to_string(),
                title: Some("chat".to_string()),
                use_propagation_node: false,
            })
            .expect("send chat message");
        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == body)
        })
        .expect("node b received chat message");

        assert_packet_received(event, &node_a_status.lxmf_destination_hex, body, None);
        assert!(!message_id.is_empty());

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_emergency_message_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("emergency").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "emergency: request medevac";
        let fields = mission_command_fields(
            "cmd-eam-123",
            "corr-eam-123",
            "mission.registry.eam.upsert",
            vec![
                ("eam_uid", MsgPackValue::from("eam-123")),
                ("team_member_uid", MsgPackValue::from("member-1")),
                ("team_uid", MsgPackValue::from("team-1")),
                ("mission_uid", MsgPackValue::from("mission-1")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                false,
            )
            .expect("send emergency packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received emergency packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_event_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("event").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "event: checkpoint reached";
        let fields = mission_event_fields(
            "mission.registry.log_entry.upserted",
            "event-123",
            vec![
                ("entry_uid", MsgPackValue::from("event-123")),
                ("mission_uid", MsgPackValue::from("mission-1")),
                ("content", MsgPackValue::from("Checkpoint reached")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                false,
            )
            .expect("send event packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received event packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("event metadata");
        assert_eq!(
            metadata.event_type.as_deref(),
            Some("mission.registry.log_entry.upserted")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("event-123"));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn send_telemetry_is_received_as_mission_packet() {
        let _guard = test_lock().lock().await;
        let (relay, node_a, node_b) = start_node_pair("telemetry").await;

        let node_a_status = node_a.get_status();
        let node_b_status = node_b.get_status();
        let body = "telemetry: position sample";
        let fields = mission_command_fields(
            "cmd-telemetry-123",
            "corr-telemetry-123",
            "mission.registry.telemetry.upsert",
            vec![
                ("event_uid", MsgPackValue::from("telemetry-123")),
                ("team_member_uid", MsgPackValue::from("member-1")),
                ("team_uid", MsgPackValue::from("team-1")),
                ("mission_uid", MsgPackValue::from("mission-1")),
            ],
        );
        let subscription = node_b.subscribe_events();
        node_a
            .send_bytes(
                node_b_status.lxmf_destination_hex.clone(),
                body.as_bytes().to_vec(),
                Some(fields.clone()),
                false,
            )
            .expect("send telemetry packet");

        let event = wait_for_event(&subscription, TEST_TIMEOUT, |event| {
            matches!(event, NodeEvent::PacketReceived { bytes, .. } if bytes.as_slice() == body.as_bytes())
        })
        .expect("node b received telemetry packet");

        assert_packet_received(
            event,
            &node_a_status.lxmf_destination_hex,
            body,
            Some(fields.as_slice()),
        );
        let metadata = parse_mission_sync_metadata(fields.as_slice()).expect("telemetry metadata");
        assert_eq!(
            metadata.command_type.as_deref(),
            Some("mission.registry.telemetry.upsert")
        );
        assert_eq!(metadata.event_uid.as_deref(), Some("telemetry-123"));

        stop_node(node_a).await;
        stop_node(node_b).await;
        relay.shutdown().await;
    }
}
