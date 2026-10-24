use serde::{Deserialize, Serialize};

use crate::types::TelemetryPositionRecord;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum TelemetryPublicationState {
    StoredLocally,
    QueuedNoRoute,
    QueuedForDelivery,
    Transmitted,
    TransportDelivered,
    Acknowledged,
    Rejected,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TelemetryRecipient {
    pub destination_hex: String,
    pub callsign: String,
    pub fix_updated_at_ms: u64,
    pub command_id: String,
    pub state: TelemetryPublicationState,
    pub message_id_hex: Option<String>,
    pub representation: Option<String>,
    pub detail: Option<String>,
    pub updated_at_ms: u64,
    pub attempt_route: Option<String>,
    #[serde(default)]
    pub custody_message_id_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TelemetryReceiveFailure {
    pub source_hex: Option<String>,
    pub destination_hex: String,
    pub message_id_hex: Option<String>,
    pub command_id: Option<String>,
    pub reason: String,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct TelemetryPublication {
    pub fix: Option<TelemetryPositionRecord>,
    pub recipients: Vec<TelemetryRecipient>,
    pub last_receive_failure: Option<TelemetryReceiveFailure>,
}

impl TelemetryPublication {
    pub fn snapshot_json(&self) -> serde_json::Value {
        serde_json::json!({
            "local_fix_updated_at_ms": self.fix.as_ref().map(|fix| fix.updated_at_ms),
            "callsign": self.fix.as_ref().map(|fix| &fix.callsign),
            "state": if self.recipients.is_empty() { "StoredLocally" } else { "PendingRecipients" },
            "recipients": self.recipients,
            "last_receive_failure": self.last_receive_failure,
        })
    }
}

impl TelemetryPublication {
    pub(crate) fn recover_uncertain_sends(&mut self) {
        for row in &mut self.recipients {
            if row.custody_message_id_hex.is_none()
                && matches!(
                    row.state,
                    TelemetryPublicationState::QueuedForDelivery
                        | TelemetryPublicationState::Transmitted
                        | TelemetryPublicationState::TransportDelivered
                )
            {
                row.state = TelemetryPublicationState::QueuedNoRoute;
                row.attempt_route = None;
                row.detail = Some("recovering uncertain delivery after restart".to_string());
            }
        }
    }
}
