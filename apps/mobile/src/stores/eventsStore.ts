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

type UpsertOutcome = "inserted" | "updated" | "ignored";

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

function normalizeEvent(entry: Partial<EventRecord> & Record<string, unknown>): EventRecord {
  const entryUid = String(
    entry.entryUid
      ?? entry.entry_uid
      ?? entry.uid
      ?? createEventUid(),
  ).trim();
  const missionUid = String(
    entry.missionUid
      ?? entry.mission_uid
      ?? entry.mission_id
      ?? DEFAULT_R3AKT_MISSION_UID,
  ).trim() || DEFAULT_R3AKT_MISSION_UID;
  const keywords = encodeEventTypeKeywords(
    String(entry.type ?? decodeEventType(normalizeKeywords(entry.keywords), "Incident")).trim() || "Incident",
    normalizeKeywords(entry.keywords),
  );
  const content = String(entry.content ?? entry.summary ?? "").trim();
  const sourceIdentity = asTrimmedString(entry.sourceIdentity ?? entry.source_identity ?? entry.rns_identity);
  const sourceDisplayName = asTrimmedString(
    entry.sourceDisplayName
      ?? entry.source_display_name
      ?? entry.display_name,
  );
  const callsign = fallbackCallsign({
    callsign: asTrimmedString(entry.callsign ?? entry.source_callsign ?? entry.sourceCallsign),
    sourceDisplayName,
    sourceIdentity,
  });
  const serverTime = toIsoString(entry.serverTime ?? entry.server_time ?? entry.servertime);
  const clientTime = toIsoString(entry.clientTime ?? entry.client_time ?? entry.clientTime ?? entry.clienttime);
  const updatedAt = asNumber(
    entry.updatedAt
      ?? entry.updated_at
      ?? serverTime
      ?? clientTime
      ?? entry.timestamp
      ?? entry.created_at,
    Date.now(),
  );

  return {
    uid: entryUid,
    entryUid,
    missionUid,
    callsign,
    sourceIdentity: sourceIdentity || undefined,
    sourceDisplayName: sourceDisplayName || undefined,
    type: decodeEventType(keywords, "Incident"),
    summary: content,
    content,
    serverTime: serverTime || new Date(updatedAt).toISOString(),
    clientTime: clientTime || undefined,
    keywords,
    updatedAt: toTimestampMs(updatedAt, Date.now()),
    deletedAt: entry.deletedAt ? Number(entry.deletedAt) : undefined,
  };
}

function mergeEvents(current: EventRecord | undefined, incoming: EventRecord): EventRecord {
  if (!current) {
    return incoming;
  }

  return {
    ...current,
    ...incoming,
    callsign: incoming.callsign || current.callsign,
    sourceIdentity: incoming.sourceIdentity || current.sourceIdentity,
    sourceDisplayName: incoming.sourceDisplayName || current.sourceDisplayName,
    type: incoming.type || current.type,
    summary: incoming.summary || current.summary,
    content: incoming.content || current.content,
    serverTime: incoming.serverTime || current.serverTime,
    clientTime: incoming.clientTime || current.clientTime,
    keywords: incoming.keywords.length > 0 ? incoming.keywords : current.keywords,
    deletedAt: incoming.deletedAt ?? current.deletedAt,
  };
}

