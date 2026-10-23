#[test]
fn telemetry_m2_new_fix_replaces_uncertain_delivery_before_cadence() {
    for state in [
        PublicationState::TimedOut,
        PublicationState::Failed,
        PublicationState::QueuedNoRoute,
        PublicationState::QueuedForDelivery,
        PublicationState::Transmitted,
        PublicationState::TransportDelivered,
        PublicationState::Acknowledged,
        PublicationState::Rejected,
    ] {
        let (node, mut commands, peer) =
            publication_test_node(&format!("telemetry-m2-coalesce-{state:?}"));
        let app_state = node.inner.lock().unwrap().app_state.clone();
        {
            let inner = node.inner.lock().unwrap();
            inner
                .status
                .lock()
                .unwrap()
                .interfaces
                .push(crate::types::InterfaceStatusRecord {
                    interface_hex: "e".repeat(32),
                    label: "rnode-ble:test".to_string(),
                    kind: "RNode".to_string(),
                    state: "ready".to_string(),
                    last_error: None,
                    rx_packets: 0,
                    rx_bytes: 0,
                    last_activity_ms: 0,
                });
        }
        let mut fix = build_telemetry();
        fix.updated_at_ms = now_ms();
        node.record_local_telemetry_fix(fix.clone()).unwrap();
        let mut inflight = HashMap::new();
        node.flush_pending_telemetry(&mut inflight).unwrap();
        let Command::SendBytes {
            fields_bytes, resp, ..
        } = commands.try_recv().unwrap()
        else {
            panic!("first telemetry send");
        };
        let old_command = parse_mission_sync_metadata(&fields_bytes.unwrap())
            .unwrap()
            .command_id
            .unwrap();
        resp.send(Ok(())).unwrap();
        node.inner.lock().unwrap().next_telemetry_publish_at_ms = Some(now_ms() + 600_000);
        app_state
            .update_telemetry_publication(|publication| {
                let row = &mut publication.recipients[0];
                row.state = state;
                row.message_id_hex = Some("a".repeat(64));
                row.representation = Some("Packet".to_string());
                row.detail = Some("previous attempt".to_string());
                row.attempt_route = Some("previous-route".to_string());
            })
            .unwrap();

        node.record_local_telemetry_fix(fix.clone()).unwrap();
        let unchanged = app_state.telemetry_publication().unwrap();
        assert_eq!(
            unchanged.recipients[0].state, state,
            "same fix must not reset {state:?}"
        );
        assert_eq!(unchanged.recipients[0].command_id, old_command);
        assert_eq!(
            unchanged.recipients[0].attempt_route.as_deref(),
            Some("previous-route")
        );

        fix.updated_at_ms += 1;
        fix.lat += 0.01;
        node.record_local_telemetry_fix(fix.clone()).unwrap();
        if matches!(
            state,
            PublicationState::Acknowledged | PublicationState::Rejected
        ) {
            let prior = app_state.telemetry_publication().unwrap();
            assert_eq!(prior.recipients[0].state, state);
            assert_eq!(prior.recipients[0].command_id, old_command);
            node.flush_pending_telemetry(&mut inflight).unwrap();
            assert!(
                commands.try_recv().is_err(),
                "completed results retain normal cadence"
            );
            node.inner.lock().unwrap().next_telemetry_publish_at_ms = Some(0);
            node.record_local_telemetry_fix(fix.clone()).unwrap();
        }
        let pending = app_state.telemetry_publication().unwrap();
        assert_eq!(pending.fix, Some(fix.clone()));
        let row = &pending.recipients[0];
        assert_eq!(
            row.fix_updated_at_ms, fix.updated_at_ms,
            "new fix must replace {state:?}"
        );
        assert_eq!(row.state, PublicationState::QueuedNoRoute);
        assert!(row.command_id.is_empty());
        assert!(row.message_id_hex.is_none() && row.representation.is_none());
        assert!(row.detail.is_none() && row.attempt_route.is_none());

        app_state
            .record_telemetry_delivery(&crate::types::LxmfDeliveryUpdate {
                message_id_hex: "a".repeat(64),
                destination_hex: peer.lxmf_destination_hex.clone().unwrap(),
                source_hex: None,
                command_id: Some(old_command.clone()),
                correlation_id: Some(old_command.clone()),
                command_type: Some("mission.registry.telemetry.upsert".to_string()),
                event_uid: None,
                mission_uid: None,
                status: crate::types::LxmfDeliveryStatus::Acknowledged {},
                transport_state: TransportDeliveryState::TransportDelivered {},
                application_ack_state: ApplicationAckState::Accepted {},
                method: crate::types::LxmfDeliveryMethod::Direct {},
                representation: crate::types::LxmfDeliveryRepresentation::Packet {},
                relay_destination_hex: None,
                fallback_stage: None,
                detail: None,
                sent_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            })
            .unwrap();
        assert_eq!(
            app_state.telemetry_publication().unwrap().recipients[0].state,
            PublicationState::QueuedNoRoute
        );

        // The peer and route are unchanged; replacement must not wait for the periodic deadline.
        node.flush_pending_telemetry(&mut inflight).unwrap();
        let Command::SendBytes {
            destination_hex,
            bytes,
            fields_bytes,
            resp,
            ..
        } = commands.try_recv().unwrap()
        else {
            panic!("replacement telemetry send");
        };
        assert_eq!(Some(destination_hex), peer.lxmf_destination_hex);
        assert_eq!(bytes, b"T");
        let next_command = parse_mission_sync_metadata(&fields_bytes.unwrap())
            .unwrap()
            .command_id
            .unwrap();
        assert_ne!(next_command, old_command);
        assert_eq!(
            app_state.telemetry_publication().unwrap().recipients[0].command_id,
            next_command
        );
        resp.send(Ok(())).unwrap();
    }
}
