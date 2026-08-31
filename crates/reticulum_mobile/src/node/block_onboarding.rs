const BLOCK_ENVELOPE_PREFIX: &str = "REMBC1:";
const BLOCK_ENVELOPE_KIND: &str = "rem.block-onboarding";
const BLOCK_ENVELOPE_VERSION: u8 = 1;
// REMBC1: is seven bytes and URL-safe unpadded Base64 cannot have a length that
// is 1 mod 4, so 1,999 is the largest representable envelope below 2,000 bytes.
const BLOCK_ENVELOPE_MAX_BYTES: usize = 1_999;
const BLOCK_MAX_TRUSTED_DESTINATIONS: usize = 32;
const BLOCK_MAX_EXPIRY_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockSignedContentV1 {
    expires_at_ms: u64,
    issued_at_ms: u64,
    issuer_app_destination_hex: String,
    issuer_lxmf_destination_hex: String,
    issuer_public_identity_hex: String,
    kind: String,
    network: BlockNetworkSettings,
    preferred_map_layer: PreferredMapLayer,
    trusted_destination_hashes: Vec<String>,
    version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockWireEnvelopeV1 {
    content: BlockSignedContentV1,
    signature: String,
}

fn validate_block_endpoint(endpoint: &str) -> Result<String, NodeError> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty()
        || endpoint.contains("://")
        || endpoint.contains(['@', '/', '?', '#'])
    {
        return Err(NodeError::InvalidConfig {});
    }
    let (host, port) = if let Some(rest) = endpoint.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or(NodeError::InvalidConfig {})?;
        let port = suffix.strip_prefix(':').ok_or(NodeError::InvalidConfig {})?;
        let address = host
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        (format!("[{address}]"), port)
    } else {
        let (host, port) = endpoint.rsplit_once(':').ok_or(NodeError::InvalidConfig {})?;
        validate_block_hostname(host)?;
        (host.to_ascii_lowercase(), port)
    };
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or(NodeError::InvalidConfig {})?;
    if host.is_empty() || host.len() > 253 {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(format!("{host}:{port}"))
}

fn validate_block_hostname(host: &str) -> Result<(), NodeError> {
    if host.is_empty()
        || host.len() > 253
        || !host.is_ascii()
        || host.contains([':', '[', ']', '\\'])
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(())
}

fn validate_block_url(value: &str) -> Result<String, NodeError> {
    let value = value.trim();
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or(NodeError::InvalidConfig {})?;
    if rest.is_empty() || rest.contains(['@', '?', '#', '\\']) {
        return Err(NodeError::InvalidConfig {});
    }
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() || path.len() > 128 {
        return Err(NodeError::InvalidConfig {});
    }
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed
            .split_once(']')
            .ok_or(NodeError::InvalidConfig {})?;
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| NodeError::InvalidConfig {})?;
        if !suffix.is_empty() {
            let port = suffix.strip_prefix(':').ok_or(NodeError::InvalidConfig {})?;
            if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
                return Err(NodeError::InvalidConfig {});
            }
        }
    } else {
        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        validate_block_hostname(host)?;
        if port.is_some_and(|port| {
            port.parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .is_none()
        }) {
            return Err(NodeError::InvalidConfig {});
        }
    }
    Ok(value.to_string())
}

fn validate_block_radio(radio: &BlockRadioSettings) -> Result<(), NodeError> {
    crate::runtime::validate_shared_radio_settings(
        &radio.region,
        &radio.profile,
        radio.frequency_hz,
        false,
    )
    .map(|_| ())
    .map_err(|_| NodeError::InvalidConfig {})
}

fn validate_block_network(network: &BlockNetworkSettings) -> Result<BlockNetworkSettings, NodeError> {
    if network.tcp_clients.len() > 8 || network.hub_refresh_interval_seconds < 60 {
        return Err(NodeError::InvalidConfig {});
    }
    let mut tcp_clients = Vec::new();
    for endpoint in &network.tcp_clients {
        let endpoint = validate_block_endpoint(endpoint)?;
        if tcp_clients.contains(&endpoint) {
            return Err(NodeError::InvalidConfig {});
        }
        tcp_clients.push(endpoint);
    }
    let hub_identity_hash = network
        .hub_identity_hash
        .as_deref()
        .map(|value| normalize_hex_32(value).ok_or(NodeError::InvalidConfig {}))
        .transpose()?;
    let hub_api_base_url = network
        .hub_api_base_url
        .as_deref()
        .map(validate_block_url)
        .transpose()?;
    if let Some(radio) = &network.radio {
        validate_block_radio(radio)?;
    }
    Ok(BlockNetworkSettings {
        tcp_clients,
        broadcast: network.broadcast,
        hub_mode: network.hub_mode,
        hub_identity_hash,
        hub_api_base_url,
        hub_refresh_interval_seconds: network.hub_refresh_interval_seconds,
        radio: network.radio.clone(),
    })
}

