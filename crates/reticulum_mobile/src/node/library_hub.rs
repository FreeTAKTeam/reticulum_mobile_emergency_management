// Mission Library RCH source (REM #357): queries over the existing LXMF
// command channel (field 9 -> field 10). Lists are metadata only; bytes move
// only after an explicit Download, over RPP.

/// Command ID prefixes REM's receiver treats as Library answers.
pub(crate) const LIBRARY_LIST_PREFIX: &str = "library-list-";
pub(crate) const LIBRARY_FETCH_PREFIX: &str = "library-fetch-";

/// The fetch command ID carries its item (hex), so the answer finds it even
/// though RCH echoes REM's command ID, not REM's correlation ID.
fn library_fetch_command_id(item_id: &str) -> String {
    format!("{LIBRARY_FETCH_PREFIX}{}-{}", hex::encode(item_id), map_marker_command_id())
}

/// The item a fetch command ID (or a correlation echoing one) was sent for.
pub(crate) fn library_fetch_item_id(command_id: &str) -> Option<String> {
    let (item_hex, _) = command_id.strip_prefix(LIBRARY_FETCH_PREFIX)?.split_once('-')?;
    String::from_utf8(hex::decode(item_hex).ok()?).ok()
}

fn library_hub_fields(
    identity_hex: &str,
    command_id: String,
    command_type: &str,
    args: JsonValue,
    correlation_id: Option<String>,
) -> Result<Vec<u8>, NodeError> {
    let command = r3akt_profile_rch::MissionCommandEnvelope {
        command_id,
        source: r3akt_profile_rch::RchSource::new(identity_hex),
        timestamp: r3akt_map_core::time::iso8601_from_unix_ms(crate::numeric::u64_to_i64_saturating(
            now_ms(),
        )),
        command_type: command_type.to_string(),
        args,
        correlation_id,
        topics: Vec::new(),
    };
    r3akt_profile_rch::encode_commands(&[command]).map_err(|error| {
        crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
    })
}

impl Node {
    /// The configured hub and this node's identity hash; autonomous REM never
    /// asks RCH.
    fn library_hub_target(&self) -> Result<(String, String), NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let config = inner.active_config.as_ref().ok_or(NodeError::NotRunning {})?;
        if matches!(config.hub_mode, HubMode::Autonomous {}) {
            return Err(crate::error_context::contextual_node_error(
                NodeError::InvalidConfig {},
                "the Library hub source needs a configured RCH",
            ));
        }
        let hub = configured_hub_destination(config)?;
        let identity_hex = inner
            .status
            .lock()
            .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
            .identity_hex
            .clone();
        Ok((hub, identity_hex))
    }

    /// Asks the hub for one page of authorized Library metadata.
    pub(crate) fn library_refresh_hub(
        &self,
        query: &r3akt_library_core::LibraryQuery,
    ) -> Result<(), NodeError> {
        let (hub, identity_hex) = self.library_hub_target()?;
        let mut args = serde_json::to_value(query).map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        if let Some(map) = args.as_object_mut() {
            map.remove("source");
        }
        let fields = library_hub_fields(
            &identity_hex,
            format!("{LIBRARY_LIST_PREFIX}{}", map_marker_command_id()),
            r3akt_library_core::LIST_COMMAND,
            args,
            None,
        )?;
        self.queue_bytes_with_class(
            hub,
            b"Library request".to_vec(),
            Some(fields),
            SendMode::DirectOnly {},
            OutboundTrafficClass::Event {},
        )
        .map(|_| ())
    }

    /// Asks the hub to make one listed item available over RPP; the answer
    /// starts the verified download.
    pub(crate) fn library_download_hub(&self, item_id: String) -> Result<(), NodeError> {
        self.library_bulk_admitted()?;
        let (hub, identity_hex) = self.library_hub_target()?;
        let fields = library_hub_fields(
            &identity_hex,
            library_fetch_command_id(&item_id),
            r3akt_library_core::ITEM_FETCH_COMMAND,
            json!({ "item_id": item_id, "rpp_identity": identity_hex }),
            Some(item_id.clone()),
        )?;
        self.queue_bytes_with_class(
            hub,
            b"Library download".to_vec(),
            Some(fields),
            SendMode::DirectOnly {},
            OutboundTrafficClass::Event {},
        )?;
        self.with_app_state(|app_state, bus| {
            app_state.library_set_hub_item_state(&item_id, "downloading")?;
            self.library_changed(bus, app_state, Some(item_id));
            Ok(())
        })
    }
}
