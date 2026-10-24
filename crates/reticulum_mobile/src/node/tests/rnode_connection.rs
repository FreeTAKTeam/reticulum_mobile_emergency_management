async fn capture_relay_packets(
    frames: &mut Vec<u8>,
    chunk: &[u8],
    packets: &AsyncMutex<Vec<reticulum::packet::Packet>>,
) {
    use reticulum::transport::buffer::OutputBuffer;
    use reticulum::{iface::hdlc::Hdlc, packet::Packet};
    frames.extend_from_slice(chunk);
    while let Some((start, end)) = Hdlc::find(frames) {
        let mut decoded = vec![0; end - start + 1];
        if let Ok(length) = Hdlc::decode(&frames[start..=end], &mut OutputBuffer::new(&mut decoded))
        {
            if let Ok(packet) = Packet::from_bytes(&decoded[..length]) {
                let mut captured = packets.lock().await;
                if captured.len() < 256 {
                    captured.push(packet);
                }
            }
        }
        frames.drain(..=end);
    }
    assert!(frames.len() < 8192, "relay received an unterminated frame");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pending_relay_fetch_does_not_block_explicit_peer_connection() {
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair_with_profile("fetch_registry", true).await;
    let storage = node_a
        .inner
        .lock()
        .unwrap()
        .active_config
        .as_ref()
        .unwrap()
        .storage_dir
        .clone()
        .unwrap();
    let config = build_config(
        "fetch_registry-a",
        Path::new(&storage),
        &format!("rnode-ble:test-loopback:{}", relay.address()),
    );
    node_a
        .inner
        .lock()
        .unwrap()
        .app_state
        .upsert_announce(&AnnounceRecord {
            destination_hex: "cccccccccccccccccccccccccccccccc".to_owned(),
            identity_hex: "dddddddddddddddddddddddddddddddd".to_owned(),
            destination_kind: "lxmf_propagation".to_owned(),
            announce_class: AnnounceClass::PropagationNode {},
            app_data: String::new(),
            display_name: None,
            hops: 1,
            interface_hex: String::new(),
            received_at_ms: now_ms(),
        })
        .expect("cache unavailable relay hint");
    let node_a = tokio::task::spawn_blocking(move || {
        node_a.restart(config).expect("restore cached relay");
        node_a
    })
    .await
    .expect("restart worker");
    node_b.announce_now().expect("refresh recipient route");
    node_a
        .request_peer_identity(node_b.get_status().lxmf_destination_hex)
        .expect("resolve recipient");
    node_a
        .set_active_propagation_node(Some("cccccccccccccccccccccccccccccccc".to_owned()))
        .expect("select unavailable relay");
    node_a.request_lxmf_sync(Some(1)).expect("schedule fetch");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(
        node_a.get_lxmf_sync_status().unwrap().phase,
        SyncPhase::RequestSent {}
    ) {
        assert!(Instant::now() < deadline, "fetch did not start");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let (resp, response) = cb::bounded(1);
    let tx = node_a.inner.lock().unwrap().cmd_tx.clone().unwrap();
    dispatch_command(
        &tx,
        Command::ConnectPeer {
            destination_hex: node_b.get_status().lxmf_destination_hex,
            resp,
        },
    )
    .expect("dispatch connect while fetch is pending");
    let deadline = Instant::now() + Duration::from_secs(3);
    let connected = loop {
        if let Ok(result) = response.try_recv() {
            break result.is_ok();
        }
        if Instant::now() >= deadline {
            break false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
    assert!(
        connected,
        "relay network wait held the interface registry needed by Connect"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rnode_explicit_connect_uses_lxmf_and_clears_disconnected_aliases() {
    use reticulum::packet::{DestinationType, PacketType};
    let _guard = test_lock().lock().await;
    let (relay, node_a, node_b) = start_node_pair_with_profile("rnode_reconnect", true).await;
    let peer = node_b.get_status();
    node_a
        .set_saved_peers(vec![SavedPeerRecord {
            destination_hex: peer.app_destination_hex.clone(),
            label: Some("peer-b".to_owned()),
            saved_at_ms: now_ms(),
            identity_hex: Some(peer.identity_hex.clone()),
            lxmf_destination_hex: Some(peer.lxmf_destination_hex.clone()),
            app_data: None,
            display_name: None,
            last_route_seen_at_ms: None,
            last_hops: Some(1),
            circle_tier: CircleTier::Inner {},
        }])
        .expect("save peer");
    // Cover a maintenance tick: LoRa announces must not proactively open sessions.
    tokio::time::sleep(Duration::from_secs(11)).await;
    assert!(!relay
        .packets
        .lock()
        .await
        .iter()
        .any(|packet| packet.header.packet_type == PacketType::LinkRequest));

    for destination in [&peer.app_destination_hex, &peer.lxmf_destination_hex] {
        node_a
            .disconnect_peer(peer.app_destination_hex.clone())
            .expect("disconnect");
        assert!(node_a
            .inner
            .lock()
            .unwrap()
            .app_state
            .get_ignored_peer_destinations()
            .unwrap()
            .contains(&peer.lxmf_destination_hex));
        relay.packets.lock().await.clear();
        node_a.connect_peer(destination.clone()).expect("reconnect");
        assert_eq!(
            node_a
                .inner
                .lock()
                .unwrap()
                .app_state
                .get_ignored_peer_destinations()
                .unwrap(),
            Vec::<String>::new(),
            "ignored aliases after reconnect through {destination}"
        );
        let packets = relay.packets.lock().await;
        let app_address = hex::decode(&peer.app_destination_hex).expect("app address");
        assert!(
            !packets
                .iter()
                .any(|packet| packet.header.packet_type == PacketType::Data
                    && packet.header.destination_type == DestinationType::Plain
                    && packet.data.as_slice().starts_with(&app_address)),
            "known LXMF route must not request the legacy app path"
        );
        let requests: Vec<_> = packets
            .iter()
            .filter(|packet| packet.header.packet_type == PacketType::LinkRequest)
            .collect();
        assert!(
            !requests.is_empty(),
            "explicit connect must establish a session"
        );
        assert!(
            requests
                .iter()
                .all(|packet| packet.destination.to_hex_string() == peer.lxmf_destination_hex),
            "explicit connect must use the verified LXMF destination"
        );
    }
    let mut saved = node_a.get_saved_peers().expect("saved contact").remove(0);
    for destination in [
        &peer.app_destination_hex,
        &peer.lxmf_destination_hex,
        &peer.lxmf_destination_hex,
        &peer.app_destination_hex,
    ] {
        saved.destination_hex = destination.clone();
        node_a
            .set_saved_peers(vec![saved.clone()])
            .expect("migrate contact alias");
        assert!(
            node_a
                .inner
                .lock()
                .unwrap()
                .app_state
                .get_ignored_peer_destinations()
                .unwrap()
                .is_empty(),
            "retaining a contact through either alias must not block it"
        );
    }
    node_a.set_saved_peers(vec![]).expect("remove contact");
    assert!(node_a
        .inner
        .lock()
        .unwrap()
        .app_state
        .get_ignored_peer_destinations()
        .unwrap()
        .contains(&peer.app_destination_hex));
    node_a
        .set_saved_peers(vec![saved])
        .expect("save contact again");
    let body = "Synthetic REM reconnect regression";
    let id = node_a
        .send_lxmf(SendLxmfRequest {
            destination_hex: peer.app_destination_hex,
            body_utf8: body.to_owned(),
            title: None,
            send_mode: SendMode::Auto {},
        })
        .expect("chat admitted after reconnect");
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        if node_b
            .list_messages(None)
            .expect("receiver history")
            .iter()
            .any(|message| {
                message.message_id_hex == id
                    && message.body_utf8 == body
                    && message.direction == MessageDirection::Inbound {}
            })
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "receiver did not persist the chat"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop_node(node_a).await;
    stop_node(node_b).await;
    relay.shutdown().await;
}
