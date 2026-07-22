#[derive(Debug, Clone)]
struct PendingLxmfResend {
    requested_destination_hex: String,
    body: Vec<u8>,
    title: Option<String>,
    fields_bytes: Option<Vec<u8>>,
    metadata: MissionSyncMetadata,
    send_task_class: SendTaskClass,
    original_send_mode: SendMode,
    direct_ack_retry_attempted: bool,
    propagation_fallback_attempted: bool,
}

#[derive(Debug, Clone)]
struct PendingLxmfDelivery {
    message_id_hex: String,
    destination_hex: String,
    correlation_id: Option<String>,
    command_id: Option<String>,
    command_type: Option<String>,
    event_uid: Option<String>,
    mission_uid: Option<String>,
    method: LxmfDeliveryMethod,
    representation: LxmfDeliveryRepresentation,
    relay_destination_hex: Option<String>,
    fallback_stage: Option<LxmfFallbackStage>,
    resend: Option<PendingLxmfResend>,
    sent_at_ms: u64,
}

#[derive(Debug, Clone)]
struct PendingLxmfAcknowledgement {
    source_hex: String,
    detail: Option<String>,
    application_ack_state: ApplicationAckState,
    buffered_at_ms: u64,
}

#[derive(Debug, Clone)]
struct RegisteredPendingLxmfDelivery {
    pending: PendingLxmfDelivery,
    buffered_ack: Option<PendingLxmfAcknowledgement>,
}

#[derive(Debug, Clone)]
pub(crate) struct LxmfSendReport {
    pub(crate) outcome: RnsSendOutcome,
    pub(crate) message_id_hex: String,
    pub(crate) resolved_destination_hex: String,
    pub(crate) metadata: Option<MissionSyncMetadata>,
    pub(crate) track_delivery_timeout: bool,
    pub(crate) used_propagation_node: bool,
    pub(crate) method: LxmfDeliveryMethod,
    pub(crate) representation: LxmfDeliveryRepresentation,
    pub(crate) relay_destination_hex: Option<String>,
    pub(crate) fallback_stage: Option<LxmfFallbackStage>,
    pub(crate) receipt_hash_hex: Option<String>,
}

struct RuntimeReceiptBridge {
    tracker: ReceiptTracker,
}

#[derive(Clone)]
struct ReceiptTracker {
    receipt_message_ids: Arc<Mutex<HashMap<String, ReceiptMessageTracking>>>,
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Debug, Clone)]
enum ReceiptMessageTracking {
    Pending {
        message_id_hex: String,
        recorded_at_ms: u64,
    },
    Observed {
        recorded_at_ms: u64,
    },
}

impl ReceiptMessageTracking {
    fn recorded_at_ms(&self) -> u64 {
        match self {
            Self::Pending { recorded_at_ms, .. } | Self::Observed { recorded_at_ms } => {
                *recorded_at_ms
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendTaskClass {
    Mission,
    MissionAck,
    MissionPropagation,
    MissionRecovery,
    General,
}

impl SendTaskClass {
    fn from_lxmf_request(
        has_fields: bool,
        metadata: Option<&MissionSyncMetadata>,
        send_mode: &SendMode,
    ) -> Self {
        if !has_fields {
            return Self::General;
        }
        if !metadata.is_some_and(MissionSyncMetadata::is_mission_related) {
            return Self::General;
        }
        if is_accepted_result_metadata(metadata) {
            return Self::MissionAck;
        }
        if is_sos_status_metadata(metadata)
            && !matches!(send_mode, SendMode::PropagationOnly {})
        {
            return Self::MissionRecovery;
        }
        if matches!(send_mode, SendMode::PropagationOnly {}) {
            Self::MissionPropagation
        } else {
            Self::Mission
        }
    }

    fn propagation_equivalent(self) -> Self {
        match self {
            Self::Mission | Self::MissionAck | Self::MissionPropagation => Self::MissionPropagation,
            Self::MissionRecovery => Self::MissionRecovery,
            Self::General => Self::General,
        }
    }

    fn direct_recovery_equivalent(self) -> Self {
        match self {
            Self::Mission | Self::MissionAck | Self::MissionPropagation | Self::MissionRecovery => {
                Self::MissionRecovery
            }
            Self::General => Self::General,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mission => "mission-direct",
            Self::MissionAck => "mission-ack",
            Self::MissionPropagation => "mission-propagation",
            Self::MissionRecovery => "mission-recovery",
            Self::General => "general",
        }
    }
}

fn should_emit_global_send_bytes_error(send_task_class: SendTaskClass) -> bool {
    matches!(send_task_class, SendTaskClass::General)
}

#[derive(Clone)]
struct SendTaskPermits {
    general: Arc<Semaphore>,
    mission: Arc<Semaphore>,
    mission_ack: Arc<Semaphore>,
    mission_propagation: Arc<Semaphore>,
    mission_recovery: Arc<Semaphore>,
}

impl SendTaskPermits {
    fn new() -> Self {
        Self {
            general: Arc::new(Semaphore::new(GENERAL_SEND_TASK_CONCURRENCY_LIMIT)),
            mission: Arc::new(Semaphore::new(MISSION_SEND_TASK_RESERVED_LIMIT)),
            mission_ack: Arc::new(Semaphore::new(MISSION_ACK_SEND_TASK_RESERVED_LIMIT)),
            mission_propagation: Arc::new(Semaphore::new(
                MISSION_PROPAGATION_SEND_TASK_RESERVED_LIMIT,
            )),
            mission_recovery: Arc::new(Semaphore::new(MISSION_RECOVERY_SEND_TASK_RESERVED_LIMIT)),
        }
    }

    #[cfg(test)]
    fn with_limits(general: usize, mission: usize) -> Self {
        Self {
            general: Arc::new(Semaphore::new(general)),
            mission: Arc::new(Semaphore::new(mission)),
            mission_ack: Arc::new(Semaphore::new(1)),
            mission_propagation: Arc::new(Semaphore::new(mission)),
            mission_recovery: Arc::new(Semaphore::new(1)),
        }
    }

    async fn acquire(&self, class: SendTaskClass) -> Result<OwnedSemaphorePermit, NodeError> {
        match class {
            SendTaskClass::Mission => self
                .mission
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error)),
            SendTaskClass::MissionAck => self
                .mission_ack
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error)),
            SendTaskClass::MissionPropagation => self
                .mission_propagation
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error)),
            SendTaskClass::MissionRecovery => self
                .mission_recovery
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error)),
            SendTaskClass::General => self
                .general
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error)),
        }
    }
}

