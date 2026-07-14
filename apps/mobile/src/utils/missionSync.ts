import {
  base64ToBytes as decodeBase64ToBytes,
  bytesToBase64 as encodeBytesToBase64,
} from "@reticulum/node-client";
import { pack, unpack } from "msgpackr";

import { asRecord } from "./records";
import {
  canonicalCommandType,
  commandWireValue,
  compactMissionCommandArgs,
  expandChecklistCommandArgs,
} from "./missionSyncCodebook";
import {
  LXMF_FIELD_COMMANDS,
  LXMF_FIELD_EVENT,
  LXMF_FIELD_RESULTS,
  type MissionAcceptedPayload,
  type MissionCommandEnvelope,
  type MissionCommandSource,
  type MissionEventEnvelope,
  type MissionRejectedPayload,
  type MissionResponsePayload,
  type MissionResultPayload,
  type ParsedMissionSyncFields,
} from "./missionSyncContracts";

const DEFAULT_R3AKT_MISSION_UID = "r3akt-default-mission";
export { LXMF_FIELD_COMMANDS, LXMF_FIELD_EVENT, LXMF_FIELD_RESULTS } from "./missionSyncContracts";
export type {
  MissionAcceptedPayload, MissionCommandEnvelope, MissionCommandSource, MissionEventEnvelope,
  MissionRejectedPayload, MissionResponsePayload, MissionResultPayload,
  ParsedMissionSyncFields,
} from "./missionSyncContracts";

function getMapValue(source: unknown, key: number): unknown {
  if (!source || typeof source !== "object") {
    return undefined;
  }
  if (source instanceof Map) {
    return source.get(key) ?? source.get(String(key));
  }

  const record = source as Record<string, unknown>;
  return record[String(key)] ?? record[key as unknown as keyof typeof record];
}

function asString(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim();
  return normalized || undefined;
}

