mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_advertisement_services() {
    common::test_cases::test_advertisement_services().await;
}
