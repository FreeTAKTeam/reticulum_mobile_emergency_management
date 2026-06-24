mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_write_with_response() {
    common::test_cases::test_write_with_response().await;
}
