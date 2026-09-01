use reticulum_mobile_jni_boundary::jni_boundary;

include!("jni_bridge/core_inputs.rs");
include!("jni_bridge/domain_inputs.rs");
include!("jni_bridge/parsing.rs");
include!("jni_bridge/team_conversions.rs");
include!("jni_bridge/conversions.rs");
include!("jni_bridge/json_records.rs");
include!("jni_bridge/wire_events.rs");
include!("jni_bridge/lifecycle_api.rs");
include!("jni_bridge/delivery_api.rs");
include!("jni_bridge/messaging_api.rs");
include!("jni_bridge/state_api.rs");
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