#[derive(Clone, Default)]
struct DirectDeliveryHealth {
    cooldown_until_ms: Arc<Mutex<HashMap<String, u64>>>,
}

impl DirectDeliveryHealth {
    fn mark_unhealthy<'a, I>(&self, destinations: I, until_ms: u64)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return;
        };
        for destination in destinations {
            if let Some(normalized) = normalize_hex_32(destination) {
                guard.insert(normalized, until_ms);
            }
        }
    }

    fn clear<'a, I>(&self, destinations: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return;
        };
        for destination in destinations {
            if let Some(normalized) = normalize_hex_32(destination) {
                guard.remove(normalized.as_str());
            }
        }
    }

    fn is_available(&self, destination: &str, now_ms: u64) -> bool {
        let Some(normalized) = normalize_hex_32(destination) else {
            return true;
        };
        let Ok(mut guard) = self.cooldown_until_ms.lock() else {
            return true;
        };
        match guard.get(normalized.as_str()).copied() {
            Some(until_ms) if until_ms > now_ms => false,
            Some(_) => {
                guard.remove(normalized.as_str());
                true
            }
            None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManagedPeerLinkKind {
    App,
    LxmfDelivery,
}

impl ManagedPeerLinkKind {
    fn destination_name(self) -> DestinationName {
        match self {
            Self::App => DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
            Self::LxmfDelivery => DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManagedPeerLinkTarget {
    destination_hex: String,
    kind: ManagedPeerLinkKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ManagedPeerLinkBackoff {
    attempts: u32,
    next_retry_at_ms: u64,
    last_failure_reason: Option<String>,
}

impl ManagedPeerLinkBackoff {
    fn next_delay_ms(&self) -> u64 {
        let exponent = self
            .attempts
            .saturating_sub(1)
            .min(SAVED_PEER_LINK_BACKOFF_MAX_ATTEMPTS);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        SAVED_PEER_LINK_BACKOFF_BASE_MS
            .saturating_mul(multiplier)
            .min(SAVED_PEER_LINK_BACKOFF_MAX_MS)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ManagedPeerReconnectStart {
    Started(ManagedPeerLinkTarget),
    Backoff {
        next_retry_at_ms: u64,
        last_failure_reason: Option<String>,
    },
    AlreadyReconnecting,
    NotDesired,
}

#[derive(Clone, Default)]
struct ManagedPeerLinks {
    desired: Arc<TokioMutex<HashMap<String, ManagedPeerLinkTarget>>>,
    reconnecting: Arc<TokioMutex<HashMap<String, ManagedPeerLinkKind>>>,
    failures: Arc<TokioMutex<HashMap<String, ManagedPeerLinkBackoff>>>,
}

impl ManagedPeerLinks {
    async fn add_desired(&self, target: ManagedPeerLinkTarget) {
        self.desired
            .lock()
            .await
            .insert(target.destination_hex.clone(), target);
    }

    async fn remove_desired<'a, I>(&self, destinations: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let normalized = destinations
            .into_iter()
            .filter_map(normalize_hex_32)
            .collect::<Vec<_>>();
        if normalized.is_empty() {
            return;
        }
        {
            let mut desired = self.desired.lock().await;
            for destination in &normalized {
                desired.remove(destination.as_str());
            }
        }
        let mut reconnecting = self.reconnecting.lock().await;
        for destination in normalized {
            reconnecting.remove(destination.as_str());
            self.failures.lock().await.remove(destination.as_str());
        }
    }

    async fn desired_targets(&self) -> Vec<ManagedPeerLinkTarget> {
        let now = now_ms();
        let desired = self.desired.lock().await;
        let failures = self.failures.lock().await;
        desired
            .values()
            .filter(|target| {
                failures
                    .get(target.destination_hex.as_str())
                    .is_none_or(|failure| failure.next_retry_at_ms <= now)
            })
            .cloned()
            .collect()
    }

    async fn clear_failure(&self, destination_hex: &str) {
        if let Some(normalized) = normalize_hex_32(destination_hex) {
            self.failures.lock().await.remove(normalized.as_str());
        }
    }

    async fn begin_reconnect(&self, destination_hex: &str) -> ManagedPeerReconnectStart {
        let Some(normalized) = normalize_hex_32(destination_hex) else {
            return ManagedPeerReconnectStart::NotDesired;
        };
        let now = now_ms();
        let Some(target) = self.desired.lock().await.get(normalized.as_str()).cloned() else {
            return ManagedPeerReconnectStart::NotDesired;
        };
        if let Some(failure) = self.failures.lock().await.get(normalized.as_str()) {
            if failure.next_retry_at_ms > now {
                return ManagedPeerReconnectStart::Backoff {
                    next_retry_at_ms: failure.next_retry_at_ms,
                    last_failure_reason: failure.last_failure_reason.clone(),
                };
            }
        }
        let mut reconnecting = self.reconnecting.lock().await;
        if let Some(active_kind) = reconnecting.get(normalized.as_str()) {
            if *active_kind == target.kind {
                return ManagedPeerReconnectStart::AlreadyReconnecting;
            }
            if !matches!(
                (*active_kind, target.kind),
                (ManagedPeerLinkKind::App, ManagedPeerLinkKind::LxmfDelivery)
            ) {
                return ManagedPeerReconnectStart::AlreadyReconnecting;
            }
        }
        reconnecting.insert(normalized.clone(), target.kind);
        ManagedPeerReconnectStart::Started(target)
    }

    async fn finish_reconnect(&self, target: &ManagedPeerLinkTarget, result: Result<(), String>) {
        if let Some(normalized) = normalize_hex_32(target.destination_hex.as_str()) {
            let obsolete_reconnect = {
                let mut reconnecting = self.reconnecting.lock().await;
                match reconnecting.get(normalized.as_str()).copied() {
                    Some(kind) if kind == target.kind => {
                        reconnecting.remove(normalized.as_str());
                        false
                    }
                    Some(_) => true,
                    None => false,
                }
            };
            if obsolete_reconnect {
                return;
            }
            match result {
                Ok(()) => {
                    self.failures.lock().await.remove(normalized.as_str());
                }
                Err(reason) => {
                    let mut failures = self.failures.lock().await;
                    let failure = failures.entry(normalized).or_default();
                    failure.attempts = failure
                        .attempts
                        .saturating_add(1)
                        .min(SAVED_PEER_LINK_BACKOFF_MAX_ATTEMPTS);
                    failure.last_failure_reason = Some(reason);
                    failure.next_retry_at_ms = now_ms().saturating_add(failure.next_delay_ms());
                }
            }
        }
    }
}

#[derive(Clone)]
struct MissionDestinationLocks {
    locks: Arc<TokioMutex<HashMap<String, Arc<TokioMutex<()>>>>>,
}

impl MissionDestinationLocks {
    fn new() -> Self {
        Self {
            locks: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    async fn acquire(&self, destination_hex: &str) -> Result<OwnedMutexGuard<()>, NodeError> {
        let key = normalize_hex_32(destination_hex)
            .unwrap_or_else(|| destination_hex.trim().to_ascii_lowercase());
        if key.is_empty() {
            return Err(NodeError::InvalidConfig {});
        }
        let lock = {
            let mut guard = self.locks.lock().await;
            guard
                .entry(key)
                .or_insert_with(|| Arc::new(TokioMutex::new(())))
                .clone()
        };
        Ok(lock.lock_owned().await)
    }
}
