use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::types::{
    AnnounceClass, AnnounceRecord, AppSettingsRecord, ApplicationAckState, ChecklistCellRecord,
    ChecklistColumnRecord, ChecklistColumnType, ChecklistCreateFromTemplateRequest,
    ChecklistMode, ChecklistOriginType, ChecklistRecord, ChecklistSettingsRecord,
    ChecklistStatusCounts, ChecklistSystemColumnKey, ChecklistTaskCellSetRequest,
    ChecklistTaskRecord, ChecklistTaskRowAddRequest, ChecklistTaskRowDeleteRequest,
    ChecklistTaskRowStyleSetRequest, ChecklistTaskStatus, ChecklistTaskStatusSetRequest,
    ChecklistTemplateImportCsvRequest, ChecklistUpdatePatch, ChecklistUpdateRequest,
    ChecklistUserTaskStatus, HubMode, HubSettingsRecord, MessageDirection, MessageMethod,
    MessageState, ProjectionScope, SosAlertRecord, SosLocationRecord, SosMessageKind,
    TelemetrySettingsRecord, TransportDeliveryState,
};
use serde_json::json;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_storage_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "reticulum-mobile-app-state-{name}-{}-{}",
        std::process::id(),
        TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    path
}

fn discovered_plugin(fingerprint: &str) -> DiscoveredPluginRecord {
    DiscoveredPluginRecord {
        plugin_id: "org.freetakteam.rem.plugin.test".to_string(),
        display_name: "Test Plugin".to_string(),
        version: "1.0.0".to_string(),
        api_major: 1,
        api_minor: 0,
        package_name: "org.freetakteam.rem.plugin.test".to_string(),
        service_class_name: ".TestPluginService".to_string(),
        publisher_fingerprint: fingerprint.to_string(),
        publisher_history: Vec::new(),
        android_permissions: Vec::new(),
        declared_capabilities: PluginCapabilityRecord {
            sensors_publish: true,
            lxmf_send: true,
            ..PluginCapabilityRecord::default()
        },
        messages: Vec::new(),
        configuration_entrypoint: Some("rem-plugin-config/index.html".to_string()),
    }
}
use std::path::PathBuf;
