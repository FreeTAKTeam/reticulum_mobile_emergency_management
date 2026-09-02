use reticulum_mobile_jni_boundary::jni_boundary;

include!("jni_bridge/core_inputs.rs");
include!("jni_bridge/domain_inputs.rs");
include!("jni_bridge/parsing.rs");
include!("jni_bridge/config_parsing.rs");
include!("jni_bridge/team_conversions.rs");
include!("jni_bridge/conversions.rs");
include!("jni_bridge/settings_conversions.rs");
include!("jni_bridge/json_records.rs");
include!("jni_bridge/wire_events.rs");
include!("jni_bridge/lifecycle_api.rs");
include!("jni_bridge/delivery_api.rs");
include!("jni_bridge/messaging_api.rs");
include!("jni_bridge/state_api.rs");
include!("jni_bridge/community_power_onboarding_api.rs");
include!("jni_bridge/checklist_api.rs");
include!("jni_bridge/mission_plugin_api.rs");
include!("jni_bridge/hil_api.rs");
include!("jni_bridge/sos_telemetry_api.rs");
include!("jni_bridge/plugin_host_api.rs");
include!("jni_bridge/plugin_decode_api.rs");

#[cfg(test)]
mod rnode_config_tests {
    use super::*;

    #[test]
    fn rnode_config_defaults_missing_legacy_radio_fields() {
        let settings = to_rnode_settings_record(Some(RnodeSettingsInput::default()))
            .expect("legacy RNode settings should use defaults");

        assert_eq!(settings.region, "US915");
        assert_eq!(settings.profile, "REM-LF-RURAL-v1");
        assert_eq!(settings.frequency_hz, 915_000_000);
    }

    #[test]
    fn rnode_config_rejects_explicit_unknown_region_and_profile() {
        let bad_region = to_rnode_settings_record(Some(RnodeSettingsInput {
            region: Some("XX000".to_string()),
            ..RnodeSettingsInput::default()
        }));
        assert!(matches!(bad_region, Err(NodeError::InvalidConfig {})));

        let bad_profile = to_rnode_settings_record(Some(RnodeSettingsInput {
            profile: Some("REM-TYPO-v1".to_string()),
            ..RnodeSettingsInput::default()
        }));
        assert!(matches!(bad_profile, Err(NodeError::InvalidConfig {})));
    }

    #[test]
    fn rnode_config_rejects_frequency_outside_hardware_bounds() {
        for frequency_hz in [1, RNODE_FREQUENCY_MAX_HZ + 1] {
            let result = to_rnode_settings_record(Some(RnodeSettingsInput {
                frequency_hz: Some(frequency_hz),
                ..RnodeSettingsInput::default()
            }));
            assert!(matches!(result, Err(NodeError::InvalidConfig {})));
        }
    }
}

#[cfg(test)]
mod community_contract_tests {
    use super::*;

    #[test]
    fn legacy_jni_settings_and_peers_receive_migration_defaults() {
        let settings: AppSettingsInput = serde_json::from_value(serde_json::json!({
            "displayName": "Legacy",
            "autoConnectSaved": false,
            "announceCapabilities": "R3AKT,EMergencyMessages",
            "tcpClients": [],
            "broadcast": true,
            "transportNodeEnabled": true,
            "announceIntervalSeconds": 1800,
            "telemetry": {
                "enabled": false,
                "publishIntervalSeconds": 360,
                "accuracyThresholdMeters": null,
                "staleAfterMinutes": 30,
                "expireAfterMinutes": 180
            },
            "hub": {
                "mode": "Autonomous",
                "identityHash": "",
                "apiBaseUrl": "",
                "apiKey": "",
                "refreshIntervalSeconds": 3600
            }
        }))
        .expect("legacy settings input");
        let settings = to_app_settings_record(settings).expect("legacy settings conversion");
        assert_eq!(settings.community, CommunitySettingsRecord::default());
        assert_eq!(settings.power, PowerPolicyRecord::default());
        assert_eq!(
            app_settings_json(&settings)["community"]["status"],
            "all_home"
        );

        let peer: SavedPeerInput = serde_json::from_value(serde_json::json!({
            "destination": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "label": null,
            "savedAt": 1
        }))
        .expect("legacy peer input");
        let peer = to_saved_peer_record(peer).expect("legacy peer conversion");
        assert_eq!(peer.circle_tier, CircleTier::Inner {});
        assert_eq!(saved_peer_json(&peer)["circleTier"], "inner");
    }

    #[test]
    fn jni_contract_rejects_malformed_tier_and_power_threshold() {
        let malformed_peer: SavedPeerInput = serde_json::from_value(serde_json::json!({
            "destination": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "label": null,
            "savedAt": 1,
            "circleTier": "trusted"
        }))
        .expect("malformed peer input shape");
        assert!(matches!(
            to_saved_peer_record(malformed_peer),
            Err(NodeError::InvalidConfig {})
        ));
        assert!(matches!(
            parse_power_threshold(Some(25)),
            Err(NodeError::InvalidConfig {})
        ));
    }
}
