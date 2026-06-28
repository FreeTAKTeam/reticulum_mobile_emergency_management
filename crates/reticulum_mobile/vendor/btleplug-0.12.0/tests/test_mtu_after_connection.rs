mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_mtu_after_connection() {
    common::test_cases::test_mtu_after_connection().await;
}
