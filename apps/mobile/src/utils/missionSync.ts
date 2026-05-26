import { pack, unpack } from "msgpackr";

import { asRecord } from "./records";

export const LXMF_FIELD_COMMANDS = 0x09;
export const LXMF_FIELD_RESULTS = 0x0A;
export const LXMF_FIELD_EVENT = 0x0D;
const DEFAULT_R3AKT_MISSION_UID = "r3akt-default-mission";

export interface MissionCommandSource {
  rns_identity: string;
  display_name?: string;
}

export interface MissionCommandEnvelope {
  command_id: string;
  source: MissionCommandSource;
  timestamp: string;
  command_type: string;
  args: Record<string, unknown>;
  correlation_id?: string;
  topics: string[];
}

export interface MissionAcceptedPayload {
  command_id: string;
  status: "accepted";
  accepted_at: string;
  correlation_id?: string;
  by_identity?: string;
}

export interface MissionRejectedPayload {
  command_id: string;
  status: "rejected";
  reason_code: string;
  reason?: string;
  correlation_id?: string;
  required_capabilities?: string[];
}

export interface MissionResultPayload {
  command_id: string;
  status: "result";
  result: Record<string, unknown>;
  correlation_id?: string;
}

export type MissionResponsePayload =
  | MissionAcceptedPayload
  | MissionRejectedPayload
  | MissionResultPayload;

export interface MissionEventEnvelope {
  event_id: string;
  source: MissionCommandSource;
  timestamp: string;
  event_type: string;
  topics: string[];
  payload: Record<string, unknown>;
  meta?: Record<string, unknown>;
}

export interface ParsedMissionSyncFields {
  commands: MissionCommandEnvelope[];
  result: MissionResponsePayload | null;
  event: MissionEventEnvelope | null;
}

const COMMAND_TYPE_BY_CODE: Record<string, string> = {
  E1: "mission.registry.log_entry.upsert",
  E2: "mission.registry.log_entry.upserted",
  M1: "mission.registry.eam.upsert",
  M2: "mission.registry.eam.delete",
  M3: "mission.registry.eam.upserted",
  M4: "mission.registry.eam.list",
  M5: "mission.registry.eam.get",
  M6: "mission.registry.eam.latest",
  M7: "mission.registry.eam.team.summary",
  M8: "mission.registry.eam.listed",
  M9: "mission.registry.eam.retrieved",
  MA: "mission.registry.eam.latest_retrieved",
  MB: "mission.registry.eam.deleted",
  MC: "mission.registry.eam.team_summary.retrieved",
  H1: "mission.registry.team.list",
  H2: "mission.registry.team.upsert",
  H3: "mission.registry.team_member.list",
  H4: "mission.registry.team_member.upsert",
  H5: "mission.registry.team_member.client.link",
  T1: "mission.registry.telemetry.upsert",
  S1: "sos.status",
  C1: "checklist.create.online",
  C2: "checklist.upload",
  C3: "checklist.update",
  C4: "checklist.delete",
  C5: "checklist.join",
  C6: "checklist.task.status.set",
  C7: "checklist.task.row.add",
  C8: "checklist.task.row.delete",
  C9: "checklist.task.row.style.set",
  CA: "checklist.task.cell.set",
};

const COMMAND_CODE_BY_TYPE: Record<string, string> = Object.fromEntries(
  Object.entries(COMMAND_TYPE_BY_CODE).map(([code, commandType]) => [commandType, code]),
);

