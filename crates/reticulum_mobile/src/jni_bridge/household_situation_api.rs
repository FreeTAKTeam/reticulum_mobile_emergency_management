use crate::types::{HouseholdMode, HouseholdSituation, HouseholdSituationRecord};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SituationInput {
    situation: HouseholdSituation,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HouseholdModeInput {
    mode: HouseholdMode,
}

fn household_situation_json(record: &HouseholdSituationRecord) -> Value {
    json!({
        "situation": record.situation,
        "detail": record.detail,
        "mecpCode": record.mecp_code,
        "eventUid": record.event_uid,
        "sequence": record.sequence,
        "updatedAtMs": record.updated_at_ms,
        "householdMode": record.household_mode,
        "householdId": record.household_id,
        "situationEamFactors": record.situation_eam_factors,
        "heldByPowerSaver": record.held_by_power_saver,
        "duplicateReporter": record.duplicate_reporter.as_ref().map(|reporter| json!({
            "sourceIdentity": reporter.source_identity,
            "householdName": reporter.household_name,
            "updatedAtMs": reporter.updated_at_ms,
        })),
        "messages": record.messages.iter().map(|message| json!({
            "kind": message.kind,
            "outcome": message.outcome,
            "reference": message.reference,
        })).collect::<Vec<_>>(),
    })
}

fn parse_jni_payload<T: for<'de> Deserialize<'de>>(env: &mut JNIEnv, payload: JString) -> Option<T> {
    let raw = match jstring_to_rust(env, payload) {
        Ok(value) => value,
        Err(error) => {
            set_last_error("InvalidConfig", error);
            return None;
        }
    };
    match serde_json::from_str(&raw) {
        Ok(value) => Some(value),
        Err(error) => {
            set_last_error("InvalidConfig", error.to_string());
            None
        }
    }
}

fn household_situation_result(
    env: &mut JNIEnv,
    result: Result<HouseholdSituationRecord, NodeError>,
) -> jstring {
    match result {
        Ok(record) => ok_json_result(env, &household_situation_json(&record)),
        Err(error) => {
            set_last_node_error(error);
            ptr::null_mut()
        }
    }
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setSituationJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = parse_jni_payload::<SituationInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    household_situation_result(&mut env, node.set_situation(input.situation, input.detail))
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_getHouseholdSituationJson(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let node = initialized_node_or_return!();
    household_situation_result(&mut env, node.get_household_situation())
}

#[jni_boundary]
#[no_mangle]
pub extern "system" fn Java_network_reticulum_emergency_ReticulumBridge_setHouseholdModeJson(
    mut env: JNIEnv,
    _class: JClass,
    payload: JString,
) -> jstring {
    let Some(input) = parse_jni_payload::<HouseholdModeInput>(&mut env, payload) else {
        return ptr::null_mut();
    };
    let node = initialized_node_or_return!();
    household_situation_result(&mut env, node.set_household_mode(input.mode))
}

#[cfg(test)]
mod household_situation_contract_tests {
    use super::*;
    use crate::types::{SituationMessageKind, SituationMessageOutcome, SituationMessageRecord};

    #[test]
    fn situation_inputs_use_snake_case_wire_values() {
        let input: SituationInput =
            serde_json::from_value(json!({"situation": "need_supplies", "detail": "water"}))
                .expect("situation input");
        assert_eq!(input.situation, HouseholdSituation::NeedSupplies {});
        assert_eq!(input.detail.as_deref(), Some("water"));
        let input: HouseholdModeInput =
            serde_json::from_value(json!({"mode": "another_phone_reports"})).expect("mode input");
        assert_eq!(input.mode, HouseholdMode::AnotherPhoneReports {});
        assert!(serde_json::from_value::<SituationInput>(json!({"situation": "panic"})).is_err());
    }

    #[test]
    fn situation_record_json_is_camel_case() {
        let record = HouseholdSituationRecord {
            situation: Some(HouseholdSituation::SomeoneMissing {}),
            detail: None,
            mecp_code: Some("P07".to_string()),
            event_uid: Some("rem-sit-v1:aa:1".to_string()),
            sequence: 1,
            updated_at_ms: 5,
            household_mode: HouseholdMode::ThisPhoneReports {},
            household_id: Some("0123456789abcdef".to_string()),
            situation_eam_factors: vec!["security".to_string()],
            held_by_power_saver: false,
            duplicate_reporter: None,
            messages: vec![SituationMessageRecord {
                kind: SituationMessageKind::CommunityStatus {},
                outcome: SituationMessageOutcome::HeldByPowerSaver {},
                reference: None,
            }],
        };
        let value = household_situation_json(&record);
        assert_eq!(value["situation"], "someone_missing");
        assert_eq!(value["householdMode"], "this_phone_reports");
        assert_eq!(value["situationEamFactors"][0], "security");
        assert_eq!(value["messages"][0]["kind"], "community_status");
        assert_eq!(value["messages"][0]["outcome"], "held_by_power_saver");
        assert!(value["duplicateReporter"].is_null());
    }
}
