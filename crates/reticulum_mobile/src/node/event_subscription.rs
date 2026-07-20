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
            .recv_timeout(Duration::from_millis(u64::from(timeout_ms)))
            .ok()
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}
