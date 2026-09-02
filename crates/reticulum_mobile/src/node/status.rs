impl Node {
    pub fn get_status(&self) -> NodeStatus {
        let inner = self.inner.lock().ok();
        let Some(inner) = inner else {
            return NodeStatus {
                running: false,
                name: String::new(),
                identity_hex: String::new(),
                app_destination_hex: String::new(),
                lxmf_destination_hex: String::new(),
                readiness: RuntimeReadinessSnapshot::default(),
                interfaces: Vec::new(),
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
                readiness: RuntimeReadinessSnapshot::default(),
                interfaces: Vec::new(),
            })
    }
}
