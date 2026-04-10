import {
  createReticulumNodeClient,
  type EventProjectionRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  notifyOperationalUpdateOnce,
  primeOperationalNotificationScope,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import {
  DEFAULT_R3AKT_MISSION_NAME,
  DEFAULT_R3AKT_MISSION_UID,
} from "../utils/r3akt";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

const EVENT_STORAGE_KEY = "reticulum.mobile.events.v1";
const EVENT_TYPE_KEYWORD_PREFIX = "r3akt:event-type:";

type EventTimelineRecord = {
  uid: string;
  type: string;
  summary: string;
  callsign: string;
  updatedAt: number;
};

type ProjectionClientCache = typeof globalThis & {
  __reticulumEventsProjectionClient?: ReticulumNodeClient;
};

function nowMs(): number {
  return Date.now();
}

function createEventUid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `evt-${crypto.randomUUID()}`;
  }
  return `evt-${Date.now().toString(36)}-${Math.floor(Math.random() * 1_000_000).toString(36)}`;
}

function createTrackingId(prefix: string, suffix?: string): string {
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

function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function toIsoString(value: unknown): string | undefined {
  if (typeof value === "string") {
    const normalized = value.trim();
    return normalized || undefined;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return new Date(value).toISOString();
  }
  return undefined;
}

function toTimestampMs(value: unknown, fallback = nowMs()): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value > 1_000_000_000_000 ? Math.floor(value) : Math.floor(value * 1000);
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    if (!Number.isNaN(parsed)) {
      return parsed;
    }
  }
  return fallback;
}

function normalizeKeywords(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return [...new Set(
    value
      .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
      .filter((entry) => entry.length > 0),
  )];
}

function normalizeContentHashes(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return [...new Set(
    value
      .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
      .filter((entry) => entry.length > 0),
  )];
}

function normalizeTopics(value: unknown, missionUid: string): string[] {
  const topics = Array.isArray(value)
    ? value
      .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
      .filter((entry) => entry.length > 0)
    : [];

  return topics.length > 0 ? [...new Set(topics)] : [missionUid];
}

function decodeEventType(keywords: string[], fallback = "Incident"): string {
  const tagged = keywords.find((keyword) => keyword.startsWith(EVENT_TYPE_KEYWORD_PREFIX));
  if (!tagged) {
    return fallback;
  }
  const decoded = tagged.slice(EVENT_TYPE_KEYWORD_PREFIX.length).trim();
  return decoded || fallback;
}

function encodeEventTypeKeywords(type: string, keywords: string[] = []): string[] {
  const normalizedType = type.trim() || "Incident";
  const filtered = normalizeKeywords(keywords).filter(
    (keyword) => !keyword.startsWith(EVENT_TYPE_KEYWORD_PREFIX),
  );
  return [...filtered, `${EVENT_TYPE_KEYWORD_PREFIX}${normalizedType}`];
}

function getEventUid(record: EventProjectionRecord): string {
  return record.args.entry_uid;
}

function getEventContent(record: EventProjectionRecord): string {
  return record.args.content;
}

function getEventType(record: EventProjectionRecord): string {
  return decodeEventType(normalizeKeywords(record.args.keywords), "Incident");
}

function getEventUpdatedAt(record: EventProjectionRecord): number {
  return toTimestampMs(
    record.deleted_at
      ?? record.updatedAt
      ?? record.args.server_time
      ?? record.args.client_time
      ?? record.timestamp,
    nowMs(),
  );
}

function isDeletedEvent(record: EventProjectionRecord): boolean {
  return typeof record.deleted_at === "number" && Number.isFinite(record.deleted_at);
}

function toTimelineRecord(record: EventProjectionRecord): EventTimelineRecord {
  return {
    uid: getEventUid(record),
    type: getEventType(record),
    summary: getEventContent(record),
    callsign: asTrimmedString(record.args.callsign) || "Unknown",
    updatedAt: getEventUpdatedAt(record),
  };
}

