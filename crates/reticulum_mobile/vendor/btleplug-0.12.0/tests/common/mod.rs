#[allow(dead_code)]
pub mod gatt_uuids;
pub mod peripheral_finder;
pub mod test_cases;

/// Find a descriptor by UUID on a specific characteristic.
pub fn find_descriptor(
    peripheral: &btleplug::platform::Peripheral,
    char_uuid: uuid::Uuid,
    descriptor_uuid: uuid::Uuid,
) -> btleplug::api::Descriptor {
    use btleplug::api::Peripheral as _;
    let services = peripheral.services();
    for service in &services {
        for char in &service.characteristics {
            if char.uuid == char_uuid {
                for desc in &char.descriptors {
                    if desc.uuid == descriptor_uuid {
                        return desc.clone();
                    }
                }
            }
        }
    }
    panic!(
        "descriptor {} not found on characteristic {}",
        descriptor_uuid, char_uuid
    );
}
