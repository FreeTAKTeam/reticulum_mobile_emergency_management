mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_subscribe_and_receive_indications() {
    common::test_cases::test_subscribe_and_receive_indications().await;
}
