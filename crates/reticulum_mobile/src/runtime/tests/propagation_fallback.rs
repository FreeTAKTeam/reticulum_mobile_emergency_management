#[test]
fn auto_eligible_recipient_direct_failure_uses_propagation_when_relay_exists() {
    assert!(should_try_propagation_after_direct_failure(
        SendMode::Auto {},
        false,
        true,
        true,
        true,
    ));
    assert!(!should_try_propagation_after_direct_failure(
        SendMode::Auto {},
        false,
        true,
        true,
        false,
    ));
    assert!(!should_try_propagation_after_direct_failure(
        SendMode::DirectOnly {},
        false,
        true,
        true,
        true,
    ));
    assert!(!should_try_propagation_after_direct_failure(
        SendMode::Auto {},
        false,
        false,
        true,
        true,
    ));
    assert!(!should_try_propagation_after_direct_failure(
        SendMode::Auto {},
        false,
        true,
        false,
        true,
    ));
    assert!(!should_try_propagation_after_direct_failure(
        SendMode::Auto {},
        true,
        true,
        true,
        true,
    ));
}
