use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use fs_err as fs;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::event_bus::EventBus;
use crate::runtime::now_ms;
use crate::types::{
    MessageDirection, MessageMethod, MessageRecord, MessageState, NodeEvent, PeerRecord, PeerState,
    ProjectionInvalidation, ProjectionScope, SyncPhase, SyncStatus,
};

const PERSIST_FILENAME: &str = "runtime_projection.json";
const INVALIDATION_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPeerRecord {
    destination_hex: String,
    identity_hex: Option<String>,
    lxmf_destination_hex: Option<String>,
    display_name: Option<String>,
    app_data: Option<String>,
    state: String,
    #[serde(default)]
    saved: Option<bool>,
    #[serde(default)]
    management_state: Option<String>,
    stale: bool,
    active_link: bool,
    #[serde(default)]
    hub_derived: bool,
    last_resolution_error: Option<String>,
    last_resolution_attempt_at_ms: Option<u64>,
    last_seen_at_ms: u64,
    announce_last_seen_at_ms: Option<u64>,
    lxmf_last_seen_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMessageRecord {
    message_id_hex: String,
    conversation_id: String,
    direction: String,
    destination_hex: String,
    source_hex: Option<String>,
    title: Option<String>,
    body_utf8: String,
    method: String,
    state: String,
    detail: Option<String>,
    sent_at_ms: Option<u64>,
    received_at_ms: Option<u64>,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSyncStatus {
    phase: String,
    active_propagation_node_hex: Option<String>,
    requested_at_ms: Option<u64>,
    completed_at_ms: Option<u64>,
    messages_received: u32,
    detail: Option<String>,
}

impl Default for PersistedSyncStatus {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            active_propagation_node_hex: None,
            requested_at_ms: None,
            completed_at_ms: None,
            messages_received: 0,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectionRevisionEntry {
    scope: ProjectionScope,
    revision: u64,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct RuntimeProjectionSnapshot {
    revisions: Vec<ProjectionRevisionEntry>,
    peers: Vec<PersistedPeerRecord>,
    messages: Vec<PersistedMessageRecord>,
    sync_status: PersistedSyncStatus,
    updated_at_ms: u64,
}

impl RuntimeProjectionSnapshot {
    pub(crate) fn peers(&self) -> Vec<PeerRecord> {
        self.peers
            .clone()
            .into_iter()
            .map(runtime_peer_from_persisted)
            .collect::<Vec<_>>()
    }

    pub(crate) fn restored_peers(&self) -> Vec<PeerRecord> {
        self.peers()
            .into_iter()
            .filter(|peer| peer.saved)
            .collect::<Vec<_>>()
    }

    pub(crate) fn pruned_for_restore(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.peers =
            persisted_saved_peers(self.restored_peers().as_slice()).unwrap_or_default();
        snapshot
    }

    pub(crate) fn messages(&self) -> Vec<MessageRecord> {
        self.messages
            .clone()
            .into_iter()
            .map(runtime_message_from_persisted)
            .collect::<Vec<_>>()
    }

    pub(crate) fn sync_status(&self) -> SyncStatus {
        runtime_sync_from_persisted(self.sync_status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PersistedPeerRecord, PersistedSyncStatus, ProjectionRevisionEntry,
        RuntimeProjectionJournal, RuntimeProjectionSnapshot,
    };
    use crate::event_bus::EventBus;
    use crate::types::{PeerRecord, PeerState, ProjectionScope};

    fn build_persisted_peer(
        destination_hex: &str,
        saved: Option<bool>,
        management_state: Option<&str>,
    ) -> PersistedPeerRecord {
        PersistedPeerRecord {
            destination_hex: destination_hex.to_string(),
            identity_hex: Some(format!("identity-{destination_hex}")),
            lxmf_destination_hex: Some(format!("lxmf-{destination_hex}")),
            display_name: Some(format!("peer-{destination_hex}")),
            app_data: Some("R3AKT,EMergencyMessages,Telemetry".to_string()),
            state: "connected".to_string(),
            saved,
            management_state: management_state.map(str::to_string),
            stale: false,
            active_link: false,
            hub_derived: false,
            last_resolution_error: None,
            last_resolution_attempt_at_ms: Some(1),
            last_seen_at_ms: 2,
            announce_last_seen_at_ms: Some(2),
            lxmf_last_seen_at_ms: Some(2),
        }
    }

    #[test]
    fn restored_peers_only_keep_saved_entries() {
        let snapshot = RuntimeProjectionSnapshot {
            peers: vec![
                build_persisted_peer("saved-peer", Some(true), None),
                build_persisted_peer("unsaved-peer", Some(false), None),
            ],
            ..RuntimeProjectionSnapshot::default()
        };

        let restored = snapshot.restored_peers();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].destination_hex, "saved-peer");
        assert!(restored[0].saved);
    }

    #[test]
    fn restored_peers_respect_legacy_managed_flag() {
        let snapshot = RuntimeProjectionSnapshot {
            peers: vec![
                build_persisted_peer("legacy-managed", None, Some("managed")),
                build_persisted_peer("legacy-unmanaged", None, Some("unmanaged")),
            ],
            ..RuntimeProjectionSnapshot::default()
        };

        let restored = snapshot.restored_peers();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].destination_hex, "legacy-managed");
        assert!(restored[0].saved);
    }

    #[test]
    fn pruned_for_restore_drops_unsaved_peers_but_keeps_other_projection_data() {
        let snapshot = RuntimeProjectionSnapshot {
            peers: vec![
                build_persisted_peer("saved-peer", Some(true), None),
                build_persisted_peer("unsaved-peer", Some(false), None),
            ],
            revisions: vec![ProjectionRevisionEntry {
                scope: ProjectionScope::Peers {},
                revision: 7,
                updated_at_ms: 123,
            }],
            sync_status: PersistedSyncStatus {
                phase: "idle".to_string(),
                active_propagation_node_hex: Some("relay".to_string()),
                requested_at_ms: Some(456),
                completed_at_ms: Some(789),
                messages_received: 0,
                detail: Some("none".to_string()),
            },
            updated_at_ms: 999,
            ..RuntimeProjectionSnapshot::default()
        };

        let pruned = snapshot.pruned_for_restore();
        let restored = pruned.peers();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].destination_hex, "saved-peer");
        assert_eq!(
            pruned.sync_status().active_propagation_node_hex.as_deref(),
            Some("relay")
        );
        assert_eq!(pruned.updated_at_ms, 999);
    }

    #[test]
    fn record_peers_persists_saved_entries_only() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let journal = RuntimeProjectionJournal::new(None, EventBus::new());
            let changed = journal.record_peers(
                vec![
                    PeerRecord {
                        destination_hex: "saved-peer".to_string(),
                        identity_hex: Some("identity-a".to_string()),
                        lxmf_destination_hex: Some("lxmf-a".to_string()),
                        display_name: Some("Saved".to_string()),
                        app_data: Some("R3AKT,EMergencyMessages".to_string()),
                        state: PeerState::Connected {},
                        saved: true,
                        stale: false,
                        active_link: true,
                        hub_derived: false,
                        last_resolution_error: None,
                        last_resolution_attempt_at_ms: Some(10),
                        last_seen_at_ms: 20,
                        announce_last_seen_at_ms: Some(20),
                        lxmf_last_seen_at_ms: Some(20),
                    },
                    PeerRecord {
                        destination_hex: "unsaved-peer".to_string(),
                        identity_hex: Some("identity-b".to_string()),
                        lxmf_destination_hex: Some("lxmf-b".to_string()),
                        display_name: Some("Unsaved".to_string()),
                        app_data: Some("R3AKT,EMergencyMessages".to_string()),
                        state: PeerState::Disconnected {},
                        saved: false,
                        stale: false,
                        active_link: false,
                        hub_derived: false,
                        last_resolution_error: None,
                        last_resolution_attempt_at_ms: Some(30),
                        last_seen_at_ms: 40,
                        announce_last_seen_at_ms: Some(40),
                        lxmf_last_seen_at_ms: Some(40),
                    },
                ],
                Some("test"),
            );

            assert!(changed);
            let current = journal.current_peers().unwrap_or_default();
            assert_eq!(current.len(), 1);
            assert_eq!(current[0].destination_hex, "saved-peer");
            assert!(current[0].saved);
        });
    }
}

