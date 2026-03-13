import { pack, unpack } from "msgpackr";

export const LXMF_FIELD_COMMANDS = 0x09;
export const LXMF_FIELD_RESULTS = 0x0A;
export const LXMF_FIELD_EVENT = 0x0D;

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
  return normalized || undefined;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => asString(entry))
    .filter((entry): entry is string => entry !== undefined);
}

function normalizeSource(value: unknown): MissionCommandSource | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }
  const rnsIdentity = asString(record.rns_identity);
  if (!rnsIdentity) {
    return null;
  }
  const displayName = asString(record.display_name);
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
  const source = normalizeSource(record.source);
  const commandId = asString(record.command_id);
  const timestamp = asString(record.timestamp);
  const commandType = asString(record.command_type);
  const args = asRecord(record.args) ?? {};
  if (!source || !commandId || !timestamp || !commandType) {
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
    Uint8Array.from(pack(new Map<number, unknown>([[LXMF_FIELD_COMMANDS, commands]]))),
  );
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
