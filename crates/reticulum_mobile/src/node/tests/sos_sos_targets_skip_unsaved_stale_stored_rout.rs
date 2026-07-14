#[test]
fn sos_targets_skip_unsaved_stale_stored_route() {
    let status = build_status_for_tests();
    let mut peer = build_peer_record(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        false,
        false,
        false,
    );
    peer.stale = true;
    peer.announce_last_seen_at_ms = None;
    peer.lxmf_last_seen_at_ms = None;

    let targets = build_sos_replication_targets(
        &status,
        &[peer],
        &[],
        Some("99999999999999999999999999999999"),
    );

    assert!(targets.is_empty());
}

#[test]
fn sos_priority_keeps_saved_low_hop_routes_before_unsaved_active_peers() {
    let status = build_status_for_tests();
    let peers = vec![
        build_peer_record(
            "a3e80acf291d9f57ee455493946b3763",
            "182af0c5afb2ab7176406539c83676dc",
            true,
            false,
            false,
        ),
        build_peer_record(
            "0e252632c57fa999a15c4a05c0d1bae2",
            "a1c8126d7cb806e6bde086d582b6cb0d",
            true,
            false,
            false,
        ),
        build_peer_record(
            "5ea3149a58998316f6c8200a006e14c2",
            "34eb6bec21defd52736eaca1adff75eb",
            true,
            false,
            false,
        ),
        build_peer_record(
            "1a9a24c8bb16e1951cb954aef32158b1",
            "fe2a758c2bb875886a83a3bf6859d213",
            false,
            true,
            true,
        ),
        build_peer_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "4d30bc17c71d98436558ff63da188f2a",
            true,
            false,
            false,
        ),
        build_peer_record(
            "a133b8b1fe137f92210a048efded46db",
            "1080d475b77274d7fdabaed81878d916",
            true,
            false,
            false,
        ),
    ];
    let saved_peers = peers
        .iter()
        .filter(|peer| peer.saved)
        .map(|peer| build_saved_peer_for(peer.destination_hex.as_str()))
        .collect::<Vec<_>>();
    let announces = vec![
        build_announce_record(
            "0e252632c57fa999a15c4a05c0d1bae2",
            "app",
            AnnounceClass::PeerApp {},
            1,
            100,
        ),
        build_announce_record(
            "5ea3149a58998316f6c8200a006e14c2",
            "app",
            AnnounceClass::PeerApp {},
            1,
            90,
        ),
        build_announce_record(
            "3457d5ba744e89bbae543c5ff9c679fb",
            "app",
            AnnounceClass::PeerApp {},
            4,
            110,
        ),
        build_announce_record(
            "a3e80acf291d9f57ee455493946b3763",
            "app",
            AnnounceClass::PeerApp {},
            5,
            120,
        ),
        build_announce_record(
            "a133b8b1fe137f92210a048efded46db",
            "app",
            AnnounceClass::PeerApp {},
            8,
            130,
        ),
    ];
    let route_hops = announce_route_hops(announces.as_slice());
    let mut targets =
        build_sos_replication_targets(&status, peers.as_slice(), saved_peers.as_slice(), None);

    prioritize_sos_replication_targets(
        targets.as_mut_slice(),
        peers.as_slice(),
        saved_peers.as_slice(),
        &route_hops,
    );

    assert_eq!(
        targets
            .iter()
            .map(|target| target.app_destination_hex.as_str())
            .collect::<Vec<_>>(),
        vec![
            "0e252632c57fa999a15c4a05c0d1bae2",
            "5ea3149a58998316f6c8200a006e14c2",
            "3457d5ba744e89bbae543c5ff9c679fb",
            "a3e80acf291d9f57ee455493946b3763",
            "a133b8b1fe137f92210a048efded46db",
            "1a9a24c8bb16e1951cb954aef32158b1",
        ]
    );
}

fn msgpack_map(entries: Vec<(&str, MsgPackValue)>) -> MsgPackValue {
    MsgPackValue::Map(
        entries
            .into_iter()
            .map(|(key, value)| (MsgPackValue::from(key), value))
            .collect(),
    )
}

fn mission_command_fields(
    command_id: &str,
    correlation_id: &str,
    command_type: &str,
    args: Vec<(&str, MsgPackValue)>,
) -> Vec<u8> {
    let fields = msgpack_map(vec![(
        "9",
        MsgPackValue::Array(vec![msgpack_map(vec![
            ("command_id", MsgPackValue::from(command_id)),
            ("correlation_id", MsgPackValue::from(correlation_id)),
            ("command_type", MsgPackValue::from(command_type)),
            ("args", msgpack_map(args)),
        ])]),
    )]);
    rmp_serde::to_vec(&fields).expect("msgpack command fields")
}

fn mission_event_fields(
    event_type: &str,
    event_uid: &str,
    payload: Vec<(&str, MsgPackValue)>,
) -> Vec<u8> {
    let fields = msgpack_map(vec![(
        "13",
        MsgPackValue::Map(vec![
            (
                MsgPackValue::from("event_type"),
                MsgPackValue::from(event_type),
            ),
            (
                MsgPackValue::from("event_id"),
                MsgPackValue::from(event_uid),
            ),
            (MsgPackValue::from("payload"), msgpack_map(payload)),
        ]),
    )]);
    rmp_serde::to_vec(&fields).expect("msgpack event fields")
}
