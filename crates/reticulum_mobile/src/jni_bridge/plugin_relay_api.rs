fn handle_plugin_relay_operation(
    node: &Node,
    plugin_id: &str,
    operation: &str,
    payload: &Value,
) -> Option<PluginMapResult> {
    if matches!(
        operation,
        "relay.identity"
            | "relay.chat.accept"
            | "relay.chat.sent"
            | "relay.chat.pending"
            | "relay.event.accept"
    ) {
        return Some((|| {
            let plugin = node
                .list_plugins()
                .map_err(plugin_map_node_error)?
                .into_iter()
                .find(|plugin| {
                    plugin.discovered.plugin_id == plugin_id
                        && crate::app_state::relay_allowed(plugin)
                })
                .ok_or_else(|| plugin_map_denied("Communication relay grant required"))?;
            if operation == "relay.identity" {
                return node
                    .relay_identity(plugin_id)
                    .map_err(plugin_map_node_error);
            }
            if operation == "relay.event.accept"
                && !(plugin.discovered.declared_capabilities.events_publish
                    && plugin.granted_capabilities.events_publish)
            {
                return Err(plugin_map_denied("Event publication grant required"));
            }
            let input = serde_json::from_value::<crate::node::RelayChatInput>(payload.clone())
                .map_err(|error| plugin_map_invalid(error.to_string()))?;
            if operation == "relay.chat.accept" {
                let status = node.get_status();
                if input.destination_hex != status.lxmf_destination_hex
                    && input.destination_hex != status.app_destination_hex
                    && !(plugin.discovered.declared_capabilities.lxmf_send
                        && plugin.granted_capabilities.lxmf_send)
                {
                    return Err((
                        "RecipientNotLocal",
                        "Chat is for another recipient and forwarding is not granted".to_string(),
                    ));
                }
            }
            if operation == "relay.event.accept" {
                node.accept_relay_event(plugin_id, input)
            } else {
                node.accept_relay_chat(
                    plugin_id,
                    input,
                    operation != "relay.chat.accept",
                    operation == "relay.chat.sent",
                )
            }
            .map_err(|error| match error {
                NodeError::Timeout {} => (
                    "SubmissionUnconfirmed",
                    "A previous relay submission is unconfirmed; it was not retried".to_string(),
                ),
                other => plugin_map_node_error(other),
            })
        })());
    }
    if operation != "relay.poll" {
        return None;
    }
    Some((|| {
        let cursor = payload
            .get("cursor")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| plugin_map_invalid("cursor must be an unsigned integer"))
            })
            .transpose()?;
        let limit = payload.get("limit").map_or(Ok(16), |value| {
            value
                .as_u64()
                .filter(|value| (1..=16).contains(value))
                .ok_or_else(|| plugin_map_invalid("limit must be 1–16"))
        })?;
        node.poll_plugin_relay(plugin_id, cursor, usize::try_from(limit).map_err(|_| plugin_map_invalid("limit must fit the platform"))?).map_err(|error| match error {
            NodeError::InvalidConfig {} => plugin_map_denied("relay.poll requires API 1.5 and communicationsRelay declared and granted to a trusted enabled plugin"),
            other => plugin_map_node_error(other),
        })
    })())
}
