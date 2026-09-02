#[test]
fn replication_delivery_failure_preserves_the_underlying_error_code() {
    let bus = EventBus::new();
    let events = bus.subscribe();

    emit_replication_delivery_failure(
        &bus,
        "event replication delivery failed destination=e3f1c3f63adef0c0d771ddff8b0eeed5 uid=event-1 reason=network error"
            .to_string(),
        &NodeError::NetworkError {},
    );

    let event = events.try_recv().expect("replication failure event");
    match event {
        NodeEvent::Error { code, message } => {
            assert_eq!(code, "NetworkError");
            assert!(message.contains("destination=e3f1c3f63adef0c0d771ddff8b0eeed5"));
            assert!(message.contains("uid=event-1"));
        }
        other => panic!("expected per-destination replication error, got {other:?}"),
    }
}
