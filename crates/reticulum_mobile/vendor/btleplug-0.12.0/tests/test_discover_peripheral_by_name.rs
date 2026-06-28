mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_discover_peripheral_by_name() {
    common::test_cases::test_discover_peripheral_by_name().await;
}
