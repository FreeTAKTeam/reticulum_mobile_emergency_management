mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_write_without_response() {
    common::test_cases::test_write_without_response().await;
}
