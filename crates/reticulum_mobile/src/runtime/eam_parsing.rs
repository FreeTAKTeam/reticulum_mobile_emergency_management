#[derive(Debug)]
enum EamCommandAction {
    Upsert(Box<EamProjectionRecord>),
    Delete {
        callsign: String,
        deleted_at_ms: u64,
    },
}

fn eam_command_action_from_command(
    envelope: MissionCommandEnvelope<EamUpsertCommandArgs>,
    projection: Option<EamProjectionRecord>,
    received_at_ms: u64,
) -> Option<EamCommandAction> {
    let command_type = canonical_command_type(envelope.command_type.as_str());
    if command_type != "mission.registry.eam.upsert" {
        return None;
    }

    if let Some(mut projection) = projection {
        if projection.callsign.trim().is_empty() {
            return None;
        }
        projection.group_name = if projection.group_name.trim().is_empty() {
            DEFAULT_EAM_GROUP_NAME.to_string()
        } else {
            projection.group_name.trim().to_string()
        };
        if projection.overall_status.is_none() {
            projection.overall_status = derive_eam_overall_status(&projection);
        }
        projection.sync_state = Some("synced".to_string());
        projection.sync_error = None;
        projection.last_synced_at_ms = Some(received_at_ms);
        projection.updated_at_ms = projection.updated_at_ms.max(received_at_ms);
        return Some(EamCommandAction::Upsert(Box::new(projection)));
    }

    if envelope.args.callsign.trim().is_empty()
        || envelope.args.team_member_uid.trim().is_empty()
        || envelope.args.team_uid.trim().is_empty()
    {
        return None;
    }

    let mut record = EamProjectionRecord {
        callsign: envelope.args.callsign.trim().to_string(),
        group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
        security_status: envelope.args.security_status,
        capability_status: envelope.args.capability_status,
        preparedness_status: envelope.args.preparedness_status,
        medical_status: envelope.args.medical_status,
        mobility_status: envelope.args.mobility_status,
        comms_status: envelope.args.comms_status,
        notes: envelope.args.notes,
        updated_at_ms: received_at_ms,
        deleted_at_ms: None,
        eam_uid: envelope.args.eam_uid,
        team_member_uid: Some(envelope.args.team_member_uid),
        team_uid: Some(envelope.args.team_uid),
        reported_at: envelope.args.reported_at.or(Some(envelope.timestamp)),
        reported_by: envelope
            .args
            .reported_by
            .or(envelope.source.display_name.clone()),
        overall_status: None,
        confidence: envelope.args.confidence,
        ttl_seconds: envelope.args.ttl_seconds,
        source: Some(EamSourceRecord {
            rns_identity: envelope
                .args
                .source
                .as_ref()
                .map(|value| value.rns_identity.clone())
                .unwrap_or(envelope.source.rns_identity),
            display_name: envelope
                .args
                .source
                .and_then(|value| value.display_name)
                .or(envelope.source.display_name),
        }),
        sync_state: Some("synced".to_string()),
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: Some(received_at_ms),
    };
    record.overall_status = derive_eam_overall_status(&record);
    Some(EamCommandAction::Upsert(Box::new(record)))
}

fn compact_eam_fallback_callsign(
    explicit: Option<String>,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<String> {
    explicit
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            source_display_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            source_hex
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.chars().take(8).collect())
        })
}

