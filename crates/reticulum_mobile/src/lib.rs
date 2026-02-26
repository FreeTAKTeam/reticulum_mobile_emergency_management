mod event_bus;
mod jni_bridge;
mod logger;
mod node;
mod runtime;
mod types;

pub use node::{EventSubscription, Node};
pub use types::{
    HubMode, LogLevel, NodeConfig, NodeError, NodeEvent, NodeStatus, PeerChange, PeerState,
    SendOutcome,
};

pub fn healthcheck() -> String {
    "reticulum-mobile-ready".to_string()
}

// Include UniFFI-generated scaffolding (built from `reticulum_mobile.udl`).
uniffi::include_scaffolding!("reticulum_mobile");
