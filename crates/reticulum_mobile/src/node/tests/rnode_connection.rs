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
