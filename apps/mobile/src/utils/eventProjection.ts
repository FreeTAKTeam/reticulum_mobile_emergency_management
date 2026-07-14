import type { EventProjectionRecord } from "@reticulum/node-client";

import {
  type DecodedMecpMessage,
  decodeMecpMessage,
  mecpCategoryLabel,
  mecpSeverityLabel,
  parseMecpMessage,
} from "./mecp";
import { DEFAULT_R3AKT_MISSION_UID } from "./r3akt";

const EVENT_STORAGE_KEY = "reticulum.mobile.events.v1";
const EVENT_TYPE_KEYWORD_PREFIX = "r3akt:event-type:";

export type EventTimelineRecord = {
  uid: string;
  type: string;
  summary: string;
  callsign: string;
  updatedAt: number;
  mecp?: {
    raw: string;
    severity: string;
    severityStatus: string;
    category: string;
    categoryCode: string;
    codes: string[];
    codeLabels: string[];
    details: string;
    extras: string[];
    warnings: string[];
    byteLength: number;
  };
};

export function createEventUid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `evt-${crypto.randomUUID()}`;
  }
  return `evt-${Date.now().toString(36)}-${Math.floor(Math.random() * 1_000_000).toString(36)}`;
}

export function createTrackingId(prefix: string, suffix?: string): string {
  const normalizedSuffix = suffix?.trim();
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return normalizedSuffix
      ? `${prefix}-${normalizedSuffix}-${crypto.randomUUID()}`
      : `${prefix}-${crypto.randomUUID()}`;
  }
  const entropy = Math.floor(Math.random() * 1_000_000).toString(36);
  return normalizedSuffix
    ? `${prefix}-${normalizedSuffix}-${Date.now().toString(36)}-${entropy}`
    : `${prefix}-${Date.now().toString(36)}-${entropy}`;
}

export function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function toIsoString(value: unknown): string | undefined {
  if (typeof value === "string") {
    return value.trim() || undefined;
  }
  return typeof value === "number" && Number.isFinite(value)
    ? new Date(value).toISOString()
    : undefined;
}

function toTimestampMs(value: unknown, fallback = Date.now()): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value > 1_000_000_000_000 ? Math.floor(value) : Math.floor(value * 1000);
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    if (!Number.isNaN(parsed)) return parsed;
  }
  return fallback;
}

function normalizeStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(value
    .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
    .filter((entry) => entry.length > 0))];
}

function normalizeTopics(value: unknown, missionUid: string): string[] {
  const topics = normalizeStringList(value);
  return topics.length > 0 ? topics : [missionUid];
}

function decodeEventType(keywords: string[], fallback = "Incident"): string {
  const tagged = keywords.find((keyword) => keyword.startsWith(EVENT_TYPE_KEYWORD_PREFIX));
  return tagged?.slice(EVENT_TYPE_KEYWORD_PREFIX.length).trim() || fallback;
}

export function encodeEventTypeKeywords(type: string, keywords: string[] = []): string[] {
  const normalizedType = type.trim() || "Incident";
  const filtered = normalizeStringList(keywords)
    .filter((keyword) => !keyword.startsWith(EVENT_TYPE_KEYWORD_PREFIX));
  return [...filtered, `${EVENT_TYPE_KEYWORD_PREFIX}${normalizedType}`];
}

export function getEventUid(record: EventProjectionRecord): string {
  return record.args.entry_uid;
}

export function getEventContent(record: EventProjectionRecord): string {
  return record.args.content;
}

function getEventType(record: EventProjectionRecord): string {
  const parsedMecp = parseMecpMessage(record.args.content);
  const fallback = parsedMecp.valid && parsedMecp.category ? parsedMecp.category : "Incident";
  return decodeEventType(normalizeStringList(record.args.keywords), fallback);
}

export function getEventUpdatedAt(record: EventProjectionRecord): number {
  return toTimestampMs(
    record.deleted_at ?? record.updatedAt ?? record.args.server_time
      ?? record.args.client_time ?? record.timestamp,
  );
}

