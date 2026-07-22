#[test]
fn receipt_observed_before_registration_is_delivered_to_message_listener() {
    let receipt_hash = "ab".repeat(32);
    let tracking = Arc::new(Mutex::new(HashMap::from([(
        receipt_hash.clone(),
        ReceiptMessageTracking::Observed {
            recorded_at_ms: now_ms(),
        },
    )])));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let tracker = ReceiptTracker {
        receipt_message_ids: tracking.clone(),
        tx,
    };

    register_receipt_tracking(
        &tracker,
        Some(receipt_hash.as_str()),
        "message-id",
    );

    assert_eq!(rx.try_recv().expect("receipt notification"), "message-id");
    assert!(!tracking.lock().expect("tracking lock").contains_key(&receipt_hash));
}

#[test]
fn receipt_registration_waits_for_a_future_transport_proof() {
    let receipt_hash = "cd".repeat(32);
    let tracking = Arc::new(Mutex::new(HashMap::new()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let tracker = ReceiptTracker {
        receipt_message_ids: tracking.clone(),
        tx,
    };

    register_receipt_tracking(
        &tracker,
        Some(receipt_hash.as_str()),
        "message-id",
    );

    assert!(rx.try_recv().is_err());
    assert!(matches!(
        tracking.lock().expect("tracking lock").get(&receipt_hash),
        Some(ReceiptMessageTracking::Pending { message_id_hex, .. })
            if message_id_hex == "message-id"
    ));
}
