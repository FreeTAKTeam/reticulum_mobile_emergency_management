fn signed_block_fixture(
    identity: &reticulum::transport::identity::PrivateIdentity,
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> String {
    let public = *identity.as_identity();
    let app = SingleOutputDestination::new(
        public,
        DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
    )
    .desc
    .address_hash
    .to_hex_string();
    let lxmf = SingleOutputDestination::new(
        public,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    )
    .desc
    .address_hash
    .to_hex_string();
    let content = BlockSignedContentV1 {
        expires_at_ms,
        issued_at_ms,
        issuer_app_destination_hex: app,
        issuer_lxmf_destination_hex: lxmf,
        issuer_public_identity_hex: identity.as_identity().to_hex_string(),
        kind: BLOCK_ENVELOPE_KIND.to_string(),
        network: BlockNetworkSettings {
            tcp_clients: vec!["mesh.example:4242".to_string()],
            broadcast: true,
            hub_mode: HubMode::Autonomous {},
            hub_identity_hash: None,
            hub_api_base_url: None,
            hub_refresh_interval_seconds: 3600,
            radio: Some(BlockRadioSettings {
                region: "US915".to_string(),
                profile: "REM-LF-RURAL-v1".to_string(),
                frequency_hz: 915_000_000,
            }),
        },
        preferred_map_layer: PreferredMapLayer::Base {},
        trusted_destination_hashes: vec!["dddddddddddddddddddddddddddddddd".to_string()],
        version: 1,
    };
    let canonical = serde_json::to_vec(&content).expect("canonical content");
    let wire = BlockWireEnvelopeV1 {
        signature: URL_SAFE_NO_PAD.encode(
            reticulum::transport::identity::lxmf_sign(identity, &canonical),
        ),
        content,
    };
    format!(
        "{BLOCK_ENVELOPE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&wire).expect("wire"))
    )
}

fn sized_signed_block_fixture(
    identity: &reticulum::transport::identity::PrivateIdentity,
    issued_at_ms: u64,
    expires_at_ms: u64,
    trusted_count: usize,
    url_path_len: usize,
) -> String {
    let public = *identity.as_identity();
    let trusted_destination_hashes = (0..trusted_count)
        .map(|index| format!("{index:032x}"))
        .collect();
    let content = BlockSignedContentV1 {
        expires_at_ms,
        issued_at_ms,
        issuer_app_destination_hex: SingleOutputDestination::new(
            public,
            DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
        )
        .desc
        .address_hash
        .to_hex_string(),
        issuer_lxmf_destination_hex: SingleOutputDestination::new(
            public,
            DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
        )
        .desc
        .address_hash
        .to_hex_string(),
        issuer_public_identity_hex: identity.as_identity().to_hex_string(),
        kind: BLOCK_ENVELOPE_KIND.to_string(),
        network: BlockNetworkSettings {
            tcp_clients: vec!["mesh.example:4242".to_string()],
            broadcast: true,
            hub_mode: HubMode::Autonomous {},
            hub_identity_hash: None,
            hub_api_base_url: Some(format!("https://mesh.example/{}", "a".repeat(url_path_len))),
            hub_refresh_interval_seconds: 3600,
            radio: Some(BlockRadioSettings {
                region: "US915".to_string(),
                profile: "REM-LF-RURAL-v1".to_string(),
                frequency_hz: 915_000_000,
            }),
        },
        preferred_map_layer: PreferredMapLayer::Base {},
        trusted_destination_hashes,
        version: 1,
    };
    let canonical = serde_json::to_vec(&content).expect("canonical content");
    let wire = BlockWireEnvelopeV1 {
        signature: URL_SAFE_NO_PAD.encode(
            reticulum::transport::identity::lxmf_sign(identity, &canonical),
        ),
        content,
    };
    format!(
        "{BLOCK_ENVELOPE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&wire).expect("wire"))
    )
}

#[test]
fn maximum_block_onboarding_fixed_vector_is_valid_and_reproducible() {
    let identity = reticulum::transport::identity::PrivateIdentity::new_from_name(
        "rem-block-onboarding-public-test-vector-v1",
    );
    let encoded = sized_signed_block_fixture(
        &identity,
        1_900_000_000_000,
        1_900_000_060_000,
        16,
        97,
    );
    let checked_in = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/mobile/android/app/src/test/resources/block-onboarding-max-v1.txt"
    ));
    assert_eq!(encoded, checked_in);
    assert_eq!(encoded.len(), BLOCK_ENVELOPE_MAX_BYTES);
    let inspection = decode_and_verify_block_envelope(&encoded, 1_900_000_000_001)
        .expect("checked-in signed vector must verify");
    assert_eq!(inspection.trusted_destination_hashes.len(), 16);
}

