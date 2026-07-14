#[derive(Debug, Clone)]
pub enum NodeEvent {
    StatusChanged {
        status: NodeStatus,
    },
    InterfaceStatusChanged {
        status: InterfaceStatusRecord,
    },
    AnnounceReceived {
        destination_hex: String,
        identity_hex: String,
        destination_kind: String,
        announce_class: AnnounceClass,
        app_data: String,
        display_name: Option<String>,
        hops: u8,
        interface_hex: String,
        received_at_ms: u64,
    },
    PeerChanged {
        change: PeerChange,
    },
    PacketReceived {
        destination_hex: String,
        source_hex: Option<String>,
        bytes: Vec<u8>,
        fields_bytes: Option<Vec<u8>>,
    },
    PacketSent {
        destination_hex: String,
        bytes: Vec<u8>,
        outcome: SendOutcome,
    },
    LxmfDelivery {
        update: LxmfDeliveryUpdate,
    },
    PeerResolved {
        peer: PeerRecord,
    },
    MessageReceived {
        message: MessageRecord,
    },
    MessageUpdated {
        message: MessageRecord,
    },
    SyncUpdated {
        status: SyncStatus,
    },
    HubDirectoryUpdated {
        snapshot: HubDirectorySnapshot,
    },
    OperationalNotice {
        notice: OperationalNotice,
    },
    ProjectionInvalidated {
        invalidation: ProjectionInvalidation,
    },
    PluginEventPublished {
        event: PluginEventRecord,
    },
    SosStatusChanged {
        status: SosStatusRecord,
    },
    SosAlertChanged {
        alert: SosAlertRecord,
    },
    SosTelemetryRequested {},
    SosAudioRecordingRequested {
        incident_id: String,
        duration_seconds: u32,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}
