import type { CommunityStatusProjectionRecord, EamProjectionRecord, EamReadinessMessageRecord, EamReadinessStatusMetricRecord, EamReadinessSummaryRecord, EamSourceRecord, EamTeamSummaryRecord, EventProjectionRecord, HouseholdStatus, LegacyImportPayload, SavedPeerRecord, TelemetryPositionRecord } from "./contracts";
import { toOptionalNumber } from "./converters";
import { hasValue, normalizeHex, pluginRecord, toOptionalHex } from "./runtime-converters";

export function toSavedPeerRecord(raw: Record<string, unknown>): SavedPeerRecord {
  return {
    destination: normalizeHex(raw.destination ?? raw.destinationHex ?? ""),
    label: typeof raw.label === "string" ? raw.label : undefined,
    savedAt: Number(raw.savedAt ?? raw.saved_at_ms ?? raw.savedAtMs ?? Date.now()),
    identityHex: toOptionalHex(raw.identityHex ?? raw.identity_hex),
    lxmfDestinationHex: toOptionalHex(raw.lxmfDestinationHex ?? raw.lxmf_destination_hex),
    appData: typeof raw.appData === "string"
      ? raw.appData
      : typeof raw.app_data === "string"
        ? raw.app_data
        : undefined,
    displayName: typeof raw.displayName === "string"
      ? raw.displayName
      : typeof raw.display_name === "string"
        ? raw.display_name
        : undefined,
    lastRouteSeenAtMs: toOptionalNumber(raw.lastRouteSeenAtMs ?? raw.last_route_seen_at_ms),
    lastHops: toOptionalNumber(raw.lastHops ?? raw.last_hops),
    circleTier: String(raw.circleTier ?? raw.circle_tier ?? "").trim().toLowerCase() === "outer"
      ? "outer"
      : "inner",
  };
}

export function toEamProjectionRecord(raw: Record<string, unknown>): EamProjectionRecord {
  const source = raw.source && typeof raw.source === "object" && !Array.isArray(raw.source)
    ? raw.source as Record<string, unknown>
    : null;
  return {
    callsign: String(raw.callsign ?? ""),
    groupName: String(raw.groupName ?? raw.group_name ?? ""),
    securityStatus: String(raw.securityStatus ?? raw.security_status ?? "Unknown"),
    capabilityStatus: String(raw.capabilityStatus ?? raw.capability_status ?? "Unknown"),
    preparednessStatus: String(raw.preparednessStatus ?? raw.preparedness_status ?? "Unknown"),
    medicalStatus: String(raw.medicalStatus ?? raw.medical_status ?? "Unknown"),
    mobilityStatus: String(raw.mobilityStatus ?? raw.mobility_status ?? "Unknown"),
    commsStatus: String(raw.commsStatus ?? raw.comms_status ?? "Unknown"),
    notes: typeof raw.notes === "string" ? raw.notes : undefined,
    updatedAt: Number(raw.updatedAt ?? raw.updated_at_ms ?? Date.now()),
    deletedAt: toOptionalNumber(raw.deletedAt ?? raw.deleted_at_ms),
    eamUid: typeof raw.eamUid === "string" ? raw.eamUid : typeof raw.eam_uid === "string" ? raw.eam_uid : undefined,
    teamMemberUid:
      typeof raw.teamMemberUid === "string"
        ? raw.teamMemberUid
        : typeof raw.team_member_uid === "string"
          ? raw.team_member_uid
          : undefined,
    teamUid:
      typeof raw.teamUid === "string"
        ? raw.teamUid
        : typeof raw.team_uid === "string"
          ? raw.team_uid
          : undefined,
    reportedAt:
      typeof raw.reportedAt === "string"
        ? raw.reportedAt
        : typeof raw.reported_at === "string"
          ? raw.reported_at
          : undefined,
    reportedBy:
      typeof raw.reportedBy === "string"
        ? raw.reportedBy
        : typeof raw.reported_by === "string"
          ? raw.reported_by
          : undefined,
    overallStatus:
      typeof raw.overallStatus === "string"
        ? raw.overallStatus
        : typeof raw.overall_status === "string"
          ? raw.overall_status
          : undefined,
    confidence: toOptionalNumber(raw.confidence),
    ttlSeconds: toOptionalNumber(raw.ttlSeconds ?? raw.ttl_seconds),
    source: source
      ? {
          rns_identity: String(source.rns_identity ?? source.rnsIdentity ?? ""),
          display_name:
            typeof source.display_name === "string"
              ? source.display_name
              : typeof source.displayName === "string"
                ? source.displayName
                : undefined,
        }
      : undefined,
    syncState:
      typeof raw.syncState === "string"
        ? raw.syncState
        : typeof raw.sync_state === "string"
          ? raw.sync_state
          : undefined,
    syncError:
      typeof raw.syncError === "string"
        ? raw.syncError
        : typeof raw.sync_error === "string"
          ? raw.sync_error
          : undefined,
    draftCreatedAt: toOptionalNumber(raw.draftCreatedAt ?? raw.draft_created_at_ms),
    lastSyncedAt: toOptionalNumber(raw.lastSyncedAt ?? raw.last_synced_at_ms),
  };
}

