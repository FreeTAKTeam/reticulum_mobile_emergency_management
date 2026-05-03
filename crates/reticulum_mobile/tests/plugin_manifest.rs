use reticulum_mobile::plugins::{
    NativePluginLibrary, NativePluginLoadError, NativePluginRuntime, PersistedPluginRegistry,
    PersistedPluginState, PluginCatalog, PluginHostApi, PluginHostError, PluginInstaller,
    PluginInstallerError, PluginLoader, PluginLoaderError, PluginLxmfMessage,
    PluginLxmfMessageError, PluginManifest, PluginManifestError, PluginMessageSchemaMap,
    PluginPermissions, PluginRegistry, PluginRegistryError, PluginState, RemPluginStatusCode,
    REM_PLUGIN_ABI_VERSION,
};
use reticulum_mobile::SendMode;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;

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

fn write_valid_package_archive(archive_path: &Path) {
    let archive_file = fs::File::create(archive_path).expect("archive file is created");
    let mut archive = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default();
    for (relative_path, contents) in [
        ("plugin.toml", VALID_MANIFEST.as_bytes()),
        (
            "logic/android/arm64-v8a/libexample_status_plugin.so",
            b"native".as_slice(),
        ),
        (
            "ui/settings.schema.json",
            br#"{"type":"object"}"#.as_slice(),
        ),
        (
            "schemas/status_test.schema.json",
            br#"{"type":"object"}"#.as_slice(),
        ),
    ] {
        archive
            .start_file(relative_path, options)
            .expect("archive entry starts");
        archive.write_all(contents).expect("archive entry writes");
    }
    archive.finish().expect("archive writes");
}

#[cfg(unix)]
fn write_symlink_package_archive(archive_path: &Path) {
    let archive_file = fs::File::create(archive_path).expect("archive file is created");
    let mut archive = zip::ZipWriter::new(archive_file);
    let symlink_options = SimpleFileOptions::default().unix_permissions(0o120777);
    archive
        .start_file("plugin.toml", symlink_options)
        .expect("symlink archive entry starts");
    archive
        .write_all(b"logic/android/arm64-v8a/libexample_status_plugin.so")
        .expect("symlink archive entry writes");
    archive.finish().expect("archive writes");
}

fn test_dynamic_library_name() -> String {
    format!(
        "{}example_status_plugin.{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_EXTENSION
    )
}

fn compile_test_plugin_library(plugin_dir: &Path, metadata_id: &str, init_status: i32) -> PathBuf {
    let library_relative_path = format!("logic/android/arm64-v8a/{}", test_dynamic_library_name());
    let library_path = plugin_dir.join(library_relative_path);
    let source_path = plugin_dir.join("example_status_plugin.rs");
    if let Some(parent) = library_path.parent() {
        fs::create_dir_all(parent).expect("library parent exists");
    }
    fs::write(
        source_path.as_path(),
        format!(
            r#"
#[repr(C)]
pub struct RemPluginHostApi {{
    pub abi_major: u16,
    pub abi_minor: u16,
}}

#[no_mangle]
pub extern "C" fn rem_plugin_metadata() -> *const std::os::raw::c_char {{
    b"{{\"id\":\"{metadata_id}\",\"name\":\"Example Status Plugin\",\"version\":\"0.1.0\",\"rem_api_version\":\">=1.0.0,<2.0.0\",\"abi_major\":1,\"abi_minor\":0}}\0".as_ptr() as *const std::os::raw::c_char
}}

#[no_mangle]
pub extern "C" fn rem_plugin_init(_host: *const RemPluginHostApi) -> i32 {{
    {init_status}
}}

#[no_mangle]
pub extern "C" fn rem_plugin_start() -> i32 {{
    0
}}

#[no_mangle]
pub extern "C" fn rem_plugin_stop() -> i32 {{
    0
}}

#[no_mangle]
pub extern "C" fn rem_plugin_handle_event(_event: *const std::os::raw::c_char) -> i32 {{
    0
}}
"#
        ),
    )
    .expect("test plugin source is written");

    let output = Command::new("rustc")
        .arg("--crate-type")
        .arg("cdylib")
        .arg(source_path.as_path())
        .arg("-o")
        .arg(library_path.as_path())
        .output()
        .expect("rustc can be launched");
    assert!(
        output.status.success(),
        "test plugin compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(output.stdout.as_slice()),
        String::from_utf8_lossy(output.stderr.as_slice())
    );
    library_path
}

