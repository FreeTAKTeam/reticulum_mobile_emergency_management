mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_read_counter_increments() {
    common::test_cases::test_read_counter_increments().await;
}
