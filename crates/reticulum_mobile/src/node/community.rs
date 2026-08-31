use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub(crate) const COMMUNITY_MISSION_UID: &str = "rem-community";
pub(crate) const COMMUNITY_TOPIC: &str = "rem.community-status.v1";
pub(crate) const COMMUNITY_PREFIX: &str = "MECP/2/B04";
const COMMUNITY_PAYLOAD_PREFIX: &str = "REMCS1:";
const COMMUNITY_MAX_FUTURE_MS: u64 = 5 * 60 * 1_000;
const COMMUNITY_MAX_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CommunityStatusPayloadV1 {
    a: u8,
    b: bool,
    c: u8,
    h: String,
    n: String,
    p: u8,
    r: Vec<String>,
    s: HouseholdStatus,
    u: u64,
    v: u8,
}

fn normalize_community_settings(
    settings: &CommunitySettingsRecord,
) -> Result<CommunitySettingsRecord, NodeError> {
    let household_id = settings.household_id.trim().to_ascii_lowercase();
    if household_id.len() != 16 || !household_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NodeError::InvalidConfig {});
    }
    let household_name = settings.household_name.trim().to_string();
    if !(1..=64).contains(&household_name.chars().count())
        || settings.adults > 20
        || settings.children > 20
        || settings.pets > 20
    {
        return Err(NodeError::InvalidConfig {});
    }
    let mut roles = Vec::new();
    for raw in &settings.role_badges {
        let role = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if !(1..=24).contains(&role.chars().count()) {
            return Err(NodeError::InvalidConfig {});
        }
        if !roles.iter().any(|existing: &String| existing.eq_ignore_ascii_case(&role)) {
            roles.push(role);
        }
    }
    if roles.len() > 5 {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(CommunitySettingsRecord {
        household_id,
        household_name,
        adults: settings.adults,
        children: settings.children,
        pets: settings.pets,
        role_badges: roles,
        status: settings.status,
        preferred_map_layer: settings.preferred_map_layer,
    })
}

fn encode_community_status_body(
    settings: &CommunitySettingsRecord,
    saver_active: bool,
    updated_at_ms: u64,
) -> Result<String, NodeError> {
    let normalized = normalize_community_settings(settings)?;
    let payload = CommunityStatusPayloadV1 {
        a: normalized.adults,
        b: saver_active,
        c: normalized.children,
        h: normalized.household_id.clone(),
        n: normalized.household_name,
        p: normalized.pets,
        r: normalized.role_badges,
        s: normalized.status,
        u: updated_at_ms,
        v: 1,
    };
    let canonical = serde_json::to_vec(&payload)
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?;
    Ok(format!(
        "{COMMUNITY_PREFIX} #HH_{} {COMMUNITY_PAYLOAD_PREFIX}{}",
        payload.h,
        URL_SAFE_NO_PAD.encode(canonical)
    ))
}

fn parse_community_status_body(
    body: &str,
    now_ms: u64,
) -> Result<CommunityStatusPayloadV1, NodeError> {
    parse_community_status_body_with_staleness(body, now_ms, false)
}

fn parse_community_status_body_with_staleness(
    body: &str,
    now_ms: u64,
    allow_stale: bool,
) -> Result<CommunityStatusPayloadV1, NodeError> {
    let mut parts = body.split_whitespace();
    if parts.next() != Some(COMMUNITY_PREFIX) {
        return Err(NodeError::InvalidConfig {});
    }
    let tag = parts.next().ok_or(NodeError::InvalidConfig {})?;
    let encoded = parts
        .next()
        .and_then(|value| value.strip_prefix(COMMUNITY_PAYLOAD_PREFIX))
        .ok_or(NodeError::InvalidConfig {})?;
    if parts.next().is_some() {
        return Err(NodeError::InvalidConfig {});
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| NodeError::InvalidConfig {})?;
    let payload: CommunityStatusPayloadV1 =
        serde_json::from_slice(&bytes).map_err(|_| NodeError::InvalidConfig {})?;
    let canonical = serde_json::to_vec(&payload).map_err(|_| NodeError::InternalError {})?;
    if bytes != canonical {
        return Err(NodeError::InvalidConfig {});
    }
    if payload.v != 1 || tag != format!("#HH_{}", payload.h) {
        return Err(NodeError::InvalidConfig {});
    }
    let settings = CommunitySettingsRecord {
        household_id: payload.h.clone(),
        household_name: payload.n.clone(),
        adults: payload.a,
        children: payload.c,
        pets: payload.p,
        role_badges: payload.r.clone(),
        status: payload.s,
        preferred_map_layer: PreferredMapLayer::Base {},
    };
    normalize_community_settings(&settings)?;
    if payload.u > now_ms.saturating_add(COMMUNITY_MAX_FUTURE_MS)
        || (!allow_stale && now_ms.saturating_sub(payload.u) > COMMUNITY_MAX_AGE_MS)
    {
        return Err(NodeError::InvalidConfig {});
    }
    Ok(payload)
}

