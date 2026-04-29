use reticulum_mobile::plugins::{
    PluginLxmfMessage, PluginLxmfMessageError, PluginManifest, PluginManifestError, PluginRegistry,
    PluginRegistryError, PluginState,
};
use serde_json::json;

const VALID_MANIFEST: &str = r#"
id = "rem.plugin.example_status"
name = "Example Status Plugin"
version = "0.1.0"
rem_api_version = ">=1.0.0,<2.0.0"
plugin_type = "native"

[library.android]
arm64_v8a = "logic/android/arm64-v8a/libexample_status_plugin.so"

[settings]
schema = "ui/settings.schema.json"

[permissions]
storage_plugin = true
lxmf_send = true
lxmf_receive = true

[[messages]]
name = "status_test"
version = "1.0.0"
direction = ["send", "receive"]
schema = "schemas/status_test.schema.json"
"#;

#[test]
fn parses_android_manifest_with_settings_and_lxmf_message_descriptor() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");

    assert_eq!(manifest.id.as_str(), "rem.plugin.example_status");
    assert_eq!(
        manifest
            .android_library_for_abi("arm64-v8a")
            .expect("android arm64 library"),
        "logic/android/arm64-v8a/libexample_status_plugin.so"
    );
    assert!(manifest.permissions.storage_plugin);
    assert!(manifest.permissions.lxmf_send);
    assert!(manifest.permissions.lxmf_receive);
    assert_eq!(
        manifest.messages[0].wire_type(manifest.id.as_str()),
        "plugin.rem.plugin.example_status.status_test"
    );
}

#[test]
fn rejects_non_reverse_dns_plugin_id() {
    let err = PluginManifest::from_toml_str(
        &VALID_MANIFEST.replace("rem.plugin.example_status", "example_status"),
    )
    .expect_err("invalid id is rejected");

    assert!(matches!(err, PluginManifestError::InvalidPluginId { .. }));
}

#[test]
fn rejects_missing_android_library_for_current_abi() {
    let manifest = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "arm64_v8a = \"logic/android/arm64-v8a/libexample_status_plugin.so\"",
        "",
    ))
    .expect("manifest parses without arm64 entry");

    let err = manifest
        .android_library_for_abi("arm64-v8a")
        .expect_err("missing ABI-specific library is rejected");

    assert!(matches!(
        err,
        PluginManifestError::MissingAndroidLibrary { .. }
    ));
}

#[test]
fn rejects_message_name_that_cannot_be_namespaced_safely() {
    let err = PluginManifest::from_toml_str(
        &VALID_MANIFEST.replace("name = \"status_test\"", "name = \"../status\""),
    )
    .expect_err("unsafe message name is rejected");

    assert!(matches!(
        err,
        PluginManifestError::InvalidMessageName { .. }
    ));
}

#[test]
fn builds_host_owned_lxmf_fields_for_declared_plugin_message() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let message = PluginLxmfMessage::new(
        &manifest,
        "status_test",
        json!({
            "status": "ok",
            "batteryPercent": 87
        }),
    )
    .expect("message is declared");

    assert_eq!(
        message.wire_type.as_str(),
        "plugin.rem.plugin.example_status.status_test"
    );

    let fields = message.to_fields_bytes().expect("fields encode");
    let decoded: rmpv::Value = rmp_serde::from_slice(fields.as_slice()).expect("msgpack fields");
    let rmpv::Value::Map(entries) = decoded else {
        panic!("plugin LXMF fields must be a map");
    };
    let payload = entries
        .iter()
        .find(|(key, _)| key.as_str() == Some("rem.plugin.message"))
        .and_then(|(_, value)| value.as_map())
        .expect("plugin message field exists");

    assert!(payload.iter().any(|(key, value)| {
        key.as_str() == Some("plugin_id") && value.as_str() == Some("rem.plugin.example_status")
    }));
    assert!(payload.iter().any(|(key, value)| {
        key.as_str() == Some("message_name") && value.as_str() == Some("status_test")
    }));
}

#[test]
fn rejects_lxmf_message_not_declared_by_plugin() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let err = PluginLxmfMessage::new(&manifest, "unknown", json!({ "status": "ok" }))
        .expect_err("undeclared message is rejected");

    assert!(matches!(
        err,
        PluginLxmfMessageError::UndeclaredMessage { .. }
    ));
}

#[test]
fn registry_discovers_plugins_disabled_by_default() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    let plugin = registry
        .get("rem.plugin.example_status")
        .expect("plugin is registered");

    assert_eq!(plugin.state, PluginState::Disabled);
    assert_eq!(registry.list().len(), 1);
}

#[test]
fn registry_rejects_duplicate_plugin_ids() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let err = PluginRegistry::from_manifests(vec![manifest.clone(), manifest])
        .expect_err("duplicates are rejected");

    assert!(matches!(err, PluginRegistryError::DuplicatePluginId { .. }));
}

#[test]
fn registry_enable_disable_updates_state_without_granting_permissions() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");

    registry
        .enable("rem.plugin.example_status")
        .expect("enable succeeds");
    let plugin = registry
        .get("rem.plugin.example_status")
        .expect("plugin is registered");
    assert_eq!(plugin.state, PluginState::Enabled);
    assert!(!plugin.granted_permissions.lxmf_send);

    registry
        .disable("rem.plugin.example_status")
        .expect("disable succeeds");
    assert_eq!(
        registry
            .get("rem.plugin.example_status")
            .expect("plugin is registered")
            .state,
        PluginState::Disabled
    );
}
