fn parse_address_hash(hex_32: &str) -> Result<AddressHash, NodeError> {
    let normalized = normalize_hex_32(hex_32).ok_or(NodeError::InvalidConfig {})?;
    AddressHash::new_from_hex_string(&normalized).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InvalidConfig {}, error)
    })
}