export function eamProjectionRecordToPlugin(record: EamProjectionRecord): Record<string, unknown> {
  const normalized = toEamProjectionRecord(record as unknown as Record<string, unknown>);
  return {
    callsign: normalized.callsign,
    groupName: normalized.groupName,
    securityStatus: normalized.securityStatus,
    capabilityStatus: normalized.capabilityStatus,
    preparednessStatus: normalized.preparednessStatus,
    medicalStatus: normalized.medicalStatus,
    mobilityStatus: normalized.mobilityStatus,
    commsStatus: normalized.commsStatus,
    notes: normalized.notes,
    updatedAt: normalized.updatedAt,
    deletedAt: normalized.deletedAt,
    eamUid: normalized.eamUid,
    teamMemberUid: normalized.teamMemberUid,
    teamUid: normalized.teamUid,
    reportedAt: normalized.reportedAt,
    reportedBy: normalized.reportedBy,
    overallStatus: normalized.overallStatus,
    confidence: normalized.confidence,
    ttlSeconds: normalized.ttlSeconds,
    source: normalized.source
      ? {
          rnsIdentity: normalized.source.rns_identity,
          displayName: normalized.source.display_name,
        }
      : undefined,
    syncState: normalized.syncState,
    syncError: normalized.syncError,
    draftCreatedAt: normalized.draftCreatedAt,
    lastSyncedAt: normalized.lastSyncedAt,
  };
}

export function toEamTeamSummaryRecord(raw: Record<string, unknown>): EamTeamSummaryRecord | null {
  if (!raw || Object.keys(raw).length === 0 || raw.summary === null) {
    return null;
  }
  const source = raw.summary && typeof raw.summary === "object"
    ? raw.summary as Record<string, unknown>
    : raw;
  return {
    teamUid: String(source.teamUid ?? ""),
    total: Number(source.total ?? 0),
    activeTotal: Number(source.activeTotal ?? 0),
    deletedTotal: Number(source.deletedTotal ?? 0),
    overallStatus: typeof source.overallStatus === "string" ? source.overallStatus : undefined,
    greenTotal: Number(source.greenTotal ?? 0),
    yellowTotal: Number(source.yellowTotal ?? 0),
    redTotal: Number(source.redTotal ?? 0),
    updatedAt: Number(source.updatedAt ?? Date.now()),
  };
}

export function emptyEamReadinessSummary(): EamReadinessSummaryRecord {
  return {
    activeTotal: 0,
    updatedAt: 0,
    statusMetrics: [],
    messages: [],
  };
}

export function toEamReadinessStatusMetricRecord(raw: Record<string, unknown>): EamReadinessStatusMetricRecord {
  return {
    field: String(raw.field ?? ""),
    label: String(raw.label ?? ""),
    score: Number(raw.score ?? 0),
    band: String(raw.band ?? "Red"),
    ringColor: String(raw.ringColor ?? raw.ring_color ?? "#ff3648"),
  };
}

