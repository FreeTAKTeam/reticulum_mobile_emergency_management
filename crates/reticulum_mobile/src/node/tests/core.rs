#[test]
fn countdown_guard_rejects_deactivated_or_replaced_incident() {
    let deadline = now_ms().saturating_add(5_000);
    let pending = countdown_status(
        "incident-a".to_string(),
        SosTriggerSource::Manual {},
        deadline,
    );
    assert!(is_pending_sos_countdown_for_incident(
        &pending,
        "incident-a"
    ));
    assert!(!is_pending_sos_countdown_for_incident(
        &pending,
        "incident-b"
    ));
    assert!(!is_pending_sos_countdown_for_incident(
        &idle_status(),
        "incident-a"
    ));
}
