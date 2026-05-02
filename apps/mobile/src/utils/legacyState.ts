import { Capacitor } from "@capacitor/core";
import type {
  AppSettingsRecord,
  EamProjectionRecord,
  EventProjectionRecord,
  HubMode,
  LegacyImportPayload,
  MessageDirection,
  MessageMethod,
  MessageRecord,
  MessageState,
  SavedPeerRecord,
  TelemetryPositionRecord,
} from "@reticulum/node-client";

import type { NodeUiSettings } from "../types/domain";
import {
  ensureRequiredAnnounceCapabilities,
  isValidDestinationHex,
  normalizeDestinationHex,
  normalizeDisplayName,
} from "./peers";
import {
  DEFAULT_R3AKT_MISSION_NAME,
  DEFAULT_R3AKT_MISSION_UID,
  DEFAULT_R3AKT_TEAM_COLOR,
  normalizeR3aktTeamColor,
} from "./r3akt";
import {
  DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  normalizeTcpCommunityClients,
} from "./tcpCommunityServers";

export const LEGACY_SETTINGS_STORAGE_KEY = "reticulum.mobile.settings.v1";
export const LEGACY_SAVED_STORAGE_KEY = "reticulum.mobile.savedPeers.v1";
export const LEGACY_EAM_STORAGE_KEY = "reticulum.mobile.messages.v1";
export const LEGACY_EVENT_STORAGE_KEY = "reticulum.mobile.events.v1";
export const LEGACY_INBOX_STORAGE_KEY = "reticulum.mobile.inbox.v1";
export const LEGACY_TELEMETRY_STORAGE_KEY = "reticulum.mobile.telemetry.v1";
export const UI_SETTINGS_STORAGE_KEY = "reticulum.mobile.uiSettings.v1";

export interface NodeUiPreferences {
  clientMode: NodeUiSettings["clientMode"];
}

export interface LegacyProjectionState {
  payload: LegacyImportPayload;
  uiSettings: NodeUiPreferences;
}

type JsonRecord = Record<string, unknown>;

const MESSAGE_METHODS = new Set<MessageMethod>(["Direct", "Opportunistic", "Propagated", "Resource"]);
const MESSAGE_STATES = new Set<MessageState>([
  "Queued",
  "PathRequested",
  "LinkEstablishing",
  "Sending",
  "SentDirect",
  "SentToPropagation",
  "Delivered",
  "Failed",
  "TimedOut",
  "Cancelled",
  "Received",
]);
const MESSAGE_DIRECTIONS = new Set<MessageDirection>(["Inbound", "Outbound"]);
const HUB_MODES = new Set<string>([
  "Autonomous",
  "SemiAutonomous",
  "Connected",
  "Disabled",
  "RchLxmf",
  "RchHttp",
]);

function nowMs(): number {
  return Date.now();
}

function readJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) {
      return null;
    }
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

function asRecord(value: unknown): JsonRecord | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as JsonRecord;
}

function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function optionalNumber(value: unknown): number | undefined {
  if (value === undefined || value === null || value === "") {
    return undefined;
  }
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function normalizeClientMode(
  value: unknown,
  fallback: NodeUiSettings["clientMode"],
): NodeUiSettings["clientMode"] {
  const requested = value === "capacitor" ? "capacitor" : "auto";
  if (requested === "capacitor" && Capacitor.getPlatform() === "web") {
    return fallback === "capacitor" ? "auto" : fallback;
  }
  return requested;
}

function normalizeTelemetrySettings(
  telemetry: Partial<NodeUiSettings["telemetry"]> | undefined,
  defaults: NodeUiSettings["telemetry"],
): NodeUiSettings["telemetry"] {
  const staleAfterMinutes = Math.max(
    1,
    Number(telemetry?.staleAfterMinutes ?? defaults.staleAfterMinutes),
  );
  const expireAfterMinutes = Math.max(
    staleAfterMinutes,
    Number(telemetry?.expireAfterMinutes ?? defaults.expireAfterMinutes),
  );
  return {
    ...defaults,
    ...telemetry,
    publishIntervalSeconds: Math.max(
      1,
      Number(telemetry?.publishIntervalSeconds ?? defaults.publishIntervalSeconds),
    ),
    accuracyThresholdMeters:
      telemetry?.accuracyThresholdMeters === undefined || telemetry?.accuracyThresholdMeters === null
        ? undefined
        : Math.max(0, Number(telemetry.accuracyThresholdMeters)),
    staleAfterMinutes,
    expireAfterMinutes,
  };
}

function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => asTrimmedString(entry))
    .filter((entry) => entry.length > 0);
}

