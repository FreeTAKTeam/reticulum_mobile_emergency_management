mod common;

#[tokio::test]
#[ignore = "requires BLE test peripheral"]
async fn test_read_write_descriptor_roundtrip() {
    common::test_cases::test_read_write_descriptor_roundtrip().await;
}
