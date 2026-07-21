#[test]
fn eam_readiness_score_bands_and_colors_match_dashboard_contract() {
    assert_eq!(eam_status_score("Green"), 100);
    assert_eq!(eam_status_score("Yellow"), 50);
    assert_eq!(eam_status_score("Red"), 25);
    assert_eq!(eam_status_score("Unknown"), 0);
    assert_eq!(eam_status_score("Offline"), 0);

    assert_eq!(readiness_band(75), "Green");
    assert_eq!(readiness_band(74), "Yellow");
    assert_eq!(readiness_band(50), "Yellow");
    assert_eq!(readiness_band(49), "Orange");
    assert_eq!(readiness_band(25), "Orange");
    assert_eq!(readiness_band(24), "Red");

    assert_eq!(readiness_ring_color(0), "#ff3648");
    assert_eq!(readiness_ring_color(25), "#ff9f1c");
    assert_eq!(readiness_ring_color(50), "#f5cc19");
    assert_eq!(readiness_ring_color(75), "#16ce79");
    assert_eq!(readiness_ring_color(100), "#3df58f");
}

#[test]
fn eam_readiness_summary_aggregates_active_records_and_excludes_deleted() {
    let green = readiness_eam(
        "Green-1",
        ["Green", "Green", "Green", "Green", "Green", "Green"],
        100,
        None,
    );
    let mixed = readiness_eam(
        "Mixed-1",
        ["Red", "Yellow", "Unknown", "Green", "Yellow", "Red"],
        200,
        None,
    );
    let deleted = readiness_eam(
        "Deleted-1",
        ["Red", "Red", "Red", "Red", "Red", "Red"],
        300,
        Some(300),
    );

    let summary = build_eam_readiness_summary(vec![green, mixed, deleted]);

    assert_eq!(summary.active_total, 2);
    assert_eq!(summary.updated_at_ms, 300);
    assert_eq!(summary.messages.len(), 2);
    assert!(summary
        .messages
        .iter()
        .all(|message| message.callsign != "Deleted-1"));
    assert_eq!(summary.status_metrics[0].field, "securityStatus");
    assert_eq!(summary.status_metrics[0].score, 63);
    assert_eq!(summary.status_metrics[1].field, "capabilityStatus");
    assert_eq!(summary.status_metrics[1].score, 75);
    assert_eq!(summary.status_metrics[2].field, "preparednessStatus");
    assert_eq!(summary.status_metrics[2].score, 50);
    assert_eq!(summary.status_metrics[5].field, "commsStatus");
    assert_eq!(summary.status_metrics[5].score, 63);
}

#[test]
fn eam_readiness_summary_returns_neutral_metrics_when_empty() {
    let summary = build_eam_readiness_summary(Vec::new());

    assert_eq!(summary.active_total, 0);
    assert_eq!(summary.updated_at_ms, 0);
    assert!(summary.messages.is_empty());
    assert_eq!(summary.status_metrics.len(), 6);
    for metric in summary.status_metrics {
        assert_eq!(metric.score, 0);
        assert_eq!(metric.band, "Red");
        assert_eq!(metric.ring_color, "#ff3648");
    }
}

fn app_settings_with_due_step(default_task_due_step_minutes: u32) -> AppSettingsRecord {
    AppSettingsRecord {
        display_name: "Test Operator".to_string(),
        auto_connect_saved: true,
        announce_capabilities: "R3AKT,EMergencyMessages".to_string(),
        tcp_clients: Vec::new(),
        broadcast: true,
        transport_node_enabled: true,
        announce_interval_seconds: 1800,
        telemetry: TelemetrySettingsRecord {
            enabled: false,
            publish_interval_seconds: 60,
            accuracy_threshold_meters: None,
            stale_after_minutes: 30,
            expire_after_minutes: 180,
        },
        hub: HubSettingsRecord {
            mode: HubMode::Autonomous {},
            identity_hash: String::new(),
            api_base_url: String::new(),
            api_key: String::new(),
            refresh_interval_seconds: 3600,
        },
        teams: crate::types::TeamSettingsRecord::default(),
        checklists: ChecklistSettingsRecord {
            default_task_due_step_minutes,
        },
        rnode: crate::types::RnodeSettingsRecord::default(),
    }
}