fn compact_eam_fallback_team_member_uid(
    explicit: Option<String>,
    source_hex: Option<&str>,
) -> Option<String> {
    explicit
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            source_hex
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn compact_eam_status_char(value: char) -> Option<String> {
    match value {
        'G' => Some("Green".to_string()),
        'Y' => Some("Yellow".to_string()),
        'R' => Some("Red".to_string()),
        'U' => Some("Unknown".to_string()),
        _ => None,
    }
}

fn compact_eam_action_from_body(
    body_utf8: &str,
    received_at_ms: u64,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<EamCommandAction> {
    let mut parts = body_utf8.trim().split('|');
    if parts.next()? != "E" {
        return None;
    }
    let callsign = compact_eam_fallback_callsign(
        parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        source_hex,
        source_display_name,
    )?;
    let status_codes = parts.next()?.trim();
    if parts.next().is_some() || status_codes.chars().count() != 6 {
        return None;
    }
    let mut statuses = status_codes.chars().map(compact_eam_status_char);
    let mut record = EamProjectionRecord {
        callsign,
        group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
        security_status: statuses.next()??,
        capability_status: statuses.next()??,
        preparedness_status: statuses.next()??,
        medical_status: statuses.next()??,
        mobility_status: statuses.next()??,
        comms_status: statuses.next()??,
        notes: None,
        updated_at_ms: received_at_ms,
        deleted_at_ms: None,
        eam_uid: None,
        team_member_uid: compact_eam_fallback_team_member_uid(None, source_hex),
        team_uid: None,
        reported_at: None,
        reported_by: source_display_name.map(str::to_string),
        overall_status: None,
        confidence: None,
        ttl_seconds: None,
        source: source_hex.map(|source_hex| EamSourceRecord {
            rns_identity: source_hex.to_string(),
            display_name: source_display_name.map(str::to_string),
        }),
        sync_state: Some("synced".to_string()),
        sync_error: None,
        draft_created_at_ms: None,
        last_synced_at_ms: Some(received_at_ms),
    };
    record.overall_status = derive_eam_overall_status(&record);
    Some(EamCommandAction::Upsert(Box::new(record)))
}

fn eam_command_action_from_fields(
    fields_bytes: &[u8],
    received_at_ms: u64,
    source_hex: Option<&str>,
    source_display_name: Option<&str>,
) -> Option<EamCommandAction> {
    let fields = rmp_serde::from_slice::<MsgPackValue>(fields_bytes).ok()?;
    let field_entries = msgpack_map_entries(&fields)?;
    let commands = msgpack_get_indexed(field_entries, FIELD_COMMANDS)?;
    let MsgPackValue::Array(command_entries) = commands else {
        return None;
    };

    for command in command_entries {
        let command_map = msgpack_map_entries(command)?;
        let command_type = msgpack_get_named(command_map, &["command_type", "t"])
            .and_then(msgpack_string)
            .map(|value| canonical_command_type(value.as_str()).to_string())?;
        if command_type == "mission.registry.eam.delete" {
            let args =
                msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
            let callsign = msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string)?;
            if callsign.trim().is_empty() {
                return None;
            }
            let deleted_at_ms = msgpack_get_named(args, &["deleted_at_ms", "d"])
                .and_then(msgpack_u64)
                .unwrap_or(received_at_ms);
            return Some(EamCommandAction::Delete {
                callsign,
                deleted_at_ms,
            });
        }
        if command_type != "mission.registry.eam.upsert" {
            continue;
        }
        let args = msgpack_get_named(command_map, &["args", "a"]).and_then(msgpack_map_entries)?;
        let source = msgpack_get_named(command_map, &["source", "s"])
            .and_then(msgpack_map_entries)
            .or_else(|| msgpack_get_named(args, &["source", "s"]).and_then(msgpack_map_entries));
        let field_source_display_name = source
            .and_then(|source_map| msgpack_get_named(source_map, &["display_name", "n"]))
            .and_then(msgpack_string);
        let source_display_name = field_source_display_name
            .clone()
            .or_else(|| source_display_name.map(str::to_string));
        let callsign = compact_eam_fallback_callsign(
            msgpack_get_named(args, &["callsign", "cs"]).and_then(msgpack_string),
            source_hex,
            source_display_name.as_deref(),
        )?;
        let team_member_uid = compact_eam_fallback_team_member_uid(
            msgpack_get_named(args, &["team_member_uid", "tm"]).and_then(msgpack_hex_or_string),
            source_hex,
        );
        let compact_statuses = msgpack_eam_status_array(args);
        let mut record = EamProjectionRecord {
            callsign,
            group_name: DEFAULT_EAM_GROUP_NAME.to_string(),
            security_status: compact_statuses[0]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["security_status", "ss"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            capability_status: compact_statuses[1]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["capability_status", "ca"])
                        .and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            preparedness_status: compact_statuses[2]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["preparedness_status", "pr"])
                        .and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            medical_status: compact_statuses[3]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["medical_status", "me"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            mobility_status: compact_statuses[4]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["mobility_status", "mo"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            comms_status: compact_statuses[5]
                .and_then(msgpack_eam_status)
                .or_else(|| {
                    msgpack_get_named(args, &["comms_status", "co"]).and_then(msgpack_eam_status)
                })
                .unwrap_or_else(|| "Unknown".to_string()),
            notes: msgpack_get_named(args, &["notes", "no"]).and_then(msgpack_string),
            updated_at_ms: received_at_ms,
            deleted_at_ms: None,
            eam_uid: msgpack_get_named(args, &["eam_uid", "u"]).and_then(msgpack_eam_uid),
            team_member_uid,
            team_uid: msgpack_get_named(args, &["team_uid", "tu"]).and_then(msgpack_string),
            reported_at: msgpack_get_named(args, &["reported_at", "ra"]).and_then(msgpack_string),
            reported_by: msgpack_get_named(args, &["reported_by", "rb"])
                .and_then(msgpack_string)
                .or_else(|| source_display_name.clone()),
            overall_status: msgpack_get_named(args, &["overall_status", "os"])
                .and_then(msgpack_eam_status),
            confidence: msgpack_get_named(args, &["confidence", "cf"]).and_then(msgpack_f64),
            ttl_seconds: msgpack_get_named(args, &["ttl_seconds", "ttl"]).and_then(msgpack_u64),
            source: source
                .map(|source_map| EamSourceRecord {
                    rns_identity: msgpack_get_named(source_map, &["rns_identity", "r"])
                        .and_then(msgpack_hex_or_string)
                        .or_else(|| source_hex.map(str::to_string))
                        .unwrap_or_default(),
                    display_name: source_display_name.clone(),
                })
                .or_else(|| {
                    source_hex.map(|source_hex| EamSourceRecord {
                        rns_identity: source_hex.to_string(),
                        display_name: source_display_name,
                    })
                }),
            sync_state: Some("synced".to_string()),
            sync_error: None,
            draft_created_at_ms: None,
            last_synced_at_ms: Some(received_at_ms),
        };
        if record.callsign.trim().is_empty() {
            return None;
        }
        record.overall_status = derive_eam_overall_status(&record);
        return Some(EamCommandAction::Upsert(Box::new(record)));
    }

    None
}
