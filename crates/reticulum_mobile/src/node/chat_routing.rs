fn routed_chat_destination_hex<F>(
    requested_destination_hex: String,
    active_config: Option<&NodeConfigFingerprint>,
    hub_directory_snapshot: Option<&HubDirectorySnapshot>,
    load_routing_context: F,
) -> Result<String, NodeError>
where
    F: FnOnce() -> Result<(Vec<MessageRecord>, Vec<PeerRecord>), NodeError>,
{
    match routed_destination_hex(
        requested_destination_hex.clone(),
        active_config,
        hub_directory_snapshot,
    ) {
        Ok(destination_hex) => Ok(destination_hex),
        Err(NodeError::InvalidConfig {}) => {
            let (messages, peers) = load_routing_context()?;
            let resolver = conversation_peer_resolver(peers.as_slice());
            let mut destinations = HashSet::from([requested_destination_hex.clone()]);
            destinations.extend(
                resolver
                    .aliases_for_canonical(requested_destination_hex.as_str())
                    .into_iter()
                    .filter_map(|destination| normalize_hex_32(destination.as_str())),
            );
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