function normalizeUiPreferences(
  value: Partial<NodeUiSettings> | Partial<NodeUiPreferences> | null,
  defaults: Pick<NodeUiSettings, "clientMode">,
): NodeUiPreferences {
  return {
    clientMode: normalizeClientMode(value?.clientMode, defaults.clientMode),
  };
}

function normalizeRuntimeSettings(
  value: Partial<NodeUiSettings>,
  defaults: NodeUiSettings,
): AppSettingsRecord {
  const hubMode = asTrimmedString(value.hub?.mode);
  return {
    displayName:
      normalizeDisplayName(typeof value.displayName === "string" ? value.displayName : "")
      ?? defaults.displayName,
    autoConnectSaved: false,
    announceCapabilities: ensureRequiredAnnounceCapabilities(
      typeof value.announceCapabilities === "string"
        ? value.announceCapabilities
        : defaults.announceCapabilities,
    ),
    tcpClients: normalizeTcpCommunityClients(
      value.tcpClients,
      defaults.tcpClients.length > 0 ? defaults.tcpClients : DEFAULT_TCP_COMMUNITY_ENDPOINTS,
    ),
    broadcast: typeof value.broadcast === "boolean" ? value.broadcast : defaults.broadcast,
    announceIntervalSeconds: Math.max(
      60,
      Number(value.announceIntervalSeconds ?? defaults.announceIntervalSeconds),
    ),
    telemetry: normalizeTelemetrySettings(value.telemetry, defaults.telemetry),
    hub: {
      mode: HUB_MODES.has(hubMode as HubMode) ? (hubMode as HubMode) : defaults.hub.mode,
      identityHash: asTrimmedString(value.hub?.identityHash),
      apiBaseUrl: asTrimmedString(value.hub?.apiBaseUrl),
      apiKey: asTrimmedString(value.hub?.apiKey),
      refreshIntervalSeconds: Math.max(
        60,
        Number(value.hub?.refreshIntervalSeconds ?? defaults.hub.refreshIntervalSeconds),
      ),
    },
    checklists: {
      defaultTaskDueStepMinutes: Math.max(
        1,
        Number(
          value.checklists?.defaultTaskDueStepMinutes
            ?? defaults.checklists.defaultTaskDueStepMinutes,
        ),
      ),
    },
  };
}

function normalizeSavedPeers(): SavedPeerRecord[] {
  const parsed = readJson<Array<{ destination?: unknown; label?: unknown; savedAt?: unknown }>>(
    LEGACY_SAVED_STORAGE_KEY,
  );
  if (!Array.isArray(parsed)) {
    return [];
  }
  const out: SavedPeerRecord[] = [];
  for (const peer of parsed) {
    const destination = normalizeDestinationHex(asTrimmedString(peer.destination));
    if (!isValidDestinationHex(destination)) {
      continue;
    }
    out.push({
      destination,
      label: asTrimmedString(peer.label) || undefined,
      savedAt: optionalNumber(peer.savedAt) ?? nowMs(),
    });
  }
  return out;
}

function normalizeLegacyEams(): EamProjectionRecord[] {
  const parsed = readJson<unknown[]>(LEGACY_EAM_STORAGE_KEY);
  if (!Array.isArray(parsed)) {
    return [];
  }
  const out: EamProjectionRecord[] = [];
  for (const entry of parsed) {
    const record = asRecord(entry);
    if (!record) {
      continue;
    }
    const callsign = asTrimmedString(record.callsign);
    if (!callsign) {
      continue;
    }
    const source = asRecord(record.source);
    out.push({
      callsign,
      groupName: normalizeR3aktTeamColor(record.groupName, DEFAULT_R3AKT_TEAM_COLOR),
      securityStatus: asTrimmedString(record.securityStatus) || "Unknown",
      capabilityStatus: asTrimmedString(record.capabilityStatus) || "Unknown",
      preparednessStatus: asTrimmedString(record.preparednessStatus) || "Unknown",
      medicalStatus: asTrimmedString(record.medicalStatus) || "Unknown",
      mobilityStatus: asTrimmedString(record.mobilityStatus) || "Unknown",
      commsStatus: asTrimmedString(record.commsStatus) || "Unknown",
      notes: asTrimmedString(record.notes) || undefined,
      updatedAt: optionalNumber(record.updatedAt) ?? nowMs(),
      deletedAt: optionalNumber(record.deletedAt),
      eamUid: asTrimmedString(record.eamUid) || undefined,
      teamMemberUid: asTrimmedString(record.teamMemberUid) || undefined,
      teamUid: asTrimmedString(record.teamUid) || undefined,
      reportedAt: asTrimmedString(record.reportedAt) || undefined,
      reportedBy: asTrimmedString(record.reportedBy) || undefined,
      overallStatus: asTrimmedString(record.overallStatus) || undefined,
      confidence: optionalNumber(record.confidence),
      ttlSeconds: optionalNumber(record.ttlSeconds),
      source:
        source && asTrimmedString(source.rns_identity)
          ? {
              rns_identity: asTrimmedString(source.rns_identity),
              display_name: asTrimmedString(source.display_name) || undefined,
            }
          : undefined,
      syncState: asTrimmedString(record.syncState) || undefined,
      syncError: asTrimmedString(record.syncError) || undefined,
      draftCreatedAt: optionalNumber(record.draftCreatedAt),
      lastSyncedAt: optionalNumber(record.lastSyncedAt),
    });
  }
  return out;
}