function normalizeEvent(entry: EventProjectionRecord | Record<string, unknown>): EventProjectionRecord {
  const raw = entry as Record<string, unknown>;
  const rawSource = (raw.source ?? {}) as Record<string, unknown>;
  const rawArgs = (raw.args ?? {}) as Record<string, unknown>;

  const entryUid = asTrimmedString(rawArgs.entry_uid)
    || asTrimmedString(rawArgs.entryUid)
    || asTrimmedString(raw.entry_uid)
    || asTrimmedString(raw.entryUid)
    || asTrimmedString(raw.uid)
    || createEventUid();
  const missionUid = asTrimmedString(rawArgs.mission_uid)
    || asTrimmedString(rawArgs.missionUid)
    || asTrimmedString(raw.mission_uid)
    || asTrimmedString(raw.missionUid)
    || DEFAULT_R3AKT_MISSION_UID;
  const updatedAt = toTimestampMs(raw.updatedAt ?? raw.deleted_at ?? raw.deletedAt, nowMs());
  const content = asTrimmedString(rawArgs.content)
    || asTrimmedString(raw.content)
    || asTrimmedString(raw.summary);
  const sourceIdentity = asTrimmedString(rawArgs.source_identity)
    || asTrimmedString(rawArgs.sourceIdentity)
    || asTrimmedString(rawSource.rns_identity)
    || asTrimmedString(raw.sourceIdentity)
    || "mobile";
  const sourceDisplayName = asTrimmedString(rawArgs.source_display_name)
    || asTrimmedString(rawArgs.sourceDisplayName)
    || asTrimmedString(rawSource.display_name)
    || asTrimmedString(raw.sourceDisplayName);
  const callsign = asTrimmedString(rawArgs.callsign)
    || asTrimmedString(raw.callsign)
    || sourceDisplayName
    || "Unknown";
  const baseKeywords = normalizeKeywords(rawArgs.keywords ?? raw.keywords);
  const normalizedType = asTrimmedString(raw.type) || decodeEventType(baseKeywords, "Incident");
  const serverTime = toIsoString(rawArgs.server_time)
    ?? toIsoString(rawArgs.serverTime)
    ?? toIsoString(raw.serverTime)
    ?? new Date(updatedAt).toISOString();
  const clientTime = toIsoString(rawArgs.client_time)
    ?? toIsoString(rawArgs.clientTime)
    ?? toIsoString(raw.clientTime)
    ?? serverTime;

  return {
    command_id: asTrimmedString(raw.command_id)
      || asTrimmedString(raw.commandId)
      || createTrackingId("log-entry", entryUid),
    source: {
      rns_identity: sourceIdentity,
      display_name: sourceDisplayName || undefined,
    },
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
      content_hashes: normalizeContentHashes(
        rawArgs.content_hashes ?? rawArgs.contentHashes ?? raw.content_hashes ?? raw.contentHashes,
      ),
      source_identity: sourceIdentity || undefined,
      source_display_name: sourceDisplayName || undefined,
    },
    correlation_id: asTrimmedString(raw.correlation_id) || undefined,
    topics: normalizeTopics(raw.topics, missionUid),
    deleted_at:
      typeof raw.deleted_at === "number"
        ? raw.deleted_at
        : typeof raw.deletedAt === "number"
          ? raw.deletedAt
          : undefined,
    updatedAt,
  };
}

