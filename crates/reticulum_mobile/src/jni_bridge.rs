use reticulum_mobile_jni_boundary::jni_boundary;

include!("jni_bridge/core_inputs.rs");
include!("jni_bridge/domain_inputs.rs");
include!("jni_bridge/parsing.rs");
include!("jni_bridge/conversions.rs");
include!("jni_bridge/json_records.rs");
include!("jni_bridge/wire_events.rs");
include!("jni_bridge/lifecycle_api.rs");
include!("jni_bridge/delivery_api.rs");
include!("jni_bridge/messaging_api.rs");
include!("jni_bridge/state_api.rs");
include!("jni_bridge/checklist_api.rs");
include!("jni_bridge/mission_plugin_api.rs");
include!("jni_bridge/sos_telemetry_api.rs");
include!("jni_bridge/plugin_host_api.rs");
include!("jni_bridge/plugin_decode_api.rs");
