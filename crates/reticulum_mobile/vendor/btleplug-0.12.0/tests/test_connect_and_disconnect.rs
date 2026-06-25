mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_connect_and_disconnect() {
    common::test_cases::test_connect_and_disconnect().await;
}
