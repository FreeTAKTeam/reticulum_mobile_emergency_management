use reticulum_mobile::plugins::{
    PluginHostApi, PluginHostError, PluginInstaller, PluginInstallerError, PluginLoader,
    PluginLoaderError, PluginLxmfMessage, PluginLxmfMessageError, PluginManifest,
    PluginManifestError, PluginRegistry, PluginRegistryError, PluginState, RemPluginStatusCode,
    REM_PLUGIN_ABI_VERSION,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(label: &str) -> Self {
        let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rem-plugin-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(path.as_path()).expect("temp dir is created");
        Self { path }
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.path.as_path());
    }
}

fn write_package_file(package_dir: &Path, relative_path: &str, contents: &[u8]) {
    let path = package_dir.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory is created");
    }
    fs::write(path, contents).expect("package file is written");
}

fn write_valid_package(package_dir: &Path) {
    write_package_file(package_dir, "plugin.toml", VALID_MANIFEST.as_bytes());
    write_package_file(
        package_dir,
        "logic/android/arm64-v8a/libexample_status_plugin.so",
        b"native",
    );
    write_package_file(
        package_dir,
        "ui/settings.schema.json",
        br#"{"type":"object"}"#,
    );
    write_package_file(
        package_dir,
        "schemas/status_test.schema.json",
        br#"{"type":"object"}"#,
    );
}

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
    assert_eq!(
        manifest.entrypoints.metadata.as_str(),
        "rem_plugin_metadata"
    );
    assert_eq!(manifest.entrypoints.init.as_str(), "rem_plugin_init");
    assert_eq!(manifest.entrypoints.start.as_str(), "rem_plugin_start");
    assert_eq!(manifest.entrypoints.stop.as_str(), "rem_plugin_stop");
    assert_eq!(
        manifest.entrypoints.handle_event.as_str(),
        "rem_plugin_handle_event"
    );
}

#[test]
fn c_abi_version_and_status_codes_are_stable() {
    assert_eq!(REM_PLUGIN_ABI_VERSION.major, 1);
    assert_eq!(REM_PLUGIN_ABI_VERSION.minor, 0);
    assert_eq!(RemPluginStatusCode::Ok as i32, 0);
    assert_eq!(RemPluginStatusCode::Error as i32, 1);
    assert_eq!(RemPluginStatusCode::PermissionDenied as i32, 2);
    assert_eq!(RemPluginStatusCode::UnsupportedApi as i32, 3);
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

#[test]
fn registry_persists_state_and_granted_permissions_separately() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");

    registry
        .enable("rem.plugin.example_status")
        .expect("enable succeeds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_send = true;
            permissions.storage_plugin = true;
        })
        .expect("grant succeeds");

    let persisted = registry.to_persisted_state();
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut restored = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    restored.apply_persisted_state(&persisted);
    let plugin = restored
        .get("rem.plugin.example_status")
        .expect("plugin is restored");

    assert_eq!(plugin.state, PluginState::Enabled);
    assert!(plugin.granted_permissions.lxmf_send);
    assert!(plugin.granted_permissions.storage_plugin);
}

#[test]
fn registry_does_not_restore_grants_for_undeclared_permissions() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");

    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.messages_write = true;
        })
        .expect("grant succeeds");

    let plugin = registry
        .get("rem.plugin.example_status")
        .expect("plugin exists");
    assert!(!plugin.granted_permissions.messages_write);
}

#[test]
fn host_api_denies_plugin_storage_without_grant() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    let mut host = PluginHostApi::new(registry);

    let err = host
        .set_plugin_storage("rem.plugin.example_status", "callsign", json!("alpha"))
        .expect_err("ungranted storage permission is denied");

    assert!(matches!(
        err,
        PluginHostError::PermissionDenied {
            permission: "storage.plugin",
            ..
        }
    ));
}

#[test]
fn host_api_allows_granted_plugin_local_storage() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.storage_plugin = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    host.set_plugin_storage("rem.plugin.example_status", "callsign", json!("alpha"))
        .expect("storage write succeeds");

    assert_eq!(
        host.get_plugin_storage("rem.plugin.example_status", "callsign")
            .expect("storage read succeeds"),
        Some(json!("alpha"))
    );
}

#[test]
fn host_api_denies_lxmf_send_without_grant() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    let mut host = PluginHostApi::new(registry);

    let err = host
        .request_lxmf_send(
            "rem.plugin.example_status",
            "status_test",
            json!({ "status": "ok" }),
        )
        .expect_err("ungranted lxmf send is denied");

    assert!(matches!(
        err,
        PluginHostError::PermissionDenied {
            permission: "lxmf.send",
            ..
        }
    ));
}

#[test]
fn host_api_builds_lxmf_message_for_granted_declared_message() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_send = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    let message = host
        .request_lxmf_send(
            "rem.plugin.example_status",
            "status_test",
            json!({ "status": "ok" }),
        )
        .expect("message request succeeds");

    assert_eq!(
        message.wire_type.as_str(),
        "plugin.rem.plugin.example_status.status_test"
    );
    assert_eq!(host.queued_lxmf_messages().len(), 1);
}

#[test]
fn host_api_denies_message_subscription_without_grant() {
    let manifest = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "lxmf_receive = true",
        "lxmf_receive = true\nmessages_read = true",
    ))
    .expect("manifest parses");
    let registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    let mut host = PluginHostApi::new(registry);

    let err = host
        .subscribe("rem.plugin.example_status", "rem.message.received")
        .expect_err("ungranted message read is denied");

    assert!(matches!(
        err,
        PluginHostError::PermissionDenied {
            permission: "messages.read",
            ..
        }
    ));
}

