struct InboundChecklistCommand<'a> {
    app_state: &'a AppStateStore,
    bus: &'a EventBus,
    command_map: &'a [(MsgPackValue, MsgPackValue)],
    args: &'a [(MsgPackValue, MsgPackValue)],
    timestamp: String,
    source_identity: Option<String>,
    content_bytes: Option<&'a [u8]>,
}

fn persist_received_checklist_if_present(
    app_state: &AppStateStore,
    bus: &EventBus,
    _metadata: Option<&MissionSyncMetadata>,
    fields_bytes: Option<&[u8]>,
    content_bytes: Option<&[u8]>,
) -> bool {
    let Some(fields_bytes) = fields_bytes else {
        return false;
    };
    let fields = match rmp_serde::from_slice::<MsgPackValue>(fields_bytes) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let Some(field_entries) = msgpack_map_entries(&fields) else {
        return false;
    };
    let Some(commands) = msgpack_get_indexed(field_entries, FIELD_COMMANDS) else {
        return false;
    };
    let MsgPackValue::Array(command_entries) = commands else {
        return false;
    };

    let mut handled_any = false;
    for command in command_entries {
        let command_map_storage;
        let args_storage;
        let (command_map, args_override) = if let Some(command_map) = msgpack_map_entries(command) {
            (command_map, None)
        } else if let Some((command_type, args)) = positional_checklist_command_args(command) {
            command_map_storage = vec![(MsgPackValue::from("t"), MsgPackValue::from(command_type))];
            args_storage = args;
            (command_map_storage.as_slice(), Some(args_storage.as_slice()))
        } else {
            continue;
        };
        let Some(command_type) = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())
        else {
            continue;
        };
        if !command_type.starts_with("checklist.") {
            continue;
        }
        let timestamp = msgpack_get_named(command_map, &["timestamp", "ts"])
            .and_then(msgpack_timestamp)
            .unwrap_or_else(current_timestamp_rfc3339);
        let source_identity = checklist_command_source_identity(command_map);
        let map_args = msgpack_get_named(command_map, &["args", "a"])
            .and_then(msgpack_map_entries)
            .unwrap_or(command_map);
        let args = args_override.unwrap_or(map_args);
        let context = InboundChecklistCommand {
            app_state,
            bus,
            command_map,
            args,
            timestamp,
            source_identity,
            content_bytes,
        };
        handled_any |= match command_type.as_str() {
            "checklist.create.online" => handle_inbound_checklist_create(&context),
            "checklist.upload" => handle_inbound_checklist_upload(&context),
            "checklist.update" => handle_inbound_checklist_update(&context),
            "checklist.delete" => handle_inbound_checklist_delete(&context),
            "checklist.task.row.add" => handle_inbound_checklist_row_add(&context),
            "checklist.task.row.delete" => handle_inbound_checklist_row_delete(&context),
            "checklist.task.status.set" => handle_inbound_checklist_status(&context),
            "checklist.task.row.style.set" => handle_inbound_checklist_row_style(&context),
            "checklist.task.cell.set" => handle_inbound_checklist_cell(&context),
            "checklist.join" => handle_inbound_checklist_join(&context),
            _ => false,
        };
    }
    handled_any
}

include!("checklist_handlers/create.rs");
include!("checklist_handlers/upload.rs");
include!("checklist_handlers/update.rs");
include!("checklist_handlers/delete.rs");
include!("checklist_handlers/row_add.rs");
include!("checklist_handlers/row_delete.rs");
include!("checklist_handlers/status.rs");
include!("checklist_handlers/row_style.rs");
include!("checklist_handlers/cell.rs");
include!("checklist_handlers/join.rs");