fn normalize_trusted_destinations(values: &[String]) -> Result<Vec<String>, NodeError> {
    if values.len() > BLOCK_MAX_TRUSTED_DESTINATIONS {
        return Err(NodeError::InvalidConfig {});
    }
    let mut normalized = Vec::new();
    for value in values {
        let value = normalize_hex_32(value).ok_or(NodeError::InvalidConfig {})?;
        if normalized.contains(&value) {
            return Err(NodeError::InvalidConfig {});
        }
        normalized.push(value);
    }
    normalized.sort();
    Ok(normalized)
}

fn decode_and_verify_block_envelope(
    encoded_text: &str,
    now_ms: u64,
) -> Result<BlockOnboardingInspection, NodeError> {
    if encoded_text.as_bytes().len() > BLOCK_ENVELOPE_MAX_BYTES {
        return Err(NodeError::InvalidConfig {});
    }
    let encoded = encoded_text
        .strip_prefix(BLOCK_ENVELOPE_PREFIX)
        .ok_or(NodeError::InvalidConfig {})?;
    let wire_bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| NodeError::InvalidConfig {})?;
    let envelope: BlockWireEnvelopeV1 =
        serde_json::from_slice(&wire_bytes).map_err(|_| NodeError::InvalidConfig {})?;
    let content = envelope.content;
    if content.kind != BLOCK_ENVELOPE_KIND
        || content.version != BLOCK_ENVELOPE_VERSION
        || content.expires_at_ms <= now_ms
        || content.expires_at_ms <= content.issued_at_ms
        || content.issued_at_ms > now_ms.saturating_add(COMMUNITY_MAX_FUTURE_MS)
        || content.expires_at_ms.saturating_sub(content.issued_at_ms) > BLOCK_MAX_EXPIRY_MS
    {
        return Err(NodeError::InvalidConfig {});
    }
    let public_identity = reticulum::transport::identity::Identity::new_from_hex_string(
        &content.issuer_public_identity_hex,
    )
    .map_err(|_| NodeError::InvalidConfig {})?;
    let canonical = serde_json::to_vec(&content).map_err(|_| NodeError::InvalidConfig {})?;
    let signature = URL_SAFE_NO_PAD
        .decode(envelope.signature)
        .map_err(|_| NodeError::InvalidConfig {})?;
    if !reticulum::transport::identity::lxmf_verify(&public_identity, &canonical, &signature) {
        return Err(NodeError::InvalidConfig {});
    }
    let derived_app = SingleOutputDestination::new(
        public_identity,
        DestinationName::new(APP_DESTINATION_NAME.0, APP_DESTINATION_NAME.1),
    )
    .desc
    .address_hash
    .to_hex_string();
    let derived_lxmf = SingleOutputDestination::new(
        public_identity,
        DestinationName::new(LXMF_DELIVERY_NAME.0, LXMF_DELIVERY_NAME.1),
    )
    .desc
    .address_hash
    .to_hex_string();
    if content.issuer_app_destination_hex != derived_app
        || content.issuer_lxmf_destination_hex != derived_lxmf
    {
        return Err(NodeError::InvalidConfig {});
    }
    let network = validate_block_network(&content.network)?;
    let trusted_destination_hashes =
        normalize_trusted_destinations(&content.trusted_destination_hashes)?;
    let fingerprint = hex::encode(Sha256::digest(content.issuer_public_identity_hex.as_bytes()));
    Ok(BlockOnboardingInspection {
        issuer_public_identity_hex: content.issuer_public_identity_hex,
        issuer_app_destination_hex: derived_app,
        issuer_lxmf_destination_hex: derived_lxmf,
        signer_fingerprint: fingerprint,
        issued_at_ms: content.issued_at_ms,
        expires_at_ms: content.expires_at_ms,
        network,
        trusted_destination_hashes,
        preferred_map_layer: content.preferred_map_layer,
    })
}