export function toEamReadinessMessageRecord(raw: Record<string, unknown>): EamReadinessMessageRecord {
  return {
    callsign: String(raw.callsign ?? ""),
    overallScore: Number(raw.overallScore ?? raw.overall_score ?? 0),
    overallBand: String(raw.overallBand ?? raw.overall_band ?? "Unknown"),
    overallRingColor: String(raw.overallRingColor ?? raw.overall_ring_color ?? "#ff3648"),
  };
}

export function toEamReadinessSummaryRecord(raw: Record<string, unknown>): EamReadinessSummaryRecord {
  const statusMetrics = Array.isArray(raw.statusMetrics)
    ? raw.statusMetrics
    : Array.isArray(raw.status_metrics)
      ? raw.status_metrics
      : [];
  const messages = Array.isArray(raw.messages) ? raw.messages : [];
  return {
    activeTotal: Number(raw.activeTotal ?? raw.active_total ?? 0),
    updatedAt: Number(raw.updatedAt ?? raw.updated_at_ms ?? raw.updated_at ?? 0),
    statusMetrics: statusMetrics
      .filter((entry): entry is Record<string, unknown> => Boolean(entry) && typeof entry === "object")
      .map(toEamReadinessStatusMetricRecord),
    messages: messages
      .filter((entry): entry is Record<string, unknown> => Boolean(entry) && typeof entry === "object")
      .map(toEamReadinessMessageRecord)
      .filter((entry) => entry.callsign.length > 0),
  };
}

export function toEventProjectionRecord(raw: Record<string, unknown>): EventProjectionRecord {
  const source = (raw.source ?? {}) as Record<string, unknown>;
  const args = (raw.args ?? {}) as Record<string, unknown>;
  const sourceIdentity = String(
    source.rns_identity
      ?? raw.source_identity
      ?? raw.sourceIdentity
      ?? args.source_identity
      ?? args.sourceIdentity
      ?? "",
  );
  const sourceDisplayName =
    typeof source.display_name === "string"
      ? source.display_name
      : typeof raw.source_display_name === "string"
        ? raw.source_display_name
        : typeof raw.sourceDisplayName === "string"
          ? raw.sourceDisplayName
          : typeof args.source_display_name === "string"
            ? args.source_display_name
            : typeof args.sourceDisplayName === "string"
              ? args.sourceDisplayName
              : undefined;
  const entryUid = String(args.entry_uid ?? args.entryUid ?? raw.uid ?? raw.entry_uid ?? raw.entryUid ?? "");
  const missionUid = String(args.mission_uid ?? args.missionUid ?? raw.mission_uid ?? raw.missionUid ?? "");
  const content = String(args.content ?? raw.content ?? "");
  const callsign = String(args.callsign ?? raw.callsign ?? "");
  const serverTime =
    typeof args.server_time === "string"
      ? args.server_time
      : typeof args.serverTime === "string"
        ? args.serverTime
        : typeof raw.server_time === "string"
          ? raw.server_time
          : typeof raw.serverTime === "string"
            ? raw.serverTime
            : undefined;
  const clientTime =
    typeof args.client_time === "string"
      ? args.client_time
      : typeof args.clientTime === "string"
        ? args.clientTime
        : typeof raw.client_time === "string"
          ? raw.client_time
          : typeof raw.clientTime === "string"
            ? raw.clientTime
            : undefined;
  const keywords = Array.isArray(args.keywords)
    ? args.keywords.map((entry) => String(entry))
    : Array.isArray(raw.keywords)
      ? raw.keywords.map((entry) => String(entry))
      : [];
  const contentHashes = Array.isArray(args.content_hashes)
    ? args.content_hashes.map((entry) => String(entry))
    : Array.isArray(args.contentHashes)
      ? args.contentHashes.map((entry) => String(entry))
      : Array.isArray(raw.content_hashes)
        ? raw.content_hashes.map((entry) => String(entry))
        : Array.isArray(raw.contentHashes)
          ? raw.contentHashes.map((entry) => String(entry))
          : [];
  return {
    command_id: String(raw.command_id ?? raw.commandId ?? ""),
    source: {
      rns_identity: sourceIdentity,
      display_name: sourceDisplayName,
    },
    timestamp: String(raw.timestamp ?? serverTime ?? clientTime ?? ""),
    command_type: String(raw.command_type ?? raw.commandType ?? ""),
    args: {
      entry_uid: entryUid,
      mission_uid: missionUid,
      content,
      callsign,
      server_time: serverTime,
      client_time: clientTime,
      keywords,
      content_hashes: contentHashes,
      source_identity: sourceIdentity || undefined,
      source_display_name: sourceDisplayName,
    },
    correlation_id:
      typeof raw.correlation_id === "string"
        ? raw.correlation_id
        : typeof raw.correlationId === "string"
          ? raw.correlationId
          : undefined,
    topics: Array.isArray(raw.topics) ? raw.topics.map((entry) => String(entry)) : [],
    deleted_at: toOptionalNumber(raw.deleted_at ?? raw.deletedAt),
    updatedAt: Number(raw.updatedAt ?? raw.updated_at ?? Date.now()),
  };
}

