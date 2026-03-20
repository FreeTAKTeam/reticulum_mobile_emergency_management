import {
  buildMissionCommandFieldsBase64,
  buildMissionResponseFieldsBase64,
  createMissionAcceptedPayload,
  createMissionCommandEnvelope,
  createMissionEventEnvelope,
  createMissionRejectedPayload,
  createMissionResultPayload,
  parseMissionSyncFields,
  type MissionAcceptedPayload,
  type MissionCommandEnvelope,
  type MissionEventEnvelope,
  type MissionRejectedPayload,
  type MissionResponsePayload,
  type MissionResultPayload,
} from "./missionSync";
import {
  EAM_COMMAND_TYPES,
  EAM_EVENT_TYPES,
  type EamCommandArgsByType,
  type EamCommandType,
  type EamDeleteResult,
  type EamEntityResult,
  type EamEventPayloadByType,
  type EamEventType,
  type EamListResult,
  type EamRecord,
  type EamResultByCommandType,
  type EamSource,
  type EamTeamSummary,
  type EamTeamSummaryResult,
  type EamWireStatus,
} from "../types/domain";

export type EamCommandEnvelope<T extends EamCommandType = EamCommandType> = Omit<
  MissionCommandEnvelope,
  "command_type" | "args"
> & {
  command_type: T;
  args: EamCommandArgsByType[T];
};

export type EamEventEnvelope<T extends EamEventType = EamEventType> = Omit<
  MissionEventEnvelope,
  "event_type" | "payload"
> & {
  event_type: T;
  payload: EamEventPayloadByType[T];
};

export type EamResultBody =
  | EamListResult
  | EamEntityResult
  | EamDeleteResult
  | EamTeamSummaryResult;

export type EamResultEnvelope<T extends EamCommandType = EamCommandType> = Omit<
  MissionResultPayload,
  "result"
> & {
  result: EamResultByCommandType[T];
};

export type EamResponsePayload =
  | MissionAcceptedPayload
  | MissionRejectedPayload
  | EamResultEnvelope;

export interface ParsedEamMissionSyncFields {
  commands: EamCommandEnvelope[];
  result: EamResponsePayload | null;
  event: EamEventEnvelope | null;
}

const EAM_STATUS_PRIORITY: Record<EamWireStatus, number> = {
  Green: 1,
  Yellow: 2,
  Red: 3,
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  if (value instanceof Map) {
    const normalized: Record<string, unknown> = {};
    for (const [key, entry] of value.entries()) {
      normalized[String(key)] = entry;
    }
    return normalized;
  }
  return value as Record<string, unknown>;
}

function asString(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function asNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function asBoolean(value: unknown): boolean | undefined {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true") {
      return true;
    }
    if (normalized === "false") {
      return false;
    }
  }
  return undefined;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => asString(entry))
    .filter((entry): entry is string => entry !== undefined);
}

function asRecordArray(value: unknown): Record<string, unknown>[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => asRecord(entry))
    .filter((entry): entry is Record<string, unknown> => entry !== null);
}

function pickString(record: Record<string, unknown>, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = asString(record[key]);
    if (value) {
      return value;
    }
  }
  return undefined;
}

function pickNumber(record: Record<string, unknown>, ...keys: string[]): number | undefined {
  for (const key of keys) {
    const value = asNumber(record[key]);
    if (typeof value === "number") {
      return value;
    }
  }
  return undefined;
}

function pickBoolean(record: Record<string, unknown>, ...keys: string[]): boolean | undefined {
  for (const key of keys) {
    const value = asBoolean(record[key]);
    if (typeof value === "boolean") {
      return value;
    }
  }
  return undefined;
}

function normalizeSource(value: unknown): EamSource | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const rnsIdentity = pickString(record, "rns_identity", "rnsIdentity");
  if (!rnsIdentity) {
    return null;
  }

  return {
    rns_identity: rnsIdentity,
    display_name: pickString(record, "display_name", "displayName"),
  };
}

function normalizeWireStatus(value: unknown): EamWireStatus | undefined {
  const status = asString(value);
  if (status === "Red" || status === "Yellow" || status === "Green") {
    return status;
  }
  return undefined;
}

