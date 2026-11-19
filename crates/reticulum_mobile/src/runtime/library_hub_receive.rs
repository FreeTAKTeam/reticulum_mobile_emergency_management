// Mission Library RCH answers (REM #357). Only the configured hub is trusted.
// A list answer caches metadata; a fetch answer starts the verified RPP pull.
// A hub that lacks the commands answers with a rejection, which is recorded
// as the hub's Library status instead of being treated as success.

fn persist_received_library_answer(
    state: &NodeRuntimeState,
    bus: &EventBus,
    source: Option<&str>,
    fields_bytes: Option<&[u8]>,
) -> bool {
    let hub = state
        .config
        .hub_identity_hash
        .as_deref()
        .and_then(crate::delivery_policy::normalize_hex_32);
    let (Some(source), Some(fields_bytes)) = (source, fields_bytes) else {
        return false;
    };
    if hub.as_deref() != Some(source) {
        return false;
    }
    let Ok(results) = r3akt_profile_rch::decode_results(fields_bytes) else {
        return false;
    };
    let mut handled = false;
    for result in results {
        if result.status == r3akt_profile_rch::CommandResultStatus::Accepted {
            continue;
        }
        let ok = result.status != r3akt_profile_rch::CommandResultStatus::Rejected;
        let reason = result.reason_code.clone().unwrap_or_else(|| "rejected".to_string());
        if result.command_id.starts_with(crate::node::LIBRARY_LIST_PREFIX) {
            handled = true;
            let page: Option<r3akt_library_core::LibraryPage> =
                serde_json::from_value(result.result.clone()).ok().filter(|_| ok);
            let outcome = match page {
                Some(page) => state
                    .app_state
                    .library_cache_hub_items(&page.items)
                    .and_then(|()| state.app_state.library_set_hub_status("ok")),
                None => state.app_state.library_set_hub_status(&reason),
            };
            if let Err(error) = outcome {
                warn!("[library] hub list not stored: {error}");
            }
        } else if result.command_id.starts_with(crate::node::LIBRARY_FETCH_PREFIX) {
            handled = true;
            let Some(item_id) = result.correlation_id.clone() else { continue };
            let package = result
                .result
                .pointer("/rpp/package_destination")
                .and_then(serde_json::Value::as_str)
                .and_then(|text| hex::decode(text).ok())
                .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
                .filter(|_| ok);
            let started = match (package, library_share()) {
                (Some(package), Ok(share)) => {
                    share.download_from_hub(item_id.clone(), package);
                    true
                }
                _ => false,
            };
            if !started {
                info!("[library] hub refused download of {item_id}: {reason}");
                if let Err(error) = state.app_state.library_set_hub_item_state(&item_id, "failed") {
                    warn!("[library] hub item state not stored: {error}");
                }
            }
        }
    }
    if handled {
        library_share_changed(&state.app_state, bus, None);
    }
    handled
}