fn compile_event_asserting_test_plugin_library(
    plugin_dir: &Path,
    metadata_id: &str,
    marker_path: &Path,
) -> PathBuf {
    let library_relative_path = format!("logic/android/arm64-v8a/{}", test_dynamic_library_name());
    let library_path = plugin_dir.join(library_relative_path);
    let source_path = plugin_dir.join("example_status_plugin.rs");
    let marker_path = marker_path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    if let Some(parent) = library_path.parent() {
        fs::create_dir_all(parent).expect("library parent exists");
    }
    fs::write(
        source_path.as_path(),
        format!(
            r#"
#[repr(C)]
pub struct RemPluginHostApi {{
    pub abi_major: u16,
    pub abi_minor: u16,
}}

#[no_mangle]
pub extern "C" fn rem_plugin_metadata() -> *const std::os::raw::c_char {{
    b"{{\"id\":\"{metadata_id}\",\"name\":\"Example Status Plugin\",\"version\":\"0.1.0\",\"rem_api_version\":\">=1.0.0,<2.0.0\",\"abi_major\":1,\"abi_minor\":0}}\0".as_ptr() as *const std::os::raw::c_char
}}

#[no_mangle]
pub extern "C" fn rem_plugin_init(_host: *const RemPluginHostApi) -> i32 {{
    0
}}

#[no_mangle]
pub extern "C" fn rem_plugin_start() -> i32 {{
    0
}}

#[no_mangle]
pub extern "C" fn rem_plugin_stop() -> i32 {{
    0
}}

#[no_mangle]
pub extern "C" fn rem_plugin_handle_event(event: *const std::os::raw::c_char) -> i32 {{
    if event.is_null() {{
        return 1;
    }}
    let event = unsafe {{ std::ffi::CStr::from_ptr(event) }}.to_string_lossy();
    if event.contains("\"topic\":\"rem.plugin.lxmf.received\"")
        && event.contains("\"pluginId\":\"{metadata_id}\"")
        && event.contains("\"messageName\":\"status_test\"")
        && event.contains("\"status\":\"ok\"")
    {{
        if std::fs::write("{marker_path}", event.as_bytes()).is_ok() {{
            0
        }} else {{
            1
        }}
    }} else {{
        1
    }}
}}
"#
        ),
    )
    .expect("test plugin source is written");

    let output = Command::new("rustc")
        .arg("--crate-type")
        .arg("cdylib")
        .arg(source_path.as_path())
        .arg("-o")
        .arg(library_path.as_path())
        .output()
        .expect("rustc can be launched");
    assert!(
        output.status.success(),
        "test plugin compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(output.stdout.as_slice()),
        String::from_utf8_lossy(output.stderr.as_slice())
    );
    library_path
}

fn write_dynamic_plugin_manifest(plugin_dir: &Path, library_path: &Path) {
    let relative_path = library_path
        .strip_prefix(plugin_dir)
        .expect("library is inside plugin dir")
        .to_string_lossy()
        .replace('\\', "/");
    write_package_file(
        plugin_dir,
        "plugin.toml",
        VALID_MANIFEST
            .replace(
                "logic/android/arm64-v8a/libexample_status_plugin.so",
                relative_path.as_str(),
            )
            .as_bytes(),
    );
    write_package_file(
        plugin_dir,
        "ui/settings.schema.json",
        br#"{"type":"object"}"#,
    );
    write_package_file(
        plugin_dir,
        "schemas/status_test.schema.json",
        br#"{"type":"object"}"#,
    );
}