struct PendingInvalidation {
    invalidation: ProjectionInvalidation,
}

fn persisted_peer_from_runtime(record: &PeerRecord) -> Option<PersistedPeerRecord> {
    Some(PersistedPeerRecord {
        destination_hex: record.destination_hex.clone(),
        identity_hex: record.identity_hex.clone(),
        lxmf_destination_hex: record.lxmf_destination_hex.clone(),
        display_name: record.display_name.clone(),
        app_data: record.app_data.clone(),
        state: peer_state_to_string(record.state),
        saved: Some(record.saved),
        management_state: None,
        stale: record.stale,
        active_link: record.active_link,
        hub_derived: record.hub_derived,
        last_resolution_error: record.last_resolution_error.clone(),
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    })
}

fn persisted_saved_peers(records: &[PeerRecord]) -> Option<Vec<PersistedPeerRecord>> {
    records
        .iter()
        .filter(|record| record.saved)
        .map(persisted_peer_from_runtime)
        .collect::<Option<Vec<_>>>()
}

fn persisted_message_from_runtime(record: &MessageRecord) -> Option<PersistedMessageRecord> {
    Some(PersistedMessageRecord {
        message_id_hex: record.message_id_hex.clone(),
        conversation_id: record.conversation_id.clone(),
        direction: message_direction_to_string(record.direction),
        destination_hex: record.destination_hex.clone(),
        source_hex: record.source_hex.clone(),
        title: record.title.clone(),
        body_utf8: record.body_utf8.clone(),
        method: message_method_to_string(record.method),
        state: message_state_to_string(record.state),
        detail: record.detail.clone(),
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    })
}

