mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_peripheral_triggered_disconnect() {
    common::test_cases::test_peripheral_triggered_disconnect().await;
}
