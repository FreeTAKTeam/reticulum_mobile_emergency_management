use std::collections::HashSet;
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
    ApplicationAckState, MessageDirection, MessageMethod, MessageRecord, MessageState, NodeEvent,
    PeerRecord, PeerState, ProjectionInvalidation, ProjectionScope, SyncPhase, SyncStatus,
    TransportDeliveryState,
};

pub(crate) const PERSIST_FILENAME: &str = "runtime_projection.json";
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
    #[serde(default)]
    requested_destination_hex: Option<String>,
    #[serde(default)]
    delivery_destination_hex: Option<String>,
    #[serde(default)]
    recipient_identity_hex: Option<String>,
    #[serde(default)]
    last_wire_message_id_hex: Option<String>,
    title: Option<String>,
    body_utf8: String,
    method: String,
    state: String,
    #[serde(default)]
    transport_state: TransportDeliveryState,
    #[serde(default)]
    application_ack_state: ApplicationAckState,
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
    include!("runtime_projection/tests/projection.rs");
}

struct PendingInvalidation {
    invalidation: ProjectionInvalidation,
}

include!("runtime_projection/persistence.rs");
#[derive(Clone)]
pub(crate) struct RuntimeProjectionJournal {
    bus: EventBus,
    path: Option<PathBuf>,
    snapshot: Arc<StdMutex<RuntimeProjectionSnapshot>>,
    pending: Arc<StdMutex<Vec<PendingInvalidation>>>,
    flush_scheduled: Arc<AtomicBool>,
}

include!("runtime_projection/journal_io.rs");

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

    pub(crate) fn record_peers(&self, peers: Vec<PeerRecord>, reason: Option<&str>) -> bool {
        let Some(persisted) = persisted_saved_peers(peers.as_slice()) else {
            return false;
        };

        let mut guard = match self.snapshot.lock() {
            Ok(v) => v,
            Err(error) => {
                warn!("[projection] failed to record peer snapshot: {error}");
                return false;
            }
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
            Err(error) => {
                warn!("[projection] failed to record sync snapshot: {error}");
                return false;
            }
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
            Err(error) => {
                warn!("[projection] failed to record message snapshot: {error}");
                return false;
            }
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

    pub(crate) fn remove_conversation_messages<'a, I>(
        &self,
        conversation_keys: I,
        reason: Option<&str>,
    ) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        let keys = conversation_keys
            .into_iter()
            .map(normalize_message_key)
            .filter(|key| !key.is_empty())
            .collect::<HashSet<_>>();
        if keys.is_empty() {
            return false;
        }

        let mut guard = match self.snapshot.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let original_len = guard.messages.len();
        guard
            .messages
            .retain(|message| !message_matches_conversation_keys(message, &keys));
        if guard.messages.len() == original_len {
            return false;
        }
        guard.updated_at_ms = now_ms();
        drop(guard);

        self.invalidate(
            ProjectionScope::Messages {},
            None,
            reason.unwrap_or("conversation-deleted"),
        );
        true
    }

    #[cfg(test)]
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

    #[cfg(test)]
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
