use std::collections::BTreeSet;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::ZipArchive;

use super::{PluginManifest, PluginManifestError, PluginState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_dir: PathBuf,
    pub state: PluginState,
}

#[derive(Debug, Error)]
pub enum PluginInstallerError {
    #[error("missing plugin.toml in package")]
    MissingManifest,
    #[error(transparent)]
    Manifest(#[from] PluginManifestError),
    #[error("required package file is missing: {relative_path}")]
    MissingPackageFile { relative_path: String },
    #[error("invalid package schema file: {relative_path}")]
    InvalidPackageSchema { relative_path: String },
    #[error("invalid package path: {path}")]
    InvalidPackagePath { path: PathBuf },
    #[error("plugin is already installed: {plugin_id}")]
    AlreadyInstalled { plugin_id: String },
    #[error("plugin install I/O failed")]
    Io(#[from] std::io::Error),
    #[error("plugin archive is invalid")]
    InvalidArchive(#[from] zip::result::ZipError),
    #[error("unsigned plugin package is not trusted")]
    UnsignedPackageRejected,
    #[error("plugin package publisher is not trusted: {publisher}")]
    UntrustedPublisher { publisher: String },
    #[error("plugin package signature is invalid")]
    InvalidSignature,
    #[error("plugin package signature does not match package contents")]
    PackageTampered,
}

#[derive(Debug, Clone)]
pub struct PluginInstaller {
    install_root: PathBuf,
    trust_policy: PluginTrustPolicy,
}

#[derive(Debug, Clone, Default)]
pub struct PluginTrustPolicy {
    allow_unsigned: bool,
    trusted_publishers: Vec<TrustedPluginPublisher>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedPluginPublisher {
    pub publisher: String,
    pub public_key_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginSignatureFile {
    plugin_id: String,
    version: String,
    manifest_sha256: String,
    package_sha256: String,
    publisher: String,
    signature: String,
}

impl PluginInstaller {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self::new_with_trust_policy(install_root, PluginTrustPolicy::developer_mode())
    }

    pub fn new_with_trust_policy(
        install_root: impl Into<PathBuf>,
        trust_policy: PluginTrustPolicy,
    ) -> Self {
        Self {
            install_root: install_root.into(),
            trust_policy,
        }
    }

    pub fn install_from_package_dir(
        &self,
        package_dir: impl AsRef<Path>,
        android_abi: &str,
    ) -> Result<InstalledPlugin, PluginInstallerError> {
        let package_dir = package_dir.as_ref();
        let manifest_path = package_dir.join("plugin.toml");
        if !manifest_path.is_file() {
            return Err(PluginInstallerError::MissingManifest);
        }

        let manifest =
            PluginManifest::from_toml_str(fs_err::read_to_string(manifest_path)?.as_str())?;
        let library_path = manifest.android_library_for_abi(android_abi)?;
        require_package_file(package_dir, library_path)?;
        if let Some(settings) = &manifest.settings {
            require_package_file(package_dir, settings.schema.as_str())?;
            let settings_schema = read_json_package_file(package_dir, settings.schema.as_str())?;
            validate_settings_schema_actions(
                &settings_schema,
                &manifest,
                settings.schema.as_str(),
            )?;
        }
        for message in &manifest.messages {
            require_package_file(package_dir, message.schema.as_str())?;
            read_json_package_file(package_dir, message.schema.as_str())?;
        }
        self.validate_package_trust(package_dir, &manifest)?;

        let install_dir = self.install_root.join(manifest.id.as_str());
        if install_dir.exists() {
            return Err(PluginInstallerError::AlreadyInstalled {
                plugin_id: manifest.id.clone(),
            });
        }

        fs_err::create_dir_all(self.install_root.as_path())?;
        let staging_dir = self.staging_install_dir(manifest.id.as_str());
        if staging_dir.exists() {
            fs_err::remove_dir_all(staging_dir.as_path())?;
        }
        if let Err(error) = copy_package_dir(package_dir, staging_dir.as_path()) {
            let _ = fs_err::remove_dir_all(staging_dir.as_path());
            return Err(error);
        }
        if let Err(error) = fs_err::rename(staging_dir.as_path(), install_dir.as_path()) {
            let _ = fs_err::remove_dir_all(staging_dir.as_path());
            return Err(PluginInstallerError::Io(error));
        }

        Ok(InstalledPlugin {
            manifest,
            install_dir,
            state: PluginState::Disabled,
        })
    }

    pub fn install_from_archive(
        &self,
        archive_path: impl AsRef<Path>,
        android_abi: &str,
    ) -> Result<InstalledPlugin, PluginInstallerError> {
        fs_err::create_dir_all(self.install_root.as_path())?;
        let extraction_dir = self.archive_extraction_dir();
        if extraction_dir.exists() {
            fs_err::remove_dir_all(extraction_dir.as_path())?;
        }
        fs_err::create_dir(extraction_dir.as_path())?;

        let result = (|| {
            let archive_file = fs_err::File::open(archive_path.as_ref())?;
            extract_archive(archive_file, extraction_dir.as_path())?;
            self.install_from_package_dir(extraction_dir.as_path(), android_abi)
        })();

        let _ = fs_err::remove_dir_all(extraction_dir.as_path());
        result
    }

    fn staging_install_dir(&self, plugin_id: &str) -> PathBuf {
        self.install_root
            .join(format!(".{plugin_id}.installing-{}", std::process::id()))
    }

    fn archive_extraction_dir(&self) -> PathBuf {
        self.install_root
            .join(format!(".archive-extract-{}", std::process::id()))
    }

    fn validate_package_trust(
        &self,
        package_dir: &Path,
        manifest: &PluginManifest,
    ) -> Result<(), PluginInstallerError> {
        let signature_path = package_dir.join("signature.json");
        if !signature_path.exists() {
            if self.trust_policy.allow_unsigned {
                return Ok(());
            }
            return Err(PluginInstallerError::UnsignedPackageRejected);
        }
        let signature_file: PluginSignatureFile =
            serde_json::from_slice(fs_err::read(signature_path.as_path())?.as_slice())
                .map_err(|_| PluginInstallerError::InvalidSignature)?;
        if signature_file.plugin_id != manifest.id || signature_file.version != manifest.version {
            return Err(PluginInstallerError::PackageTampered);
        }
        let manifest_sha256 = sha256_hex(fs_err::read(package_dir.join("plugin.toml"))?.as_slice());
        let package_sha256 = package_sha256(package_dir)?;
        if !eq_hex(
            signature_file.manifest_sha256.as_str(),
            manifest_sha256.as_str(),
        ) || !eq_hex(
            signature_file.package_sha256.as_str(),
            package_sha256.as_str(),
        ) {
            return Err(PluginInstallerError::PackageTampered);
        }
        let publisher = self
            .trust_policy
            .trusted_publishers
            .iter()
            .find(|publisher| publisher.publisher == signature_file.publisher)
            .ok_or_else(|| PluginInstallerError::UntrustedPublisher {
                publisher: signature_file.publisher.clone(),
            })?;
        verify_signature(&signature_file, publisher)
    }
}

impl PluginTrustPolicy {
    pub fn developer_mode() -> Self {
        Self {
            allow_unsigned: true,
            trusted_publishers: Vec::new(),
        }
    }

    pub fn production(trusted_publishers: Vec<TrustedPluginPublisher>) -> Self {
        Self {
            allow_unsigned: false,
            trusted_publishers,
        }
    }
}

fn extract_archive<R: Read + Seek>(
    reader: R,
    destination: &Path,
) -> Result<(), PluginInstallerError> {
    let mut archive = ZipArchive::new(reader)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let enclosed_path =
            entry
                .enclosed_name()
                .ok_or_else(|| PluginInstallerError::InvalidPackagePath {
                    path: PathBuf::from(entry.name()),
                })?;
        if is_zip_symlink(&entry) {
            return Err(PluginInstallerError::InvalidPackagePath {
                path: enclosed_path.to_path_buf(),
            });
        }
        let target = destination.join(enclosed_path);
        if entry.is_dir() {
            fs_err::create_dir_all(target.as_path())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut output = fs_err::File::create(target.as_path())?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

fn is_zip_symlink(entry: &zip::read::ZipFile<'_>) -> bool {
    const UNIX_FILE_TYPE_MASK: u32 = 0o170000;
    const UNIX_SYMLINK_TYPE: u32 = 0o120000;
    entry
        .unix_mode()
        .is_some_and(|mode| mode & UNIX_FILE_TYPE_MASK == UNIX_SYMLINK_TYPE)
}

fn verify_signature(
    signature_file: &PluginSignatureFile,
    publisher: &TrustedPluginPublisher,
) -> Result<(), PluginInstallerError> {
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(publisher.public_key_base64.as_bytes())
        .map_err(|_| PluginInstallerError::InvalidSignature)?;
    let public_key: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| PluginInstallerError::InvalidSignature)?;
    let verifying_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| PluginInstallerError::InvalidSignature)?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_file.signature.as_bytes())
        .map_err(|_| PluginInstallerError::InvalidSignature)?;
    let signature = Signature::from_slice(signature_bytes.as_slice())
        .map_err(|_| PluginInstallerError::InvalidSignature)?;
    verifying_key
        .verify(signature_payload(signature_file).as_bytes(), &signature)
        .map_err(|_| PluginInstallerError::InvalidSignature)
}

fn signature_payload(signature_file: &PluginSignatureFile) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n",
        signature_file.plugin_id,
        signature_file.version,
        signature_file.manifest_sha256.to_ascii_lowercase(),
        signature_file.package_sha256.to_ascii_lowercase(),
        signature_file.publisher
    )
}

fn package_sha256(package_dir: &Path) -> Result<String, PluginInstallerError> {
    let mut entries = Vec::new();
    collect_package_hash_entries(package_dir, package_dir, &mut entries)?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative_path, file_hash) in entries {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(file_hash.as_bytes());
        hasher.update([b'\n']);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_package_hash_entries(
    root: &Path,
    current: &Path,
    entries: &mut Vec<(String, String)>,
) -> Result<(), PluginInstallerError> {
    for entry in fs_err::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }
        if file_type.is_dir() {
            collect_package_hash_entries(root, entry.path().as_path(), entries)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }
        let relative_path = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| PluginInstallerError::InvalidPackagePath { path: entry.path() })?
            .to_string_lossy()
            .replace('\\', "/");
        if relative_path == "signature.json" {
            continue;
        }
        let contents = fs_err::read(entry.path())?;
        entries.push((relative_path, sha256_hex(contents.as_slice())));
    }
    Ok(())
}

fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

fn eq_hex(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn require_package_file(
    package_dir: &Path,
    relative_path: &str,
) -> Result<(), PluginInstallerError> {
    let path = package_dir.join(relative_path);
    if path.is_file() {
        return Ok(());
    }
    Err(PluginInstallerError::MissingPackageFile {
        relative_path: relative_path.to_string(),
    })
}

fn read_json_package_file(
    package_dir: &Path,
    relative_path: &str,
) -> Result<JsonValue, PluginInstallerError> {
    let path = package_dir.join(relative_path);
    let contents = fs_err::read(path)?;
    let schema: JsonValue = serde_json::from_slice(contents.as_slice()).map_err(|_| {
        PluginInstallerError::InvalidPackageSchema {
            relative_path: relative_path.to_string(),
        }
    })?;
    if schema.is_object() {
        return Ok(schema);
    }
    Err(PluginInstallerError::InvalidPackageSchema {
        relative_path: relative_path.to_string(),
    })
}

fn validate_settings_schema_actions(
    schema: &JsonValue,
    manifest: &PluginManifest,
    relative_path: &str,
) -> Result<(), PluginInstallerError> {
    let field_ids = settings_field_ids(schema, relative_path)?;
    let declared_messages = manifest
        .messages
        .iter()
        .map(|message| message.name.as_str())
        .collect::<BTreeSet<_>>();
    let Some(actions) = schema.get("actions").and_then(JsonValue::as_array) else {
        return Ok(());
    };
    let mut action_ids = BTreeSet::new();
    for action in actions {
        let Some(action) = action.as_object() else {
            continue;
        };
        if let Some(action_id) = action
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|action_id| !action_id.trim().is_empty())
        {
            if !action_ids.insert(action_id.to_string()) {
                return Err(invalid_schema(relative_path));
            }
        }
        let Some(action_type) = action.get("type").and_then(JsonValue::as_str) else {
            continue;
        };
        if action_type != "send_lxmf" && action_type != "sendPluginLxmf" {
            continue;
        }
        let Some(message_name) = action.get("messageName").and_then(JsonValue::as_str) else {
            return Err(invalid_schema(relative_path));
        };
        if !declared_messages.contains(message_name) {
            return Err(invalid_schema(relative_path));
        }
        for field_key in ["destinationField", "bodyField"] {
            let Some(field_id) = action.get(field_key).and_then(JsonValue::as_str) else {
                return Err(invalid_schema(relative_path));
            };
            if !field_ids.contains(field_id) {
                return Err(invalid_schema(relative_path));
            }
        }
        if let Some(payload_fields) = action.get("payloadFields").and_then(JsonValue::as_object) {
            for value in payload_fields.values() {
                let Some(field_id) = value.as_str() else {
                    return Err(invalid_schema(relative_path));
                };
                if !field_ids.contains(field_id) {
                    return Err(invalid_schema(relative_path));
                }
            }
        }
    }
    Ok(())
}

