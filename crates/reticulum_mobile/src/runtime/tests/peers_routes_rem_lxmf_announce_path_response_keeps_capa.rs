#[tokio::test]
async fn rem_lxmf_announce_path_response_keeps_capability_app_data() {
    use reticulum::transport::destination::{DestinationAnnounce, PlainInputDestination};
    use reticulum::transport::identity::EmptyIdentity;
    use reticulum::transport::iface::{IfaceSource, RxMessage, TxMessageType};
    use reticulum::transport::packet::{
        ContextFlag, DestinationType, Header, HeaderType, PacketContext, PropagationType,
    };
    use tokio::time::timeout;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &identity, true);
    let mut transport = Transport::new(config);
    let mut iface_channel = transport.iface_manager().lock().await.new_channel(16);
    let app_destination = transport
        .add_destination(
            identity.clone(),
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .await;
    let lxmf_destination = transport
        .add_destination(
            identity,
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .await;
    let transport = Arc::new(transport);
    let capabilities = Arc::new(TokioMutex::new(AnnounceProfile::new(
        "Pixel",
        "R3AKT,EMergencyMessages,Telemetry",
    )));

    announce_destinations(
        &transport,
        &app_destination,
        &lxmf_destination,
        &capabilities,
        "test",
    )
    .await;

    let first_announce = timeout(Duration::from_millis(200), iface_channel.tx_channel.recv())
        .await
        .expect("expected outbound announce")
        .expect("tx channel open");
    assert!(matches!(
        first_announce.tx_type,
        TxMessageType::Broadcast(None)
    ));

    let lxmf_destination_hash = lxmf_destination.lock().await.desc.address_hash;
    let mut request_data = PacketDataBuffer::new_from_slice(lxmf_destination_hash.as_slice());
    request_data.safe_write(&[0x44; 16]);
    let path_request_destination = PlainInputDestination::new(
        EmptyIdentity {},
        DestinationName::new("rnstransport", "path.request"),
    )
    .desc
    .address_hash;
    let path_request = Packet {
        header: Header {
            ifac_flag: reticulum::transport::packet::IfacFlag::Open,
            header_type: HeaderType::Type1,
            context_flag: ContextFlag::Unset,
            propagation_type: PropagationType::Broadcast,
            destination_type: DestinationType::Plain,
            packet_type: PacketType::Data,
            hops: 0,
        },
        ifac: None,
        destination: path_request_destination,
        transport: None,
        context: PacketContext::None,
        data: request_data,
    };

    iface_channel
        .rx_channel
        .send(RxMessage {
            address: iface_channel.address,
            packet: path_request,
            source: IfaceSource::None,
        })
        .await
        .expect("path request enqueued");

    let response = timeout(Duration::from_millis(500), iface_channel.tx_channel.recv())
        .await
        .expect("expected path response")
        .expect("tx channel open");
    assert!(
        matches!(response.tx_type, TxMessageType::Direct(iface) if iface == iface_channel.address)
    );
    assert_eq!(response.packet.destination, lxmf_destination_hash);
    assert_eq!(response.packet.context, PacketContext::PathResponse);

    let announce = DestinationAnnounce::validate(&response.packet).expect("path announce");
    let app_data_hex = hex::encode(announce.app_data);
    assert!(app_data_has_rem_peer_capabilities(app_data_hex.as_str()));
    let metadata = parse_announce_metadata(app_data_hex.as_str());
    assert_eq!(metadata.display_name.as_deref(), Some("Pixel"));
    assert!(metadata
        .capability_tokens
        .iter()
        .any(|token| token == crate::announce_metadata::STANDARD_LXMF_RECEIPTS_CAPABILITY));
}

#[test]
fn operator_announce_message_ignores_regular_lxmf_announces() {
    let message = operator_announce_message(
        AnnounceClass::LxmfDelivery {},
        false,
        Some("LXMF Chat"),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        1,
    );

    assert!(message.is_none());
}
