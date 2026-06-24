mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_connection_parameters() {
    common::test_cases::test_connection_parameters().await;
}