#[test]
fn block_onboarding_signature_tamper_and_identity_binding_are_enforced() {
    let now = 1_700_000_000_000;
    let identity = reticulum::transport::identity::PrivateIdentity::new_from_name("block-vector");
    let encoded = signed_block_fixture(&identity, now, now + 60_000);
    let inspection = decode_and_verify_block_envelope(&encoded, now + 1).expect("valid vector");
    assert_eq!(inspection.network.tcp_clients, vec!["mesh.example:4242"]);
    assert!(!encoded.contains(&identity.to_hex_string()));

    let mut tampered = encoded.into_bytes();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
    assert!(decode_and_verify_block_envelope(
        std::str::from_utf8(&tampered).expect("utf8"),
        now + 1
    )
    .is_err());

    let mut wire: BlockWireEnvelopeV1 = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(signed_block_fixture(&identity, now, now + 60_000).trim_start_matches(BLOCK_ENVELOPE_PREFIX))
            .expect("decode wire"),
    )
    .expect("wire");
    wire.content.issuer_app_destination_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    let canonical = serde_json::to_vec(&wire.content).expect("canonical");
    wire.signature = URL_SAFE_NO_PAD.encode(
        reticulum::transport::identity::lxmf_sign(&identity, &canonical),
    );
    let rebound = format!(
        "{BLOCK_ENVELOPE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&wire).expect("wire"))
    );
    assert!(decode_and_verify_block_envelope(&rebound, now + 1).is_err());
}

#[test]
fn block_onboarding_rejects_smuggling_and_endpoint_boundaries() {
    assert!(validate_block_endpoint("https://mesh.example:4242").is_err());
    assert!(validate_block_endpoint("user@mesh.example:4242").is_err());
    assert!(validate_block_endpoint("mesh.example:0").is_err());
    assert!(validate_block_endpoint("2001:db8::1:4242").is_err());
    assert!(validate_block_endpoint("[not-ipv6]:4242").is_err());
    assert_eq!(
        validate_block_endpoint("[2001:db8::1]:4242").expect("ipv6"),
        "[2001:db8::1]:4242"
    );
    assert_eq!(
        validate_block_endpoint("MESH.EXAMPLE:080").expect("canonical endpoint"),
        "mesh.example:80"
    );
    assert!(validate_block_url("https://user@example.test/path").is_err());
    assert!(validate_block_url("https://example.test:invalid/path").is_err());
    assert!(validate_block_url("https://2001:db8::1/path").is_err());
    assert!(serde_json::from_value::<BlockNetworkSettings>(serde_json::json!({
        "tcp_clients": [],
        "broadcast": true,
        "hub_mode": "Autonomous",
        "hub_identity_hash": null,
        "hub_api_base_url": null,
        "hub_refresh_interval_seconds": 3600,
        "radio": null,
        "hub_api_key": "smuggled"
    }))
    .is_err());
    assert!(decode_and_verify_block_envelope(
        &format!("{BLOCK_ENVELOPE_PREFIX}{}", "A".repeat(BLOCK_ENVELOPE_MAX_BYTES)),
        1_700_000_000_000,
    )
    .is_err());
    let identity = reticulum::transport::identity::PrivateIdentity::new_from_name("block-time");
    assert!(decode_and_verify_block_envelope(
        &signed_block_fixture(&identity, 1_700_000_000_100, 1_700_000_000_050),
        1_700_000_000_000,
    )
    .is_err());
}

