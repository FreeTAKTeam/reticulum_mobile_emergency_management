use super::*;
use crate::runtime::lxmf_private_identity;
use lxmf::message::{Payload, WireMessage as LxmfWireMessage};
use reticulum::transport::destination::SingleOutputDestination;
use reticulum::transport::identity::EncryptIdentity;

const LXMF_DELIVERY_NAME: (&str, &str) = ("lxmf", "delivery");

fn lxmf_identity(
    identity: &reticulum::transport::identity::Identity,
) -> lxmf::identity::Identity {
    lxmf::identity::Identity::new_from_slices(
        identity.public_key_bytes(),
        identity.verifying_key_bytes(),
    )
}

#[test]
fn propagation_get_response_binary_array_parses_payloads() {
    let value = rmpv::Value::Array(vec![
        rmpv::Value::Binary(vec![1, 2, 3]),
        rmpv::Value::Binary(vec![4, 5, 6]),
    ]);

    let parsed = rmpv_binary_array(&value).expect("binary array");

    assert_eq!(parsed, vec![vec![1, 2, 3], vec![4, 5, 6]]);
    assert!(rmpv_binary_array(&rmpv::Value::Nil).is_err());
    assert!(rmpv_binary_array(&rmpv::Value::Array(vec![rmpv::Value::Nil])).is_err());
}

#[test]
fn propagation_fetch_payload_array_accepts_binary_and_id_payload_pairs() {
    let value = rmpv::Value::Array(vec![
        rmpv::Value::Binary(vec![1, 2, 3]),
        rmpv::Value::Array(vec![
            rmpv::Value::Binary(vec![0xAA; 32]),
            rmpv::Value::Binary(vec![4, 5, 6]),
        ]),
    ]);

    let parsed = rmpv_propagation_payload_array(&value).expect("payload array");

    assert_eq!(parsed, vec![vec![1, 2, 3], vec![4, 5, 6]]);
}

#[test]
fn propagation_fetch_payload_array_unwraps_msgpack_envelopes() {
    let first_transient = vec![0x11; 48];
    let second_transient = vec![0x22; 64];
    let third_transient = vec![0x33; 80];
    let third_envelope = rmpv::Value::Array(vec![
        rmpv::Value::F64(1_779_000_002.0),
        rmpv::Value::Array(vec![rmpv::Value::Binary(third_transient.clone())]),
    ]);
    let first_envelope = rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::F64(1_779_000_000.0),
        rmpv::Value::Array(vec![rmpv::Value::Binary(first_transient.clone())]),
    ]))
    .expect("encode first envelope");
    let second_envelope = rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::F64(1_779_000_001.0),
        rmpv::Value::Array(vec![rmpv::Value::Binary(second_transient.clone())]),
    ]))
    .expect("encode second envelope");
    let value = rmpv::Value::Array(vec![
        rmpv::Value::Binary(first_envelope),
        rmpv::Value::Array(vec![
            rmpv::Value::Binary(vec![0xAA; 32]),
            rmpv::Value::Binary(second_envelope),
        ]),
        rmpv::Value::Array(vec![rmpv::Value::Binary(vec![0xBB; 32]), third_envelope]),
    ]);

    let parsed = rmpv_propagation_payload_array(&value).expect("payload array");

    assert_eq!(
        parsed,
        vec![first_transient, second_transient, third_transient]
    );
}

#[test]
fn propagation_link_response_frame_rejects_malformed_payloads() {
    let request_id = [0x11; 16];
    let valid = rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::Binary(request_id.to_vec()),
        rmpv::Value::Binary(vec![0x22]),
    ]))
    .expect("encode response");

    let parsed = parse_link_response_frame(valid.as_slice()).expect("valid response");
    assert_eq!(parsed.0, request_id);
    assert_eq!(parsed.1, rmpv::Value::Binary(vec![0x22]));

    let wrong_shape = rmp_serde::to_vec(&rmpv::Value::Array(vec![rmpv::Value::Binary(
        request_id.to_vec(),
    )]))
    .expect("encode malformed response");
    assert!(parse_link_response_frame(wrong_shape.as_slice()).is_none());

    let wrong_id_len = rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::Binary(vec![0x11; 15]),
        rmpv::Value::Binary(vec![0x22]),
    ]))
    .expect("encode malformed response");
    assert!(parse_link_response_frame(wrong_id_len.as_slice()).is_none());
}