function asHexBytes(value: unknown): string | undefined {
  if (!(value instanceof Uint8Array)) {
    return undefined;
  }
  return [...value].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function asWireString(value: unknown): string | undefined {
  return asString(value) ?? asHexBytes(value);
}

function asTimestampString(value: unknown): string | undefined {
  const stringValue = asString(value);
  if (stringValue) {
    return stringValue;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    const timestampMs = value < 10_000_000_000 ? value * 1000 : value;
    return new Date(timestampMs).toISOString();
  }
  return undefined;
}

function recordValue(record: Record<string, unknown>, ...keys: string[]): unknown {
  for (const key of keys) {
    if (key in record) {
      return record[key];
    }
  }
  return undefined;
}

function eventUidFromWireValue(value: unknown): string | undefined {
  const hex = asHexBytes(value);
  if (hex?.length === 32) {
    return `evt-${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(
      16,
      20,
    )}-${hex.slice(20)}`;
  }
  return asString(value);
}

function eventCommandIdFromTail(uid: string, value: unknown): string | undefined {
  const hex = asHexBytes(value);
  if (hex?.length === 32) {
    return `log-entry-${uid}-${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(
      12,
      16,
    )}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  return asString(value);
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => asString(entry))
    .filter((entry): entry is string => entry !== undefined);
}

function asEventTopics(value: unknown, missionUid: string | undefined): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => {
      if (entry === 0 && missionUid) {
        return missionUid;
      }
      if (entry === 1) {
        return "Default";
      }
      return asString(entry);
    })
    .filter((entry): entry is string => entry !== undefined);
}

function normalizeEventKeyword(value: string): string {
  if (/^[A-Za-z0-9]{1,4}$/.test(value)) {
    return `r3akt:event-type:${value}`;
  }
  return value;
}

function asEventKeywords(value: unknown): string[] {
  return asStringArray(value).map(normalizeEventKeyword);
}

function missionUidFromWireValue(value: unknown): string | undefined {
  if (value === 0) {
    return DEFAULT_R3AKT_MISSION_UID;
  }
  return asString(value);
}

function normalizeMissionCommandArgs(
  commandType: string,
  args: Record<string, unknown>,
  commandTimestamp: string,
): Record<string, unknown> {
  if (commandType.startsWith("checklist.")) {
    return expandChecklistCommandArgs(args);
  }
  if (commandType !== "mission.registry.log_entry.upsert") {
    return args;
  }

  const normalized: Record<string, unknown> = { ...args };
  const entryUid = eventUidFromWireValue(recordValue(args, "entry_uid", "entryUid", "uid", "u"));
  if (entryUid) {
    normalized.entry_uid = entryUid;
  }
  const missionUid =
    missionUidFromWireValue(recordValue(args, "mission_uid", "missionUid", "m")) ?? DEFAULT_R3AKT_MISSION_UID;
  if (missionUid) {
    normalized.mission_uid = missionUid;
  }
  const serverTime = asTimestampString(recordValue(args, "server_time", "serverTime", "st")) ?? commandTimestamp;
  if (serverTime) {
    normalized.server_time = serverTime;
  }
  const clientTime = asTimestampString(recordValue(args, "client_time", "clientTime", "ct")) ?? commandTimestamp;
  if (clientTime) {
    normalized.client_time = clientTime;
  }
  const keywords = asEventKeywords(recordValue(args, "keywords", "kw"));
  if (keywords.length > 0) {
    normalized.keywords = keywords;
  }
  const contentHashes = asStringArray(recordValue(args, "content_hashes", "contentHashes", "ch"));
  if (contentHashes.length > 0) {
    normalized.content_hashes = contentHashes;
  }
  const deletedAtMs = recordValue(args, "deleted_at_ms", "deletedAtMs", "d");
  if (typeof deletedAtMs === "number" && Number.isFinite(deletedAtMs)) {
    normalized.deleted_at_ms = deletedAtMs;
  }
  return normalized;
}

function normalizeSource(value: unknown): MissionCommandSource | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }
  const rnsIdentity = asWireString(recordValue(record, "rns_identity", "r"));
  if (!rnsIdentity) {
    return null;
  }
  const displayName = asString(recordValue(record, "display_name", "n"));
  return {
    rns_identity: rnsIdentity,
    display_name: displayName,
  };
}

function normalizeMissionCommand(value: unknown): MissionCommandEnvelope | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }
  const source = normalizeSource(recordValue(record, "source", "s"));
  const args = asRecord(recordValue(record, "args", "a")) ?? {};
  const entryUid = eventUidFromWireValue(recordValue(args, "entry_uid", "entryUid", "uid", "u"));
  const commandId =
    asString(recordValue(record, "command_id", "i")) ??
    (entryUid ? eventCommandIdFromTail(entryUid, recordValue(args, "ci")) : undefined);
  const timestamp = asTimestampString(recordValue(record, "timestamp", "ts"));
  const commandType = asString(recordValue(record, "command_type", "t"));
  if (!source || !commandId || !timestamp || !commandType) {
    return null;
  }
  const canonicalType = canonicalCommandType(commandType);
  const normalizedArgs = normalizeMissionCommandArgs(canonicalType, args, timestamp);
  const missionUid = asString(normalizedArgs.mission_uid);
  const topics = asEventTopics(recordValue(record, "topics", "to"), missionUid);
  return {
    command_id: commandId,
    source,
    timestamp,
    command_type: canonicalType,
    args: normalizedArgs,
    correlation_id: asString(recordValue(record, "correlation_id", "c")) ?? commandId,
    topics:
      topics.length > 0
        ? topics
        : canonicalType === "mission.registry.log_entry.upsert" && missionUid
          ? [missionUid, "Default"]
          : [],
  };
}

function normalizeMissionResult(value: unknown): MissionResponsePayload | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }
  const commandId = asString(record.command_id);
  const status = asString(record.status);
  if (!commandId || !status) {
    return null;
  }

  if (status === "accepted") {
    const acceptedAt = asString(record.accepted_at);
    if (!acceptedAt) {
      return null;
    }
    return {
      command_id: commandId,
      status: "accepted",
      accepted_at: acceptedAt,
      correlation_id: asString(record.correlation_id),
      by_identity: asString(record.by_identity),
    };
  }

  if (status === "rejected") {
    const reasonCode = asString(record.reason_code);
    if (!reasonCode) {
      return null;
    }
    return {
      command_id: commandId,
      status: "rejected",
      reason_code: reasonCode,
      reason: asString(record.reason),
      correlation_id: asString(record.correlation_id),
      required_capabilities: asStringArray(record.required_capabilities),
    };
  }

  if (status === "result") {
    return {
      command_id: commandId,
      status: "result",
      result: asRecord(record.result) ?? {},
      correlation_id: asString(record.correlation_id),
    };
  }

  return null;
}

function normalizeMissionEvent(value: unknown): MissionEventEnvelope | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }
  const source = normalizeSource(record.source);
  const eventId = asString(record.event_id);
  const timestamp = asString(record.timestamp);
  const eventType = asString(record.event_type);
  const payload = asRecord(record.payload) ?? {};
  const meta = asRecord(record.meta) ?? undefined;
  if (!source || !eventId || !timestamp || !eventType) {
    return null;
  }
  return {
    event_id: eventId,
    source,
    timestamp,
    event_type: eventType,
    topics: asStringArray(record.topics),
    payload,
    meta,
  };
}

export function createMissionCommandEnvelope(options: {
  commandId: string;
  sourceIdentity: string;
  sourceDisplayName?: string;
  commandType: string;
  args: Record<string, unknown>;
  correlationId?: string;
  topics?: string[];
  timestamp?: string;
}): MissionCommandEnvelope {
  return {
    command_id: options.commandId,
    source: {
      rns_identity: options.sourceIdentity,
      display_name: options.sourceDisplayName,
    },
    timestamp: options.timestamp ?? new Date().toISOString(),
    command_type: options.commandType,
    args: options.args,
    correlation_id: options.correlationId,
    topics: [...new Set((options.topics ?? []).map((topic) => topic.trim()).filter((topic) => topic.length > 0))],
  };
}

export function createMissionAcceptedPayload(options: {
  commandId: string;
  correlationId?: string;
  byIdentity?: string;
  acceptedAt?: string;
}): MissionAcceptedPayload {
  return {
    command_id: options.commandId,
    status: "accepted",
    accepted_at: options.acceptedAt ?? new Date().toISOString(),
    correlation_id: options.correlationId,
    by_identity: options.byIdentity,
  };
}

export function createMissionRejectedPayload(options: {
  commandId: string;
  reasonCode: string;
  reason?: string;
  correlationId?: string;
  requiredCapabilities?: string[];
}): MissionRejectedPayload {
  return {
    command_id: options.commandId,
    status: "rejected",
    reason_code: options.reasonCode,
    reason: options.reason,
    correlation_id: options.correlationId,
    required_capabilities: options.requiredCapabilities,
  };
}

export function createMissionResultPayload(options: {
  commandId: string;
  result: Record<string, unknown>;
  correlationId?: string;
}): MissionResultPayload {
  return {
    command_id: options.commandId,
    status: "result",
    result: options.result,
    correlation_id: options.correlationId,
  };
}

export function createMissionEventEnvelope(options: {
  sourceIdentity: string;
  sourceDisplayName?: string;
  eventType: string;
  payload: Record<string, unknown>;
  topics?: string[];
  meta?: Record<string, unknown>;
  eventId?: string;
  timestamp?: string;
}): MissionEventEnvelope {
  return {
    event_id: options.eventId ?? (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function" ? crypto.randomUUID() : `${Date.now()}`),
    source: {
      rns_identity: options.sourceIdentity,
      display_name: options.sourceDisplayName,
    },
    timestamp: options.timestamp ?? new Date().toISOString(),
    event_type: options.eventType,
    topics: [...new Set((options.topics ?? []).map((topic) => topic.trim()).filter((topic) => topic.length > 0))],
    payload: options.payload,
    meta: options.meta,
  };
}

export function buildMissionCommandFieldsBase64(commands: MissionCommandEnvelope[]): string {
  return encodeBytesToBase64(
    Uint8Array.from(
      pack(new Map<number, unknown>([[LXMF_FIELD_COMMANDS, commands.map(compactMissionCommandEnvelope)]])),
    ),
  );
}

function compactMissionCommandEnvelope(command: MissionCommandEnvelope): Record<string, unknown> {
  const compactSource: Record<string, unknown> = {
    r: command.source.rns_identity,
  };
  if (command.source.display_name) {
    compactSource.n = command.source.display_name;
  }

  const compact: Record<string, unknown> = {
    i: command.command_id,
    s: compactSource,
    ts: command.timestamp,
    t: commandWireValue(command.command_type),
    a: compactMissionCommandArgs(command.command_type, command.args),
  };
  if (command.correlation_id) {
    compact.c = command.correlation_id;
  }
  if (command.topics.length > 0) {
    compact.to = command.topics;
  }
  return compact;
}

export function buildMissionResponseFieldsBase64(options: {
  result: MissionResponsePayload;
  event?: MissionEventEnvelope;
}): string {
  const fields = new Map<number, unknown>([[LXMF_FIELD_RESULTS, options.result]]);
  if (options.event) {
    fields.set(LXMF_FIELD_EVENT, options.event);
  }
  return encodeBytesToBase64(Uint8Array.from(pack(fields)));
}

export function parseMissionSyncFields(fieldsBase64: string | undefined): ParsedMissionSyncFields | null {
  if (!fieldsBase64) {
    return null;
  }

  let unpacked: unknown;
  try {
    unpacked = unpack(decodeBase64ToBytes(fieldsBase64));
  } catch {
    return null;
  }

  const commandField = getMapValue(unpacked, LXMF_FIELD_COMMANDS);
  const resultField = getMapValue(unpacked, LXMF_FIELD_RESULTS);
  const eventField = getMapValue(unpacked, LXMF_FIELD_EVENT);

  const commands = Array.isArray(commandField)
    ? commandField.map((entry) => normalizeMissionCommand(entry)).filter((entry): entry is MissionCommandEnvelope => entry !== null)
    : [];
  const result = normalizeMissionResult(resultField);
  const event = normalizeMissionEvent(eventField);

  if (commands.length === 0 && !result && !event) {
    return null;
  }

  return {
    commands,
    result,
    event,
  };
}
