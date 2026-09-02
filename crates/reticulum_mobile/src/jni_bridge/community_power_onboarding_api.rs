#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockRadioInput {
    region: String,
    profile: String,
    frequency_hz: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockNetworkInput {
    #[serde(default)]
    tcp_clients: Vec<String>,
    #[serde(default)]
    broadcast: bool,
    hub_mode: String,
    hub_identity_hash: Option<String>,
    hub_api_base_url: Option<String>,
    hub_refresh_interval_seconds: u32,
    radio: Option<BlockRadioInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockDraftInput {
    network: BlockNetworkInput,
    #[serde(default)]
    trusted_destination_hashes: Vec<String>,
    preferred_map_layer: String,
    expires_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockEnvelopeInput {
    encoded_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockPeerTierInput {
    destination_hex: String,
    circle_tier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockImportInput {
    encoded_text: String,
    confirmed_signer_fingerprint: String,
    community: CommunitySettingsInput,
    #[serde(default)]
    peer_tiers: Vec<BlockPeerTierInput>,
}

fn block_network_record(
    input: BlockNetworkInput,
) -> Result<crate::types::BlockNetworkSettings, NodeError> {
    let hub_mode = match input.hub_mode.trim().to_ascii_lowercase().as_str() {
        "autonomous" => HubMode::Autonomous {},
        "semiautonomous" | "semi_autonomous" | "semi-autonomous" => {
            HubMode::SemiAutonomous {}
        }
        "connected" => HubMode::Connected {},
        _ => return Err(NodeError::InvalidConfig {}),
    };
    Ok(crate::types::BlockNetworkSettings {
        broadcast: input.broadcast,
        hub_api_base_url: input.hub_api_base_url,
        hub_identity_hash: input.hub_identity_hash,
        hub_mode,
        hub_refresh_interval_seconds: input.hub_refresh_interval_seconds,
        radio: input.radio.map(|radio| crate::types::BlockRadioSettings {
            frequency_hz: radio.frequency_hz,
            profile: radio.profile,
            region: radio.region,
        }),
        tcp_clients: input.tcp_clients,
    })
}

fn community_record(input: CommunitySettingsInput) -> Result<CommunitySettingsRecord, NodeError> {
    Ok(CommunitySettingsRecord {
        household_id: input.household_id.unwrap_or_default(),
        household_name: input.household_name.unwrap_or_default(),
        adults: input.adults.unwrap_or(0),
        children: input.children.unwrap_or(0),
        pets: input.pets.unwrap_or(0),
        role_badges: input.role_badges,
        status: parse_household_status(input.status.as_deref())?,
        preferred_map_layer: parse_preferred_map_layer(input.preferred_map_layer.as_deref())?,
    })
}

fn block_network_json(network: &crate::types::BlockNetworkSettings) -> Value {
    json!({
        "tcpClients": network.tcp_clients,
        "broadcast": network.broadcast,
        "hubMode": network.hub_mode.as_str(),
        "hubIdentityHash": network.hub_identity_hash,
        "hubApiBaseUrl": network.hub_api_base_url,
        "hubRefreshIntervalSeconds": network.hub_refresh_interval_seconds,
        "radio": network.radio.as_ref().map(|radio| json!({
            "region": radio.region,
            "profile": radio.profile,
            "frequencyHz": radio.frequency_hz
        }))
    })
}

fn block_inspection_json(value: &crate::types::BlockOnboardingInspection) -> Value {
    json!({
        "issuerPublicIdentityHex": value.issuer_public_identity_hex,
        "issuerAppDestinationHex": value.issuer_app_destination_hex,
        "issuerLxmfDestinationHex": value.issuer_lxmf_destination_hex,
        "signerFingerprint": value.signer_fingerprint,
        "issuedAtMs": value.issued_at_ms,
        "expiresAtMs": value.expires_at_ms,
        "network": block_network_json(&value.network),
        "trustedDestinationHashes": value.trusted_destination_hashes,
        "preferredMapLayer": value.preferred_map_layer.as_str()
    })
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_updateBatteryState(
    _env: JNIEnv,
    _class: JClass,
    percent: jint,
    charging: jni::sys::jboolean,
) -> jint {
    let Ok(percent) = u8::try_from(percent) else {
        return err_result("InvalidConfig", "battery percent is outside 0..100");
    };
    let node = initialized_node_or_return!();
    match node.update_battery_state(percent, charging != 0) {
        Ok(_) => ok_result(),
        Err(error) => {
            set_last_node_error(error);
            RESULT_ERR
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getPowerStateJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.get_power_state() {
        Ok(state) => ok_json_result(&mut env, &json!({
            "batteryPercent": state.battery_percent,
            "charging": state.charging,
            "saverActive": state.saver_active,
            "updatedAtMs": state.updated_at_ms
        })),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_publishCommunityStatusJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    match node.publish_community_status() {
        Ok(event) => ok_json_result(&mut env, &event_projection_json(&event)),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_createBlockOnboardingCodeJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, payload) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return ptr::null_mut();
        }
    };
    let input: BlockDraftInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error.to_string());
            return ptr::null_mut();
        }
    };
    let network = match block_network_record(input.network) {
        Ok(value) => value,
        Err(error) => {
            set_last_node_error(error);
            return ptr::null_mut();
        }
    };
    let preferred_map_layer = match parse_preferred_map_layer(Some(&input.preferred_map_layer)) {
        Ok(value) => value,
        Err(error) => {
            set_last_node_error(error);
            return ptr::null_mut();
        }
    };
    let draft = crate::types::BlockOnboardingDraft {
        network,
        trusted_destination_hashes: input.trusted_destination_hashes,
        preferred_map_layer,
        expires_at_ms: input.expires_at_ms,
    };
    let node = initialized_node_or_return!();
    match node.create_block_onboarding_code(draft) {
        Ok(value) => ok_json_result(&mut env, &json!({"encodedText": value.encoded_text})),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_inspectBlockOnboardingCodeJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, payload) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return ptr::null_mut();
        }
    };
    let input: BlockEnvelopeInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error.to_string());
            return ptr::null_mut();
        }
    };
    let node = initialized_node_or_return!();
    match node.inspect_block_onboarding_code(crate::types::SignedBlockOnboardingEnvelope {
        encoded_text: input.encoded_text,
    }) {
        Ok(value) => ok_json_result(&mut env, &block_inspection_json(&value)),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_importBlockOnboardingCodeJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let raw = match jstring_to_rust(&mut env, payload) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return ptr::null_mut();
        }
    };
    let input: BlockImportInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error.to_string());
            return ptr::null_mut();
        }
    };
    let community = match community_record(input.community) {
        Ok(value) => value,
        Err(error) => {
            set_last_node_error(error);
            return ptr::null_mut();
        }
    };
    let mut tiers = Vec::new();
    for tier in input.peer_tiers {
        let circle_tier = match parse_circle_tier(Some(&tier.circle_tier)) {
            Ok(value) => value,
            Err(error) => {
                set_last_node_error(error);
                return ptr::null_mut();
            }
        };
        tiers.push(crate::types::BlockPeerTierRecord {
            destination_hex: tier.destination_hex,
            circle_tier,
        });
    }
    let node = initialized_node_or_return!();
    match node.import_block_onboarding_code(crate::types::BlockOnboardingImportRequest {
        encoded_text: input.encoded_text,
        confirmed_signer_fingerprint: input.confirmed_signer_fingerprint,
        community,
        peer_tiers: tiers,
    }) {
        Ok(value) => ok_json_result(
            &mut env,
            &json!({
                "importedPeerCount": value.imported_peer_count,
                "settingsUpdated": value.settings_updated
            }),
        ),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}
