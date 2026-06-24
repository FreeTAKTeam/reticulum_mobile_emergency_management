mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_long_value_read_write() {
    common::test_cases::test_long_value_read_write().await;
}
