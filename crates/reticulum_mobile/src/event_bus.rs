use std::sync::{Arc, Mutex};

use crossbeam_channel as cb;

use crate::types::NodeEvent;

#[derive(Clone)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<cb::Sender<NodeEvent>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn subscribe(&self) -> cb::Receiver<NodeEvent> {
        let (tx, rx) = cb::unbounded();
        if let Ok(mut guard) = self.subscribers.lock() {
            guard.push(tx);
        }
        rx
    }

    pub fn emit(&self, event: NodeEvent) {
        let Ok(mut guard) = self.subscribers.lock() else {
            return;
        };
        guard.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

