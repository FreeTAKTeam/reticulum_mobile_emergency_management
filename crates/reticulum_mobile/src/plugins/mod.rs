mod installer;
mod manifest;
mod messages;
mod permissions;
mod registry;

pub use installer::{InstalledPlugin, PluginInstaller, PluginInstallerError};
pub use manifest::{PluginLibrary, PluginManifest, PluginManifestError, PluginSettings};
pub use messages::{
    PluginLxmfMessage, PluginLxmfMessageError, PluginMessageDescriptor, PluginMessageDirection,
    PLUGIN_LXMF_FIELD_KEY,
};
pub use permissions::PluginPermissions;
pub use registry::{PluginRegistry, PluginRegistryError, PluginState, RegisteredPlugin};
