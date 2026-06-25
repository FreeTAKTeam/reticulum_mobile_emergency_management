mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_unsubscribe_stops_notifications() {
    common::test_cases::test_unsubscribe_stops_notifications().await;
}