function formatMecpExtraLabels(parsed: DecodedMecpMessage): string[] {
  if (!parsed.valid) return [];
  const extras: string[] = [];
  if (parsed.extras.pax !== null) extras.push(`${parsed.extras.pax} pax`);
  if (parsed.extras.coordinates) {
    extras.push(`${parsed.extras.coordinates.latitude.toFixed(5)}, ${parsed.extras.coordinates.longitude.toFixed(5)}`);
  }
  extras.push(...parsed.extras.references);
  if (parsed.extras.etaMinutes !== null) extras.push(`ETA ${parsed.extras.etaMinutes} min`);
  if (parsed.extras.language) extras.push(`@${parsed.extras.language}`);
  if (parsed.extras.timestamp) extras.push(`@${parsed.extras.timestamp}`);
  if (parsed.extras.callsign) extras.push(`~${parsed.extras.callsign}`);
  return extras;
}

function formatMecpDisplayDetails(parsed: DecodedMecpMessage): string {
  let details = parsed.details;
  if (parsed.extras.pax !== null) {
    details = details.replace(new RegExp(`\\b${parsed.extras.pax}pax\\b`, "i"), "");
  }
  if (parsed.extras.coordinates) {
    details = details.replace(`${parsed.extras.coordinates.latitude},${parsed.extras.coordinates.longitude}`, "");
  }
  for (const reference of parsed.extras.references) details = details.replace(reference, "");
  if (parsed.extras.etaMinutes !== null) {
    details = details.replace(new RegExp(`\\b${parsed.extras.etaMinutes}(?:m|min)?\\b`, "i"), "");
  }
  if (parsed.extras.language) details = details.replace(new RegExp(`@${parsed.extras.language}\\b`, "i"), "");
  if (parsed.extras.timestamp) details = details.replace(`@${parsed.extras.timestamp}`, "");
  if (parsed.extras.callsign) details = details.replace(`~${parsed.extras.callsign}`, "");
  return details.replace(/\s+/g, " ").trim();
}

export function isDeletedEvent(record: EventProjectionRecord): boolean {
  return typeof record.deleted_at === "number" && Number.isFinite(record.deleted_at);
}

export function toTimelineRecord(record: EventProjectionRecord): EventTimelineRecord {
  const parsed = decodeMecpMessage(getEventContent(record));
  const mecp = parsed.valid
    ? {
        raw: parsed.raw,
        severity: mecpSeverityLabel(parsed.severity),
        severityStatus: parsed.severity === 0 ? "red" : parsed.severity === 1
          ? "yellow" : parsed.severity === 2 ? "green" : "unknown",
        category: mecpCategoryLabel(parsed.category),
        categoryCode: parsed.category ?? "",
        codes: parsed.codes,
        codeLabels: parsed.codeDetails.map((code) => code.known
          ? code.label.replace(/^([A-Z])/, (_match, first: string) => first.toLowerCase())
          : code.label),
        details: formatMecpDisplayDetails(parsed),
        extras: formatMecpExtraLabels(parsed),
        warnings: parsed.warnings,
        byteLength: parsed.byteLength,
      }
    : undefined;
  return {
    uid: getEventUid(record),
    type: parsed.valid ? mecpCategoryLabel(parsed.category) : getEventType(record),
    summary: getEventContent(record),
    callsign: asTrimmedString(record.args.callsign) || "Unknown",
    updatedAt: getEventUpdatedAt(record),
    mecp,
  };
}