const CHECKLIST_ARG_CODE_BY_KEY: Record<string, string> = {
  checklist_uid: "cl",
  checklistUid: "cl",
  mission_uid: "m",
  missionUid: "m",
  template_uid: "tp",
  templateUid: "tp",
  name: "n",
  description: "d",
  start_time: "st",
  startTime: "st",
  columns: "cols",
  tasks: "tasks",
  participant_rns_identities: "p",
  participantRnsIdentities: "p",
  created_at: "ca",
  createdAt: "ca",
  created_by_team_member_rns_identity: "cr",
  createdByTeamMemberRnsIdentity: "cr",
  created_by_team_member_display_name: "cdn",
  createdByTeamMemberDisplayName: "cdn",
  total_tasks: "tt",
  totalTasks: "tt",
  uploaded_at: "ua",
  uploadedAt: "ua",
  patch: "pa",
  task_uid: "tsk",
  taskUid: "tsk",
  number: "no",
  due_relative_minutes: "dr",
  dueRelativeMinutes: "dr",
  due_dtg: "dd",
  dueDtg: "dd",
  notes: "nt",
  legacy_value: "lv",
  legacyValue: "lv",
  changed_by_team_member_rns_identity: "cb",
  changedByTeamMemberRnsIdentity: "cb",
  user_status: "us",
  userStatus: "us",
  row_background_color: "bg",
  rowBackgroundColor: "bg",
  line_break_enabled: "lb",
  lineBreakEnabled: "lb",
  column_uid: "col",
  columnUid: "col",
  column_name: "cn",
  columnName: "cn",
  display_order: "ord",
  displayOrder: "ord",
  column_type: "ct",
  columnType: "ct",
  column_editable: "ce",
  columnEditable: "ce",
  text_color: "tc",
  textColor: "tc",
  is_removable: "rm",
  isRemovable: "rm",
  system_key: "sk",
  systemKey: "sk",
  value: "v",
  updated_by_team_member_rns_identity: "ub",
  updatedByTeamMemberRnsIdentity: "ub",
  task: "tr",
  snapshot: "sn",
  snapshot_json: "sj",
  snapshotJson: "sj",
};

const CHECKLIST_ARG_KEY_BY_CODE: Record<string, string> = {
  cl: "checklist_uid",
  m: "mission_uid",
  tp: "template_uid",
  n: "name",
  d: "description",
  st: "start_time",
  cols: "columns",
  tasks: "tasks",
  p: "participant_rns_identities",
  ca: "created_at",
  cr: "created_by_team_member_rns_identity",
  cdn: "created_by_team_member_display_name",
  tt: "total_tasks",
  ua: "uploaded_at",
  pa: "patch",
  tsk: "task_uid",
  no: "number",
  dr: "due_relative_minutes",
  dd: "due_dtg",
  nt: "notes",
  lv: "legacy_value",
  cb: "changed_by_team_member_rns_identity",
  us: "user_status",
  bg: "row_background_color",
  lb: "line_break_enabled",
  col: "column_uid",
  cn: "column_name",
  ord: "display_order",
  ct: "column_type",
  ce: "column_editable",
  tc: "text_color",
  rm: "is_removable",
  sk: "system_key",
  v: "value",
  ub: "updated_by_team_member_rns_identity",
  tr: "task",
  sn: "snapshot",
  sj: "snapshot_json",
};

function commandWireValue(commandType: string): string {
  return COMMAND_CODE_BY_TYPE[commandType] ?? commandType;
}

function encodeBytesToBase64(value: Uint8Array): string {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: Uint8Array): { toString(encoding: string): string } };
    }
  ).Buffer;
  if (bufferCtor) {
    return bufferCtor.from(value).toString("base64");
  }

  let binary = "";
  for (const byte of value) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function decodeBase64ToBytes(value: string): Uint8Array {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: string, encoding: string): Uint8Array };
    }
  ).Buffer;
  if (bufferCtor) {
    return Uint8Array.from(bufferCtor.from(value, "base64"));
  }

  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

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

function canonicalCommandType(value: string): string {
  return COMMAND_TYPE_BY_CODE[value] ?? value;
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

function expandChecklistCommandArgs(args: Record<string, unknown>): Record<string, unknown> {
  const normalized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const expandedKey = CHECKLIST_ARG_KEY_BY_CODE[key] ?? key;
    normalized[expandedKey] =
      expandedKey === "patch" && asRecord(value) ? expandChecklistCommandArgs(asRecord(value) ?? {}) : value;
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

function compactMissionCommandArgs(commandType: string, args: Record<string, unknown>): Record<string, unknown> {
  if (!commandType.startsWith("checklist.")) {
    return args;
  }
  const compact: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const wireKey = CHECKLIST_ARG_CODE_BY_KEY[key] ?? key;
    compact[wireKey] =
      key === "patch" && asRecord(value) ? compactMissionCommandArgs(commandType, asRecord(value) ?? {}) : value;
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
