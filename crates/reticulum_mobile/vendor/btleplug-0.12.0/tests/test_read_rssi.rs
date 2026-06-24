mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_read_rssi() {
    common::test_cases::test_read_rssi().await;
}