export function normalizeEvent(
  entry: EventProjectionRecord | Record<string, unknown>,
): EventProjectionRecord {
  const raw = entry as Record<string, unknown>;
  const rawSource = (raw.source ?? {}) as Record<string, unknown>;
  const rawArgs = (raw.args ?? {}) as Record<string, unknown>;
  const entryUid = asTrimmedString(rawArgs.entry_uid) || asTrimmedString(rawArgs.entryUid)
    || asTrimmedString(raw.entry_uid) || asTrimmedString(raw.entryUid)
    || asTrimmedString(raw.uid) || createEventUid();
  const missionUid = asTrimmedString(rawArgs.mission_uid) || asTrimmedString(rawArgs.missionUid)
    || asTrimmedString(raw.mission_uid) || asTrimmedString(raw.missionUid)
    || DEFAULT_R3AKT_MISSION_UID;
  const updatedAt = toTimestampMs(raw.updatedAt ?? raw.deleted_at ?? raw.deletedAt);
  const content = asTrimmedString(rawArgs.content) || asTrimmedString(raw.content)
    || asTrimmedString(raw.summary);
  const sourceIdentity = asTrimmedString(rawArgs.source_identity)
    || asTrimmedString(rawArgs.sourceIdentity) || asTrimmedString(rawSource.rns_identity)
    || asTrimmedString(raw.sourceIdentity) || "mobile";
  const sourceDisplayName = asTrimmedString(rawArgs.source_display_name)
    || asTrimmedString(rawArgs.sourceDisplayName) || asTrimmedString(rawSource.display_name)
    || asTrimmedString(raw.sourceDisplayName);
  const callsign = asTrimmedString(rawArgs.callsign) || asTrimmedString(raw.callsign)
    || sourceDisplayName || "Unknown";
  const baseKeywords = normalizeStringList(rawArgs.keywords ?? raw.keywords);
  const parsedMecp = parseMecpMessage(content);
  const normalizedType = asTrimmedString(raw.type)
    || decodeEventType(baseKeywords, parsedMecp.category ?? "Incident");
  const serverTime = toIsoString(rawArgs.server_time) ?? toIsoString(rawArgs.serverTime)
    ?? toIsoString(raw.serverTime) ?? new Date(updatedAt).toISOString();
  const clientTime = toIsoString(rawArgs.client_time) ?? toIsoString(rawArgs.clientTime)
    ?? toIsoString(raw.clientTime) ?? serverTime;

  return {
    command_id: asTrimmedString(raw.command_id) || asTrimmedString(raw.commandId)
      || createTrackingId("log-entry", entryUid),
    source: { rns_identity: sourceIdentity, display_name: sourceDisplayName || undefined },
    timestamp: toIsoString(raw.timestamp) ?? serverTime,
    command_type: asTrimmedString(raw.command_type) || "mission.registry.log_entry.upsert",
    args: {
      entry_uid: entryUid,
      mission_uid: missionUid,
      content,
      callsign,
      server_time: serverTime,
      client_time: clientTime,
      keywords: encodeEventTypeKeywords(normalizedType, baseKeywords),
      content_hashes: normalizeStringList(
        rawArgs.content_hashes ?? rawArgs.contentHashes ?? raw.content_hashes ?? raw.contentHashes,
      ),
      source_identity: sourceIdentity || undefined,
      source_display_name: sourceDisplayName || undefined,
    },
    correlation_id: asTrimmedString(raw.correlation_id) || undefined,
    topics: normalizeTopics(raw.topics, missionUid),
    deleted_at: typeof raw.deleted_at === "number" ? raw.deleted_at
      : typeof raw.deletedAt === "number" ? raw.deletedAt : undefined,
    updatedAt,
  };
}

export function loadWebEvents(): Record<string, EventProjectionRecord> {
  try {
    const raw = localStorage.getItem(EVENT_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Array<Partial<EventProjectionRecord> & Record<string, unknown>>;
    return Object.fromEntries(parsed.map((entry) => {
      const normalized = normalizeEvent(entry);
      return [getEventUid(normalized), normalized];
    }));
  } catch {
    return {};
  }
}

export function saveWebEvents(records: Record<string, EventProjectionRecord>): void {
  localStorage.setItem(EVENT_STORAGE_KEY, JSON.stringify(Object.values(records)));
}
