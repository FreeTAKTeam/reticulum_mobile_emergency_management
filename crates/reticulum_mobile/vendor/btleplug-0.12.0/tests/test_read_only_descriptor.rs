mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_read_only_descriptor() {
    common::test_cases::test_read_only_descriptor().await;
}