#[test]
fn host_api_delivers_events_only_to_subscribed_granted_plugins() {
    let manifest = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "lxmf_receive = true",
        "lxmf_receive = true\nmessages_read = true",
    ))
    .expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.messages_read = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    host.subscribe("rem.plugin.example_status", "rem.message.received")
        .expect("subscription succeeds");
    host.deliver_event("rem.message.received", json!({ "body": "hello" }))
        .expect("event delivery succeeds");
    host.deliver_event("rem.telemetry.updated", json!({ "callsign": "alpha" }))
        .expect("unsubscribed event is ignored");

    let inbox = host.plugin_events("rem.plugin.example_status");
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].topic.as_str(), "rem.message.received");
}

#[test]
fn installer_copies_valid_package_disabled_by_default() {
    let package_dir = TestTempDir::new("package");
    let install_root = TestTempDir::new("install-root");
    write_valid_package(package_dir.path());

    let installed = PluginInstaller::new(install_root.path())
        .install_from_package_dir(package_dir.path(), "arm64-v8a")
        .expect("package installs");

    assert_eq!(installed.manifest.id.as_str(), "rem.plugin.example_status");
    assert_eq!(installed.state, PluginState::Disabled);
    assert!(installed
        .install_dir
        .join("logic/android/arm64-v8a/libexample_status_plugin.so")
        .is_file());
    assert!(installed.install_dir.starts_with(install_root.path()));
}

#[test]
fn installer_rejects_package_missing_current_abi_library() {
    let package_dir = TestTempDir::new("missing-library");
    let install_root = TestTempDir::new("install-root");
    write_package_file(package_dir.path(), "plugin.toml", VALID_MANIFEST.as_bytes());
    write_package_file(package_dir.path(), "ui/settings.schema.json", br#"{}"#);

    let err = PluginInstaller::new(install_root.path())
        .install_from_package_dir(package_dir.path(), "arm64-v8a")
        .expect_err("missing native library is rejected");

    assert!(matches!(
        err,
        PluginInstallerError::MissingPackageFile { .. }
    ));
}

#[test]
fn installer_rejects_missing_settings_schema() {
    let package_dir = TestTempDir::new("missing-settings-schema");
    let install_root = TestTempDir::new("install-root");
    write_package_file(package_dir.path(), "plugin.toml", VALID_MANIFEST.as_bytes());
    write_package_file(
        package_dir.path(),
        "logic/android/arm64-v8a/libexample_status_plugin.so",
        b"native",
    );

    let err = PluginInstaller::new(install_root.path())
        .install_from_package_dir(package_dir.path(), "arm64-v8a")
        .expect_err("missing settings schema is rejected");

    assert!(matches!(
        err,
        PluginInstallerError::MissingPackageFile { .. }
    ));
}

#[test]
fn installer_rejects_missing_message_schema() {
    let package_dir = TestTempDir::new("missing-message-schema");
    let install_root = TestTempDir::new("install-root");
    write_package_file(package_dir.path(), "plugin.toml", VALID_MANIFEST.as_bytes());
    write_package_file(
        package_dir.path(),
        "logic/android/arm64-v8a/libexample_status_plugin.so",
        b"native",
    );
    write_package_file(package_dir.path(), "ui/settings.schema.json", br#"{}"#);

    let err = PluginInstaller::new(install_root.path())
        .install_from_package_dir(package_dir.path(), "arm64-v8a")
        .expect_err("missing message schema is rejected");

    assert!(matches!(
        err,
        PluginInstallerError::MissingPackageFile { .. }
    ));
}

#[test]
fn rejects_settings_schema_path_traversal() {
    let err = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "schema = \"ui/settings.schema.json\"",
        "schema = \"../settings.schema.json\"",
    ))
    .expect_err("settings schema path traversal is rejected");

    assert!(matches!(
        err,
        PluginManifestError::InvalidSettingsPath { .. }
    ));
}

#[test]
fn rejects_message_schema_path_traversal() {
    let err = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "schema = \"schemas/status_test.schema.json\"",
        "schema = \"../status_test.schema.json\"",
    ))
    .expect_err("message schema path traversal is rejected");

    assert!(matches!(
        err,
        PluginManifestError::InvalidMessageSchemaPath { .. }
    ));
}

#[test]
fn loader_discovers_installed_plugin_for_android_abi() {
    let install_root = TestTempDir::new("loader-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    write_valid_package(plugin_dir.as_path());

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("discovery completes");

    assert!(report.errors.is_empty());
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(
        report.candidates[0].library_path,
        plugin_dir.join("logic/android/arm64-v8a/libexample_status_plugin.so")
    );
}

#[test]
fn loader_reports_missing_installed_library_without_panicking() {
    let install_root = TestTempDir::new("loader-missing-library");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    write_package_file(
        plugin_dir.as_path(),
        "plugin.toml",
        VALID_MANIFEST.as_bytes(),
    );
    write_package_file(plugin_dir.as_path(), "ui/settings.schema.json", br#"{}"#);

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("discovery completes");

    assert!(report.candidates.is_empty());
    assert!(matches!(
        report.errors.first().expect("loader error"),
        PluginLoaderError::MissingLibrary { .. }
    ));
}
