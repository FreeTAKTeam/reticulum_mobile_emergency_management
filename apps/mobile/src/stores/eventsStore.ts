import { defineStore } from "pinia";
import { computed, reactive, ref, watch } from "vue";
import type { LxmfDeliveryEvent, PacketReceivedEvent } from "@reticulum/node-client";

import { notifyOperationalUpdate } from "../services/notifications";
import type { EventRecord } from "../types/domain";
import {
  buildMissionCommandFieldsBase64,
  buildMissionResponseFieldsBase64,
  createMissionAcceptedPayload,
  createMissionCommandEnvelope,
  createMissionEventEnvelope,
  createMissionRejectedPayload,
  createMissionResultPayload,
  parseMissionSyncFields,
  type MissionCommandEnvelope,
  type MissionEventEnvelope,
  type MissionResponsePayload,
  type MissionResultPayload,
} from "../utils/missionSync";
import {
  DEFAULT_R3AKT_MISSION_NAME,
  DEFAULT_R3AKT_MISSION_UID,
} from "../utils/r3akt";
import { asNumber, asTrimmedString, parseReplicationEnvelope } from "../utils/replicationParser";
import { useNodeStore } from "./nodeStore";

const EVENT_STORAGE_KEY = "reticulum.mobile.events.v1";
const EMPTY_BYTES = new Uint8Array(0);
const EVENT_TYPE_KEYWORD_PREFIX = "r3akt:event-type:";
const LXMF_DELIVERY_WAIT_TIMEOUT_MS = 90_000;
const BACKGROUND_MISSION_HYDRATION_ENABLED = true;

type UpsertOutcome = "inserted" | "updated" | "ignored";
type ReplicationStage = "mission.registry.mission.upsert" | "mission.registry.log_entry.upsert";
type ReplicationPeer = {
  appDestinationHex: string;
  lxmfDestinationHex: string;
  identityHex?: string;
  label: string;
  announcedName?: string;
  usePropagationNode?: boolean;
};
type ReplicationFailure = {
  stage: ReplicationStage;
  peer: ReplicationPeer;
  message: string;
};

type LegacyEventReplicationMessage =
  | {
      kind: "event_snapshot_request";
      requestedAt: number;
    }
  | {
      kind: "event_snapshot_response";
      requestedAt: number;
      events: EventRecord[];
    }
  | {
      kind: "event_upsert";
      event: EventRecord;
    }
  | {
      kind: "event_delete";
      uid: string;
      deletedAt: number;
    };

type EventTimelineRecord = {
  uid: string;
  type: string;
  summary: string;
  callsign: string;
  updatedAt: number;
};

class EventReplicationError extends Error {
  readonly failures: ReplicationFailure[];

  constructor(message: string, failures: ReplicationFailure[]) {
    super(message);
    this.name = "EventReplicationError";
    this.failures = failures;
  }
}

function createEventUid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `evt-${crypto.randomUUID()}`;
  }
  return `evt-${Date.now().toString(36)}-${Math.floor(Math.random() * 1_000_000).toString(36)}`;
}

