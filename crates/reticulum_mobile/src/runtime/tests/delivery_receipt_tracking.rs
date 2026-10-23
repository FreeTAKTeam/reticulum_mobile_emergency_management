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

#[tokio::test]
async fn result_receipt_observed_before_wait_is_not_lost() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let tracker = ReceiptTracker { receipt_message_ids: Arc::new(Mutex::new(HashMap::new())), tx };
    RuntimeReceiptBridge { tracker: tracker.clone() }.on_receipt(&DeliveryReceipt::new([0xab; 32]));
    wait_for_result_receipt(&tracker, &"ab".repeat(32), Duration::from_secs(1)).await.unwrap();
    assert!(tracker.receipt_message_ids.lock().unwrap().is_empty());
}

#[tokio::test]
async fn result_receipt_waiter_is_notified_by_the_validated_bridge() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let tracker = ReceiptTracker { receipt_message_ids: Arc::new(Mutex::new(HashMap::new())), tx };
    let waiting_tracker = tracker.clone();
    let waiting = tokio::spawn(async move {
        wait_for_result_receipt(&waiting_tracker, &"cd".repeat(32), Duration::from_secs(1)).await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if tracker.receipt_message_ids.lock().unwrap().contains_key(&"cd".repeat(32)) { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    RuntimeReceiptBridge { tracker: tracker.clone() }.on_receipt(&DeliveryReceipt::new([0xcd; 32]));
    waiting.await.unwrap().unwrap();
    assert!(tracker.receipt_message_ids.lock().unwrap().is_empty());
}

#[tokio::test]
async fn result_receipt_timeout_removes_its_waiter() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let tracker = ReceiptTracker { receipt_message_ids: Arc::new(Mutex::new(HashMap::new())), tx };
    assert!(matches!(
        wait_for_result_receipt(&tracker, &"ef".repeat(32), Duration::ZERO).await,
        Err(NodeError::Timeout {})
    ));
    assert!(tracker.receipt_message_ids.lock().unwrap().is_empty());
}