fn persisted_enabled_plugin_state(plugin_id: &str) -> PersistedPluginRegistry {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        plugin_id.to_string(),
        PersistedPluginState {
            state: PluginState::Enabled,
            granted_permissions: PluginPermissions::default(),
        },
    );
    PersistedPluginRegistry { plugins }
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
fn native_loader_loads_test_plugin_and_calls_lifecycle() {
    let install_root = TestTempDir::new("native-loader-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        RemPluginStatusCode::Ok as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("plugin discovery completes");
    let candidate = report.candidates.first().expect("plugin is discovered");

    let plugin = NativePluginLibrary::load(candidate).expect("native plugin loads");
    assert_eq!(plugin.metadata().id.as_str(), "rem.plugin.example_status");
    plugin.initialize().expect("plugin init succeeds");
    plugin.start().expect("plugin starts");
    plugin
        .handle_event_json(r#"{"topic":"rem.plugin.started","payload":{}}"#)
        .expect("plugin handles event");
    plugin.stop().expect("plugin stops");
}

#[test]
fn native_loader_rejects_metadata_id_mismatch() {
    let install_root = TestTempDir::new("native-loader-metadata-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.other",
        RemPluginStatusCode::Ok as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("plugin discovery completes");
    let candidate = report.candidates.first().expect("plugin is discovered");
    let err = NativePluginLibrary::load(candidate).expect_err("metadata mismatch is rejected");

    assert!(matches!(
        err,
        NativePluginLoadError::MetadataIdMismatch { .. }
    ));
}

#[test]
fn native_loader_reports_init_failure() {
    let install_root = TestTempDir::new("native-loader-init-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        RemPluginStatusCode::Error as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("plugin discovery completes");
    let candidate = report.candidates.first().expect("plugin is discovered");
    let plugin = NativePluginLibrary::load(candidate).expect("native plugin loads");
    let err = plugin.initialize().expect_err("init failure is reported");

    assert!(matches!(
        err,
        NativePluginLoadError::PluginCallFailed {
            entrypoint: "init",
            status: RemPluginStatusCode::Error
        }
    ));
}

#[test]
fn native_loader_rejects_invalid_status_code() {
    let install_root = TestTempDir::new("native-loader-status-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path =
        compile_test_plugin_library(plugin_dir.as_path(), "rem.plugin.example_status", 99);
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());

    let report = PluginLoader::new(install_root.path())
        .discover_installed_plugins("arm64-v8a")
        .expect("plugin discovery completes");
    let candidate = report.candidates.first().expect("plugin is discovered");
    let plugin = NativePluginLibrary::load(candidate).expect("native plugin loads");
    let err = plugin
        .initialize()
        .expect_err("invalid status code is reported");

    assert!(matches!(
        err,
        NativePluginLoadError::InvalidStatusCode {
            entrypoint: "init",
            status: 99
        }
    ));
}

#[test]
fn native_runtime_keeps_discovered_plugins_disabled_by_default() {
    let install_root = TestTempDir::new("native-runtime-disabled-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        RemPluginStatusCode::Ok as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());

    let mut runtime = NativePluginRuntime::discover(install_root.path(), "arm64-v8a", None)
        .expect("runtime discovers plugins");

    assert_eq!(runtime.loaded_plugin_count(), 0);
    assert_eq!(
        runtime
            .registry()
            .get("rem.plugin.example_status")
            .expect("registered plugin")
            .state,
        PluginState::Disabled
    );
    runtime.start_enabled_plugins();
    assert_eq!(runtime.loaded_plugin_count(), 0);
}

#[test]
fn native_runtime_starts_enabled_plugin_and_stops_it() {
    let install_root = TestTempDir::new("native-runtime-start-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        RemPluginStatusCode::Ok as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());
    let persisted = persisted_enabled_plugin_state("rem.plugin.example_status");

    let mut runtime =
        NativePluginRuntime::discover(install_root.path(), "arm64-v8a", Some(&persisted))
            .expect("runtime discovers plugins");

    runtime.start_enabled_plugins();
    assert_eq!(runtime.loaded_plugin_count(), 1);
    assert_eq!(
        runtime
            .registry()
            .get("rem.plugin.example_status")
            .expect("registered plugin")
            .state,
        PluginState::Running
    );
    runtime.dispatch_event_json(r#"{"topic":"rem.plugin.started","payload":{}}"#);
    assert!(runtime.diagnostics().is_empty());

    runtime.stop_all();
    assert_eq!(runtime.loaded_plugin_count(), 0);
    assert_eq!(
        runtime
            .registry()
            .get("rem.plugin.example_status")
            .expect("registered plugin")
            .state,
        PluginState::Stopped
    );
}

#[test]
fn native_runtime_dispatches_received_lxmf_message_to_owner_plugin() {
    let install_root = TestTempDir::new("native-runtime-lxmf-receive-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    let marker_path = install_root.path().join("received-plugin-event.json");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_event_asserting_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        marker_path.as_path(),
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());
    let mut persisted = persisted_enabled_plugin_state("rem.plugin.example_status");
    persisted
        .plugins
        .get_mut("rem.plugin.example_status")
        .expect("persisted plugin exists")
        .granted_permissions
        .lxmf_receive = true;

    let mut runtime =
        NativePluginRuntime::discover(install_root.path(), "arm64-v8a", Some(&persisted))
            .expect("runtime discovers plugins");
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let message = PluginLxmfMessage::new_for_direction(
        &manifest,
        "status_test",
        json!({ "status": "ok" }),
        reticulum_mobile::plugins::PluginMessageDirection::Receive,
    )
    .expect("received plugin message builds");

    runtime.start_enabled_plugins();
    runtime.dispatch_lxmf_message_received(&message);

    assert!(runtime.diagnostics().is_empty());
    let delivered_event =
        fs::read_to_string(marker_path.as_path()).expect("plugin receives event marker");
    assert!(delivered_event.contains("\"topic\":\"rem.plugin.lxmf.received\""));
    assert_eq!(
        runtime
            .registry()
            .get("rem.plugin.example_status")
            .expect("registered plugin")
            .state,
        PluginState::Running
    );
}

#[test]
fn native_runtime_marks_init_failure_failed() {
    let install_root = TestTempDir::new("native-runtime-fail-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    let library_path = compile_test_plugin_library(
        plugin_dir.as_path(),
        "rem.plugin.example_status",
        RemPluginStatusCode::Error as i32,
    );
    write_dynamic_plugin_manifest(plugin_dir.as_path(), library_path.as_path());
    let persisted = persisted_enabled_plugin_state("rem.plugin.example_status");

    let mut runtime =
        NativePluginRuntime::discover(install_root.path(), "arm64-v8a", Some(&persisted))
            .expect("runtime discovers plugins");

    runtime.start_enabled_plugins();
    assert_eq!(runtime.loaded_plugin_count(), 0);
    assert_eq!(
        runtime
            .registry()
            .get("rem.plugin.example_status")
            .expect("registered plugin")
            .state,
        PluginState::Failed
    );
    assert_eq!(runtime.diagnostics().len(), 1);
    assert_eq!(
        runtime.diagnostics()[0].plugin_id.as_deref(),
        Some("rem.plugin.example_status")
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
fn rejects_lxmf_message_send_when_direction_is_not_declared() {
    let manifest = PluginManifest::from_toml_str(&VALID_MANIFEST.replace(
        "direction = [\"send\", \"receive\"]",
        "direction = [\"receive\"]",
    ))
    .expect("manifest parses");

    let err = PluginLxmfMessage::new(&manifest, "status_test", json!({ "status": "ok" }))
        .expect_err("receive-only message cannot be sent");

    assert!(matches!(
        err,
        PluginLxmfMessageError::DirectionNotAllowed { .. }
    ));
}

#[test]
fn decodes_host_owned_lxmf_fields_for_declared_receive_message() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let outgoing = PluginLxmfMessage::new(
        &manifest,
        "status_test",
        json!({
            "status": "ok",
            "batteryPercent": 87
        }),
    )
    .expect("message is declared");
    let fields = outgoing.to_fields_bytes().expect("fields encode");

    let decoded =
        PluginLxmfMessage::from_fields_bytes(&manifest, fields.as_slice()).expect("fields decode");

    assert_eq!(decoded.plugin_id.as_str(), "rem.plugin.example_status");
    assert_eq!(decoded.message_name.as_str(), "status_test");
    assert_eq!(decoded.payload["status"], "ok");
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
fn persisted_registry_round_trips_to_disk() {
    let temp = TestTempDir::new("persisted-registry");
    let path = temp.path().join("plugins").join("registry.json");
    let mut persisted = persisted_enabled_plugin_state("rem.plugin.example_status");
    persisted
        .plugins
        .get_mut("rem.plugin.example_status")
        .expect("persisted plugin exists")
        .granted_permissions
        .lxmf_send = true;

    persisted
        .save_to_path(path.as_path())
        .expect("registry state saves");
    let loaded =
        PersistedPluginRegistry::load_from_path(path.as_path()).expect("registry state loads");

    let plugin = loaded
        .plugins
        .get("rem.plugin.example_status")
        .expect("plugin state persisted");
    assert_eq!(plugin.state, PluginState::Enabled);
    assert!(plugin.granted_permissions.lxmf_send);
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
    assert_eq!(host.permission_checks().len(), 1);
    assert_eq!(
        host.permission_checks()[0].plugin_id,
        "rem.plugin.example_status"
    );
    assert_eq!(host.permission_checks()[0].action, "set_plugin_storage");
    assert_eq!(host.permission_checks()[0].permission, "storage.plugin");
    assert!(!host.permission_checks()[0].allowed);
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
    assert_eq!(host.permission_checks().len(), 2);
    assert!(host
        .permission_checks()
        .iter()
        .all(|entry| entry.allowed && entry.permission == "storage.plugin"));
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
fn host_api_builds_outbound_lxmf_request_for_granted_plugin_message() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_send = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    let request = host
        .request_lxmf_send_to(
            "rem.plugin.example_status",
            "aabbccddeeff00112233445566778899",
            "status_test",
            json!({ "status": "ok" }),
            "Status test from example plug-in",
            Some("Status Test".to_string()),
            SendMode::PropagationOnly {},
        )
        .expect("request builds");

    assert_eq!(
        request.destination_hex.as_str(),
        "aabbccddeeff00112233445566778899"
    );
    assert_eq!(
        request.wire_type.as_str(),
        "plugin.rem.plugin.example_status.status_test"
    );
    assert_eq!(
        request.body_utf8.as_str(),
        "Status test from example plug-in"
    );
    assert!(matches!(request.send_mode, SendMode::PropagationOnly {}));
    assert_eq!(host.queued_lxmf_outbound_requests().len(), 1);

    let decoded = PluginLxmfMessage::from_fields_bytes(
        &PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses"),
        request.fields_bytes.as_slice(),
    )
    .expect("fields decode");
    assert_eq!(decoded.payload["status"], "ok");
}

#[test]
fn host_api_validates_lxmf_send_payload_against_message_schema() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_send = true;
        })
        .expect("grant succeeds");
    let mut schemas = PluginMessageSchemaMap::new();
    schemas.insert(
        (
            "rem.plugin.example_status".to_string(),
            "status_test".to_string(),
        ),
        json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": { "type": "string", "minLength": 1 }
            },
            "additionalProperties": false
        }),
    );
    let mut host = PluginHostApi::new_with_message_schemas(registry, schemas);

    let err = host
        .request_lxmf_send_to(
            "rem.plugin.example_status",
            "aabbccddeeff00112233445566778899",
            "status_test",
            json!({ "status": "" }),
            "Status test from example plug-in",
            None,
            SendMode::Auto {},
        )
        .expect_err("invalid payload is rejected");

    assert!(matches!(
        err,
        PluginHostError::LxmfMessage(PluginLxmfMessageError::InvalidPayload { .. })
    ));
    assert!(host.queued_lxmf_outbound_requests().is_empty());
}

#[test]
fn host_api_drains_queued_outbound_lxmf_requests() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_send = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    host.request_lxmf_send_to(
        "rem.plugin.example_status",
        "aabbccddeeff00112233445566778899",
        "status_test",
        json!({ "status": "ok" }),
        "Status test from example plug-in",
        None,
        SendMode::Auto {},
    )
    .expect("request builds");

    assert_eq!(host.drain_queued_lxmf_outbound_requests().len(), 1);
    assert!(host.queued_lxmf_outbound_requests().is_empty());
}

#[test]
fn host_api_denies_lxmf_receive_without_grant() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let message = PluginLxmfMessage::new(&manifest, "status_test", json!({ "status": "ok" }))
        .expect("message builds");
    let fields = message.to_fields_bytes().expect("fields encode");
    let registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    let mut host = PluginHostApi::new(registry);

    let err = host
        .receive_lxmf_fields(fields.as_slice())
        .expect_err("ungranted lxmf receive is denied");

    assert!(matches!(
        err,
        PluginHostError::PermissionDenied {
            permission: "lxmf.receive",
            ..
        }
    ));
}

#[test]
fn host_api_accepts_granted_declared_lxmf_receive_message() {
    let manifest = PluginManifest::from_toml_str(VALID_MANIFEST).expect("manifest parses");
    let message = PluginLxmfMessage::new(&manifest, "status_test", json!({ "status": "ok" }))
        .expect("message builds");
    let fields = message.to_fields_bytes().expect("fields encode");
    let mut registry = PluginRegistry::from_manifests(vec![manifest]).expect("registry builds");
    registry
        .grant_permissions("rem.plugin.example_status", |permissions| {
            permissions.lxmf_receive = true;
        })
        .expect("grant succeeds");
    let mut host = PluginHostApi::new(registry);

    let received = host
        .receive_lxmf_fields(fields.as_slice())
        .expect("receive succeeds")
        .expect("plugin message envelope exists");

    assert_eq!(received.message_name.as_str(), "status_test");
    assert_eq!(
        host.received_lxmf_messages("rem.plugin.example_status")
            .len(),
        1
    );
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
    assert_eq!(host.permission_checks().len(), 1);
    assert_eq!(host.permission_checks()[0].action, "subscribe");
    assert_eq!(host.permission_checks()[0].permission, "messages.read");
    assert!(!host.permission_checks()[0].allowed);
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
    assert!(host
        .permission_checks()
        .iter()
        .any(|entry| entry.action == "subscribe"
            && entry.permission == "messages.read"
            && entry.allowed));
    assert!(host
        .permission_checks()
        .iter()
        .any(|entry| entry.action == "deliver_event"
            && entry.permission == "messages.read"
            && entry.allowed));
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
fn installer_extracts_valid_archive_disabled_by_default() {
    let package_dir = TestTempDir::new("archive-package");
    let install_root = TestTempDir::new("archive-install-root");
    let archive_path = package_dir.path().join("example-status.remplugin");
    write_valid_package_archive(archive_path.as_path());

    let installed = PluginInstaller::new(install_root.path())
        .install_from_archive(archive_path.as_path(), "arm64-v8a")
        .expect("archive installs");

    assert_eq!(installed.manifest.id.as_str(), "rem.plugin.example_status");
    assert_eq!(installed.state, PluginState::Disabled);
    assert!(installed.install_dir.join("plugin.toml").is_file());
    assert!(!install_root
        .path()
        .join(format!(".archive-extract-{}", std::process::id()))
        .exists());
}

#[test]
fn installer_rejects_archive_path_traversal() {
    let package_dir = TestTempDir::new("archive-traversal-package");
    let install_root = TestTempDir::new("archive-traversal-install-root");
    let archive_path = package_dir.path().join("bad.remplugin");
    let archive_file = fs::File::create(archive_path.as_path()).expect("archive file is created");
    let mut archive = zip::ZipWriter::new(archive_file);
    archive
        .start_file("../plugin.toml", SimpleFileOptions::default())
        .expect("archive entry starts");
    archive
        .write_all(VALID_MANIFEST.as_bytes())
        .expect("archive entry writes");
    archive.finish().expect("archive writes");

    let err = PluginInstaller::new(install_root.path())
        .install_from_archive(archive_path.as_path(), "arm64-v8a")
        .expect_err("path traversal is rejected");

    assert!(matches!(
        err,
        PluginInstallerError::InvalidPackagePath { .. }
    ));
    assert!(!install_root
        .path()
        .join("rem.plugin.example_status")
        .exists());
}

#[cfg(unix)]
#[test]
fn installer_rejects_archive_symlink_entries() {
    let package_dir = TestTempDir::new("archive-symlink-package");
    let install_root = TestTempDir::new("archive-symlink-install-root");
    let archive_path = package_dir.path().join("bad.remplugin");
    write_symlink_package_archive(archive_path.as_path());

    let err = PluginInstaller::new(install_root.path())
        .install_from_archive(archive_path.as_path(), "arm64-v8a")
        .expect_err("symlink entry is rejected");

    assert!(matches!(
        err,
        PluginInstallerError::InvalidPackagePath { .. }
    ));
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

#[test]
fn catalog_lists_installed_plugin_with_settings_schema() {
    let install_root = TestTempDir::new("catalog-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    write_valid_package(plugin_dir.as_path());

    let report = PluginCatalog::new(install_root.path())
        .list_installed_plugins("arm64-v8a")
        .expect("catalog lists plugins");

    assert!(report.errors.is_empty());
    assert_eq!(report.items.len(), 1);
    let plugin = &report.items[0];
    assert_eq!(plugin.id.as_str(), "rem.plugin.example_status");
    assert_eq!(
        plugin.library_path.as_str(),
        "logic/android/arm64-v8a/libexample_status_plugin.so"
    );
    assert_eq!(
        plugin
            .settings
            .as_ref()
            .expect("settings descriptor")
            .schema["type"],
        "object"
    );
}

#[test]
fn catalog_applies_persisted_plugin_state_and_grants() {
    let install_root = TestTempDir::new("catalog-persisted-root");
    let plugin_dir = install_root.path().join("rem.plugin.example_status");
    fs::create_dir_all(plugin_dir.as_path()).expect("plugin dir exists");
    write_valid_package(plugin_dir.as_path());
    let mut persisted = persisted_enabled_plugin_state("rem.plugin.example_status");
    persisted
        .plugins
        .get_mut("rem.plugin.example_status")
        .expect("persisted plugin exists")
        .granted_permissions
        .lxmf_send = true;

    let report = PluginCatalog::new(install_root.path())
        .list_installed_plugins_with_state("arm64-v8a", Some(&persisted))
        .expect("catalog lists plugins");

    assert!(report.errors.is_empty());
    let plugin = report.items.first().expect("plugin listed");
    assert_eq!(plugin.state, PluginState::Enabled);
    assert!(plugin.granted_permissions.lxmf_send);
    assert!(plugin.permissions.lxmf_send);
}