impl Node {
    pub fn create_block_onboarding_code(
        &self,
        draft: BlockOnboardingDraft,
    ) -> Result<SignedBlockOnboardingEnvelope, NodeError> {
        let (identity, app_destination_hex, lxmf_destination_hex) = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let config = inner.active_config.as_ref().ok_or(NodeError::NotRunning {})?;
            let identity = load_or_create_identity(config.storage_dir.as_deref(), &config.name)?;
            let status = inner.status.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            (
                identity,
                status.app_destination_hex.clone(),
                status.lxmf_destination_hex.clone(),
            )
        };
        let issued_at_ms = now_ms();
        if draft.expires_at_ms <= issued_at_ms
            || draft.expires_at_ms.saturating_sub(issued_at_ms) > BLOCK_MAX_EXPIRY_MS
        {
            return Err(NodeError::InvalidConfig {});
        }
        let content = BlockSignedContentV1 {
            expires_at_ms: draft.expires_at_ms,
            issued_at_ms,
            issuer_app_destination_hex: app_destination_hex,
            issuer_lxmf_destination_hex: lxmf_destination_hex,
            issuer_public_identity_hex: identity.as_identity().to_hex_string(),
            kind: BLOCK_ENVELOPE_KIND.to_string(),
            network: validate_block_network(&draft.network)?,
            preferred_map_layer: draft.preferred_map_layer,
            trusted_destination_hashes: normalize_trusted_destinations(
                &draft.trusted_destination_hashes,
            )?,
            version: BLOCK_ENVELOPE_VERSION,
        };
        let canonical = serde_json::to_vec(&content).map_err(|_| NodeError::InternalError {})?;
        let wire = BlockWireEnvelopeV1 {
            signature: URL_SAFE_NO_PAD.encode(
                reticulum::transport::identity::lxmf_sign(&identity, &canonical),
            ),
            content,
        };
        let encoded_text = format!(
            "{BLOCK_ENVELOPE_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&wire).map_err(|_| NodeError::InternalError {})?)
        );
        if encoded_text.as_bytes().len() > BLOCK_ENVELOPE_MAX_BYTES {
            return Err(NodeError::InvalidConfig {});
        }
        Ok(SignedBlockOnboardingEnvelope { encoded_text })
    }

    pub fn inspect_block_onboarding_code(
        &self,
        envelope: SignedBlockOnboardingEnvelope,
    ) -> Result<BlockOnboardingInspection, NodeError> {
        decode_and_verify_block_envelope(&envelope.encoded_text, now_ms())
    }

    pub fn import_block_onboarding_code(
        &self,
        mut request: BlockOnboardingImportRequest,
    ) -> Result<BlockOnboardingImportResult, NodeError> {
        let inspection = decode_and_verify_block_envelope(&request.encoded_text, now_ms())?;
        if !inspection
            .signer_fingerprint
            .eq_ignore_ascii_case(request.confirmed_signer_fingerprint.trim())
        {
            return Err(NodeError::InvalidConfig {});
        }
        request.community = normalize_community_settings(&request.community)?;
        let (result, restart_config, bus) = {
            let inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let restart_config = inner.active_config.as_ref().map(|active| {
                let mut config = active.to_config();
                config.tcp_clients = inspection.network.tcp_clients.clone();
                config.broadcast = inspection.network.broadcast;
                config.hub_mode = inspection.network.hub_mode;
                config.hub_identity_hash = inspection.network.hub_identity_hash.clone();
                config.hub_api_base_url = inspection.network.hub_api_base_url.clone();
                config.hub_refresh_interval_seconds =
                    inspection.network.hub_refresh_interval_seconds;
                if let Some(radio) = &inspection.network.radio {
                    config.rnode.region = radio.region.clone();
                    config.rnode.profile = radio.profile.clone();
                    config.rnode.frequency_hz = radio.frequency_hz;
                }
                config
            });
            if let Some(config) = restart_config.as_ref() {
                NodeConfigFingerprint::from_config(config)?;
            }
            let result = inner
                .app_state
                .import_block_onboarding_atomic(&inspection, &request)?;
            (result, restart_config, inner.bus.clone())
        };
        if let Some(config) = restart_config {
            if let Err(error) = self.restart(config) {
                bus.emit(NodeEvent::Error {
                    code: "RuntimeReconcileFailed".to_string(),
                    message: format!(
                        "Block Code committed; restart required to activate imported network: {error}"
                    ),
                });
            }
        }
        Ok(result)
    }
}
