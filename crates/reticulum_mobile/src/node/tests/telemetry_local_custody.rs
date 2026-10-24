#[test]
fn telemetry_custody_remains_the_only_owner_across_route_change_and_restart() {
    use crate::types::{LocalPropagationRecord, LocalPropagationState};
    let (node, mut commands, _) = publication_test_node("telemetry-local-custody");
    let app_state = node.inner.lock().unwrap().app_state.clone();
    let mut fix = build_telemetry();
    fix.updated_at_ms = now_ms();
    node.record_local_telemetry_fix(fix.clone()).unwrap();
    let mut inflight = HashMap::new();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    let Command::SendBytes {
        fields_bytes, resp, ..
    } = commands.try_recv().unwrap()
    else {
        panic!("initial publication");
    };
    let command = parse_mission_sync_metadata(&fields_bytes.unwrap())
        .unwrap()
        .command_id
        .unwrap();
    resp.send(Ok(())).unwrap();
    let mut entry = LocalPropagationRecord {
        transient_id_hex: "aa".repeat(32),
        message_id_hex: "bb".repeat(32),
        destination_hex: "cc".repeat(16),
        state: LocalPropagationState::StoredLocally,
        encrypted_bytes: 1024,
        admitted_at_ms: 1,
        expires_at_ms: now_ms() + 1000,
        may_exist_on_relay: false,
    };
    for state in [
        LocalPropagationState::StoredLocally,
        LocalPropagationState::Uploading,
        LocalPropagationState::UploadedUnverified,
        LocalPropagationState::Delivered,
        LocalPropagationState::Expired,
        LocalPropagationState::Evicted,
    ] {
        entry.state = state;
        app_state
            .record_local_telemetry_custody(&command, &entry)
            .unwrap();
        assert!(
            app_state
                .record_local_telemetry_custody(&command, &entry)
                .unwrap()
                .is_none(),
            "replay is idempotent"
        );
        app_state
            .update_telemetry_publication(|publication| publication.recover_uncertain_sends())
            .unwrap();
        {
            let inner = node.inner.lock().unwrap();
            inner
                .sync_status_snapshot
                .lock()
                .unwrap()
                .active_propagation_node_hex = Some("dd".repeat(16));
        }
        node.flush_pending_telemetry(&mut inflight).unwrap();
        assert!(
            commands.try_recv().is_err(),
            "custody owns this fix in {state:?}"
        );
        let publication = app_state.telemetry_publication().unwrap();
        assert_eq!(publication.recipients[0].command_id, command);
        assert_eq!(
            publication.recipients[0].custody_message_id_hex.as_deref(),
            Some(entry.message_id_hex.as_str())
        );
    }
    // Even the regular cadence cannot resubmit the same immutable fix.
    node.inner.lock().unwrap().next_telemetry_publish_at_ms = Some(0);
    node.record_local_telemetry_fix(fix.clone()).unwrap();
    node.flush_pending_telemetry(&mut inflight).unwrap();
    assert!(commands.try_recv().is_err());
    // A newer fix is a new publication and may supersede the old encrypted payload.
    fix.updated_at_ms += 1;
    node.inner.lock().unwrap().next_telemetry_publish_at_ms = Some(0);
    node.record_local_telemetry_fix(fix).unwrap();
    assert!(app_state.telemetry_publication().unwrap().recipients[0]
        .custody_message_id_hex
        .is_none());
}
