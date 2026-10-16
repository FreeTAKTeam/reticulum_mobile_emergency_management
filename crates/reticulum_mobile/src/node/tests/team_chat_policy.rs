fn policy_chat_message(destination: &str) -> MessageRecord {
    MessageRecord {
        message_id_hex: "policy-chat".to_string(),
        conversation_id: destination.to_string(),
        direction: MessageDirection::Outbound {},
        destination_hex: destination.to_string(),
        source_hex: None,
        requested_destination_hex: Some(destination.to_string()),
        delivery_destination_hex: Some(destination.to_string()),
        recipient_identity_hex: None,
        last_wire_message_id_hex: None,
        title: None,
        body_utf8: "contact chat".to_string(),
        traffic_class: OutboundTrafficClass::Chat {},
        method: MessageMethod::Direct {},
        state: MessageState::Failed {},
        transport_state: TransportDeliveryState::Failed {},
        application_ack_state: ApplicationAckState::NotRequired {},
        detail: None,
        sent_at_ms: Some(1),
        received_at_ms: None,
        updated_at_ms: 1,
    }
}

#[test]
fn chat_send_and_retry_admit_discovered_peers_and_saved_contacts() {
    let destination = "abababababababababababababababab";
    for saved in [false, true] {
        let dir = prepare_storage_dir(if saved {
            "contact-chat"
        } else {
            "discovered-chat"
        });
        let node = Node::with_storage_dir(dir.to_str()).unwrap();
        if saved {
            node.set_saved_peers(vec![policy_peer(destination)])
                .unwrap();
        }
        let (tx, mut rx) = mpsc::channel(4);
        {
            let mut inner = node.inner.lock().unwrap();
            inner.cmd_tx = Some(tx);
            inner.active_config = Some(build_config_fingerprint_for_tests(
                HubMode::Connected {},
                None,
            ));
            inner
                .app_state
                .upsert_message(&policy_chat_message(destination))
                .unwrap();
            *inner.peers_snapshot.lock().unwrap() = vec![build_peer_record(
                destination,
                destination,
                saved,
                true,
                true,
            )];
        }
        let worker = std::thread::spawn(move || {
            match rx.blocking_recv().unwrap() {
                Command::SendLxmf {
                    request,
                    requested_destination_hex,
                    resp,
                    ..
                } => {
                    assert_eq!(request.destination_hex, destination);
                    assert_eq!(requested_destination_hex, destination);
                    resp.send(Ok("accepted".to_string())).unwrap();
                }
                _ => panic!("expected chat send"),
            }
            match rx.blocking_recv().unwrap() {
                Command::RetryLxmf {
                    delivery_destination_hex,
                    resp,
                    ..
                } => {
                    assert_eq!(delivery_destination_hex, destination);
                    resp.send(Ok(())).unwrap();
                }
                _ => panic!("expected retry"),
            }
        });
        assert_eq!(
            node.send_lxmf(SendLxmfRequest {
                destination_hex: destination.to_string(),
                body_utf8: "hello".to_string(),
                title: None,
                send_mode: SendMode::Auto {},
            })
            .unwrap(),
            "accepted"
        );
        node.retry_lxmf("policy-chat".to_string()).unwrap();
        worker.join().unwrap();
    }
}

