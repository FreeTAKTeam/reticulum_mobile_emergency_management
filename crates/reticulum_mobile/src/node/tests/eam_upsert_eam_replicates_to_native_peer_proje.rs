#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upsert_eam_replicates_to_native_peer_projection() {
    const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("eam_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: node_b_status.app_destination_hex.clone(),
            label: Some("peer-b".to_string()),
            saved_at_ms: now_ms(),
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm eam link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm eam link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam link")
    })
    .expect("node b received eam warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| {
                peer.saved
                    && peer.active_link
                    && peer.lxmf_destination_hex.as_deref()
                        == Some(node_b_status.lxmf_destination_hex.as_str())
            });
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became mission-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let replication_targets = build_mission_replication_targets(
        &node_a.get_status(),
        node_a.list_peers().expect("list peers").as_slice(),
        node_a.get_saved_peers().expect("saved peers").as_slice(),
        node_a
            .get_lxmf_sync_status()
            .expect("sync status")
            .active_propagation_node_hex
            .as_deref(),
    );
    assert_eq!(
        replication_targets.len(),
        1,
        "expected one eam replication target"
    );
    assert_eq!(
        replication_targets[0].app_destination_hex,
        node_b_status.app_destination_hex
    );

    let record = EamProjectionRecord {
        callsign: "Pixel".to_string(),
        group_name: "Blue".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Yellow".to_string(),
        preparedness_status: "Green".to_string(),
        medical_status: "Green".to_string(),
        mobility_status: "Green".to_string(),
        comms_status: "Yellow".to_string(),
        notes: Some("native eam replication".to_string()),
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        eam_uid: Some("eam-upsert-native".to_string()),
        team_member_uid: Some("member-1".to_string()),
        team_uid: Some("team-1".to_string()),
        reported_at: Some("2026-03-25T16:30:00Z".to_string()),
        reported_by: Some(node_a_status.name.clone()),
        overall_status: Some("Yellow".to_string()),
        confidence: Some(0.8),
        ttl_seconds: Some(3600),
        source: Some(EamSourceRecord {
            rns_identity: node_a_status.identity_hex.clone(),
            display_name: Some(node_a_status.name.clone()),
        }),
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(now_ms()),
        last_synced_at_ms: None,
    };

    node_a.upsert_eam(record.clone()).expect("upsert local eam");

    let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| {
                eam.security_status == record.security_status
                    && eam.capability_status == record.capability_status
                    && eam.preparedness_status == record.preparedness_status
                    && eam.source.is_some()
            });
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted replicated eam"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert!(!received.callsign.trim().is_empty());
    assert!(received.team_uid.is_none());
    assert_eq!(
        received.team_member_uid.as_deref(),
        Some(node_a_status.lxmf_destination_hex.as_str())
    );
    assert!(received.eam_uid.is_none());
    assert_eq!(received.security_status, record.security_status);
    assert_eq!(received.capability_status, record.capability_status);
    assert_eq!(received.overall_status.as_deref(), Some("Yellow"));
    let source_identity = received
        .source
        .as_ref()
        .map(|source| source.rns_identity.as_str());
    assert!(
        matches!(
            source_identity,
            Some(identity)
                if identity == node_a_status.lxmf_destination_hex
                    || identity == node_a_status.app_destination_hex
        ),
        "unexpected EAM source identity {source_identity:?}"
    );

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upsert_eam_defaults_and_replicates_to_native_peer_projection() {
    const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("eam_defaults_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: node_b_status.app_destination_hex.clone(),
            label: Some("peer-b".to_string()),
            saved_at_ms: now_ms(),
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm eam defaults link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm eam defaults link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam defaults link")
    })
    .expect("node b received eam defaults warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.saved && peer.active_link);
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became mission-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let record = EamProjectionRecord {
        callsign: "Pixel".to_string(),
        group_name: "Blue".to_string(),
        security_status: "Green".to_string(),
        capability_status: "Yellow".to_string(),
        preparedness_status: "Green".to_string(),
        medical_status: "Green".to_string(),
        mobility_status: "Green".to_string(),
        comms_status: "Yellow".to_string(),
        notes: Some("native eam default replication".to_string()),
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        eam_uid: Some("eam-upsert-defaults".to_string()),
        team_member_uid: None,
        team_uid: None,
        reported_at: Some("2026-03-25T16:45:00Z".to_string()),
        reported_by: None,
        overall_status: None,
        confidence: Some(0.8),
        ttl_seconds: Some(3600),
        source: None,
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(now_ms()),
        last_synced_at_ms: None,
    };

    node_a.upsert_eam(record.clone()).expect("upsert local eam");

    let local = node_a
        .get_eams()
        .expect("get local eams")
        .into_iter()
        .find(|eam| eam.callsign == record.callsign)
        .expect("local eam persisted");
    assert_eq!(
        local.team_member_uid.as_deref(),
        Some(node_a_status.app_destination_hex.as_str())
    );
    assert_eq!(local.team_uid.as_deref(), Some(TEAM_UID_BLUE));

    let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    let received = loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| eam.callsign == record.callsign);
        if let Some(received) = received {
            break received;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted replicated eam with defaults"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(
        received.team_member_uid.as_deref(),
        Some(node_a_status.lxmf_destination_hex.as_str())
    );
    assert!(received.team_uid.is_none());

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delete_eam_replicates_to_native_peer_projection() {
    const EAM_REPLICATION_TIMEOUT: Duration = Duration::from_secs(75);
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair("eam_delete_projection").await;

    let node_a_status = node_a.get_status();
    let node_b_status = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: node_b_status.app_destination_hex.clone(),
            label: Some("peer-b".to_string()),
            saved_at_ms: now_ms(),
            identity_hex: None,
            lxmf_destination_hex: None,
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: None,
        }])
        .expect("save peer b");
    node_a
        .connect_peer(node_b_status.app_destination_hex.clone())
        .expect("connect peer b");

    let warm_link_subscription = node_b.subscribe_events();
    node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: node_b_status.lxmf_destination_hex.clone(),
            body_utf8: "warm eam delete link".to_string(),
            title: Some("warmup".to_string()),
            send_mode: SendMode::Auto {},
        })
        .expect("warm eam delete link");
    wait_for_event(&warm_link_subscription, TEST_TIMEOUT, |event| {
        matches!(event, NodeEvent::MessageReceived { message } if message.body_utf8 == "warm eam delete link")
    })
    .expect("node b received eam delete warmup message");

    let peer_ready_deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let peer_ready = node_a
            .list_peers()
            .expect("list peers")
            .into_iter()
            .find(|peer| peer.destination_hex == node_b_status.app_destination_hex)
            .is_some_and(|peer| peer.saved && peer.active_link);
        if peer_ready {
            break;
        }
        assert!(
            Instant::now() < peer_ready_deadline,
            "peer b never became mission-ready"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let record = EamProjectionRecord {
        callsign: "Ciccio".to_string(),
        group_name: "Yellow".to_string(),
        security_status: "Red".to_string(),
        capability_status: "Yellow".to_string(),
        preparedness_status: "Red".to_string(),
        medical_status: "Unknown".to_string(),
        mobility_status: "Unknown".to_string(),
        comms_status: "Unknown".to_string(),
        notes: Some("native eam delete replication".to_string()),
        updated_at_ms: now_ms(),
        deleted_at_ms: None,
        eam_uid: Some("eam-delete-native".to_string()),
        team_member_uid: Some(node_a_status.app_destination_hex.clone()),
        team_uid: Some(TEAM_UID_YELLOW.to_string()),
        reported_at: Some("2026-03-27T14:00:00Z".to_string()),
        reported_by: Some(node_a_status.name.clone()),
        overall_status: Some("Red".to_string()),
        confidence: Some(0.9),
        ttl_seconds: Some(3600),
        source: Some(EamSourceRecord {
            rns_identity: node_a_status.identity_hex.clone(),
            display_name: Some(node_a_status.name.clone()),
        }),
        sync_state: Some("draft".to_string()),
        sync_error: None,
        draft_created_at_ms: Some(now_ms()),
        last_synced_at_ms: None,
    };

    node_a.upsert_eam(record.clone()).expect("upsert local eam");

    let received_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    loop {
        let received = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| eam.callsign == record.callsign && eam.deleted_at_ms.is_none());
        if received.is_some() {
            break;
        }
        assert!(
            Instant::now() < received_deadline,
            "node b never persisted replicated eam before delete"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let deleted_at_ms = now_ms();
    node_a
        .delete_eam(record.callsign.clone(), deleted_at_ms)
        .expect("delete local eam");

    let delete_deadline = Instant::now() + EAM_REPLICATION_TIMEOUT;
    let deleted = loop {
        let deleted = node_b
            .get_eams()
            .expect("get eams")
            .into_iter()
            .find(|eam| {
                eam.callsign == record.callsign && eam.deleted_at_ms == Some(deleted_at_ms)
            });
        if let Some(deleted) = deleted {
            break deleted;
        }
        assert!(
            Instant::now() < delete_deadline,
            "node b never persisted replicated eam delete"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(deleted.callsign, record.callsign);
    assert_eq!(deleted.deleted_at_ms, Some(deleted_at_ms));

    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
