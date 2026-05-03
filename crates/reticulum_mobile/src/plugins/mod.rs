mod abi;
mod abi_c;
mod catalog;
mod host_api;
mod installer;
mod loader;
mod manager;
mod manifest;
mod messages;
mod permissions;
mod registry;

pub use abi::{
    PluginEntrypoints, RemPluginAbiVersion, RemPluginHostApi, RemPluginHostBuffer,
    RemPluginStatusCode, REM_PLUGIN_ABI_VERSION,
};
pub use abi_c::{NativePluginLibrary, NativePluginLoadError, NativePluginMetadata};
pub use catalog::{
    InstalledPluginDescriptor, InstalledPluginSettingsDescriptor, PluginCatalog,
    PluginCatalogDiagnostic, PluginCatalogError, PluginCatalogReport,
};
pub use host_api::{PluginHostApi, PluginHostError, PluginPermissionCheckLog};
pub use installer::{InstalledPlugin, PluginInstaller, PluginInstallerError};
pub use loader::{PluginDiscoveryReport, PluginLoadCandidate, PluginLoader, PluginLoaderError};
pub use manager::{NativePluginRuntime, NativePluginRuntimeError, PluginRuntimeDiagnostic};
pub use manifest::{PluginLibrary, PluginManifest, PluginManifestError, PluginSettings};
pub use messages::{
    validate_plugin_message_payload, PluginLxmfMessage, PluginLxmfMessageError,
    PluginLxmfOutboundRequest, PluginLxmfSendRequest, PluginMessageDescriptor,
    PluginMessageDirection, PluginMessageSchemaMap, PLUGIN_LXMF_FIELD_KEY,
};
pub use permissions::PluginPermissions;
pub use registry::{
    PersistedPluginRegistry, PersistedPluginState, PluginRegistry, PluginRegistryError,
    PluginState, RegisteredPlugin,
};
