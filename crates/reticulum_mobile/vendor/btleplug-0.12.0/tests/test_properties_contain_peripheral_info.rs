mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_properties_contain_peripheral_info() {
    common::test_cases::test_properties_contain_peripheral_info().await;
}