fn settings_field_ids(
    schema: &JsonValue,
    relative_path: &str,
) -> Result<BTreeSet<String>, PluginInstallerError> {
    let mut explicit = BTreeSet::new();
    if let Some(fields) = schema.get("fields").and_then(JsonValue::as_array) {
        for field_id in fields
            .iter()
            .filter_map(|field| field.get("id").and_then(JsonValue::as_str))
            .filter(|field_id| !field_id.trim().is_empty())
        {
            if !explicit.insert(field_id.to_string()) {
                return Err(invalid_schema(relative_path));
            }
        }
    }
    if !explicit.is_empty() {
        return Ok(explicit);
    }
    Ok(schema
        .get("properties")
        .and_then(JsonValue::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default())
}

fn invalid_schema(relative_path: &str) -> PluginInstallerError {
    PluginInstallerError::InvalidPackageSchema {
        relative_path: relative_path.to_string(),
    }
}

fn copy_package_dir(source: &Path, destination: &Path) -> Result<(), PluginInstallerError> {
    fs_err::create_dir(destination)?;
    for entry in fs_err::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }

        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_package_dir(entry.path().as_path(), target.as_path())?;
        } else if file_type.is_file() {
            fs_err::copy(entry.path(), target)?;
        } else {
            return Err(PluginInstallerError::InvalidPackagePath { path: entry.path() });
        }
    }
    Ok(())
}
