fn routed_chat_destination_hex(
    requested_destination_hex: String,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
    messages: &[MessageRecord],
) -> Result<String, NodeError> {
    match routed_destination_hex(
        requested_destination_hex.clone(),
        active_config,
        hub_directory_snapshot,
    ) {
        Ok(destination_hex) => Ok(destination_hex),
        Err(NodeError::InvalidConfig {}) => {
            let destinations = HashSet::from([requested_destination_hex.clone()]);
            if messages.iter().any(|message| {
                delivery_policy::inbound_message_matches_destinations(message, &destinations)
            }) {
                Ok(requested_destination_hex)
            } else {
                Err(NodeError::InvalidConfig {})
            }
        }
        Err(error) => Err(error),
    }
}