fn community_event_for(
    source_identity: &str,
    source_name: Option<String>,
    settings: &CommunitySettingsRecord,
    saver_active: bool,
    updated_at_ms: u64,
) -> Result<EventProjectionRecord, NodeError> {
    let source_identity = normalize_hex_32(source_identity).ok_or(NodeError::InvalidConfig {})?;
    Ok(EventProjectionRecord {
        uid: format!("rem-community-status-v1:{source_identity}"),
        command_id: format!("community-status-{updated_at_ms}"),
        source_identity,
        source_display_name: source_name,
        timestamp: updated_at_ms.to_string(),
        command_type: "event.create".to_string(),
        mission_uid: COMMUNITY_MISSION_UID.to_string(),
        content: encode_community_status_body(settings, saver_active, updated_at_ms)?,
        callsign: settings.household_name.clone(),
        server_time: None,
        client_time: None,
        keywords: vec!["B04".to_string(), COMMUNITY_TOPIC.to_string()],
        content_hashes: Vec::new(),
        updated_at_ms,
        deleted_at_ms: None,
        correlation_id: None,
        topics: vec![COMMUNITY_TOPIC.to_string()],
    })
}

fn community_payload_matches(
    existing: &EventProjectionRecord,
    settings: &CommunitySettingsRecord,
    saver_active: bool,
    now_ms: u64,
) -> Result<bool, NodeError> {
    let payload = parse_community_status_body_with_staleness(&existing.content, now_ms, true)?;
    let settings = normalize_community_settings(settings)?;
    Ok(payload.a == settings.adults
        && payload.b == saver_active
        && payload.c == settings.children
        && payload.h == settings.household_id
        && payload.n == settings.household_name
        && payload.p == settings.pets
        && payload.r == settings.role_badges
        && payload.s == settings.status)
}

pub(crate) fn community_event_is_newer(
    existing: Option<&EventProjectionRecord>,
    incoming: &EventProjectionRecord,
    now_ms: u64,
) -> Result<bool, NodeError> {
    let incoming_payload = parse_community_status_body(&incoming.content, now_ms)?;
    if incoming.mission_uid != COMMUNITY_MISSION_UID
        || incoming.command_type != "event.create"
        || incoming.source_identity.is_empty()
        || incoming.uid != format!("rem-community-status-v1:{}", incoming.source_identity)
    {
        return Err(NodeError::InvalidConfig {});
    }
    let Some(existing) = existing else { return Ok(true) };
    let existing_payload =
        parse_community_status_body_with_staleness(&existing.content, now_ms, true)?;
    Ok(incoming_payload.u > existing_payload.u)
}

impl Node {
    pub fn get_community_statuses(
        &self,
    ) -> Result<Vec<CommunityStatusProjectionRecord>, NodeError> {
        let inner = self.inner.lock().map_err(|error| {
            crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
        })?;
        let current_time_ms = now_ms();
        Ok(inner
            .app_state
            .get_events()?
            .into_iter()
            .filter_map(|record| {
                if record.mission_uid != COMMUNITY_MISSION_UID
                    || record.command_type != "event.create"
                    || record.deleted_at_ms.is_some()
                    || !record.topics.iter().any(|topic| topic == COMMUNITY_TOPIC)
                    || record.uid
                        != format!("rem-community-status-v1:{}", record.source_identity)
                {
                    return None;
                }
                let payload = parse_community_status_body(&record.content, current_time_ms).ok()?;
                Some(CommunityStatusProjectionRecord {
                    household_id: payload.h,
                    household_name: payload.n,
                    adults: payload.a,
                    children: payload.c,
                    pets: payload.p,
                    role_badges: payload.r,
                    status: payload.s,
                    saver_active: payload.b,
                    updated_at_ms: payload.u,
                    source_identity: record.source_identity,
                })
            })
            .collect())
    }

    pub fn publish_community_status(&self) -> Result<EventProjectionRecord, NodeError> {
        let (event, should_publish, traffic_class) = {
            let mut inner = self.inner.lock().map_err(|error| {
                crate::error_context::contextual_node_error(NodeError::InternalError {}, error)
            })?;
            let settings = inner
                .app_state
                .get_app_settings()?
                .ok_or(NodeError::InvalidConfig {})?;
            let (identity_hex, node_name) = {
                let status = inner.status.lock().map_err(|error| {
                    crate::error_context::contextual_node_error(
                        NodeError::InternalError {},
                        error,
                    )
                })?;
                (status.identity_hex.clone(), status.name.clone())
            };
            let existing = inner
                .app_state
                .get_events()?
                .into_iter()
                .find(|record| record.uid == format!("rem-community-status-v1:{identity_hex}"));
            let current_time_ms = now_ms();
            let updated_at_ms = existing
                .as_ref()
                .and_then(|record| {
                    parse_community_status_body_with_staleness(
                        &record.content,
                        current_time_ms,
                        true,
                    )
                    .ok()
                })
                .map_or(current_time_ms, |payload| {
                    current_time_ms.max(payload.u.saturating_add(1))
                });
            let event = community_event_for(
                &identity_hex,
                Some(node_name),
                &settings.community,
                inner.power_state.saver_active,
                updated_at_ms,
            )?;
            let should_publish = existing
                .as_ref()
                .map(|record| {
                    community_payload_matches(
                        record,
                        &settings.community,
                        inner.power_state.saver_active,
                        current_time_ms,
                    )
                    .map(|matches| !matches)
                })
                .transpose()?
                .unwrap_or(true);
            let traffic_class = if inner.power_state.saver_active
                && inner.community_status_sent_in_saver
            {
                OutboundTrafficClass::Event {}
            } else {
                if inner.power_state.saver_active && inner.cmd_tx.is_some() {
                    inner.community_status_sent_in_saver = true;
                }
                OutboundTrafficClass::CommunityStatus {}
            };
            (event, should_publish, traffic_class)
        };
        if should_publish {
            self.upsert_event_with_class(event.clone(), traffic_class)?;
        }
        Ok(event)
    }
}
