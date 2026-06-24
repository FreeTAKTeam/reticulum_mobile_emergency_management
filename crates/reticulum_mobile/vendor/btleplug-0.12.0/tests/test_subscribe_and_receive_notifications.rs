mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_subscribe_and_receive_notifications() {
    common::test_cases::test_subscribe_and_receive_notifications().await;
}
