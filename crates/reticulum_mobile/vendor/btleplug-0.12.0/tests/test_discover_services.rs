mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_discover_services() {
    common::test_cases::test_discover_services().await;
}