function normalizeStatusFields(record: Record<string, unknown>): Partial<EamRecord> {
  const securityStatus = normalizeWireStatus(
    record.security_status ?? record.securityStatus,
  );
  const capabilityStatus = normalizeWireStatus(
    record.capability_status ?? record.capabilityStatus,
  );
  const preparednessStatus = normalizeWireStatus(
    record.preparedness_status ?? record.preparednessStatus,
  );
  const medicalStatus = normalizeWireStatus(record.medical_status ?? record.medicalStatus);
  const mobilityStatus = normalizeWireStatus(record.mobility_status ?? record.mobilityStatus);
  const commsStatus = normalizeWireStatus(record.comms_status ?? record.commsStatus);

  return {
    security_status: securityStatus,
    capability_status: capabilityStatus,
    preparedness_status: preparednessStatus,
    medical_status: medicalStatus,
    mobility_status: mobilityStatus,
    comms_status: commsStatus,
    overall_status:
      normalizeWireStatus(record.overall_status ?? record.overallStatus) ??
      deriveOverallStatus({
        security_status: securityStatus,
        capability_status: capabilityStatus,
        preparedness_status: preparednessStatus,
        medical_status: medicalStatus,
        mobility_status: mobilityStatus,
        comms_status: commsStatus,
      }),
  };
}

function deriveOverallStatus(fields: Partial<EamRecord>): EamWireStatus | undefined {
  const statuses = [
    fields.security_status,
    fields.capability_status,
    fields.preparedness_status,
    fields.medical_status,
    fields.mobility_status,
    fields.comms_status,
  ].filter((status): status is EamWireStatus => status !== undefined);

  if (statuses.length === 0) {
    return undefined;
  }

  return statuses.reduce((worst, current) => {
    return EAM_STATUS_PRIORITY[current] > EAM_STATUS_PRIORITY[worst] ? current : worst;
  });
}

function normalizeEamRecord(value: unknown): EamRecord | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const callsign = pickString(record, "callsign");
  const teamMemberUid = pickString(record, "team_member_uid", "teamMemberUid");
  const teamUid = pickString(record, "team_uid", "teamUid");
  if (!callsign || !teamMemberUid || !teamUid) {
    return null;
  }

  const source = normalizeSource(record.source);
  const statusFields = normalizeStatusFields(record);
  return {
    eam_uid: pickString(record, "eam_uid", "eamUid"),
    callsign,
    team_member_uid: teamMemberUid,
    team_uid: teamUid,
    reported_by: pickString(record, "reported_by", "reportedBy"),
    reported_at: pickString(record, "reported_at", "reportedAt"),
    notes: pickString(record, "notes"),
    confidence: pickNumber(record, "confidence"),
    ttl_seconds: pickNumber(record, "ttl_seconds", "ttlSeconds"),
    source: source ?? undefined,
    ...statusFields,
  };
}

function normalizeEamTeamSummary(value: unknown): EamTeamSummary | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const teamUid = pickString(record, "team_uid", "teamUid");
  if (!teamUid) {
    return null;
  }

  const byStatusSource = asRecord(record.by_status ?? record.byStatus ?? record.status_counts);
  const byStatus: Partial<Record<EamWireStatus, number>> = {};
  if (byStatusSource) {
    const red = pickNumber(byStatusSource, "Red", "red");
    const yellow = pickNumber(byStatusSource, "Yellow", "yellow");
    const green = pickNumber(byStatusSource, "Green", "green");
    if (typeof red === "number") {
      byStatus.Red = red;
    }
    if (typeof yellow === "number") {
      byStatus.Yellow = yellow;
    }
    if (typeof green === "number") {
      byStatus.Green = green;
    }
  }

  return {
    ...record,
    team_uid: teamUid,
    team_name: pickString(record, "team_name", "teamName"),
    total: pickNumber(record, "total", "eam_count", "eamCount"),
    active_total: pickNumber(record, "active_total", "activeTotal", "active_eam_count", "activeEamCount"),
    deleted_total: pickNumber(record, "deleted_total", "deletedTotal", "deleted_eam_count", "deletedEamCount"),
    overall_status: normalizeWireStatus(record.overall_status ?? record.overallStatus),
    by_status: Object.keys(byStatus).length > 0 ? byStatus : undefined,
    updated_at: pickString(record, "updated_at", "updatedAt"),
  } as EamTeamSummary;
}