function normalizeLegacyEvents(): EventProjectionRecord[] {
  const parsed = readJson<unknown[]>(LEGACY_EVENT_STORAGE_KEY);
  if (!Array.isArray(parsed)) {
    return [];
  }
  const out: EventProjectionRecord[] = [];
  for (const entry of parsed) {
    const record = asRecord(entry);
    if (!record) {
      continue;
    }
    const source = asRecord(record.source);
    const args = asRecord(record.args);
    const entryUid = asTrimmedString(args?.entry_uid ?? record.entry_uid ?? record.uid);
    if (!entryUid) {
      continue;
    }
    const updatedAt = optionalNumber(record.updatedAt ?? record.deleted_at ?? record.deletedAt) ?? nowMs();
    const missionUid = asTrimmedString(args?.mission_uid ?? record.mission_uid) || DEFAULT_R3AKT_MISSION_UID;
    const sourceIdentity = asTrimmedString(
      args?.source_identity ?? source?.rns_identity ?? record.source_identity,
    ) || "legacy";
    const sourceDisplayName = asTrimmedString(
      args?.source_display_name ?? source?.display_name ?? record.source_display_name,
    ) || undefined;
    out.push({
      command_id: asTrimmedString(record.command_id) || `legacy-command-${entryUid}`,
      source: {
        rns_identity: sourceIdentity,
        display_name: sourceDisplayName,
      },
      timestamp: asTrimmedString(record.timestamp) || new Date(updatedAt).toISOString(),
      command_type: asTrimmedString(record.command_type) || "mission.registry.log_entry.upsert",
      args: {
        entry_uid: entryUid,
        mission_uid: missionUid,
        content: asTrimmedString(args?.content ?? record.content),
        callsign: asTrimmedString(args?.callsign ?? record.callsign) || "Unknown",
        server_time: asTrimmedString(args?.server_time ?? record.server_time) || undefined,
        client_time: asTrimmedString(args?.client_time ?? record.client_time) || undefined,
        keywords: normalizeStringArray(args?.keywords ?? record.keywords),
        content_hashes: normalizeStringArray(args?.content_hashes ?? record.content_hashes),
        source_identity: asTrimmedString(args?.source_identity) || undefined,
        source_display_name: asTrimmedString(args?.source_display_name) || sourceDisplayName,
      },
      correlation_id: asTrimmedString(record.correlation_id) || undefined,
      topics: (() => {
        const topics = normalizeStringArray(record.topics);
        return topics.length > 0 ? topics : [missionUid, DEFAULT_R3AKT_MISSION_NAME];
      })(),
      deleted_at: optionalNumber(record.deleted_at ?? record.deletedAt),
      updatedAt,
    });
  }
  return out;
}

function normalizeLegacyInboxMessages(): MessageRecord[] {
  const parsed = readJson<unknown[]>(LEGACY_INBOX_STORAGE_KEY);
  if (!Array.isArray(parsed)) {
    return [];
  }
  const out: MessageRecord[] = [];
  for (const entry of parsed) {
    const record = asRecord(entry);
    if (!record) {
      continue;
    }
    const messageIdHex = asTrimmedString(record.messageIdHex ?? record.message_id_hex);
    const conversationId = asTrimmedString(record.conversationId ?? record.conversation_id);
    const destinationHex = normalizeDestinationHex(
      asTrimmedString(record.destinationHex ?? record.destination_hex),
    );
    if (!messageIdHex || !conversationId || !isValidDestinationHex(destinationHex)) {
      continue;
    }
    const direction = asTrimmedString(record.direction) as MessageDirection;
    const method = asTrimmedString(record.method) as MessageMethod;
    const state = asTrimmedString(record.state) as MessageState;
    out.push({
      messageIdHex,
      conversationId,
      direction: MESSAGE_DIRECTIONS.has(direction) ? direction : "Outbound",
      destinationHex,
      sourceHex: asTrimmedString(record.sourceHex ?? record.source_hex) || undefined,
      title: asTrimmedString(record.title) || undefined,
      bodyUtf8: typeof record.bodyUtf8 === "string" ? record.bodyUtf8 : "",
      method: MESSAGE_METHODS.has(method) ? method : "Direct",
      state: MESSAGE_STATES.has(state) ? state : "Queued",
      detail: asTrimmedString(record.detail) || undefined,
      sentAtMs: optionalNumber(record.sentAtMs ?? record.sent_at_ms),
      receivedAtMs: optionalNumber(record.receivedAtMs ?? record.received_at_ms),
      updatedAtMs: optionalNumber(record.updatedAtMs ?? record.updated_at_ms) ?? nowMs(),
    });
  }
  return out;
}

