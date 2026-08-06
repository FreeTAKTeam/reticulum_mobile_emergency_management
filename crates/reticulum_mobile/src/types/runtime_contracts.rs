#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NodeError {
    #[error("invalid config")]
    InvalidConfig {},
    #[error("io error")]
    IoError {},
    #[error("network error")]
    NetworkError {},
    #[error("reticulum error")]
    ReticulumError {},
    #[error("already running")]
    AlreadyRunning {},
    #[error("not running")]
    NotRunning {},
    #[error("timeout")]
    Timeout {},
    #[error("lxmf wire encode failed")]
    LxmfWireEncodeError {},
    #[error("lxmf message id parse failed")]
    LxmfMessageIdParseError {},
    #[error("lxmf packet too large")]
    LxmfPacketTooLarge {},
    #[error("lxmf packet build failed")]
    LxmfPacketBuildError {},
    #[error("event stream closed")]
    EventStreamClosed {},
    #[error("internal error")]
    InternalError {},
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeConfig {
    pub name: String,
    pub storage_dir: Option<String>,
    pub tcp_clients: Vec<String>,
    pub broadcast: bool,
    pub transport_node_enabled: bool,
    pub announce_interval_seconds: u32,
    pub stale_after_minutes: u32,
    pub announce_capabilities: String,
    pub hub_mode: HubMode,
    pub hub_identity_hash: Option<String>,
    pub hub_api_base_url: Option<String>,
    pub hub_api_key: Option<String>,
    pub hub_refresh_interval_seconds: u32,
    pub rnode: RnodeSettingsRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RnodeSettingsRecord {
    pub enabled: bool,
    #[serde(default = "default_rnode_connection_mode")]
    pub connection_mode: String,
    pub peripheral_id: String,
    pub display_name: String,
    pub region: String,
    pub profile: String,
    #[serde(default)]
    pub frequency_hz: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RnodeConnectionMode {
    #[default]
    Ble,
    BluetoothClassic,
    Usb,
    Tcp,
}

impl RnodeConnectionMode {
    pub fn parse(value: Option<&str>) -> Result<Self, NodeError> {
        let normalized = value.unwrap_or_default().trim();
        if normalized.is_empty() {
            return Ok(Self::Ble);
        }
        match normalized
            .trim()
            .to_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "bluetooth_classic" | "bluetoothclassic" | "classic" | "spp" | "rfcomm"
            | "bluetooth" => Ok(Self::BluetoothClassic),
            "usb" | "serial" => Ok(Self::Usb),
            "tcp" | "wifi" | "wi_fi" => Ok(Self::Tcp),
            "ble" | "bluetooth_le" | "le" | "gatt" => Ok(Self::Ble),
            _ => Err(NodeError::InvalidConfig {}),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ble => "ble",
            Self::BluetoothClassic => "bluetooth_classic",
            Self::Usb => "usb",
            Self::Tcp => "tcp",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeReadinessState {
    #[default]
    Pending,
    Ready,
    Failed,
    Unsupported,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInterfaceReadinessRecord {
    pub id: String,
    pub label: String,
    pub state: RuntimeReadinessState,
    pub detail: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeReadinessSnapshot {
    pub state: RuntimeReadinessState,
    pub interfaces: Vec<RuntimeInterfaceReadinessRecord>,
}

impl RuntimeReadinessSnapshot {
    pub fn for_config(config: &NodeConfig) -> Result<Self, NodeError> {
        let rnode_mode = RnodeConnectionMode::parse(Some(&config.rnode.connection_mode))?;
        let rnode = if !config.rnode.enabled {
            readiness_record(
                "rnode",
                "LoRa",
                RuntimeReadinessState::Disabled,
                "RNode disabled",
            )
        } else {
            match rnode_mode {
                RnodeConnectionMode::Ble => readiness_record(
                    "rnode",
                    "LoRa",
                    RuntimeReadinessState::Pending,
                    "Waiting for the RNode BLE interface",
                ),
                RnodeConnectionMode::BluetoothClassic => readiness_record(
                    "rnode",
                    "LoRa",
                    RuntimeReadinessState::Unsupported,
                    "RNode Bluetooth Classic/SPP is not supported by the REM runtime",
                ),
                RnodeConnectionMode::Usb => readiness_record(
                    "rnode",
                    "LoRa",
                    RuntimeReadinessState::Unsupported,
                    "RNode USB serial is not supported by the REM runtime",
                ),
                RnodeConnectionMode::Tcp => readiness_record(
                    "rnode",
                    "LoRa",
                    RuntimeReadinessState::Unsupported,
                    "RNode TCP mode is not supported by the REM runtime",
                ),
            }
        };
        let tcp = if config
            .tcp_clients
            .iter()
            .any(|value| !value.trim().is_empty())
        {
            readiness_record(
                "tcp",
                "TCP community",
                RuntimeReadinessState::Pending,
                "Waiting for a configured TCP interface",
            )
        } else {
            readiness_record(
                "tcp",
                "TCP community",
                RuntimeReadinessState::Disabled,
                "No TCP interface configured",
            )
        };
        let local = readiness_record(
            "local",
            "Reticulum Net",
            RuntimeReadinessState::Pending,
            "Runtime is starting",
        );
        let mut snapshot = Self {
            state: RuntimeReadinessState::Pending,
            interfaces: vec![rnode, tcp, local],
        };
        snapshot.refresh(false, &[]);
        Ok(snapshot)
    }

    pub fn refresh(&mut self, running: bool, interfaces: &[InterfaceStatusRecord]) {
        for readiness in &mut self.interfaces {
            if matches!(
                readiness.state,
                RuntimeReadinessState::Disabled | RuntimeReadinessState::Unsupported
            ) {
                continue;
            }
            if readiness.id == "local" {
                readiness.state = if running {
                    RuntimeReadinessState::Ready
                } else {
                    RuntimeReadinessState::Pending
                };
                readiness.detail = if running {
                    "Runtime is ready".to_string()
                } else {
                    "Runtime is starting".to_string()
                };
                readiness.last_error = None;
                continue;
            }
            let matching = interfaces
                .iter()
                .filter(|record| interface_matches_readiness(readiness.id.as_str(), record))
                .collect::<Vec<_>>();
            if matching
                .iter()
                .any(|record| record.state.eq_ignore_ascii_case("connected"))
            {
                readiness.state = RuntimeReadinessState::Ready;
                readiness.detail = "Interface connected".to_string();
                readiness.last_error = None;
            } else if let Some(error) = matching.iter().find_map(|record| record.last_error.clone())
            {
                readiness.state = RuntimeReadinessState::Failed;
                readiness.detail = "Interface failed".to_string();
                readiness.last_error = Some(error);
            } else {
                readiness.state = RuntimeReadinessState::Pending;
                readiness.detail = "Waiting for interface startup".to_string();
                readiness.last_error = None;
            }
        }
        self.recompute_state(running);
    }

    pub fn set_interface_state(
        &mut self,
        id: &str,
        state: RuntimeReadinessState,
        detail: String,
        last_error: Option<String>,
        running: bool,
    ) {
        if let Some(record) = self.interfaces.iter_mut().find(|record| record.id == id) {
            record.state = state;
            record.detail = detail;
            record.last_error = last_error;
        }
        self.recompute_state(running);
    }

    fn recompute_state(&mut self, running: bool) {
        let local_state = self
            .interfaces
            .iter()
            .find(|record| record.id == "local")
            .map(|record| &record.state);
        self.state = match local_state {
            Some(RuntimeReadinessState::Ready) if running => RuntimeReadinessState::Ready,
            Some(RuntimeReadinessState::Failed) => RuntimeReadinessState::Failed,
            Some(RuntimeReadinessState::Unsupported) => RuntimeReadinessState::Unsupported,
            _ => RuntimeReadinessState::Pending,
        };
    }
}

fn readiness_record(
    id: &str,
    label: &str,
    state: RuntimeReadinessState,
    detail: &str,
) -> RuntimeInterfaceReadinessRecord {
    RuntimeInterfaceReadinessRecord {
        id: id.to_string(),
        label: label.to_string(),
        state,
        detail: detail.to_string(),
        last_error: None,
    }
}

fn interface_matches_readiness(id: &str, record: &InterfaceStatusRecord) -> bool {
    let kind = record.kind.trim().to_ascii_lowercase();
    match id {
        "rnode" => kind == "rnode" || kind.starts_with("rnode_"),
        "tcp" => kind == "tcp" || kind == "tcp_client",
        _ => false,
    }
}

fn default_rnode_connection_mode() -> String {
    RnodeConnectionMode::Ble.as_str().to_string()
}

impl Default for RnodeSettingsRecord {
    fn default() -> Self {
        Self {
            enabled: false,
            connection_mode: default_rnode_connection_mode(),
            peripheral_id: String::new(),
            display_name: String::new(),
            region: "US915".to_string(),
            profile: "REM-LF-RURAL-v1".to_string(),
            frequency_hz: 915_000_000,
        }
    }
}