function normalizeEamResultBody(value: unknown): EamResultBody | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  if (Array.isArray(record.eams)) {
    return {
      eams: asRecordArray(record.eams)
        .map((entry) => normalizeEamRecord(entry))
        .filter((entry): entry is EamRecord => entry !== null),
    };
  }

  if (Array.isArray(record.messages)) {
    return {
      eams: asRecordArray(record.messages)
        .map((entry) => normalizeEamRecord(entry))
        .filter((entry): entry is EamRecord => entry !== null),
    };
  }

  if (record.summary) {
    const summary = normalizeEamTeamSummary(record.summary);
    if (summary) {
      return { summary };
    }
  }

  const deleteStatus = asString(record.status);
  const hasEamProperty = Object.prototype.hasOwnProperty.call(record, "eam");
  const nestedEam = hasEamProperty ? record.eam : undefined;

  if (deleteStatus === "deleted" || deleteStatus === "not_found") {
    const eam = nestedEam === null || nestedEam === undefined ? null : normalizeEamRecord(nestedEam);
    if (nestedEam !== null && nestedEam !== undefined && !eam) {
      return null;
    }
    return {
      eam,
      status: deleteStatus,
      eam_uid: pickString(record, "eam_uid", "eamUid") ?? eam?.eam_uid,
      callsign: pickString(record, "callsign") ?? eam?.callsign,
    };
  }

  if (hasEamProperty) {
    const eam = nestedEam === null || nestedEam === undefined ? null : normalizeEamRecord(nestedEam);
    if (nestedEam !== null && nestedEam !== undefined && !eam) {
      return null;
    }
    return { eam };
  }

  const eam = normalizeEamRecord(record);
  if (!eam) {
    return null;
  }

  return { eam };
}

function normalizeEamEventPayload(
  eventType: EamEventType,
  value: unknown,
): EamEventPayloadByType[EamEventType] | null {
  const payload = normalizeEamResultBody(value);
  if (!payload) {
    return null;
  }

  switch (eventType) {
    case "mission.registry.eam.listed":
      return "eams" in payload ? payload : null;
    case "mission.registry.eam.upserted":
    case "mission.registry.eam.retrieved":
    case "mission.registry.eam.latest_retrieved":
      return "eam" in payload && !("status" in payload) ? payload : null;
    case "mission.registry.eam.deleted":
      return "status" in payload ? payload : null;
    case "mission.registry.eam.team_summary.retrieved":
      return "summary" in payload ? payload : null;
    default:
      return null;
  }
}

function normalizeEamCommandArgs<T extends EamCommandType>(
  commandType: T,
  value: unknown,
): EamCommandArgsByType[T] | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  switch (commandType) {
    case "mission.registry.eam.list":
      return {
        eam_uid: pickString(record, "eam_uid", "eamUid"),
        callsign: pickString(record, "callsign"),
        team_uid: pickString(record, "team_uid", "teamUid"),
        team_member_uid: pickString(record, "team_member_uid", "teamMemberUid"),
        limit: pickNumber(record, "limit"),
        offset: pickNumber(record, "offset"),
        include_deleted: pickBoolean(record, "include_deleted", "includeDeleted"),
      } as EamCommandArgsByType[T];
    case "mission.registry.eam.upsert": {
      const callsign = pickString(record, "callsign");
      const teamMemberUid = pickString(record, "team_member_uid", "teamMemberUid");
      const teamUid = pickString(record, "team_uid", "teamUid");
      if (!callsign || !teamMemberUid || !teamUid) {
        return null;
      }

      const source = normalizeSource(record.source);
      return {
        eam_uid: pickString(record, "eam_uid", "eamUid"),
        callsign,
        team_member_uid: teamMemberUid,
        team_uid: teamUid,
        reported_by: pickString(record, "reported_by", "reportedBy"),
        reported_at: pickString(record, "reported_at", "reportedAt"),
        security_status: normalizeWireStatus(record.security_status ?? record.securityStatus),
        capability_status: normalizeWireStatus(record.capability_status ?? record.capabilityStatus),
        preparedness_status: normalizeWireStatus(
          record.preparedness_status ?? record.preparednessStatus,
        ),
        medical_status: normalizeWireStatus(record.medical_status ?? record.medicalStatus),
        mobility_status: normalizeWireStatus(record.mobility_status ?? record.mobilityStatus),
        comms_status: normalizeWireStatus(record.comms_status ?? record.commsStatus),
        notes: pickString(record, "notes"),
        confidence: pickNumber(record, "confidence"),
        ttl_seconds: pickNumber(record, "ttl_seconds", "ttlSeconds"),
        source: source ?? undefined,
      } as EamCommandArgsByType[T];
    }
    case "mission.registry.eam.get":
    case "mission.registry.eam.latest":
    case "mission.registry.eam.delete":
      return {
        eam_uid: pickString(record, "eam_uid", "eamUid"),
        callsign: pickString(record, "callsign"),
        team_uid: pickString(record, "team_uid", "teamUid"),
        team_member_uid: pickString(record, "team_member_uid", "teamMemberUid"),
      } as EamCommandArgsByType[T];
    case "mission.registry.eam.team.summary": {
      const teamUid = pickString(record, "team_uid", "teamUid");
      if (!teamUid) {
        return null;
      }
      return {
        team_uid: teamUid,
        team_member_uid: pickString(record, "team_member_uid", "teamMemberUid"),
        callsign: pickString(record, "callsign"),
      } as EamCommandArgsByType[T];
    }
    default:
      return null;
  }
}

