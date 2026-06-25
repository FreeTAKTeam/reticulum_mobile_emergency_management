mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_descriptor_discovery() {
    common::test_cases::test_descriptor_discovery().await;
}
