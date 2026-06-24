mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_characteristic_properties() {
    common::test_cases::test_characteristic_properties().await;
}
