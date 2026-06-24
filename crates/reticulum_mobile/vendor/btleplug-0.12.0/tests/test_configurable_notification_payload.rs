mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_configurable_notification_payload() {
    common::test_cases::test_configurable_notification_payload().await;
}