export function eventProjectionRecordToPlugin(record: EventProjectionRecord): Record<string, unknown> {
  const normalized = toEventProjectionRecord(record as unknown as Record<string, unknown>);
  return {
    uid: normalized.args.entry_uid,
    commandId: normalized.command_id,
    sourceIdentity: normalized.args.source_identity ?? normalized.source.rns_identity,
    sourceDisplayName: normalized.args.source_display_name ?? normalized.source.display_name,
    timestamp: normalized.timestamp,
    commandType: normalized.command_type,
    missionUid: normalized.args.mission_uid,
    content: normalized.args.content,
    callsign: normalized.args.callsign,
    serverTime: normalized.args.server_time,
    clientTime: normalized.args.client_time,
    keywords: normalized.args.keywords,
    contentHashes: normalized.args.content_hashes,
    updatedAt: normalized.updatedAt,
    deletedAt: normalized.deleted_at,
    correlationId: normalized.correlation_id,
    topics: normalized.topics,
  };
}

export function legacyImportPayloadToPlugin(payload: LegacyImportPayload): Record<string, unknown> {
  return {
    settings: payload.settings as unknown as Record<string, unknown> | undefined,
    savedPeers: payload.savedPeers as unknown as Record<string, unknown>[],
    eams: payload.eams.map(eamProjectionRecordToPlugin),
    events: payload.events.map(eventProjectionRecordToPlugin),
    messages: payload.messages as unknown as Record<string, unknown>[],
    telemetryPositions: payload.telemetryPositions as unknown as Record<string, unknown>[],
  };
}

export function toTelemetryPositionRecord(raw: Record<string, unknown>): TelemetryPositionRecord {
  return {
    callsign: String(raw.callsign ?? ""),
    lat: Number(raw.lat ?? 0),
    lon: Number(raw.lon ?? 0),
    alt: toOptionalNumber(raw.alt),
    course: toOptionalNumber(raw.course),
    speed: toOptionalNumber(raw.speed),
    accuracy: toOptionalNumber(raw.accuracy),
    updatedAt: Number(raw.updatedAt ?? Date.now()),
  };
}

export function toCommunityStatusProjectionRecord(
  raw: Record<string, unknown>,
): CommunityStatusProjectionRecord {
  return {
    householdId: String(raw.householdId ?? raw.household_id ?? ""),
    householdName: String(raw.householdName ?? raw.household_name ?? ""),
    adults: Number(raw.adults ?? 0),
    children: Number(raw.children ?? 0),
    pets: Number(raw.pets ?? 0),
    roleBadges: Array.isArray(raw.roleBadges ?? raw.role_badges)
      ? (raw.roleBadges ?? raw.role_badges) as string[]
      : [],
    status: String(raw.status ?? "all_home") as HouseholdStatus,
    saverActive: Boolean(raw.saverActive ?? raw.saver_active),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? 0),
    sourceIdentity: String(raw.sourceIdentity ?? raw.source_identity ?? ""),
  };
}