function normalizeEamCommandEnvelope(value: unknown): EamCommandEnvelope | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const commandType = asString(record.command_type);
  if (!commandType || !isEamCommandType(commandType)) {
    return null;
  }

  const source = normalizeSource(record.source);
  const commandId = asString(record.command_id);
  const timestamp = asString(record.timestamp);
  const args = normalizeEamCommandArgs(commandType, record.args);
  if (!source || !commandId || !timestamp || !args) {
    return null;
  }

  return {
    command_id: commandId,
    source,
    timestamp,
    command_type: commandType,
    args,
    correlation_id: asString(record.correlation_id),
    topics: asStringArray(record.topics),
  } as EamCommandEnvelope;
}

function normalizeEamEventEnvelope(value: unknown): EamEventEnvelope | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const eventType = asString(record.event_type);
  if (!eventType || !isEamEventType(eventType)) {
    return null;
  }

  const source = normalizeSource(record.source);
  const eventId = asString(record.event_id);
  const timestamp = asString(record.timestamp);
  const payload = normalizeEamEventPayload(eventType, record.payload);
  if (!source || !eventId || !timestamp || !payload) {
    return null;
  }

  return {
    event_id: eventId,
    source,
    timestamp,
    event_type: eventType,
    topics: asStringArray(record.topics),
    payload,
    meta: asRecord(record.meta) ?? undefined,
  } as EamEventEnvelope;
}

function isEamCommandType(value: string): value is EamCommandType {
  return EAM_COMMAND_TYPES.includes(value as (typeof EAM_COMMAND_TYPES)[number]);
}

function isEamEventType(value: string): value is EamEventType {
  return EAM_EVENT_TYPES.includes(value as (typeof EAM_EVENT_TYPES)[number]);
}

export function createEamCommandEnvelope<T extends EamCommandType>(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  commandType: T;
  args: EamCommandArgsByType[T];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<T> {
  return createMissionCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: options.commandType,
    args: options.args as unknown as Record<string, unknown>,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  }) as unknown as EamCommandEnvelope<T>;
}

export function createEamListCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.list"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.list"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.list",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamUpsertCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.upsert"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.upsert"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.upsert",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamGetCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.get"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.get"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.get",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamLatestCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.latest"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.latest"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.latest",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamDeleteCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.delete"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.delete"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.delete",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamTeamSummaryCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  args: EamCommandArgsByType["mission.registry.eam.team.summary"];
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): EamCommandEnvelope<"mission.registry.eam.team.summary"> {
  return createEamCommandEnvelope({
    commandId: options.commandId,
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    commandType: "mission.registry.eam.team.summary",
    args: options.args,
    correlationId: options.correlationId,
    topics: options.topics,
    timestamp: options.timestamp,
  });
}

export function createEamAcceptedPayload(options: {
  commandId: string;
  correlationId?: string;
  byIdentity?: string;
  acceptedAt?: string;
}): MissionAcceptedPayload {
  return createMissionAcceptedPayload(options);
}

export function createEamRejectedPayload(options: {
  commandId: string;
  reasonCode: string;
  reason?: string;
  correlationId?: string;
  requiredCapabilities?: string[];
}): MissionRejectedPayload {
  return createMissionRejectedPayload(options);
}