#[test]
fn propagation_response_destination_accepts_link_or_destination() {
    let expected_destination = AddressHash::new([0xAA; 16]);
    let expected_link_id = AddressHash::new([0xBB; 16]);

    assert!(link_response_destination_matches(
        expected_link_id,
        expected_destination,
        expected_link_id,
    ));
    assert!(link_response_destination_matches(
        expected_destination,
        expected_destination,
        expected_link_id,
    ));
    assert!(!link_response_destination_matches(
        AddressHash::new([0xCC; 16]),
        expected_destination,
        expected_link_id,
    ));
}

#[test]
fn propagation_fetch_limit_truncates_transient_ids() {
    let mut ids = vec![vec![1], vec![2], vec![3]];

    apply_fetch_limit(&mut ids, Some(2));

    assert_eq!(ids, vec![vec![1], vec![2]]);
    apply_fetch_limit(&mut ids, None);
    assert_eq!(ids, vec![vec![1], vec![2]]);
}

#[test]
fn propagation_fetch_batches_are_bounded() {
    let ids = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];

    let batches = propagation_fetch_batches(ids.as_slice());

    assert_eq!(
        batches,
        vec![
            vec![vec![1]],
            vec![vec![2]],
            vec![vec![3]],
            vec![vec![4]],
            vec![vec![5]]
        ]
    );
}

#[test]
fn propagation_purge_batches_are_bounded() {
    let ids = (0u8..20).map(|value| vec![value]).collect::<Vec<_>>();

    let batches = propagation_purge_batches(ids.as_slice());

    assert_eq!(batches.len(), 3);
    assert!(batches
        .iter()
        .all(|batch| batch.len() <= PROPAGATION_PURGE_BATCH_SIZE));
    assert_eq!(
        batches[0],
        (0u8..8).map(|value| vec![value]).collect::<Vec<_>>()
    );
    assert_eq!(
        batches[2],
        (16u8..20).map(|value| vec![value]).collect::<Vec<_>>()
    );
}

#[test]
fn propagation_decrypt_failures_with_transient_ids_are_purged() {
    let mut purge_queue = Vec::new();

    let retained = queue_fetched_transient_id_for_purge(&mut purge_queue, Some(vec![0xAA]));

    assert!(!retained);
    assert_eq!(purge_queue, vec![vec![0xAA]]);

    let retained = queue_fetched_transient_id_for_purge(&mut purge_queue, None);

    assert!(retained);
    assert_eq!(purge_queue, vec![vec![0xAA]]);
}

#[test]
fn propagated_payload_decrypts_only_for_local_destination() {
    let receiver = PrivateIdentity::new_from_name("propagation-sync-receiver");
    let sender = PrivateIdentity::new_from_name("propagation-sync-sender");
    let other = PrivateIdentity::new_from_name("propagation-sync-other");
    let mut destination = [0u8; 16];
    destination.copy_from_slice(receiver.address_hash().as_slice());
    let mut source = [0u8; 16];
    source.copy_from_slice(sender.address_hash().as_slice());
    let payload = Payload::new(
        1_779_000_000.0,
        Some(b"sync-content".to_vec()),
        Some(b"sync-title".to_vec()),
        None,
        None,
    );
    let mut wire = LxmfWireMessage::new(destination, source, payload);
    wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
        .expect("sign wire");
    let packed = wire.pack().expect("pack wire");
    let (transient, _) = wire
        .pack_propagation_transient_with_rng(&lxmf_identity(receiver.as_identity()), OsRng)
        .expect("pack transient");

    let decrypted =
        decrypt_local_propagated_wire(&receiver, receiver.address_hash(), transient.as_slice())
            .expect("decrypt local propagated wire");

    assert_eq!(decrypted, packed);
    assert!(decrypt_local_propagated_wire(
        &receiver,
        other.address_hash(),
        transient.as_slice()
    )
    .is_err());
}

#[test]
fn propagated_payload_decrypt_accepts_delivery_hash_salt() {
    let receiver = PrivateIdentity::new_from_name("propagation-delivery-salt-receiver");
    let sender = PrivateIdentity::new_from_name("propagation-delivery-salt-sender");
    let delivery_hash = AddressHash::new_from_hex_string("42424242424242424242424242424242")
        .expect("delivery hash");
    assert_ne!(delivery_hash.as_slice(), receiver.address_hash().as_slice());
    let payload = b"delivery-hash-salted-propagation";
    let receiver_public = PublicKey::from(*receiver.as_identity().public_key.as_bytes());
    let derived_key = sender.derive_key(&receiver_public, Some(delivery_hash.as_slice()));
    let mut token_buf = vec![0u8; payload.len() + 256];
    let token = sender
        .encrypt(OsRng, payload, &derived_key, &mut token_buf)
        .expect("encrypt with delivery hash salt");

    let mut transient = Vec::new();
    transient.extend_from_slice(delivery_hash.as_slice());
    transient.extend_from_slice(sender.as_identity().public_key.as_bytes());
    transient.extend_from_slice(token);

    let decrypted =
        decrypt_local_propagated_wire(&receiver, &delivery_hash, transient.as_slice())
            .expect("delivery hash salt fallback should decrypt");

    assert_eq!(&decrypted[16..], payload);
}