#[test]
fn power_saver_still_pauses_chat_send_and_retry() {
    let dir = prepare_storage_dir("team-chat-saver");
    let node = Node::with_storage_dir(dir.to_str()).unwrap();
    let destination = "abababababababababababababababab";
    let (tx, mut rx) = mpsc::channel(4);
    {
        let mut inner = node.inner.lock().unwrap();
        inner.cmd_tx = Some(tx);
        inner.power_state.saver_active = true;
        inner
            .app_state
            .upsert_message(&policy_chat_message(destination))
            .unwrap();
    }
    assert!(matches!(
        node.send_lxmf(SendLxmfRequest {
            destination_hex: destination.to_string(),
            body_utf8: "paused".to_string(),
            title: None,
            send_mode: SendMode::Auto {},
        }),
        Err(NodeError::InvalidConfig {})
    ));
    assert!(matches!(
        node.retry_lxmf("policy-chat".to_string()),
        Err(NodeError::InvalidConfig {})
    ));
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn set_peer_tier_is_ignored_and_logged_once() {
    let _guard = test_lock().lock().await;
    let dir = prepare_storage_dir("team-tier-log");
    let node = Node::with_storage_dir(dir.to_str()).unwrap();
    let bus = EventBus::new();
    let rx = bus.subscribe();
    NodeLogger::global().set_bus(Some(bus));
    node.set_saved_peers(vec![
        build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        build_saved_peer_for("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    ])
    .unwrap();
    NodeLogger::global().set_bus(None);
    let logs = rx
        .try_iter()
        .filter(|event| {
            matches!(event, NodeEvent::Log { message, .. }
        if message == "circle_tier is derived from team membership and was ignored")
        })
        .count();
    assert_eq!(logs, 1);
    assert!(node
        .get_saved_peers()
        .unwrap()
        .iter()
        .all(|peer| peer.circle_tier == CircleTier::Outer {}));
}

#[test]
fn circle_migration_rolls_back_settings_peers_and_marker_on_failure() {
    let dir = prepare_storage_dir("team-migration-rollback");
    let store = AppStateStore::new(dir.to_str()).unwrap();
    store.set_app_settings(&sample_app_settings()).unwrap();
    store
        .set_saved_peers(&[build_saved_peer_for("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")])
        .unwrap();
    let connection = rusqlite::Connection::open(dir.join("app_state.db")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_circle_migration BEFORE UPDATE ON saved_peers BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(store.get_app_settings().is_err());
    let marker: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata WHERE key = 'circle_migration_completed')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!marker);
    let raw: String = connection
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let settings: AppSettingsRecord = serde_json::from_str(&raw).unwrap();
    assert!(!settings.teams.local_teams_initialized);
    connection
        .execute_batch("DROP TRIGGER fail_circle_migration;")
        .unwrap();
    assert_eq!(
        store.get_app_settings().unwrap().unwrap().teams.local_teams[0]
            .member_destinations
            .len(),
        1
    );
}

#[test]
fn community_status_reaches_saved_contacts_and_connected_telemetry_stays_direct() {
    let destination = "abababababababababababababababab";
    let identity = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
    let hub = "56565656565656565656565656565656";
    for mode in [HubMode::Autonomous {}, HubMode::Connected {}] {
        let dir = prepare_storage_dir(&format!("team-dispatch-{mode:?}"));
        let node = Node::with_storage_dir(dir.to_str()).unwrap();
        node.set_saved_peers(vec![policy_peer(destination)])
            .unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        {
            let mut inner = node.inner.lock().unwrap();
            inner.cmd_tx = Some(tx);
            inner.active_config = Some(build_config_fingerprint_for_tests(mode, Some(hub)));
            *inner.hub_directory_snapshot.lock().unwrap() = Some(test_team_snapshot(&[]));
        }
        let worker = std::thread::spawn(move || {
            for _ in 0..2 {
                match rx.blocking_recv().unwrap() {
                    Command::SendBytes {
                        destination_hex,
                        resp,
                        ..
                    } => {
                        assert_eq!(destination_hex, destination);
                        resp.send(Ok(())).unwrap();
                    }
                    _ => panic!("expected byte send"),
                }
            }
        });
        node.send_bytes_with_class(
            destination.to_string(),
            vec![1],
            None,
            SendMode::Auto {},
            OutboundTrafficClass::CommunityStatus {},
        )
        .unwrap();
        {
            let inner = node.inner.lock().unwrap();
            let mut peer = build_peer_record(destination, destination, true, true, true);
            peer.identity_hex = Some(identity.to_string());
            *inner.peers_snapshot.lock().unwrap() = vec![peer];
            *inner.hub_directory_snapshot.lock().unwrap() = Some(test_team_snapshot(&[identity]));
        }
        node.send_bytes_with_class(
            destination.to_string(),
            vec![2],
            None,
            SendMode::Auto {},
            OutboundTrafficClass::Telemetry {},
        )
        .unwrap();
        assert!(matches!(
            node.send_bytes_with_class(
                hub.to_string(),
                vec![3],
                None,
                SendMode::Auto {},
                OutboundTrafficClass::Telemetry {}
            ),
            Err(NodeError::InvalidConfig {})
        ));
        worker.join().unwrap();
    }
}

#[test]
fn maintained_links_follow_any_local_team_and_open_conversations() {
    let dir = prepare_storage_dir("team-maintained-links");
    let node = Node::with_storage_dir(dir.to_str()).unwrap();
    let contact = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let member = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    node.set_saved_peers(vec![policy_peer(contact), policy_peer(member)])
        .unwrap();
    set_test_team(&node, vec![member.to_string()]);
    let store = node.inner.lock().unwrap().app_state.clone();
    assert_eq!(
        crate::runtime::maintained_link_destinations(&store).unwrap(),
        HashSet::from([member.to_string()])
    );
    let message = policy_chat_message(contact);
    store.upsert_message(&message).unwrap();
    assert_eq!(
        crate::runtime::maintained_link_destinations(&store).unwrap(),
        HashSet::from([member.to_string(), contact.to_string()])
    );
    set_test_team(&node, vec![]);
    assert_eq!(
        crate::runtime::maintained_link_destinations(&store).unwrap(),
        HashSet::from([contact.to_string()])
    );
    node.delete_conversation(contact.to_string()).unwrap();
    assert!(crate::runtime::maintained_link_destinations(&store)
        .unwrap()
        .is_empty());
}