fn persisted_sync_from_runtime(status: &SyncStatus) -> Option<PersistedSyncStatus> {
    Some(PersistedSyncStatus {
        phase: sync_phase_to_string(status.phase),
        active_propagation_node_hex: status.active_propagation_node_hex.clone(),
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail.clone(),
    })
}

fn runtime_peer_from_persisted(record: PersistedPeerRecord) -> PeerRecord {
    PeerRecord {
        destination_hex: record.destination_hex,
        identity_hex: record.identity_hex,
        lxmf_destination_hex: record.lxmf_destination_hex,
        display_name: record.display_name,
        app_data: record.app_data,
        state: peer_state_from_string(record.state).unwrap_or(PeerState::Disconnected {}),
        saved: record.saved.unwrap_or_else(|| {
            record
                .management_state
                .as_deref()
                .is_some_and(|value| value == "managed")
        }),
        stale: record.stale,
        active_link: record.active_link,
        hub_derived: record.hub_derived,
        last_resolution_error: record.last_resolution_error,
        last_resolution_attempt_at_ms: record.last_resolution_attempt_at_ms,
        last_seen_at_ms: record.last_seen_at_ms,
        announce_last_seen_at_ms: record.announce_last_seen_at_ms,
        lxmf_last_seen_at_ms: record.lxmf_last_seen_at_ms,
    }
}