function createMissionTrackingId(prefix: string, suffix?: string): string {
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

function normalizeHex(value: string | undefined | null): string {
  return value?.trim().toLowerCase() ?? "";
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

function toTimestampMs(value: unknown, fallback = Date.now()): number {
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

  if (topics.length === 0) {
    return [missionUid];
  }

  return [...new Set(topics)];
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

function fallbackCallsign(options: {
  callsign?: string;
  sourceDisplayName?: string;
  sourceIdentity?: string;
}): string {
  return options.callsign?.trim()
    || options.sourceDisplayName?.trim()
    || options.sourceIdentity?.trim()
    || "Unknown";
}

function getEventUid(record: EventRecord): string {
  return record.args.entry_uid;
}

function getEventMissionUid(record: EventRecord): string {
  return record.args.mission_uid;
}

function getEventContent(record: EventRecord): string {
  return record.args.content;
}

function getEventKeywords(record: EventRecord): string[] {
  return normalizeKeywords(record.args.keywords);
}

function getEventType(record: EventRecord): string {
  return decodeEventType(getEventKeywords(record), "Incident");
}

function getEventSourceIdentity(record: EventRecord): string | undefined {
  return asTrimmedString(record.args.source_identity) ?? asTrimmedString(record.source.rns_identity);
}

function getEventSourceDisplayName(record: EventRecord): string | undefined {
  return asTrimmedString(record.args.source_display_name) ?? asTrimmedString(record.source.display_name);
}

function getEventUpdatedAt(record: EventRecord): number {
  return toTimestampMs(
    record.deleted_at
      ?? record.args.server_time
      ?? record.args.client_time
      ?? record.timestamp,
    Date.now(),
  );
}

function getEventMissionCommandType(record: EventRecord): ReplicationStage {
  return record.command_type === "mission.registry.mission.upsert"
    ? "mission.registry.mission.upsert"
    : "mission.registry.log_entry.upsert";
}

function isDeletedEvent(record: EventRecord): boolean {
  return typeof record.deleted_at === "number" && Number.isFinite(record.deleted_at);
}

function toTimelineRecord(record: EventRecord): EventTimelineRecord {
  return {
    uid: getEventUid(record),
    type: getEventType(record),
    summary: getEventContent(record),
    callsign: record.args.callsign,
    updatedAt: getEventUpdatedAt(record),
  };
}

function normalizeEvent(entry: Partial<EventRecord> & Record<string, unknown>): EventRecord {
  const sourceRecord = (
    entry.source && typeof entry.source === "object" && !Array.isArray(entry.source)
      ? entry.source
      : {}
  ) as Record<string, unknown>;
  const argsRecord = (
    entry.args && typeof entry.args === "object" && !Array.isArray(entry.args)
      ? entry.args
      : {}
  ) as Record<string, unknown>;
  const entryUid = String(
    argsRecord.entry_uid
      ?? argsRecord.entryUid
      ?? entry.entry_uid
      ?? entry.entryUid
      ?? entry.uid
      ?? createEventUid(),
  ).trim();
  const missionUid = String(
    argsRecord.mission_uid
      ?? argsRecord.missionUid
      ?? entry.mission_uid
      ?? entry.missionUid
      ?? entry.mission_id
      ?? DEFAULT_R3AKT_MISSION_UID,
  ).trim() || DEFAULT_R3AKT_MISSION_UID;
  const keywords = encodeEventTypeKeywords(
    String(
      entry.type
        ?? argsRecord.type
        ?? decodeEventType(normalizeKeywords(argsRecord.keywords ?? entry.keywords), "Incident"),
    ).trim() || "Incident",
    normalizeKeywords(argsRecord.keywords ?? entry.keywords),
  );
  const content = String(
    argsRecord.content
      ?? entry.content
      ?? entry.summary
      ?? "",
  ).trim();
  const sourceIdentity = asTrimmedString(
    argsRecord.source_identity
      ?? entry.sourceIdentity
      ?? entry.source_identity
      ?? entry.rns_identity
      ?? sourceRecord.rns_identity,
  );
  const sourceDisplayName = asTrimmedString(
    argsRecord.source_display_name
      ?? entry.sourceDisplayName
      ?? entry.source_display_name
      ?? entry.display_name
      ?? sourceRecord.display_name,
  );
  const callsign = fallbackCallsign({
    callsign: asTrimmedString(
      argsRecord.callsign
        ?? entry.callsign
        ?? entry.source_callsign
        ?? entry.sourceCallsign,
    ),
    sourceDisplayName,
    sourceIdentity,
  });
  const deletedAt = asNumber(entry.deleted_at ?? entry.deletedAt, 0) || undefined;
  const updatedAt = asNumber(
    entry.updatedAt
      ?? entry.updated_at
      ?? deletedAt
      ?? argsRecord.server_time
      ?? entry.serverTime
      ?? entry.server_time
      ?? argsRecord.client_time
      ?? entry.clientTime
      ?? entry.client_time
      ?? entry.timestamp
      ?? entry.created_at,
    Date.now(),
  );
  const timestamp = toIsoString(entry.timestamp ?? updatedAt) ?? new Date(updatedAt).toISOString();
  const serverTime = toIsoString(
    argsRecord.server_time
      ?? entry.serverTime
      ?? entry.server_time
      ?? entry.servertime
      ?? timestamp,
  ) ?? timestamp;
  const clientTime = toIsoString(
    argsRecord.client_time
      ?? entry.clientTime
      ?? entry.client_time
      ?? entry.clienttime,
  );

  return {
    command_id: asTrimmedString(entry.command_id ?? entry.commandId) ?? createMissionTrackingId("log-entry", entryUid),
    source: {
      rns_identity: sourceIdentity || "mobile",
      display_name: sourceDisplayName || undefined,
    },
    timestamp,
    command_type: asTrimmedString(entry.command_type ?? entry.commandType) ?? "mission.registry.log_entry.upsert",
    args: {
      entry_uid: entryUid,
      mission_uid: missionUid,
      content,
      callsign,
      server_time: serverTime,
      client_time: clientTime || undefined,
      keywords,
      content_hashes: normalizeContentHashes(argsRecord.content_hashes ?? entry.content_hashes),
      source_identity: sourceIdentity || undefined,
      source_display_name: sourceDisplayName || undefined,
    },
    correlation_id: asTrimmedString(entry.correlation_id ?? entry.correlationId) ?? undefined,
    topics: normalizeTopics(entry.topics, missionUid),
    deleted_at: deletedAt,
  };
}

function mergeEvents(current: EventRecord | undefined, incoming: EventRecord): EventRecord {
  if (!current) {
    return incoming;
  }

  return normalizeEvent({
    ...current,
    ...incoming,
    source: {
      ...current.source,
      ...incoming.source,
    },
    args: {
      ...current.args,
      ...incoming.args,
      content_hashes: incoming.args.content_hashes.length > 0
        ? [...incoming.args.content_hashes]
        : [...current.args.content_hashes],
      keywords: incoming.args.keywords.length > 0
        ? [...incoming.args.keywords]
        : [...current.args.keywords],
    },
    topics: incoming.topics.length > 0 ? [...incoming.topics] : [...current.topics],
    deleted_at: incoming.deleted_at ?? current.deleted_at,
  });
}

function summarizeEvent(record: EventRecord): string {
  const summary = getEventContent(record).length > 96
    ? `${getEventContent(record).slice(0, 93)}...`
    : getEventContent(record);
  return `${record.args.callsign} | ${getEventType(record)}: ${summary}`;
}

function notifyIncomingEvent(record: EventRecord, outcome: Exclude<UpsertOutcome, "ignored">, localIdentity: string): void {
  if (getEventSourceIdentity(record) === localIdentity) {
    return;
  }

  const sourceLabel = getEventSourceDisplayName(record)?.trim()
    || record.args.callsign.trim()
    || getEventSourceIdentity(record)
    || "mesh";
  notifyOperationalUpdate(
    outcome === "inserted" ? `New event from ${sourceLabel}` : `Updated event from ${sourceLabel}`,
    summarizeEvent(record),
  ).catch(() => undefined);
}

function missionResponseMatches(
  expected: {
    correlationId: string;
    commandId: string;
  },
  result: MissionResponsePayload | null,
): result is MissionResponsePayload {
  if (!result) {
    return false;
  }
  return result.correlation_id === expected.correlationId || result.command_id === expected.commandId;
}

function missionCompletionEventType(commandType: string): string | null {
  switch (commandType) {
    case "mission.registry.mission.upsert":
      return "mission.registry.mission.upserted";
    case "mission.registry.log_entry.upsert":
      return "mission.registry.log_entry.upserted";
    default:
      return null;
  }
}

function missionEventMatches(
  expected: {
    correlationId: string;
    commandId: string;
    commandType: string;
    eventUid?: string;
    missionUid?: string;
  },
  event: MissionEventEnvelope | null,
): event is MissionEventEnvelope {
  if (!event) {
    return false;
  }

  const expectedEventType = missionCompletionEventType(expected.commandType);
  if (!expectedEventType || event.event_type !== expectedEventType) {
    return false;
  }

  const meta = event.meta ?? {};
  const metaCorrelationId = asTrimmedString(meta.correlation_id);
  const metaCommandId = asTrimmedString(meta.command_id);
  if (metaCorrelationId || metaCommandId) {
    return metaCorrelationId === expected.correlationId || metaCommandId === expected.commandId;
  }

  if (expected.commandType === "mission.registry.mission.upsert") {
    const missionUid = asTrimmedString(event.payload.mission_uid ?? event.payload.uid);
    return Boolean(missionUid && missionUid === expected.missionUid);
  }

  if (expected.commandType === "mission.registry.log_entry.upsert") {
    const eventUid = asTrimmedString(event.payload.entry_uid ?? event.payload.entryUid);
    const missionUid = asTrimmedString(event.payload.mission_uid ?? event.payload.missionUid);
    return Boolean(
      eventUid
      && eventUid === expected.eventUid
      && (!expected.missionUid || missionUid === expected.missionUid),
    );
  }

  return false;
}

function missionResponseDetail(result: MissionResponsePayload): string {
  if (result.status === "rejected") {
    return result.reason?.trim() || result.reason_code;
  }
  if (result.status === "accepted") {
    return `accepted at ${result.accepted_at}`;
  }
  return "result received";
}

function loadEvents(): Record<string, EventRecord> {
  try {
    const raw = localStorage.getItem(EVENT_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Array<Partial<EventRecord> & Record<string, unknown>>;
    const out: Record<string, EventRecord> = {};
    for (const item of parsed) {
      const normalized = normalizeEvent(item);
      if (!getEventUid(normalized) || !getEventContent(normalized)) {
        continue;
      }
      out[getEventUid(normalized)] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

function saveEvents(records: Record<string, EventRecord>): void {
  localStorage.setItem(EVENT_STORAGE_KEY, JSON.stringify(Object.values(records)));
}

function parseLegacyEventReplicationMessage(raw: string): LegacyEventReplicationMessage | null {
  const envelope = parseReplicationEnvelope(raw);
  if (!envelope) {
    return null;
  }

  const { kind, payload } = envelope;
  switch (kind) {
    case "event_snapshot_request":
      return {
        kind: "event_snapshot_request",
        requestedAt: asNumber(payload.requestedAt, Date.now()),
      };
    case "event_snapshot_response":
      return {
        kind: "event_snapshot_response",
        requestedAt: asNumber(payload.requestedAt, Date.now()),
        events: Array.isArray(payload.events)
          ? payload.events.map((entry) => normalizeEvent(entry as Record<string, unknown>))
          : [],
      };
    case "event_upsert":
      if (!payload.event || typeof payload.event !== "object") {
        return null;
      }
      return {
        kind: "event_upsert",
        event: normalizeEvent(payload.event as Record<string, unknown>),
      };
    case "event_delete":
      return {
        kind: "event_delete",
        uid: asTrimmedString(payload.uid),
        deletedAt: asNumber(payload.deletedAt, Date.now()),
      };
    default:
      return null;
  }
}

function extractEventsFromMissionPayload(payload: Record<string, unknown>): EventRecord[] {
  const logEntries = Array.isArray(payload.log_entries)
    ? payload.log_entries
    : Array.isArray(payload.logEntries)
      ? payload.logEntries
      : null;

  if (logEntries) {
    return logEntries
      .map((entry) => (entry && typeof entry === "object" ? normalizeEvent(entry as Record<string, unknown>) : null))
      .filter((entry): entry is EventRecord => entry !== null && getEventContent(entry).length > 0);
  }

  if (
    "entry_uid" in payload
    || "entryUid" in payload
    || "content" in payload
    || "summary" in payload
  ) {
    return [normalizeEvent(payload)];
  }

  return [];
}

function buildMissionPayload(record: EventRecord): Record<string, unknown> {
  return {
    entry_uid: record.args.entry_uid,
    mission_uid: record.args.mission_uid,
    content: record.args.content,
    server_time: record.args.server_time ?? record.timestamp,
    client_time: record.args.client_time,
    keywords: [...record.args.keywords],
    content_hashes: [...record.args.content_hashes],
    callsign: record.args.callsign,
    source_identity: record.args.source_identity,
    source_display_name: record.args.source_display_name,
  };
}

function buildDefaultMissionPayload(at = new Date().toISOString()): Record<string, unknown> {
  return {
    uid: DEFAULT_R3AKT_MISSION_UID,
    mission_name: DEFAULT_R3AKT_MISSION_NAME,
    description: "",
    topic_id: null,
    path: null,
    classification: null,
    tool: null,
    keywords: [],
    parent_uid: null,
    children: [],
    feeds: [],
    zones: [],
    password_hash: null,
    default_role: null,
    mission_priority: null,
    mission_status: "MISSION_ACTIVE",
    owner_role: null,
    token: null,
    invite_only: false,
    expiration: null,
    mission_rde_role: null,
    created_at: at,
    updated_at: at,
  };
}

export const useEventsStore = defineStore("events", () => {
  const byUid = reactive<Record<string, EventRecord>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const nodeStore = useNodeStore();
  const peerSyncInFlight = new Set<string>();

  function persist(): void {
    saveEvents(byUid);
  }

  function localSourceIdentity(): string {
    return nodeStore.status.identityHex.trim().toLowerCase();
  }

  function localCallsign(): string {
    return nodeStore.settings.displayName.trim();
  }

  function applyUpsert(record: EventRecord): UpsertOutcome {
    const normalized = normalizeEvent(record as unknown as Partial<EventRecord> & Record<string, unknown>);
    if (!getEventUid(normalized) || !getEventContent(normalized)) {
      return "ignored";
    }

    const eventUid = getEventUid(normalized);
    const existing = byUid[eventUid];
    if (existing && getEventUpdatedAt(existing) > getEventUpdatedAt(normalized)) {
      return "ignored";
    }

    const merged = mergeEvents(existing, normalized);
    const outcome: UpsertOutcome = existing ? "updated" : "inserted";
    byUid[eventUid] = merged;
    persist();
    return outcome;
  }

  function applyDelete(uid: string, deletedAt: number): void {
    const eventUid = uid.trim();
    if (!eventUid) {
      return;
    }
    const existing = byUid[eventUid];
    if (!existing) {
      return;
    }
    if (getEventUpdatedAt(existing) > deletedAt) {
      return;
    }
    byUid[eventUid] = normalizeEvent({
      ...existing,
      deleted_at: deletedAt,
      timestamp: new Date(deletedAt).toISOString(),
    });
    persist();
  }

  function snapshotEvents(): EventRecord[] {
    return Object.values(byUid)
      .filter((entry) => !isDeletedEvent(entry))
      .map((entry) => ({ ...entry }));
  }

  function errorMessage(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }

  function formatPeerLabel(peer: {
    label?: string;
    announcedName?: string;
    appDestinationHex: string;
  }): string {
    return peer.label?.trim()
      || peer.announcedName?.trim()
      || peer.appDestinationHex;
  }

  function formatPeerRoute(peer: ReplicationPeer): string {
    return `${peer.label} (app=${peer.appDestinationHex} lxmf=${peer.lxmfDestinationHex}${peer.identityHex ? ` identity=${peer.identityHex}` : ""})`;
  }

  function isEventDelivery(event: LxmfDeliveryEvent): boolean {
    return Boolean(event.commandType?.startsWith("mission.registry.log_entry"))
      || Boolean(event.eventUid);
  }

  function replicationPeers(logMissing = false): ReplicationPeer[] {
    const localIdentity = normalizeHex(nodeStore.status.identityHex);
    const localAppDestination = normalizeHex(nodeStore.status.appDestinationHex);
    const localLxmfDestination = normalizeHex(nodeStore.status.lxmfDestinationHex);
    const directPeers = nodeStore.connectedEventPeerRoutes;
    const propagationPeers = nodeStore.bestPropagationNodeHex
      ? nodeStore.propagationEligibleEventPeerRoutes
      : [];
    const selectedPeers = [...directPeers, ...propagationPeers];
    const deliverable: ReplicationPeer[] = [];
    const seenByAppDestination = new Set<string>();

    for (const peer of selectedPeers) {
      const label = formatPeerLabel(peer);
      const appDestinationHex = normalizeHex(peer.appDestinationHex);
      const lxmfDestinationHex = normalizeHex(peer.lxmfDestinationHex);
      const peerIdentity = normalizeHex(peer.identityHex);
      if (
        !appDestinationHex
        || appDestinationHex === localAppDestination
        || appDestinationHex === localLxmfDestination
        || lxmfDestinationHex === localAppDestination
        || lxmfDestinationHex === localLxmfDestination
        || (peerIdentity.length > 0 && peerIdentity === localIdentity)
      ) {
        if (logMissing) {
          nodeStore.logUi(
            "Debug",
            `[events] skipping self route for ${label} (app=${peer.appDestinationHex}${peer.lxmfDestinationHex ? ` lxmf=${peer.lxmfDestinationHex}` : ""}${peer.identityHex ? ` identity=${peer.identityHex}` : ""}).`,
          );
        }
        continue;
      }
      if (seenByAppDestination.has(appDestinationHex)) {
        continue;
      }
      if (!peer.lxmfDestinationHex) {
        if (logMissing) {
          nodeStore.logUi(
            "Debug",
            `[events] peer ${label} is connected on app destination ${peer.appDestinationHex} but has no tracked LXMF delivery destination yet; skipping event fanout until LXMF route is known.`,
          );
        }
        continue;
      }
      seenByAppDestination.add(appDestinationHex);
      deliverable.push({
        appDestinationHex: peer.appDestinationHex,
        lxmfDestinationHex: peer.lxmfDestinationHex,
        identityHex: peer.identityHex,
        label,
        announcedName: peer.announcedName,
        usePropagationNode: peer.usePropagationNode,
      });
    }

    if (logMissing) {
      if (deliverable.length === 0) {
        nodeStore.logUi(
          "Debug",
          `[events] no deliverable event peers. connectedRoutes=${nodeStore.connectedEventPeerRoutes.length} connectedDestinations=${nodeStore.connectedDestinations.length} discoveredPeers=${nodeStore.discoveredPeers.length}.`,
        );
      } else if (directPeers.length === 0 && propagationPeers.length > 0) {
        nodeStore.logUi(
          "Info",
          `[events] no direct LXMF peer routes are available; using propagation relay ${nodeStore.bestPropagationNodeHex}.`,
        );
      } else if (propagationPeers.length > 0) {
        nodeStore.logUi(
          "Info",
          `[events] using ${directPeers.length} direct route(s) and ${Math.max(propagationPeers.length - directPeers.length, 0)} propagation route(s).`,
        );
      }
    }

    return deliverable;
  }

  function createReplicationFailure(
    stage: ReplicationStage,
    peer: ReplicationPeer,
    error: unknown,
  ): ReplicationFailure {
    return {
      stage,
      peer,
      message: errorMessage(error),
    };
  }

  function createMissionDeliveryTracker(
    peer: ReplicationPeer,
    expected: {
      stage: ReplicationStage;
      correlationId: string;
      commandId: string;
      commandType: string;
      eventUid?: string;
      missionUid?: string;
    },
  ): {
    promise: Promise<void>;
    cancel: () => void;
  } {
    let cleanup = () => undefined;

    const promise = new Promise<void>((resolve, reject) => {
      let settled = false;
      let unsubscribeDelivery: () => void = () => undefined;
      let unsubscribePacket: () => void = () => undefined;
      let timeoutId: ReturnType<typeof setTimeout> | undefined;

      const finish = (effect: () => void) => {
        if (settled) {
          return;
        }
        settled = true;
        if (timeoutId !== undefined) {
          clearTimeout(timeoutId);
        }
        unsubscribeDelivery();
        unsubscribePacket();
        effect();
      };

      unsubscribeDelivery = nodeStore.onLxmfDelivery((event: LxmfDeliveryEvent) => {
        if (!isEventDelivery(event)) {
          return;
        }
        if (event.correlationId !== expected.correlationId && event.commandId !== expected.commandId) {
          return;
        }
        if (event.commandType && event.commandType !== expected.commandType) {
          return;
        }
        if (expected.eventUid && event.eventUid && event.eventUid !== expected.eventUid) {
          return;
        }
        if (expected.missionUid && event.missionUid && event.missionUid !== expected.missionUid) {
          return;
        }

        const destinationMatches = normalizeHex(event.destinationHex) === normalizeHex(peer.lxmfDestinationHex);
        if (!destinationMatches) {
          return;
        }

        if (event.status === "Sent" || event.status === "SentToPropagation") {
          nodeStore.logUi(
            "Debug",
            `[events] ${expected.stage} ${event.status === "SentToPropagation" ? "queued on propagation relay" : "sent"} to ${formatPeerRoute(peer)} (message ${event.messageIdHex}).`,
          );
          return;
        }

        if (event.status === "Acknowledged") {
          finish(() => resolve());
          return;
        }

        const detail = event.detail?.trim() || "delivery failed";
        finish(() => reject(new EventReplicationError(
          `[events] ${expected.stage} failed for ${formatPeerRoute(peer)}: ${detail}`,
          [{
            stage: expected.stage,
            peer,
            message: detail,
          }],
        )));
      });

      unsubscribePacket = nodeStore.onPacket((packet: PacketReceivedEvent) => {
        const missionSync = parseMissionSyncFields(packet.fieldsBase64);
        const matchingResult = missionResponseMatches(expected, missionSync?.result ?? null)
          ? missionSync?.result ?? null
          : null;
        const matchingEvent = missionEventMatches(expected, missionSync?.event ?? null)
          ? missionSync?.event ?? null
          : null;

        if (!matchingResult && !matchingEvent) {
          return;
        }

        if (matchingEvent) {
          nodeStore.logUi(
            "Debug",
            `[events] ${expected.stage} completion event ${matchingEvent.event_type} received from ${formatPeerRoute(peer)}.`,
          );
        }

        if (!matchingResult) {
          return;
        }

        const result = matchingResult;
        if (result.status === "accepted") {
          nodeStore.logUi(
            "Debug",
            `[events] ${expected.stage} accepted by ${formatPeerRoute(peer)} (${missionResponseDetail(result)}).`,
          );
          return;
        }

        if (result.status === "rejected") {
          const detail = missionResponseDetail(result);
          finish(() => reject(new EventReplicationError(
            `[events] ${expected.stage} rejected by ${formatPeerRoute(peer)}: ${detail}`,
            [{
              stage: expected.stage,
              peer,
              message: detail,
            }],
          )));
          return;
        }

        nodeStore.logUi(
          "Debug",
          `[events] ${expected.stage} result received from ${formatPeerRoute(peer)}.`,
        );
      });

      timeoutId = setTimeout(() => {
        finish(() => reject(new EventReplicationError(
          `[events] ${expected.stage} timed out for ${formatPeerRoute(peer)} after ${LXMF_DELIVERY_WAIT_TIMEOUT_MS}ms.`,
          [{
            stage: expected.stage,
            peer,
            message: `Timed out waiting for LXMF acknowledgement after ${LXMF_DELIVERY_WAIT_TIMEOUT_MS}ms.`,
          }],
        )));
      }, LXMF_DELIVERY_WAIT_TIMEOUT_MS);

      cleanup = () => {
        if (settled) {
          return;
        }
        settled = true;
        if (timeoutId !== undefined) {
          clearTimeout(timeoutId);
        }
        unsubscribeDelivery();
        unsubscribePacket();
      };
    });

    return {
      promise,
      cancel: () => cleanup(),
    };
  }

  async function sendMissionCommand(
    destination: string,
    command: MissionCommandEnvelope,
    options?: {
      usePropagationNode?: boolean;
    },
  ): Promise<void> {
    await nodeStore.sendBytes(destination, EMPTY_BYTES, {
      fieldsBase64: buildMissionCommandFieldsBase64([command]),
      usePropagationNode: options?.usePropagationNode,
    });
  }

  async function sendMissionCommandAwaitingDelivery(
    peer: ReplicationPeer,
    command: MissionCommandEnvelope,
    stage: ReplicationStage,
  ): Promise<void> {
    const tracker = createMissionDeliveryTracker(peer, {
      stage,
      correlationId: command.correlation_id ?? command.command_id,
      commandId: command.command_id,
      commandType: command.command_type,
      eventUid: asTrimmedString(command.args.entry_uid),
      missionUid: asTrimmedString(command.args.mission_uid ?? command.args.uid),
    });

    try {
      await sendMissionCommand(peer.lxmfDestinationHex, command, {
        usePropagationNode: peer.usePropagationNode,
      });
    } catch (error: unknown) {
      tracker.cancel();
      throw new EventReplicationError(
        `[events] ${stage} send failed for ${formatPeerRoute(peer)}: ${errorMessage(error)}`,
        [createReplicationFailure(stage, peer, error)],
      );
    }

    await tracker.promise;
  }

  async function sendMissionResponse(
    destination: string,
    result: MissionResponsePayload,
    event?: MissionEventEnvelope,
  ): Promise<void> {
    await nodeStore.sendBytes(destination, EMPTY_BYTES, {
      fieldsBase64: buildMissionResponseFieldsBase64({ result, event }),
    });
  }

  async function requestEventList(destination: string): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    const correlationId = createMissionTrackingId("log-list", DEFAULT_R3AKT_MISSION_UID);
    await sendMissionCommand(destination, createMissionCommandEnvelope({
      commandId: createMissionTrackingId("log-list-command", DEFAULT_R3AKT_MISSION_UID),
      sourceIdentity,
      sourceDisplayName: localCallsign() || undefined,
      commandType: "mission.registry.log_entry.list",
      args: { mission_uid: DEFAULT_R3AKT_MISSION_UID },
      correlationId,
      topics: [DEFAULT_R3AKT_MISSION_UID],
    }));
  }

  async function replicateLogUpsertToPeer(
    peer: ReplicationPeer,
    record: EventRecord,
    options?: {
      logLevel?: "Info" | "Debug";
      verb?: "replicated" | "replayed";
    },
  ): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    const eventUid = getEventUid(record);
    const routeSuffix = peer.lxmfDestinationHex.slice(0, 8);
    const correlationId = createMissionTrackingId("log-upsert", `${eventUid}-${routeSuffix}`);
    await sendMissionCommandAwaitingDelivery(peer, createMissionCommandEnvelope({
      commandId: createMissionTrackingId("log-upsert-command", `${eventUid}-${routeSuffix}`),
      sourceIdentity: record.source.rns_identity || sourceIdentity,
      sourceDisplayName: record.source.display_name ?? (localCallsign() || undefined),
      commandType: getEventMissionCommandType(record),
      args: buildMissionPayload(record),
      correlationId,
      topics: [...record.topics],
    }), "mission.registry.log_entry.upsert");
    nodeStore.logUi(
      options?.logLevel ?? "Info",
      `[events] ${options?.verb ?? "replicated"} ${eventUid} to ${formatPeerRoute(peer)}.`,
    );
  }

  async function fanoutLogUpsert(record: EventRecord): Promise<void> {
    const peers = replicationPeers(true);
    if (peers.length === 0) {
      if (nodeStore.connectedDestinations.length > 0 || nodeStore.discoveredPeers.length > 0) {
        nodeStore.logUi(
          "Warn",
          `[events] event ${getEventUid(record)} was stored locally but no connected peer route is currently available for fanout.`,
        );
      }
      return;
    }

    const eventUid = getEventUid(record);
    const failures = (await Promise.all(peers.map(async (peer) => {
      try {
        await replicateLogUpsertToPeer(peer, record);
        return null;
      } catch (error: unknown) {
        nodeStore.logUi(
          "Error",
          `[events] failed to send ${eventUid} to ${peer.label} (app=${peer.appDestinationHex}${peer.lxmfDestinationHex ? ` lxmf=${peer.lxmfDestinationHex}` : ""}): ${errorMessage(error)}`,
        );
        if (error instanceof EventReplicationError) {
          return error.failures;
        }
        return [createReplicationFailure("mission.registry.log_entry.upsert", peer, error)];
      }
    }))).flat().filter((failure): failure is ReplicationFailure => failure !== null);

    if (failures.length > 0) {
      throw new EventReplicationError(
        `[events] replicated ${eventUid} to ${peers.length - failures.length}/${peers.length} peers.`,
        failures,
      );
    }
  }

  async function hydrateDestination(peer: ReplicationPeer): Promise<void> {
    if (!BACKGROUND_MISSION_HYDRATION_ENABLED) {
      return;
    }

    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    await requestEventList(peer.lxmfDestinationHex);
  }

  async function syncPeerEvents(peer: ReplicationPeer): Promise<void> {
    const destination = normalizeHex(peer.lxmfDestinationHex);
    if (!destination || peerSyncInFlight.has(destination)) {
      return;
    }

    peerSyncInFlight.add(destination);
    try {
      const localEvents = snapshotEvents()
        .sort((a, b) => getEventUpdatedAt(a) - getEventUpdatedAt(b));
      if (localEvents.length > 0) {
        nodeStore.logUi(
          "Debug",
          `[events] replaying ${localEvents.length} local event(s) to ${formatPeerRoute(peer)} before hydration.`,
        );
      }
      for (const record of localEvents) {
        await replicateLogUpsertToPeer(peer, record, {
          logLevel: "Debug",
          verb: "replayed",
        });
      }
      await requestEventList(peer.lxmfDestinationHex);
    } finally {
      peerSyncInFlight.delete(destination);
    }
  }

  function resultPayloadEvents(result: MissionResultPayload | null, event: MissionEventEnvelope | null): EventRecord[] {
    if (event && (
      event.event_type === "mission.registry.log_entry.upserted"
      || event.event_type === "mission.registry.log_entry.listed"
    )) {
      return extractEventsFromMissionPayload(event.payload);
    }

    if (!result || result.status !== "result") {
      return [];
    }

    return extractEventsFromMissionPayload(result.result);
  }

  async function handleMissionCommand(destination: string, command: MissionCommandEnvelope): Promise<void> {
    if (
      !command.command_type.startsWith("mission.registry.mission.")
      && !command.command_type.startsWith("mission.registry.log_entry.")
    ) {
      return;
    }

    const localIdentity = localSourceIdentity();
    const localDisplayName = localCallsign() || undefined;
    const accepted = createMissionAcceptedPayload({
      commandId: command.command_id,
      correlationId: command.correlation_id,
      byIdentity: localIdentity || undefined,
    });

    if (command.command_type === "mission.registry.mission.upsert") {
      const missionUid = String(command.args.mission_uid ?? command.args.uid ?? "").trim() || DEFAULT_R3AKT_MISSION_UID;
      if (missionUid !== DEFAULT_R3AKT_MISSION_UID) {
        await sendMissionResponse(
          destination,
          createMissionRejectedPayload({
            commandId: command.command_id,
            correlationId: command.correlation_id,
            reasonCode: "invalid_payload",
            reason: "Only the Default mission is supported on mobile.",
          }),
        );
        return;
      }

      const missionPayload = buildDefaultMissionPayload();
      await sendMissionResponse(destination, accepted);
      await sendMissionResponse(
        destination,
        createMissionResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          result: missionPayload,
        }),
        createMissionEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.mission.upserted",
          payload: missionPayload,
          topics: [missionUid],
          meta: {
            command_id: command.command_id,
            correlation_id: command.correlation_id,
          },
        }),
      );
      return;
    }

    if (command.command_type === "mission.registry.log_entry.upsert") {
      const missionUid = String(command.args.mission_uid ?? command.args.missionUid ?? "").trim() || DEFAULT_R3AKT_MISSION_UID;
      if (missionUid !== DEFAULT_R3AKT_MISSION_UID) {
        await sendMissionResponse(
          destination,
          createMissionRejectedPayload({
            commandId: command.command_id,
            correlationId: command.correlation_id,
            reasonCode: "invalid_payload",
            reason: "mission_uid must be Default.",
          }),
        );
        return;
      }

      const incoming = normalizeEvent({
        command_id: command.command_id,
        source: command.source,
        timestamp: command.timestamp,
        command_type: command.command_type,
        correlation_id: command.correlation_id,
        topics: command.topics,
        args: {
          ...command.args,
          mission_uid: DEFAULT_R3AKT_MISSION_UID,
          source_identity: command.source.rns_identity,
          source_display_name: asTrimmedString(command.args.source_display_name) ?? command.source.display_name,
          callsign: asTrimmedString(command.args.callsign) ?? command.source.display_name,
        } as Record<string, unknown>,
      } as unknown as Partial<EventRecord> & Record<string, unknown>);
      const outcome = applyUpsert(incoming);
      const stored = byUid[getEventUid(incoming)];
      await sendMissionResponse(destination, accepted);
      await sendMissionResponse(
        destination,
        createMissionResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          result: buildMissionPayload(stored),
        }),
        createMissionEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.log_entry.upserted",
          payload: buildMissionPayload(stored),
          topics: [...stored.topics],
          meta: {
            command_id: command.command_id,
            correlation_id: command.correlation_id,
          },
        }),
      );
      if (outcome !== "ignored") {
        notifyIncomingEvent(stored, outcome, localIdentity);
      }
      return;
    }

    if (command.command_type === "mission.registry.log_entry.list") {
      const missionUid = String(command.args.mission_uid ?? "").trim() || DEFAULT_R3AKT_MISSION_UID;
      if (missionUid !== DEFAULT_R3AKT_MISSION_UID) {
        await sendMissionResponse(
          destination,
          createMissionRejectedPayload({
            commandId: command.command_id,
            correlationId: command.correlation_id,
            reasonCode: "invalid_payload",
            reason: "mission_uid must be Default.",
          }),
        );
        return;
      }

      const payload = {
        log_entries: snapshotEvents().map((entry) => buildMissionPayload(entry)),
      };
      await sendMissionResponse(destination, accepted);
      await sendMissionResponse(
        destination,
        createMissionResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          result: payload,
        }),
        createMissionEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.log_entry.listed",
          payload,
          topics: [DEFAULT_R3AKT_MISSION_UID],
          meta: {
            command_id: command.command_id,
            correlation_id: command.correlation_id,
          },
        }),
      );
      return;
    }

    await sendMissionResponse(
      destination,
      createMissionRejectedPayload({
        commandId: command.command_id,
        correlationId: command.correlation_id,
        reasonCode: "unknown_command",
        reason: `Unsupported mission command '${command.command_type}'`,
      }),
    );
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    const loaded = loadEvents();
    for (const [uid, entry] of Object.entries(loaded)) {
      byUid[uid] = entry;
    }
  }

  async function upsertLocal(input: {
    type: string;
    summary: string;
    uid?: string;
    updatedAt?: number;
  }): Promise<void> {
    nodeStore.assertReadyForOutbound("send events");
    const callsign = localCallsign();
    const now = Number(input.updatedAt ?? Date.now());
    const uid = input.uid?.trim() || createEventUid();
    const event: EventRecord = normalizeEvent({
      command_id: createMissionTrackingId("log-entry", uid),
      source: {
        rns_identity: localSourceIdentity() || "mobile",
        display_name: callsign || undefined,
      },
      timestamp: new Date(now).toISOString(),
      command_type: "mission.registry.log_entry.upsert",
      correlation_id: createMissionTrackingId("ui-save", uid),
      topics: [DEFAULT_R3AKT_MISSION_UID, "audit"],
      args: {
        entry_uid: uid,
        mission_uid: DEFAULT_R3AKT_MISSION_UID,
        content: input.summary,
        callsign,
        source_identity: localSourceIdentity() || undefined,
        source_display_name: callsign || undefined,
        keywords: encodeEventTypeKeywords(input.type),
        content_hashes: [],
        client_time: new Date(now).toISOString(),
        server_time: new Date(now).toISOString(),
      },
    });
    applyUpsert(event);
    await fanoutLogUpsert(event);
  }

  async function deleteLocal(uid: string): Promise<void> {
    const deletedAt = Date.now();
    applyDelete(uid, deletedAt);
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;

    const decoder = new TextDecoder();
    nodeStore.onPacket((event: PacketReceivedEvent) => {
      const missionSync = parseMissionSyncFields(event.fieldsBase64);
      if (missionSync) {
        if (missionSync.commands.length > 0 && event.sourceHex) {
          for (const command of missionSync.commands) {
            void handleMissionCommand(event.sourceHex, command);
          }
          return;
        }

        const missionEvents = resultPayloadEvents(
          missionSync.result?.status === "result" ? missionSync.result : null,
          missionSync.event,
        );
        const localIdentity = localSourceIdentity();
        if (missionEvents.length > 0) {
          const shouldNotify = missionSync.event?.event_type === "mission.registry.log_entry.upserted";
          for (const incoming of missionEvents) {
            const outcome = applyUpsert(incoming);
            if (missionSync.event?.event_type === "mission.registry.log_entry.upserted") {
              nodeStore.logUi(
                "Info",
                `[events] received ${getEventUid(incoming)} from ${getEventSourceDisplayName(incoming) ?? getEventSourceIdentity(incoming) ?? event.sourceHex ?? "unknown"} via LXMF.`,
              );
            }
            if (
              shouldNotify
              && outcome !== "ignored"
            ) {
              notifyIncomingEvent(incoming, outcome, localIdentity);
            }
          }
        }

        return;
      }

      const legacy = parseLegacyEventReplicationMessage(decoder.decode(event.bytes));
      if (!legacy) {
        return;
      }

      if (legacy.kind === "event_snapshot_response") {
        for (const incoming of legacy.events) {
          if (incoming.deleted_at) {
            applyDelete(getEventUid(incoming), incoming.deleted_at);
          } else {
            applyUpsert(incoming);
          }
        }
        return;
      }

      if (legacy.kind === "event_upsert") {
        const outcome = applyUpsert(legacy.event);
        if (outcome !== "ignored") {
          notifyIncomingEvent(legacy.event, outcome, localSourceIdentity());
        }
        return;
      }

      if (legacy.kind === "event_delete") {
        applyDelete(legacy.uid, legacy.deletedAt);
      }
    });

    nodeStore.onLxmfDelivery((event: LxmfDeliveryEvent) => {
      if (!isEventDelivery(event)) {
        return;
      }

      const eventUid = event.eventUid ?? "-";
      const detail = event.detail?.trim();
      if (event.status === "Sent") {
        nodeStore.logUi(
          "Info",
          `[events] sent ${eventUid} over LXMF to ${event.destinationHex} (message ${event.messageIdHex}).`,
        );
        return;
      }

      if (event.status === "Acknowledged") {
        nodeStore.logUi(
          "Info",
          `[events] acknowledged ${eventUid} from ${event.destinationHex} (message ${event.messageIdHex}).`,
        );
        return;
      }

      if (event.status === "TimedOut") {
        nodeStore.logUi(
          "Warn",
          `[events] timed out waiting for acknowledgement for ${eventUid} to ${event.destinationHex} (${detail || "ack timeout"}).`,
        );
        return;
      }

      nodeStore.logUi(
        "Error",
        `[events] delivery failed for ${eventUid} to ${event.destinationHex} (${detail || "send failed"}).`,
      );
    });

    watch(
      () => ({
        identity: nodeStore.status.identityHex.trim().toLowerCase(),
        peers: replicationPeers(false).map((peer) => ({
          appDestinationHex: peer.appDestinationHex,
          lxmfDestinationHex: peer.lxmfDestinationHex,
        })),
      }),
      (current, previous) => {
        if (!current.identity) {
          return;
        }
        const previousDestinations = previous?.identity
          ? new Set(previous.peers.map((peer) => peer.lxmfDestinationHex))
          : new Set<string>();
        for (const peer of replicationPeers(false)) {
          if (previousDestinations.has(peer.lxmfDestinationHex)) {
            continue;
          }
          void syncPeerEvents(peer).catch((error: unknown) => {
            nodeStore.logUi(
              "Warn",
              `[events] peer sync failed for ${formatPeerRoute(peer)}: ${errorMessage(error)}`,
            );
          });
        }
      },
      { immediate: true, deep: true },
    );
  }

  const records = computed(() =>
    Object.values(byUid)
      .filter((entry) => !isDeletedEvent(entry))
      .sort((a, b) => getEventUpdatedAt(b) - getEventUpdatedAt(a))
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