function normalizeLegacyTelemetry(): TelemetryPositionRecord[] {
  const parsed = readJson<unknown[]>(LEGACY_TELEMETRY_STORAGE_KEY);
  if (!Array.isArray(parsed)) {
    return [];
  }
  const out: TelemetryPositionRecord[] = [];
  for (const entry of parsed) {
    const record = asRecord(entry);
    if (!record) {
      continue;
    }
    const callsign = asTrimmedString(record.callsign);
    const lat = optionalNumber(record.lat);
    const lon = optionalNumber(record.lon);
    if (!callsign || lat === undefined || lon === undefined) {
      continue;
    }
    out.push({
      callsign,
      lat,
      lon,
      alt: optionalNumber(record.alt),
      course: optionalNumber(record.course),
      speed: optionalNumber(record.speed),
      accuracy: optionalNumber(record.accuracy),
      updatedAt: optionalNumber(record.updatedAt) ?? nowMs(),
    });
  }
  return out;
}

export function loadUiSettingsProjection(
  defaults: Pick<NodeUiSettings, "clientMode">,
): NodeUiPreferences {
  const stored = readJson<Partial<NodeUiPreferences>>(UI_SETTINGS_STORAGE_KEY);
  if (stored) {
    return normalizeUiPreferences(stored, defaults);
  }
  const legacySettings = readJson<Partial<NodeUiSettings>>(LEGACY_SETTINGS_STORAGE_KEY);
  return normalizeUiPreferences(legacySettings, defaults);
}

export function persistUiSettingsProjection(settings: NodeUiPreferences): void {
  localStorage.setItem(UI_SETTINGS_STORAGE_KEY, JSON.stringify(settings));
}

export function buildLegacyProjectionState(defaults: NodeUiSettings): LegacyProjectionState | null {
  const legacySettings = readJson<Partial<NodeUiSettings>>(LEGACY_SETTINGS_STORAGE_KEY);
  const uiSettings = normalizeUiPreferences(legacySettings, defaults);
  const payload: LegacyImportPayload = {
    settings: legacySettings ? normalizeRuntimeSettings(legacySettings, defaults) : undefined,
    savedPeers: normalizeSavedPeers(),
    eams: normalizeLegacyEams(),
    events: normalizeLegacyEvents(),
    messages: normalizeLegacyInboxMessages(),
    telemetryPositions: normalizeLegacyTelemetry(),
  };

  if (
    !payload.settings
    && payload.savedPeers.length === 0
    && payload.eams.length === 0
    && payload.events.length === 0
    && payload.messages.length === 0
    && payload.telemetryPositions.length === 0
  ) {
    return null;
  }

  return {
    payload,
    uiSettings,
  };
}

export function clearLegacyProjectionStorage(): void {
  localStorage.removeItem(LEGACY_SETTINGS_STORAGE_KEY);
  localStorage.removeItem(LEGACY_SAVED_STORAGE_KEY);
  localStorage.removeItem(LEGACY_EAM_STORAGE_KEY);
  localStorage.removeItem(LEGACY_EVENT_STORAGE_KEY);
  localStorage.removeItem(LEGACY_INBOX_STORAGE_KEY);
  localStorage.removeItem(LEGACY_TELEMETRY_STORAGE_KEY);
}

export function persistWebLegacySettings(settings: NodeUiSettings): void {
  localStorage.setItem(LEGACY_SETTINGS_STORAGE_KEY, JSON.stringify(settings));
}

export function persistWebLegacySavedPeers(records: SavedPeerRecord[]): void {
  localStorage.setItem(LEGACY_SAVED_STORAGE_KEY, JSON.stringify(records));
}