function summarizeEvent(record: EventRecord): string {
  const summary = record.summary.length > 96
    ? `${record.summary.slice(0, 93)}...`
    : record.summary;
  return `${record.callsign} | ${record.type}: ${summary}`;
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
      if (!normalized.uid || !normalized.summary) {
        continue;
      }
      out[normalized.uid] = normalized;
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
      .filter((entry): entry is EventRecord => entry !== null && entry.summary.length > 0);
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
    entry_uid: record.entryUid,
    mission_uid: record.missionUid,
    content: record.content,
    server_time: record.serverTime ?? new Date(record.updatedAt).toISOString(),
    client_time: record.clientTime,
    keywords: [...record.keywords],
    content_hashes: [],
    callsign: record.callsign,
    source_identity: record.sourceIdentity,
    source_display_name: record.sourceDisplayName,
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
    if (!normalized.uid || !normalized.summary) {
      return "ignored";
    }

    const existing = byUid[normalized.uid];
    if (existing && existing.updatedAt > normalized.updatedAt) {
      return "ignored";
    }

    const merged = mergeEvents(existing, normalized);
    const outcome: UpsertOutcome = existing ? "updated" : "inserted";
    byUid[merged.uid] = merged;
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
    if (existing.updatedAt > deletedAt) {
      return;
    }
    byUid[eventUid] = {
      ...existing,
      deletedAt,
      updatedAt: deletedAt,
    };
    persist();
  }

  function snapshotEvents(): EventRecord[] {
    return Object.values(byUid)
      .filter((entry) => !entry.deletedAt)
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

  function isEventDelivery(event: LxmfDeliveryEvent): boolean {
    return Boolean(event.commandType?.startsWith("mission.registry.log_entry"))
      || Boolean(event.eventUid);
  }

  function replicationPeers(logMissing = false): Array<{
    appDestinationHex: string;
    lxmfDestinationHex: string;
    label: string;
  }> {
    const deliverable: Array<{
      appDestinationHex: string;
      lxmfDestinationHex: string;
      label: string;
    }> = [];

    for (const peer of nodeStore.connectedEventPeerRoutes) {
      const label = formatPeerLabel(peer);
      if (!peer.lxmfDestinationHex) {
        if (logMissing) {
          nodeStore.logUi(
            "Warn",
            `[events] skipped peer ${label}: no announced LXMF delivery destination is tracked for ${peer.appDestinationHex}.`,
          );
        }
        continue;
      }
      deliverable.push({
        appDestinationHex: peer.appDestinationHex,
        lxmfDestinationHex: peer.lxmfDestinationHex,
        label,
      });
    }

    return deliverable;
  }

  async function sendMissionCommand(destination: string, command: MissionCommandEnvelope): Promise<void> {
    await nodeStore.sendBytes(destination, EMPTY_BYTES, {
      fieldsBase64: buildMissionCommandFieldsBase64([command]),
    });
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

  async function ensureDefaultMission(destination: string): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    const correlationId = createMissionTrackingId("mission-upsert", DEFAULT_R3AKT_MISSION_UID);
    await sendMissionCommand(destination, createMissionCommandEnvelope({
      commandId: createMissionTrackingId("mission-upsert-command", DEFAULT_R3AKT_MISSION_UID),
      sourceIdentity,
      sourceDisplayName: localCallsign() || undefined,
      commandType: "mission.registry.mission.upsert",
      args: buildDefaultMissionPayload(),
      correlationId,
    }));
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
    }));
  }

  async function fanoutLogUpsert(record: EventRecord): Promise<void> {
    const peers = replicationPeers(true);
    if (peers.length === 0) {
      if (nodeStore.connectedDestinations.length > 0) {
        nodeStore.logUi(
          "Warn",
          `[events] event ${record.entryUid} was stored locally but no connected peer has a usable LXMF delivery destination.`,
        );
      }
      return;
    }

    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }

    await Promise.all(peers.map(async (peer) => {
      try {
        await ensureDefaultMission(peer.lxmfDestinationHex);
        const correlationId = createMissionTrackingId("log-upsert", record.entryUid);
        await sendMissionCommand(peer.lxmfDestinationHex, createMissionCommandEnvelope({
          commandId: createMissionTrackingId("log-upsert-command", record.entryUid),
          sourceIdentity,
          sourceDisplayName: localCallsign() || undefined,
          commandType: "mission.registry.log_entry.upsert",
          args: buildMissionPayload(record),
          correlationId,
        }));
      } catch (error: unknown) {
        nodeStore.logUi(
          "Error",
          `[events] failed to send ${record.entryUid} to ${peer.label} (${peer.lxmfDestinationHex}): ${errorMessage(error)}`,
        );
      }
    }));
  }

  async function hydrateDestination(destination: string): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    await ensureDefaultMission(destination);
    await requestEventList(destination);
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
        ...command.args,
        mission_uid: DEFAULT_R3AKT_MISSION_UID,
        source_identity: command.source.rns_identity,
        source_display_name: asTrimmedString(command.args.source_display_name) ?? command.source.display_name,
        callsign: asTrimmedString(command.args.callsign) ?? command.source.display_name,
      });
      const outcome = applyUpsert(incoming);
      const stored = byUid[incoming.uid];
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
        }),
      );
      if (outcome !== "ignored" && stored.sourceIdentity !== localIdentity) {
        notifyOperationalUpdate(
          outcome === "inserted" ? "New event" : "Updated event",
          summarizeEvent(stored),
        ).catch(() => undefined);
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
    const callsign = localCallsign();
    const now = Number(input.updatedAt ?? Date.now());
    const uid = input.uid?.trim() || createEventUid();
    const event: EventRecord = normalizeEvent({
      uid,
      entry_uid: uid,
      mission_uid: DEFAULT_R3AKT_MISSION_UID,
      callsign,
      source_identity: localSourceIdentity() || undefined,
      source_display_name: callsign,
      type: input.type,
      content: input.summary,
      summary: input.summary,
      keywords: encodeEventTypeKeywords(input.type),
      client_time: new Date(now).toISOString(),
      server_time: new Date(now).toISOString(),
      updatedAt: now,
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
                `[events] received ${incoming.entryUid} from ${incoming.sourceDisplayName ?? incoming.sourceIdentity ?? event.sourceHex ?? "unknown"} via LXMF.`,
              );
            }
            if (
              shouldNotify
              && outcome !== "ignored"
              && incoming.sourceIdentity !== localIdentity
            ) {
              notifyOperationalUpdate(
                outcome === "inserted" ? "New event" : "Updated event",
                summarizeEvent(incoming),
              ).catch(() => undefined);
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
          if (incoming.deletedAt) {
            applyDelete(incoming.uid, incoming.deletedAt);
          } else {
            applyUpsert(incoming);
          }
        }
        return;
      }

      if (legacy.kind === "event_upsert") {
        const outcome = applyUpsert(legacy.event);
        if (outcome !== "ignored") {
          notifyOperationalUpdate(
            outcome === "inserted" ? "New event" : "Updated event",
            summarizeEvent(legacy.event),
          ).catch(() => undefined);
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
        destinations: replicationPeers(false).map((peer) => peer.lxmfDestinationHex),
      }),
      (current, previous) => {
        if (!current.identity) {
          return;
        }
        const previousDestinations = previous?.identity
          ? new Set(previous.destinations)
          : new Set<string>();
        for (const destination of current.destinations) {
          if (previousDestinations.has(destination)) {
            continue;
          }
          void hydrateDestination(destination);
        }
      },
      { immediate: true, deep: true },
    );
  }

  const records = computed(() =>
    Object.values(byUid)
      .filter((entry) => !entry.deletedAt)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );

  return {
    records,
    init,
    initReplication,
    upsertLocal,
    deleteLocal,
  };
});
