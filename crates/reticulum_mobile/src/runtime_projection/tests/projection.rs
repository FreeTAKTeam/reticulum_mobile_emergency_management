use super::{
    PersistedPeerRecord, PersistedSyncStatus, ProjectionRevisionEntry,
    RuntimeProjectionJournal, RuntimeProjectionSnapshot,
};
use crate::event_bus::EventBus;
use crate::types::{
    ApplicationAckState, MessageDirection, MessageMethod, MessageRecord, MessageState,
    OutboundTrafficClass, PeerRecord, PeerState, ProjectionScope, TransportDeliveryState,
};

fn temporary_projection_path(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir()
        .join(format!("rem-projection-{name}-{}-{unique}", std::process::id()))
        .join("runtime_projection.json")
}

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

fn build_message(
    message_id_hex: &str,
    conversation_id: &str,
    destination_hex: &str,
    source_hex: Option<&str>,
) -> MessageRecord {
    MessageRecord {
        message_id_hex: message_id_hex.to_string(),
        conversation_id: conversation_id.to_string(),
        direction: MessageDirection::Inbound {},
        destination_hex: destination_hex.to_string(),
        source_hex: source_hex.map(str::to_string),
        requested_destination_hex: Some(destination_hex.to_string()),
        delivery_destination_hex: Some(destination_hex.to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: Some(message_id_hex.to_string()),
        title: Some("chat".to_string()),
        body_utf8: format!("body {message_id_hex}"),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Received {},
        transport_state: TransportDeliveryState::TransportDelivered {},
        application_ack_state: ApplicationAckState::NotRequired {},
        detail: None,
        sent_at_ms: None,
        received_at_ms: Some(1_700_000_000_000),
        updated_at_ms: 1_700_000_000_000,
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
fn restored_peers_do_not_restore_transport_state() {
    let mut peer = build_persisted_peer("saved-peer", Some(true), None);
    peer.active_link = true;
    peer.state = "connected".to_string();
    let snapshot = RuntimeProjectionSnapshot {
        peers: vec![peer],
        ..RuntimeProjectionSnapshot::default()
    };

    let restored = snapshot.restored_peers();

    assert_eq!(restored.len(), 1);
    assert!(!restored[0].active_link);
    assert!(matches!(restored[0].state, PeerState::Disconnected {}));
    assert_eq!(restored[0].last_seen_at_ms, 0);
    assert_eq!(restored[0].announce_last_seen_at_ms, None);
    assert_eq!(restored[0].lxmf_last_seen_at_ms, None);
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
fn load_snapshot_distinguishes_missing_and_malformed_files_without_panicking() {
    let missing_path = temporary_projection_path("missing");
    let missing = RuntimeProjectionJournal::new(Some(missing_path), EventBus::new());
    assert!(missing.load_snapshot().is_none());

    let malformed_path = temporary_projection_path("malformed");
    let parent = malformed_path
        .parent()
        .expect("temporary path parent")
        .to_path_buf();
    std::fs::create_dir_all(&parent).expect("create temporary projection directory");
    std::fs::write(&malformed_path, b"{not-json").expect("write malformed projection");
    let malformed = RuntimeProjectionJournal::new(Some(malformed_path), EventBus::new());
    assert!(malformed.load_snapshot().is_none());
    std::fs::remove_dir_all(&parent).expect("remove temporary projection directory");
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
        assert!(!current[0].active_link);
        assert!(matches!(current[0].state, PeerState::Disconnected {}));
        assert_eq!(current[0].last_seen_at_ms, 0);
        assert_eq!(current[0].announce_last_seen_at_ms, None);
        assert_eq!(current[0].lxmf_last_seen_at_ms, None);
    });
}

#[test]
fn remove_conversation_messages_removes_alias_matches_from_snapshot() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let journal = RuntimeProjectionJournal::new(None, EventBus::new());
        let deleted_message =
            build_message("delete-message", "identity", "lxmfdest", Some("appdest"));
        assert!(journal.record_message(deleted_message, Some("test"),));
        assert!(journal.record_message(
            build_message("keep-message", "other", "other", None),
            Some("test"),
        ));

        assert!(journal.remove_conversation_messages(
            ["appdest", "lxmfdest", "identity"],
            Some("conversation-deleted"),
        ));
        journal.flush_now().await;

        let messages = journal.current_messages().expect("current messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id_hex, "keep-message");
    });
}
