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
use crate::types::{LogLevel, NodeConfig, NodeError, NodeEvent, NodeStatus};

const APP_DESTINATION_NAME: (&str, &str) = ("r3akt", "emergency");
const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");

struct NodeInner {
    bus: EventBus,
    status: Arc<Mutex<NodeStatus>>,
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
            inner
                .bus
                .emit(NodeEvent::StatusChanged { status: guard.clone() });
        }

        let runtime = Runtime::new().map_err(|_| NodeError::InternalError {})?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        runtime.spawn(run_node(
            config,
            identity,
            inner.status.clone(),
            inner.bus.clone(),
            cmd_rx,
        ));

        inner.runtime = Some(runtime);
        inner.cmd_tx = Some(cmd_tx);

        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        let (runtime, cmd_tx, bus, status) = {
            let mut inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            (
                inner.runtime.take(),
                inner.cmd_tx.take(),
                inner.bus.clone(),
                inner.status.clone(),
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
            bus.emit(NodeEvent::StatusChanged { status: guard.clone() });
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

        inner.status.lock().map(|v| v.clone()).unwrap_or(NodeStatus {
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
            .recv_timeout(Duration::from_secs(10))
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

    pub fn send_bytes(&self, destination_hex: String, bytes: Vec<u8>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::SendBytes {
            destination_hex,
            bytes,
            resp: resp_tx,
        })
        .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
    }

    pub fn broadcast_bytes(&self, bytes: Vec<u8>) -> Result<(), NodeError> {
        let tx = {
            let inner = self.inner.lock().map_err(|_| NodeError::InternalError {})?;
            inner.cmd_tx.clone().ok_or(NodeError::NotRunning {})?
        };

        let (resp_tx, resp_rx) = cb::bounded(1);
        tx.send(Command::BroadcastBytes { bytes, resp: resp_tx })
            .map_err(|_| NodeError::NotRunning {})?;
        resp_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or(Err(NodeError::Timeout {}))
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