export function createEamResultPayload<T extends EamCommandType>(options: {
  commandId: string;
  result: EamResultByCommandType[T];
  correlationId?: string;
}): EamResultEnvelope<T> {
  return createMissionResultPayload({
    commandId: options.commandId,
    result: options.result as unknown as Record<string, unknown>,
    correlationId: options.correlationId,
  }) as unknown as EamResultEnvelope<T>;
}

export function createEamListResultPayload(options: {
  commandId: string;
  eams: EamRecord[];
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.list"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: { eams: options.eams },
    correlationId: options.correlationId,
  });
}

export function createEamUpsertResultPayload(options: {
  commandId: string;
  eam: EamRecord | null;
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.upsert"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: { eam: options.eam },
    correlationId: options.correlationId,
  });
}

export function createEamGetResultPayload(options: {
  commandId: string;
  eam: EamRecord | null;
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.get"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: { eam: options.eam },
    correlationId: options.correlationId,
  });
}

export function createEamLatestResultPayload(options: {
  commandId: string;
  eam: EamRecord | null;
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.latest"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: { eam: options.eam },
    correlationId: options.correlationId,
  });
}

export function createEamDeleteResultPayload(options: {
  commandId: string;
  eam: EamRecord | null;
  status?: "deleted" | "not_found";
  eamUid?: string;
  callsign?: string;
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.delete"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: {
      eam: options.eam,
      status: options.status,
      eam_uid: options.eamUid,
      callsign: options.callsign,
    },
    correlationId: options.correlationId,
  });
}

export function createEamTeamSummaryResultPayload(options: {
  commandId: string;
  summary: EamTeamSummary;
  correlationId?: string;
}): EamResultEnvelope<"mission.registry.eam.team.summary"> {
  return createEamResultPayload({
    commandId: options.commandId,
    result: { summary: options.summary },
    correlationId: options.correlationId,
  });
}

export function createEamEventEnvelope<T extends EamEventType>(options: {
  sourceIdentity: string;
  sourceDisplayName?: string;
  eventType: T;
  payload: EamEventPayloadByType[T];
  topics?: string[];
  meta?: Record<string, unknown>;
  eventId?: string;
  timestamp?: string;
}): EamEventEnvelope<T> {
  return createMissionEventEnvelope({
    sourceIdentity: options.sourceIdentity,
    sourceDisplayName: options.sourceDisplayName,
    eventType: options.eventType,
    payload: options.payload as unknown as Record<string, unknown>,
    topics: options.topics,
    meta: options.meta,
    eventId: options.eventId,
    timestamp: options.timestamp,
  }) as unknown as EamEventEnvelope<T>;
}

export function buildEamCommandFieldsBase64(commands: EamCommandEnvelope[]): string {
  return buildMissionCommandFieldsBase64(commands as unknown as MissionCommandEnvelope[]);
}

export function buildEamResponseFieldsBase64(options: {
  result: EamResponsePayload;
  event?: EamEventEnvelope;
}): string {
  return buildMissionResponseFieldsBase64({
    result: options.result as unknown as MissionResponsePayload,
    event: options.event as unknown as MissionEventEnvelope | undefined,
  });
}

export function parseEamMissionSyncFields(
  fieldsBase64: string | undefined,
): ParsedEamMissionSyncFields | null {
  const parsed = parseMissionSyncFields(fieldsBase64);
  if (!parsed) {
    return null;
  }

  const commands = parsed.commands
    .map((command) => normalizeEamCommandEnvelope(command))
    .filter((command): command is EamCommandEnvelope => command !== null);
  const result = normalizeEamResponsePayload(parsed.result);
  const event = normalizeEamEventEnvelope(parsed.event);

  if (commands.length === 0 && !result && !event) {
    return null;
  }

  return {
    commands,
    result,
    event,
  };
}

function normalizeEamResponsePayload(value: MissionResponsePayload | null): EamResponsePayload | null {
  if (!value) {
    return null;
  }

  if (value.status === "accepted") {
    return value;
  }

  if (value.status === "rejected") {
    return value;
  }

  if (value.status === "result") {
    const result = normalizeEamResultBody(value.result);
    if (!result) {
      return null;
    }
    return {
      command_id: value.command_id,
      status: "result",
      result: result as unknown as EamResultByCommandType[EamCommandType],
      correlation_id: value.correlation_id,
    } as unknown as EamResultEnvelope;
  }

  return null;
}
