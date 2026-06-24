mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_reconnect_after_disconnect() {
    common::test_cases::test_reconnect_after_disconnect().await;
}
