mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_scan_filter_by_service_uuid() {
    common::test_cases::test_scan_filter_by_service_uuid().await;
}