#[test]
fn propagated_payload_decrypts_lxmf_delivery_destination_hash() {
    let receiver = PrivateIdentity::new_from_name("propagation-lxmf-delivery-receiver");
    let sender = PrivateIdentity::new_from_name("propagation-lxmf-delivery-sender");
    let delivery_destination = SingleOutputDestination::new(
        *receiver.as_identity(),
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    );
    let delivery_hash = delivery_destination.desc.address_hash;
    assert_ne!(delivery_hash.as_slice(), receiver.address_hash().as_slice());
    let mut destination = [0u8; 16];
    destination.copy_from_slice(delivery_hash.as_slice());
    let mut source = [0u8; 16];
    source.copy_from_slice(sender.address_hash().as_slice());
    let payload = Payload::new(
        1_779_000_050.0,
        Some(b"delivery-destination-content".to_vec()),
        Some(b"delivery-destination-title".to_vec()),
        None,
        None,
    );
    let mut wire = LxmfWireMessage::new(destination, source, payload);
    wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
        .expect("sign wire");
    let packed = wire.pack().expect("pack wire");
    let envelope = wire
        .pack_propagation_with_rng(
            &lxmf_identity(receiver.as_identity()),
            1_779_000_050.0,
            OsRng,
        )
        .expect("pack envelope");
    let payloads = propagation_payloads_from_bytes(envelope.as_slice());

    let decrypted =
        decrypt_local_propagated_wire(&receiver, &delivery_hash, payloads[0].as_slice())
            .expect("decrypt delivery-destination propagated wire");

    assert_eq!(decrypted, packed);
}

#[test]
fn propagated_payload_decrypt_error_identifies_destination_mismatch() {
    let receiver = PrivateIdentity::new_from_name("propagation-error-receiver");
    let other = PrivateIdentity::new_from_name("propagation-error-other");
    let mut payload = vec![0u8; 16 + 32 + 1];
    payload[..16].copy_from_slice(other.address_hash().as_slice());

    let err =
        decrypt_local_propagated_wire(&receiver, receiver.address_hash(), payload.as_slice())
            .expect_err("wrong destination should fail before decrypt");

    assert!(err
        .to_string()
        .contains("destination prefix mismatch expected="));
    assert!(err
        .to_string()
        .contains(receiver.address_hash().to_hex_string().as_str()));
    assert!(err
        .to_string()
        .contains(other.address_hash().to_hex_string().as_str()));
}

#[test]
fn propagated_envelope_from_fetch_response_decrypts_for_local_destination() {
    let receiver = PrivateIdentity::new_from_name("propagation-envelope-receiver");
    let sender = PrivateIdentity::new_from_name("propagation-envelope-sender");
    let mut destination = [0u8; 16];
    destination.copy_from_slice(receiver.address_hash().as_slice());
    let mut source = [0u8; 16];
    source.copy_from_slice(sender.address_hash().as_slice());
    let payload = Payload::new(
        1_779_000_100.0,
        Some(b"enveloped-sync-content".to_vec()),
        Some(b"enveloped-sync-title".to_vec()),
        None,
        None,
    );
    let mut wire = LxmfWireMessage::new(destination, source, payload);
    wire.sign(&lxmf_private_identity(&sender).expect("lxmf signer"))
        .expect("sign wire");
    let packed = wire.pack().expect("pack wire");
    let envelope = wire
        .pack_propagation_with_rng(
            &lxmf_identity(receiver.as_identity()),
            1_779_000_100.0,
            OsRng,
        )
        .expect("pack envelope");
    let fetched = rmpv::Value::Array(vec![rmpv::Value::Binary(envelope)]);
    let payloads = rmpv_propagation_payload_array(&fetched).expect("payloads");

    assert_eq!(payloads.len(), 1);
    let decrypted = decrypt_local_propagated_wire(
        &receiver,
        receiver.address_hash(),
        payloads[0].as_slice(),
    )
    .expect("decrypt local propagated wire");

    assert_eq!(decrypted, packed);
}
