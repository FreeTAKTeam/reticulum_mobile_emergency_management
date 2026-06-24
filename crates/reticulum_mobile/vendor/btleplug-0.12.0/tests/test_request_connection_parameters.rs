mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_request_connection_parameters() {
    common::test_cases::test_request_connection_parameters().await;
}