function loadWebEvents(): Record<string, EventProjectionRecord> {
  try {
    const raw = localStorage.getItem(EVENT_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Array<Partial<EventProjectionRecord> & Record<string, unknown>>;
    const out: Record<string, EventProjectionRecord> = {};
    for (const entry of parsed) {
      const normalized = normalizeEvent(entry);
      out[getEventUid(normalized)] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

function saveWebEvents(records: Record<string, EventProjectionRecord>): void {
  localStorage.setItem(EVENT_STORAGE_KEY, JSON.stringify(Object.values(records)));
}

function getProjectionClient(mode: "auto" | "capacitor"): ReticulumNodeClient {
  const cache = globalThis as ProjectionClientCache;
  if (!cache.__reticulumEventsProjectionClient) {
    cache.__reticulumEventsProjectionClient = createReticulumNodeClient({ mode });
  }
  return cache.__reticulumEventsProjectionClient;
}

export const useEventsStore = defineStore("events", () => {
  const nodeStore = useNodeStore();
  const byUid = ref<Record<string, EventProjectionRecord>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const notificationsPrimed = ref(false);

  let refreshPromise: Promise<void> | null = null;
  const cleanups: Array<() => void> = [];

  function webPersist(): void {
    if (!supportsNativeNodeRuntime) {
      saveWebEvents(byUid.value);
    }
  }

  function eventNotificationKey(record: EventProjectionRecord): string {
    return `${getEventUid(record)}:${getEventUpdatedAt(record)}`;
  }

  function isLocalEventRecord(record: EventProjectionRecord): boolean {
    const localIdentity = asTrimmedString(nodeStore.status.identityHex).toLowerCase();
    const eventIdentity = asTrimmedString(record.args.source_identity ?? record.source.rns_identity).toLowerCase();
    if (localIdentity && eventIdentity) {
      return localIdentity === eventIdentity;
    }
    const localDisplayName = asTrimmedString(nodeStore.settings.displayName).toLowerCase();
    if (!localDisplayName) {
      return false;
    }
    const sourceDisplayName = asTrimmedString(
      record.args.source_display_name ?? record.source.display_name,
    ).toLowerCase();
    const callsign = asTrimmedString(record.args.callsign).toLowerCase();
    return sourceDisplayName === localDisplayName || callsign === localDisplayName;
  }

  async function notifyForInboundEvents(records: Record<string, EventProjectionRecord>): Promise<void> {
    const activeRecords = Object.values(records).filter((record) => !isDeletedEvent(record));
    if (!notificationsPrimed.value) {
      primeOperationalNotificationScope(
        "event",
        activeRecords.map((record) => eventNotificationKey(record)),
      );
      notificationsPrimed.value = true;
      return;
    }

    for (const record of activeRecords) {
      if (isLocalEventRecord(record)) {
        continue;
      }
      await notifyOperationalUpdateOnce(
        "event",
        eventNotificationKey(record),
        `Event from ${asTrimmedString(record.args.callsign) || "Unknown"}`,
        truncateNotificationBody(getEventContent(record)),
      );
    }
  }

  async function refreshFromNative(): Promise<void> {
    if (!supportsNativeNodeRuntime || !nodeStore.status.running) {
      return;
    }
    if (refreshPromise) {
      await refreshPromise;
      return;
    }
    const promise = (async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      const records = await client.getEvents();
      const next: Record<string, EventProjectionRecord> = {};
      for (const record of records) {
        const normalized = normalizeEvent(record);
        next[getEventUid(normalized)] = normalized;
      }
      byUid.value = next;
      await notifyForInboundEvents(next);
    })();
    refreshPromise = promise;
    try {
      await promise;
    } finally {
      if (refreshPromise === promise) {
        refreshPromise = null;
      }
    }
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    if (!supportsNativeNodeRuntime) {
      byUid.value = loadWebEvents();
      return;
    }

    void refreshFromNative();
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Events") {
      void refreshFromNative();
    }
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;

    if (!supportsNativeNodeRuntime) {
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    cleanups.push(client.on("projectionInvalidated", handleProjectionInvalidation));
    cleanups.push(client.on("statusChanged", () => {
      void refreshFromNative();
    }));
  }

  async function upsertLocal(input: {
    type: string;
    summary: string;
    uid?: string;
  }): Promise<void> {
    const localDisplayName = nodeStore.settings.displayName.trim() || "Unknown";
    const updatedAt = nowMs();
    const entryUid = input.uid?.trim() || createEventUid();
    const nextRecord = normalizeEvent({
      uid: entryUid,
      command_id: createTrackingId("log-entry", entryUid),
      timestamp: new Date(updatedAt).toISOString(),
      command_type: "mission.registry.log_entry.upsert",
      source: {
        rns_identity: nodeStore.status.identityHex || "mobile",
        display_name: localDisplayName,
      },
      args: {
        entry_uid: entryUid,
        mission_uid: DEFAULT_R3AKT_MISSION_UID,
        content: input.summary.trim(),
        callsign: localDisplayName,
        server_time: new Date(updatedAt).toISOString(),
        client_time: new Date(updatedAt).toISOString(),
        keywords: encodeEventTypeKeywords(input.type.trim() || "Incident"),
        content_hashes: [],
        source_identity: nodeStore.status.identityHex || undefined,
        source_display_name: localDisplayName,
      },
      topics: [DEFAULT_R3AKT_MISSION_UID, DEFAULT_R3AKT_MISSION_NAME],
      updatedAt,
    });

    if (!supportsNativeNodeRuntime) {
      byUid.value = {
        ...byUid.value,
        [entryUid]: nextRecord,
      };
      webPersist();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await client.upsertEvent(nextRecord);
    await refreshFromNative();
  }

  async function deleteLocal(uid: string): Promise<void> {
    const normalizedUid = uid.trim();
    if (!normalizedUid) {
      return;
    }
    const deletedAt = nowMs();

    if (!supportsNativeNodeRuntime) {
      const existing = byUid.value[normalizedUid];
      if (!existing) {
        return;
      }
      byUid.value = {
        ...byUid.value,
        [normalizedUid]: {
          ...existing,
          deleted_at: deletedAt,
          updatedAt: deletedAt,
        },
      };
      webPersist();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await client.deleteEvent(normalizedUid, deletedAt);
    await refreshFromNative();
  }

  const records = computed(() =>
    Object.values(byUid.value)
      .filter((entry) => !isDeletedEvent(entry))
      .sort((left, right) => getEventUpdatedAt(right) - getEventUpdatedAt(left))
      .map((entry) => toTimelineRecord(entry)),
  );

  return {
    records,
    init,
    initReplication,
    upsertLocal,
    deleteLocal,
  };
});