fn runtime_message_from_persisted(record: PersistedMessageRecord) -> MessageRecord {
    MessageRecord {
        message_id_hex: record.message_id_hex,
        conversation_id: record.conversation_id,
        direction: message_direction_from_string(record.direction)
            .unwrap_or(MessageDirection::Outbound {}),
        destination_hex: record.destination_hex,
        source_hex: record.source_hex,
        title: record.title,
        body_utf8: record.body_utf8,
        method: message_method_from_string(record.method).unwrap_or(MessageMethod::Direct {}),
        state: message_state_from_string(record.state).unwrap_or(MessageState::Queued {}),
        detail: record.detail,
        sent_at_ms: record.sent_at_ms,
        received_at_ms: record.received_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn runtime_sync_from_persisted(status: PersistedSyncStatus) -> SyncStatus {
    SyncStatus {
        phase: sync_phase_from_string(status.phase).unwrap_or(SyncPhase::Idle {}),
        active_propagation_node_hex: status.active_propagation_node_hex,
        requested_at_ms: status.requested_at_ms,
        completed_at_ms: status.completed_at_ms,
        messages_received: status.messages_received,
        detail: status.detail,
    }
}

fn peer_state_to_string(state: PeerState) -> String {
    match state {
        PeerState::Connecting {} => "connecting".to_string(),
        PeerState::Connected {} => "connected".to_string(),
        PeerState::Disconnected {} => "disconnected".to_string(),
    }
}

fn peer_state_from_string(value: String) -> Option<PeerState> {
    match value.as_str() {
        "connecting" => Some(PeerState::Connecting {}),
        "connected" => Some(PeerState::Connected {}),
        "disconnected" => Some(PeerState::Disconnected {}),
        _ => None,
    }
}

fn message_direction_to_string(direction: MessageDirection) -> String {
    match direction {
        MessageDirection::Inbound {} => "inbound".to_string(),
        MessageDirection::Outbound {} => "outbound".to_string(),
    }
}

fn message_direction_from_string(value: String) -> Option<MessageDirection> {
    match value.as_str() {
        "inbound" => Some(MessageDirection::Inbound {}),
        "outbound" => Some(MessageDirection::Outbound {}),
        _ => None,
    }
}

fn message_method_to_string(method: MessageMethod) -> String {
    match method {
        MessageMethod::Direct {} => "direct".to_string(),
        MessageMethod::Opportunistic {} => "opportunistic".to_string(),
        MessageMethod::Propagated {} => "propagated".to_string(),
        MessageMethod::Resource {} => "resource".to_string(),
    }
}

fn message_method_from_string(value: String) -> Option<MessageMethod> {
    match value.as_str() {
        "direct" => Some(MessageMethod::Direct {}),
        "opportunistic" => Some(MessageMethod::Opportunistic {}),
        "propagated" => Some(MessageMethod::Propagated {}),
        "resource" => Some(MessageMethod::Resource {}),
        _ => None,
    }
}

fn message_state_to_string(state: MessageState) -> String {
    match state {
        MessageState::Queued {} => "queued".to_string(),
        MessageState::PathRequested {} => "path-requested".to_string(),
        MessageState::LinkEstablishing {} => "link-establishing".to_string(),
        MessageState::Sending {} => "sending".to_string(),
        MessageState::SentDirect {} => "sent-direct".to_string(),
        MessageState::SentToPropagation {} => "sent-to-propagation".to_string(),
        MessageState::Delivered {} => "delivered".to_string(),
        MessageState::Failed {} => "failed".to_string(),
        MessageState::TimedOut {} => "timed-out".to_string(),
        MessageState::Cancelled {} => "cancelled".to_string(),
        MessageState::Received {} => "received".to_string(),
    }
}

fn message_state_from_string(value: String) -> Option<MessageState> {
    match value.as_str() {
        "queued" => Some(MessageState::Queued {}),
        "path-requested" => Some(MessageState::PathRequested {}),
        "link-establishing" => Some(MessageState::LinkEstablishing {}),
        "sending" => Some(MessageState::Sending {}),
        "sent-direct" => Some(MessageState::SentDirect {}),
        "sent-to-propagation" => Some(MessageState::SentToPropagation {}),
        "delivered" => Some(MessageState::Delivered {}),
        "failed" => Some(MessageState::Failed {}),
        "timed-out" => Some(MessageState::TimedOut {}),
        "cancelled" => Some(MessageState::Cancelled {}),
        "received" => Some(MessageState::Received {}),
        _ => None,
    }
}

fn sync_phase_to_string(phase: SyncPhase) -> String {
    match phase {
        SyncPhase::Idle {} => "idle".to_string(),
        SyncPhase::PathRequested {} => "path-requested".to_string(),
        SyncPhase::LinkEstablishing {} => "link-establishing".to_string(),
        SyncPhase::RequestSent {} => "request-sent".to_string(),
        SyncPhase::Receiving {} => "receiving".to_string(),
        SyncPhase::Complete {} => "complete".to_string(),
        SyncPhase::Failed {} => "failed".to_string(),
    }
}

fn sync_phase_from_string(value: String) -> Option<SyncPhase> {
    match value.as_str() {
        "idle" => Some(SyncPhase::Idle {}),
        "path-requested" => Some(SyncPhase::PathRequested {}),
        "link-establishing" => Some(SyncPhase::LinkEstablishing {}),
        "request-sent" => Some(SyncPhase::RequestSent {}),
        "receiving" => Some(SyncPhase::Receiving {}),
        "complete" => Some(SyncPhase::Complete {}),
        "failed" => Some(SyncPhase::Failed {}),
        _ => None,
    }
}

fn message_matches(a: &MessageRecord, b: &MessageRecord) -> bool {
    serde_json::to_string(a)
        .ok()
        .zip(serde_json::to_string(b).ok())
        .is_some_and(|(left, right)| left == right)
}

fn peers_match(left: &[PeerRecord], right: &[PersistedPeerRecord]) -> bool {
    let left = serde_json::to_string(left).ok();
    let right = serde_json::to_string(right).ok();
    left.zip(right).is_some_and(|(l, r)| l == r)
}

fn sync_match(left: &SyncStatus, right: &PersistedSyncStatus) -> bool {
    let left = serde_json::to_string(left).ok();
    let right = serde_json::to_string(right).ok();
    left.zip(right).is_some_and(|(l, r)| l == r)
}

#[derive(Clone)]
pub(crate) struct RuntimeProjectionJournal {
    bus: EventBus,
    path: Option<PathBuf>,
    snapshot: Arc<StdMutex<RuntimeProjectionSnapshot>>,
    pending: Arc<StdMutex<Vec<PendingInvalidation>>>,
    flush_scheduled: Arc<AtomicBool>,
}

impl RuntimeProjectionJournal {
    pub(crate) fn new(path: Option<PathBuf>, bus: EventBus) -> Self {
        Self {
            bus,
            path,
            snapshot: Arc::new(StdMutex::new(RuntimeProjectionSnapshot::default())),
            pending: Arc::new(StdMutex::new(Vec::new())),
            flush_scheduled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn load_snapshot(&self) -> Option<RuntimeProjectionSnapshot> {
        let path = self.path.as_ref()?;
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub(crate) fn seed_snapshot(&self, snapshot: RuntimeProjectionSnapshot) {
        if let Ok(mut guard) = self.snapshot.lock() {
            *guard = snapshot;
        }
    }

    pub(crate) fn record_peers(&self, peers: Vec<PeerRecord>, reason: Option<&str>) -> bool {
        let Some(persisted) = persisted_saved_peers(peers.as_slice()) else {
            return false;
        };

        let mut guard = match self.snapshot.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        if peers_match(&peers, &guard.peers) {
            return false;
        }
        guard.peers = persisted;
        guard.updated_at_ms = now_ms();
        drop(guard);

        self.invalidate(
            ProjectionScope::Peers {},
            None,
            reason.unwrap_or("peer-projection-updated"),
        );
        true
    }

    pub(crate) fn record_sync_status(&self, status: SyncStatus, reason: Option<&str>) -> bool {
        let Some(persisted) = persisted_sync_from_runtime(&status) else {
            return false;
        };

        let mut guard = match self.snapshot.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        if sync_match(&status, &guard.sync_status) {
            return false;
        }
        guard.sync_status = persisted;
        guard.updated_at_ms = now_ms();
        drop(guard);

        self.invalidate(
            ProjectionScope::SyncStatus {},
            None,
            reason.unwrap_or("sync-projection-updated"),
        );
        true
    }

    pub(crate) fn record_message(&self, message: MessageRecord, reason: Option<&str>) -> bool {
        let Some(persisted) = persisted_message_from_runtime(&message) else {
            return false;
        };

        let mut guard = match self.snapshot.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let mut changed = false;
        if let Some(existing) = guard
            .messages
            .iter_mut()
            .find(|candidate| candidate.message_id_hex == message.message_id_hex)
        {
            if !message_matches(&runtime_message_from_persisted(existing.clone()), &message) {
                *existing = persisted;
                changed = true;
            }
        } else {
            guard.messages.push(persisted);
            changed = true;
        }
        if !changed {
            return false;
        }
        guard.updated_at_ms = now_ms();
        drop(guard);

        self.invalidate(
            ProjectionScope::Messages {},
            Some(message.message_id_hex),
            reason.unwrap_or("message-projection-updated"),
        );
        true
    }

    pub(crate) fn current_sync_status(&self) -> Option<SyncStatus> {
        self.snapshot
            .lock()
            .ok()
            .map(|snapshot| runtime_sync_from_persisted(snapshot.sync_status.clone()))
    }

    pub(crate) fn current_peers(&self) -> Option<Vec<PeerRecord>> {
        self.snapshot.lock().ok().map(|snapshot| {
            snapshot
                .peers
                .clone()
                .into_iter()
                .map(runtime_peer_from_persisted)
                .collect::<Vec<_>>()
        })
    }

    pub(crate) fn current_messages(&self) -> Option<Vec<MessageRecord>> {
        self.snapshot.lock().ok().map(|snapshot| {
            snapshot
                .messages
                .clone()
                .into_iter()
                .map(runtime_message_from_persisted)
                .collect::<Vec<_>>()
        })
    }

    pub(crate) async fn flush_now(&self) {
        self.flush_once().await;
    }

    fn invalidate(&self, scope: ProjectionScope, key: Option<String>, reason: &str) {
        let revision = {
            let mut snapshot = match self.snapshot.lock() {
                Ok(v) => v,
                Err(_) => return,
            };
            let updated_at_ms = now_ms();
            let next_revision = if let Some(entry) = snapshot
                .revisions
                .iter_mut()
                .find(|entry| entry.scope == scope)
            {
                entry.revision = entry.revision.saturating_add(1);
                entry.updated_at_ms = updated_at_ms;
                entry.revision
            } else {
                snapshot.revisions.push(ProjectionRevisionEntry {
                    scope,
                    revision: 1,
                    updated_at_ms,
                });
                1
            };
            next_revision
        };

        let updated_at_ms = now_ms();
        let mut pending = match self.pending.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(existing) = pending
            .iter_mut()
            .find(|candidate| candidate.invalidation.scope == scope)
        {
            if existing.invalidation.key != key {
                existing.invalidation.key = None;
            } else if existing.invalidation.key.is_none() {
                existing.invalidation.key = key.clone();
            }
            existing.invalidation.revision = revision;
            existing.invalidation.updated_at_ms = updated_at_ms;
            existing.invalidation.reason = Some(reason.to_string());
        } else {
            pending.push(PendingInvalidation {
                invalidation: ProjectionInvalidation {
                    scope,
                    key,
                    revision,
                    updated_at_ms,
                    reason: Some(reason.to_string()),
                },
            });
        }
        drop(pending);

        self.schedule_flush();
    }

    fn schedule_flush(&self) {
        if self.flush_scheduled.swap(true, Ordering::AcqRel) {
            return;
        }

        let this = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(INVALIDATION_DEBOUNCE).await;
            this.flush_once().await;
            this.flush_scheduled.store(false, Ordering::Release);
            let pending = this
                .pending
                .lock()
                .ok()
                .map(|guard| !guard.is_empty())
                .unwrap_or(false);
            if pending {
                this.schedule_flush();
            }
        });
    }

    async fn flush_once(&self) {
        let pending = {
            let mut guard = match self.pending.lock() {
                Ok(v) => v,
                Err(_) => return,
            };
            guard
                .drain(..)
                .map(|entry| entry.invalidation)
                .collect::<Vec<_>>()
        };

        if pending.is_empty() {
            return;
        }

        let snapshot = match self.snapshot.lock() {
            Ok(v) => v.clone(),
            Err(_) => return,
        };

        if let Some(path) = self.path.as_ref() {
            if let Some(parent) = path.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    warn!(
                        "[projection] failed to create projection directory {}: {}",
                        parent.display(),
                        err
                    );
                }
            }

            let temp_path = path.with_extension("json.tmp");
            match serde_json::to_vec_pretty(&snapshot) {
                Ok(raw) => {
                    if let Err(err) = fs::write(&temp_path, raw) {
                        warn!(
                            "[projection] failed to write projection snapshot {}: {}",
                            temp_path.display(),
                            err
                        );
                    } else if let Err(err) = fs::rename(&temp_path, path) {
                        warn!(
                            "[projection] failed to replace projection snapshot {} -> {}: {}",
                            temp_path.display(),
                            path.display(),
                            err
                        );
                    }
                }
                Err(err) => warn!("[projection] failed to serialize projection snapshot: {err}"),
            }
        }

        for invalidation in pending {
            self.bus
                .emit(NodeEvent::ProjectionInvalidated { invalidation });
        }
    }
}
