mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_advertisement_manufacturer_data() {
    common::test_cases::test_advertisement_manufacturer_data().await;
}