#[test]
fn node_created_block_code_inspects_with_distinct_bound_destinations() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _guard = runtime.block_on(test_lock().lock());
    let _cwd = isolate_current_dir("block-create-inspect");
    let relay = runtime.block_on(TcpRelayHandle::start());
    let storage = prepare_storage_dir("block-create-inspect");
    let node = Node::new().expect("node");
    node.start(build_config(
        "block-create-inspect",
        storage.as_path(),
        relay.address().as_str(),
    ))
    .expect("start");
    let created = node
        .create_block_onboarding_code(BlockOnboardingDraft {
            network: BlockNetworkSettings {
                tcp_clients: vec!["mesh.example:4242".to_string()],
                broadcast: true,
                hub_mode: HubMode::Autonomous {},
                hub_identity_hash: None,
                hub_api_base_url: None,
                hub_refresh_interval_seconds: 3600,
                radio: None,
            },
            trusted_destination_hashes: Vec::new(),
            preferred_map_layer: PreferredMapLayer::Base {},
            expires_at_ms: now_ms() + 60_000,
        })
        .expect("create");
    let inspection = node
        .inspect_block_onboarding_code(created)
        .expect("inspect created code");
    assert_ne!(
        inspection.issuer_app_destination_hex,
        inspection.issuer_lxmf_destination_hex
    );
    let status = node.get_status();
    assert_eq!(status.app_destination_hex, inspection.issuer_app_destination_hex);
    assert_eq!(status.lxmf_destination_hex, inspection.issuer_lxmf_destination_hex);
    node.stop().expect("stop");
    runtime.block_on(relay.shutdown());
}

#[test]
fn block_onboarding_import_is_atomic_and_preserves_local_secrets() {
    let now = 1_700_000_000_000;
    let identity = reticulum::transport::identity::PrivateIdentity::new_from_name("block-import");
    let encoded = signed_block_fixture(&identity, now, now + 60_000);
    let inspection = decode_and_verify_block_envelope(&encoded, now + 1).expect("inspection");
    let storage = prepare_storage_dir("block-import");
    let store = AppStateStore::new(storage.to_str()).expect("store");
    let mut settings = sample_app_settings();
    settings.hub.api_key = "local-secret".to_string();
    settings.transport_node_enabled = true;
    settings.rnode.peripheral_id = "local-radio".to_string();
    store.set_app_settings(&settings).expect("settings");
    let community = CommunitySettingsRecord {
        household_id: "0123456789abcdef".to_string(),
        household_name: "Imported Household".to_string(),
        adults: 2,
        children: 1,
        pets: 0,
        role_badges: vec!["Medic".to_string()],
        status: HouseholdStatus::AllHome {},
        preferred_map_layer: PreferredMapLayer::Satellite {},
    };
    let tiers = vec![
        BlockPeerTierRecord {
            destination_hex: inspection.issuer_app_destination_hex.clone(),
            circle_tier: CircleTier::Inner {},
        },
        BlockPeerTierRecord {
            destination_hex: "dddddddddddddddddddddddddddddddd".to_string(),
            circle_tier: CircleTier::Outer {},
        },
    ];
    let request = BlockOnboardingImportRequest {
        encoded_text: encoded,
        confirmed_signer_fingerprint: inspection.signer_fingerprint.clone(),
        community,
        peer_tiers: tiers,
    };
    let settings_before_failure = store.get_app_settings().expect("read before failure");
    let peers_before_failure = store.get_saved_peers().expect("peers before failure");
    assert!(store
        .import_block_onboarding_with_injected_transaction_failure(&inspection, &request)
        .is_err());
    assert_eq!(
        serde_json::to_value(store.get_app_settings().expect("settings after rollback"))
            .expect("serialize settings after rollback"),
        serde_json::to_value(settings_before_failure).expect("serialize settings before failure")
    );
    assert_eq!(
        serde_json::to_value(store.get_saved_peers().expect("peers after rollback"))
            .expect("serialize peers after rollback"),
        serde_json::to_value(peers_before_failure).expect("serialize peers before failure")
    );
    let result = store
        .import_block_onboarding_atomic(&inspection, &request)
        .expect("atomic import");
    assert_eq!(result.imported_peer_count, 2);
    let imported = store.get_app_settings().expect("read").expect("settings");
    assert_eq!(imported.hub.api_key, "local-secret");
    assert!(imported.transport_node_enabled);
    assert_eq!(imported.rnode.peripheral_id, "local-radio");
    assert_eq!(imported.community.household_name, "Imported Household");

    let mut incomplete = request;
    incomplete.peer_tiers.pop();
    assert!(store
        .import_block_onboarding_atomic(&inspection, &incomplete)
        .is_err());
    assert_eq!(store.get_saved_peers().expect("peers").len(), 2);
}
