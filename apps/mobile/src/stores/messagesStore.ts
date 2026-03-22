import { defineStore } from "pinia";
import { computed, reactive, ref, watch } from "vue";
import type { LxmfDeliveryEvent, PacketReceivedEvent } from "@reticulum/node-client";

import { notifyOperationalUpdate } from "../services/notifications";
import type {
  ActionMessage,
  EamCommandArgsByType,
  EamCommandType,
  EamRecord,
  EamStatus,
  EamTeamSummary,
  EamWireStatus,
  ReplicationMessage,
} from "../types/domain";
import {
  DEFAULT_R3AKT_TEAM_COLOR,
  formatR3aktTeamColor,
  normalizeR3aktTeamColor,
} from "../utils/r3akt";
import {
  buildEamCommandFieldsBase64,
  buildEamResponseFieldsBase64,
  createEamAcceptedPayload,
  createEamDeleteCommandEnvelope,
  createEamDeleteResultPayload,
  createEamEventEnvelope,
  createEamGetCommandEnvelope,
  createEamGetResultPayload,
  createEamLatestCommandEnvelope,
  createEamLatestResultPayload,
  createEamListCommandEnvelope,
  createEamListResultPayload,
  createEamRejectedPayload,
  createEamTeamSummaryCommandEnvelope,
  createEamTeamSummaryResultPayload,
  createEamUpsertCommandEnvelope,
  createEamUpsertResultPayload,
  parseEamMissionSyncFields,
  type EamCommandEnvelope,
  type EamEventEnvelope,
  type EamResponsePayload,
} from "../utils/eamMissionSync";
import { asNumber, asTrimmedString, parseReplicationEnvelope } from "../utils/replicationParser";
import { useNodeStore } from "./nodeStore";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.messages.v1";
const EMPTY_BYTES = new Uint8Array(0);
const STATUS_ROTATION: EamStatus[] = ["Unknown", "Green", "Yellow", "Red"];
const LXMF_DELIVERY_WAIT_TIMEOUT_MS = 90_000;

type UpsertOutcome = "inserted" | "updated" | "ignored";
type ReplicationPeer = {
  appDestinationHex: string;
  lxmfDestinationHex: string;
  identityHex?: string;
  label: string;
  announcedName?: string;
};

type LegacyMessageReplication =
  | { kind: "snapshot_request"; requestedAt: number }
  | { kind: "snapshot_response"; requestedAt: number; messages: ActionMessage[] }
  | { kind: "message_upsert"; message: ActionMessage }
  | { kind: "message_delete"; callsign: string; deletedAt: number };
type EamFilterArgs = {
  include_deleted?: boolean;
  eam_uid?: string;
  callsign?: string;
  team_uid?: string;
  team_member_uid?: string;
};
type EamTrackingExpectation = {
  commandId: string;
  correlationId: string;
  commandType: EamCommandType;
  eamUid?: string;
  callsign?: string;
  teamUid?: string;
};
type PeerFailure = {
  peer: ReplicationPeer;
  detail: string;
};

function nowMs(): number {
  return Date.now();
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

function normalizeHex(value: string | undefined | null): string {
  return value?.trim().toLowerCase() ?? "";
}

function createLocalRegistryId(prefix: string, seed: string): string {
  const normalized = seed
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  return normalized ? `${prefix}-${normalized}` : `${prefix}-unknown`;
}

function normalizeStatus(value: unknown): EamStatus {
  if (value === "Green" || value === "Yellow" || value === "Red") {
    return value;
  }
  return "Unknown";
}

function toWireStatus(value: EamStatus): EamWireStatus | undefined {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : undefined;
}

function normalizeSyncState(value: unknown): ActionMessage["syncState"] {
  return value === "draft" || value === "syncing" || value === "synced" || value === "error"
    ? value
    : undefined;
}

function optionalNumber(value: unknown): number | undefined {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function normalizeMessage(entry: Partial<ActionMessage>): ActionMessage {
  const updatedAt = Number(entry.updatedAt ?? nowMs());
  return {
    callsign: String(entry.callsign ?? "").trim(),
    groupName: normalizeR3aktTeamColor(entry.groupName, DEFAULT_R3AKT_TEAM_COLOR),
    securityStatus: normalizeStatus(entry.securityStatus),
    capabilityStatus: normalizeStatus(entry.capabilityStatus),
    preparednessStatus: normalizeStatus(entry.preparednessStatus),
    medicalStatus: normalizeStatus(entry.medicalStatus),
    mobilityStatus: normalizeStatus(entry.mobilityStatus),
    commsStatus: normalizeStatus(entry.commsStatus),
    notes: asTrimmedString(entry.notes),
    updatedAt: Number.isFinite(updatedAt) ? updatedAt : nowMs(),
    deletedAt: optionalNumber(entry.deletedAt),
    eamUid: asTrimmedString(entry.eamUid),
    teamMemberUid: asTrimmedString(entry.teamMemberUid),
    teamUid: asTrimmedString(entry.teamUid),
    reportedAt: asTrimmedString(entry.reportedAt),
    reportedBy: asTrimmedString(entry.reportedBy),
    overallStatus: toWireStatus(normalizeStatus(entry.overallStatus)),
    confidence: optionalNumber(entry.confidence),
    ttlSeconds: optionalNumber(entry.ttlSeconds),
    source:
      entry.source && typeof entry.source === "object" && !Array.isArray(entry.source)
        ? {
            rns_identity: String((entry.source as { rns_identity?: unknown }).rns_identity ?? "").trim(),
            display_name: asTrimmedString((entry.source as { display_name?: unknown }).display_name),
          }
        : undefined,
    syncState: normalizeSyncState(entry.syncState),
    syncError: asTrimmedString(entry.syncError),
    draftCreatedAt: optionalNumber(entry.draftCreatedAt),
    lastSyncedAt: optionalNumber(entry.lastSyncedAt),
  };
}

function loadMessages(): Record<string, ActionMessage> {
  try {
    const raw = localStorage.getItem(MESSAGE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Array<Partial<ActionMessage> & Record<string, unknown>>;
    const out: Record<string, ActionMessage> = {};
    for (const entry of parsed) {
      const normalized = normalizeMessage(entry);
      if (!normalized.callsign) {
        continue;
      }
      out[normalized.callsign.toLowerCase()] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

function saveMessages(messages: Record<string, ActionMessage>): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

function summarizeMessage(message: ActionMessage): string {
  const state = message.syncState && message.syncState !== "synced" ? ` | ${message.syncState}` : "";
  return `${message.callsign} | ${formatR3aktTeamColor(message.groupName)}${state}`;
}

function parseLegacyMessageReplication(raw: string): LegacyMessageReplication | null {
  const envelope = parseReplicationEnvelope(raw);
  if (!envelope) {
    return null;
  }
  const { kind, payload } = envelope;
  switch (kind) {
    case "snapshot_request":
      return { kind, requestedAt: asNumber(payload.requestedAt, nowMs()) };
    case "snapshot_response":
      return {
        kind,
        requestedAt: asNumber(payload.requestedAt, nowMs()),
        messages: Array.isArray(payload.messages)
          ? payload.messages.map((entry) => normalizeMessage(entry as Record<string, unknown>))
          : [],
      };
    case "message_upsert":
      if (!payload.message || typeof payload.message !== "object") {
        return null;
      }
      return { kind, message: normalizeMessage(payload.message as Record<string, unknown>) };
    case "message_delete":
      return {
        kind,
        callsign: asTrimmedString(payload.callsign),
        deletedAt: asNumber(payload.deletedAt, nowMs()),
      };
    default:
      return null;
  }
}

export const useMessagesStore = defineStore("messages", () => {
  const byCallsign = reactive<Record<string, ActionMessage>>({});
  const teamSummary = ref<EamTeamSummary | null>(null);
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const replayInFlight = ref(false);
  const recoveryInFlight = ref(false);
  const nodeStore = useNodeStore();
  const peerSyncInFlight = new Set<string>();

  function persist(): void {
    saveMessages(byCallsign);
  }

  function keyFor(callsign: string): string {
    return callsign.trim().toLowerCase();
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;
    const loaded = loadMessages();
    for (const [key, message] of Object.entries(loaded)) {
      byCallsign[key] = { ...message };
    }
  }

  function localSourceIdentity(): string {
    return nodeStore.status.identityHex.trim().toLowerCase();
  }

  function localCallsign(): string {
    return nodeStore.settings.displayName.trim();
  }

  function fallbackTeamUid(message: Pick<ActionMessage, "groupName">): string {
    return createLocalRegistryId("local-team", message.groupName || DEFAULT_R3AKT_TEAM_COLOR);
  }

  function fallbackTeamMemberUid(message: Pick<ActionMessage, "callsign" | "groupName">): string {
    const sourceIdentity = localSourceIdentity();
    return createLocalRegistryId(
      "local-member",
      sourceIdentity || `${message.callsign}-${message.groupName}`,
    );
  }

  function eamTopics(teamUid?: string): string[] {
    return teamUid ? [teamUid, "eam"] : ["eam"];
  }

  function isDraftModeActive(): boolean {
    return nodeStore.settings.hub.mode !== "Disabled" && !nodeStore.hubRegistrationReady;
  }

  function isDeleted(message: ActionMessage): boolean {
    return typeof message.deletedAt === "number" && Number.isFinite(message.deletedAt);
  }

  function formatPeerLabel(peer: {
    label?: string;
    announcedName?: string;
    appDestinationHex: string;
  }): string {
    return peer.label?.trim() || peer.announcedName?.trim() || peer.appDestinationHex;
  }

  function replicationPeers(logMissing = false): ReplicationPeer[] {
    const localIdentity = normalizeHex(nodeStore.status.identityHex);
    const localAppDestination = normalizeHex(nodeStore.status.appDestinationHex);
    const localLxmfDestination = normalizeHex(nodeStore.status.lxmfDestinationHex);
    const seen = new Set<string>();
    const peers: ReplicationPeer[] = [];

    for (const peer of nodeStore.connectedEventPeerRoutes) {
      const appDestinationHex = normalizeHex(peer.appDestinationHex);
      const lxmfDestinationHex = normalizeHex(peer.lxmfDestinationHex);
      const peerIdentity = normalizeHex(peer.identityHex);
      if (
        !appDestinationHex
        || !lxmfDestinationHex
        || appDestinationHex === localAppDestination
        || appDestinationHex === localLxmfDestination
        || lxmfDestinationHex === localAppDestination
        || lxmfDestinationHex === localLxmfDestination
        || (peerIdentity.length > 0 && peerIdentity === localIdentity)
        || seen.has(lxmfDestinationHex)
      ) {
        continue;
      }
      seen.add(lxmfDestinationHex);
      peers.push({
        appDestinationHex: peer.appDestinationHex,
        lxmfDestinationHex: peer.lxmfDestinationHex,
        identityHex: peer.identityHex,
        label: formatPeerLabel(peer),
        announcedName: peer.announcedName,
      });
    }

    if (logMissing && peers.length === 0) {
      nodeStore.logUi("Debug", "[eam] no deliverable LXMF routes are available.");
    }
    return peers;
  }

  function formatPeerRoute(peer: ReplicationPeer): string {
    return `${peer.label} (app=${peer.appDestinationHex} lxmf=${peer.lxmfDestinationHex}${peer.identityHex ? ` identity=${peer.identityHex}` : ""})`;
  }

  function toEamRecord(message: ActionMessage): EamRecord {
    return {
      eam_uid: message.eamUid,
      callsign: message.callsign,
      team_member_uid: message.teamMemberUid ?? fallbackTeamMemberUid(message),
      team_uid: message.teamUid ?? fallbackTeamUid(message),
      reported_by: message.reportedBy ?? (localCallsign() || undefined),
      reported_at: message.reportedAt ?? new Date(message.updatedAt).toISOString(),
      overall_status: message.overallStatus,
      security_status: toWireStatus(message.securityStatus),
      capability_status: toWireStatus(message.capabilityStatus),
      preparedness_status: toWireStatus(message.preparednessStatus),
      medical_status: toWireStatus(message.medicalStatus),
      mobility_status: toWireStatus(message.mobilityStatus),
      comms_status: toWireStatus(message.commsStatus),
      notes: message.notes,
      confidence: message.confidence,
      ttl_seconds: message.ttlSeconds,
      source: message.source ?? (
        localSourceIdentity()
          ? {
              rns_identity: localSourceIdentity(),
              display_name: localCallsign() || undefined,
            }
          : undefined
      ),
    };
  }

  function messageFromEamRecord(record: EamRecord, existing?: ActionMessage): ActionMessage {
    const linkage = nodeStore.hubRegistration.linkage;
    return normalizeMessage({
      ...existing,
      callsign: record.callsign,
      groupName:
        existing?.groupName
        ?? (linkage && linkage.teamUid === record.team_uid ? linkage.teamColor : DEFAULT_R3AKT_TEAM_COLOR),
      securityStatus: normalizeStatus(record.security_status),
      capabilityStatus: normalizeStatus(record.capability_status),
      preparednessStatus: normalizeStatus(record.preparedness_status),
      medicalStatus: normalizeStatus(record.medical_status),
      mobilityStatus: normalizeStatus(record.mobility_status),
      commsStatus: normalizeStatus(record.comms_status),
      notes: record.notes,
      updatedAt: Date.parse(record.reported_at ?? "") || nowMs(),
      eamUid: record.eam_uid,
      teamMemberUid: record.team_member_uid,
      teamUid: record.team_uid,
      reportedAt: record.reported_at,
      reportedBy: record.reported_by,
      overallStatus: record.overall_status,
      confidence: record.confidence,
      ttlSeconds: record.ttl_seconds,
      source: record.source,
      syncState: "synced",
      syncError: undefined,
      lastSyncedAt: nowMs(),
    });
  }

  function applyUpsert(message: ActionMessage): UpsertOutcome {
    const normalized = normalizeMessage(message);
    if (!normalized.callsign) {
      return "ignored";
    }
    const key = keyFor(normalized.callsign);
    const existing = byCallsign[key];
    if (existing && existing.updatedAt > normalized.updatedAt) {
      return "ignored";
    }
    byCallsign[key] = {
      ...existing,
      ...normalized,
      groupName: normalizeR3aktTeamColor(
        normalized.groupName || existing?.groupName,
        DEFAULT_R3AKT_TEAM_COLOR,
      ),
    };
    persist();
    return existing ? "updated" : "inserted";
  }

  function applyDelete(callsign: string, deletedAt: number): void {
    const key = keyFor(callsign);
    const existing = byCallsign[key];
    if (!existing) {
      return;
    }
    if (existing.updatedAt > deletedAt) {
      return;
    }
    byCallsign[key] = normalizeMessage({
      ...existing,
      deletedAt,
      updatedAt: deletedAt,
      syncState: existing.syncState === "draft" ? "draft" : "synced",
      syncError: undefined,
      lastSyncedAt: existing.syncState === "draft" ? existing.lastSyncedAt : nowMs(),
    });
    persist();
  }

  function markMessageState(
    callsign: string,
    syncState: ActionMessage["syncState"],
    syncError?: string,
  ): void {
    const key = keyFor(callsign);
    const existing = byCallsign[key];
    if (!existing) {
      return;
    }
    byCallsign[key] = normalizeMessage({
      ...existing,
      syncState,
      syncError,
      lastSyncedAt: syncState === "synced" ? nowMs() : existing.lastSyncedAt,
    });
    persist();
  }

  function snapshotMessages(): ActionMessage[] {
    return Object.values(byCallsign).map((message) => ({ ...message }));
  }

  function syncedSnapshot(): ActionMessage[] {
    return snapshotMessages().filter((message) => !isDeleted(message));
  }

  function computeTeamSummary(teamUid: string): EamTeamSummary {
    const eams = syncedSnapshot().filter((message) => message.teamUid === teamUid);
    const byStatus: Partial<Record<EamWireStatus, number>> = {};
    let overallStatus: EamWireStatus | undefined;
    for (const message of eams) {
      const status = toWireStatus(message.overallStatus ?? normalizeStatus(message.overallStatus));
      if (!status) {
        continue;
      }
      byStatus[status] = (byStatus[status] ?? 0) + 1;
      if (!overallStatus || status === "Red" || (status === "Yellow" && overallStatus === "Green")) {
        overallStatus = status;
      }
    }
    return {
      team_uid: teamUid,
      member_count: eams.length,
      aggregation_method: "worst-of",
      overall_status: overallStatus,
      by_status: byStatus,
      computed_at: new Date().toISOString(),
    };
  }

  function localUpsertInput(
    next: Omit<ActionMessage, "updatedAt" | "deletedAt"> & { updatedAt?: number },
  ): ActionMessage {
    const linkage = nodeStore.hubRegistration.linkage;
    const updatedAt = Number(next.updatedAt ?? nowMs());
    return normalizeMessage({
      ...next,
      updatedAt,
      teamMemberUid: next.teamMemberUid ?? linkage?.teamMemberUid,
      teamUid: next.teamUid ?? linkage?.teamUid,
      reportedAt: next.reportedAt ?? new Date(updatedAt).toISOString(),
      reportedBy: next.reportedBy ?? (localCallsign() || undefined),
      source: next.source ?? (
        localSourceIdentity()
          ? {
              rns_identity: localSourceIdentity(),
              display_name: localCallsign() || undefined,
            }
          : undefined
      ),
      syncState: isDraftModeActive() ? "draft" : "syncing",
      syncError: isDraftModeActive() ? "Hub registration pending." : undefined,
      draftCreatedAt: next.draftCreatedAt ?? updatedAt,
    });
  }

  async function sendEamCommand(destination: string, command: EamCommandEnvelope): Promise<void> {
    await nodeStore.sendBytes(destination, EMPTY_BYTES, {
      fieldsBase64: buildEamCommandFieldsBase64([command]),
    });
  }

  async function sendEamResponse(
    destination: string,
    result: EamResponsePayload,
    event?: EamEventEnvelope,
  ): Promise<void> {
    await nodeStore.sendBytes(destination, EMPTY_BYTES, {
      fieldsBase64: buildEamResponseFieldsBase64({ result, event }),
    });
  }

  function commandArgsForMessage(message: ActionMessage): EamCommandArgsByType["mission.registry.eam.upsert"] {
    const record = toEamRecord(message);
    return {
      eam_uid: record.eam_uid,
      callsign: record.callsign,
      team_member_uid: record.team_member_uid,
      team_uid: record.team_uid,
      reported_by: record.reported_by,
      reported_at: record.reported_at,
      security_status: record.security_status,
      capability_status: record.capability_status,
      preparedness_status: record.preparedness_status,
      medical_status: record.medical_status,
      mobility_status: record.mobility_status,
      comms_status: record.comms_status,
      notes: record.notes,
      confidence: record.confidence,
      ttl_seconds: record.ttl_seconds,
      source: record.source,
    };
  }

  function replicationPayload(message: ActionMessage): ActionMessage {
    return normalizeMessage({
      ...message,
      teamMemberUid: message.teamMemberUid ?? nodeStore.hubRegistration.linkage?.teamMemberUid ?? fallbackTeamMemberUid(message),
      teamUid: message.teamUid ?? nodeStore.hubRegistration.linkage?.teamUid ?? fallbackTeamUid(message),
      syncState: message.syncState === "draft" ? "draft" : "syncing",
      syncError: undefined,
    });
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function describePeerFailures(failures: PeerFailure[]): string {
    return failures
      .map(({ peer, detail }) => `${formatPeerRoute(peer)}: ${detail}`)
      .join("; ");
  }

  async function sendAcrossPeers(
    action: string,
    peers: ReplicationPeer[],
    send: (peer: ReplicationPeer) => Promise<void>,
  ): Promise<void> {
    if (peers.length === 0) {
      throw new Error(`[eam] ${action} has no deliverable LXMF routes.`);
    }

    return await new Promise<void>((resolve, reject) => {
      let settled = false;
      let completed = 0;
      let successCount = 0;
      const failures: PeerFailure[] = [];

      const finalize = () => {
        if (completed !== peers.length) {
          return;
        }

        if (successCount === 0) {
          const summary = describePeerFailures(failures);
          if (!settled) {
            settled = true;
            reject(new Error(`[eam] ${action} failed for all ${peers.length} route(s): ${summary}`));
          }
          return;
        }

        if (failures.length > 0) {
          nodeStore.logUi(
            "Warn",
            `[eam] ${action} completed on ${successCount}/${peers.length} route(s). Failed routes: ${describePeerFailures(failures)}`,
          );
        }
      };

      for (const peer of peers) {
        void send(peer)
          .then(() => {
            successCount += 1;
            if (!settled) {
              settled = true;
              resolve();
            }
          })
          .catch((error: unknown) => {
            const detail = errorMessage(error);
            failures.push({ peer, detail });
            nodeStore.logUi("Warn", `[eam] ${action} failed for ${formatPeerRoute(peer)}: ${detail}`);
          })
          .finally(() => {
            completed += 1;
            finalize();
          });
      }
    });
  }

  function isTimeoutError(error: unknown): boolean {
    return errorMessage(error).trim().toLowerCase().includes("timeout");
  }

  function eventTypeForCommand(commandType: EamCommandType): EamEventEnvelope["event_type"] {
    switch (commandType) {
      case "mission.registry.eam.list":
        return "mission.registry.eam.listed";
      case "mission.registry.eam.upsert":
        return "mission.registry.eam.upserted";
      case "mission.registry.eam.get":
        return "mission.registry.eam.retrieved";
      case "mission.registry.eam.latest":
        return "mission.registry.eam.latest_retrieved";
      case "mission.registry.eam.delete":
        return "mission.registry.eam.deleted";
      case "mission.registry.eam.team.summary":
        return "mission.registry.eam.team_summary.retrieved";
      default:
        return "mission.registry.eam.upserted";
    }
  }

  function responseDetail(result: EamResponsePayload): string {
    if (result.status === "accepted") {
      return "accepted";
    }
    if (result.status === "rejected") {
      return result.reason || result.reason_code || "rejected";
    }
    return "result received";
  }

  function matchesTrackingIdentity(
    expected: EamTrackingExpectation,
    commandId?: string,
    correlationId?: string,
  ): boolean {
    return Boolean(
      (commandId && commandId === expected.commandId)
      || (correlationId && correlationId === expected.correlationId),
    );
  }

  function payloadMatchesExpectation(
    expected: EamTrackingExpectation,
    payload: unknown,
  ): boolean {
    const record = payload && typeof payload === "object" && !Array.isArray(payload)
      ? payload as Record<string, unknown>
      : null;
    if (!record) {
      return false;
    }
    if (expected.eamUid) {
      const payloadEamUid = asTrimmedString(record.eam_uid)
        ?? (record.eam && typeof record.eam === "object"
          ? asTrimmedString((record.eam as Record<string, unknown>).eam_uid)
          : undefined);
      if (payloadEamUid === expected.eamUid) {
        return true;
      }
    }
    if (expected.callsign) {
      const payloadCallsign = asTrimmedString(record.callsign)
        ?? (record.eam && typeof record.eam === "object"
          ? asTrimmedString((record.eam as Record<string, unknown>).callsign)
          : undefined);
      if (payloadCallsign?.toLowerCase() === expected.callsign.toLowerCase()) {
        return true;
      }
    }
    if (expected.teamUid) {
      const payloadTeamUid = asTrimmedString(record.team_uid)
        ?? (record.eam && typeof record.eam === "object"
          ? asTrimmedString((record.eam as Record<string, unknown>).team_uid)
          : undefined)
        ?? (record.summary && typeof record.summary === "object"
          ? asTrimmedString((record.summary as Record<string, unknown>).team_uid)
          : undefined);
      if (payloadTeamUid === expected.teamUid) {
        return true;
      }
    }
    return false;
  }

  function responseMatchesExpectation(
    expected: EamTrackingExpectation,
    result: EamResponsePayload | null,
  ): boolean {
    if (!result) {
      return false;
    }
    if (matchesTrackingIdentity(expected, result.command_id, result.correlation_id)) {
      return true;
    }
    if (result.status !== "result") {
      return false;
    }
    return payloadMatchesExpectation(expected, result.result);
  }

  function eventMatchesExpectation(
    expected: EamTrackingExpectation,
    event: EamEventEnvelope | null,
  ): boolean {
    if (!event || event.event_type !== eventTypeForCommand(expected.commandType)) {
      return false;
    }
    const meta = event.meta && typeof event.meta === "object" ? event.meta : undefined;
    if (
      matchesTrackingIdentity(
        expected,
        asTrimmedString(meta?.command_id),
        asTrimmedString(meta?.correlation_id),
      )
    ) {
      return true;
    }
    return payloadMatchesExpectation(expected, event.payload);
  }

  function createEamDeliveryTracker(peer: ReplicationPeer, expected: EamTrackingExpectation): {
    promise: Promise<void>;
    cancel: () => void;
  } {
    let settled = false;
    let timeoutId: number | undefined;
    let unsubscribePacket: () => void = () => undefined;
    let unsubscribeDelivery: () => void = () => undefined;

    const finish = (callback?: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId);
      }
      unsubscribePacket();
      unsubscribeDelivery();
      callback?.();
    };

    const promise = new Promise<void>((resolve, reject) => {
      unsubscribeDelivery = nodeStore.onLxmfDelivery((event: LxmfDeliveryEvent) => {
        if (normalizeHex(event.destinationHex) !== normalizeHex(peer.lxmfDestinationHex)) {
          return;
        }
        if (event.commandType && event.commandType !== expected.commandType) {
          return;
        }
        if (
          !matchesTrackingIdentity(expected, event.commandId, event.correlationId)
          && !event.commandType
        ) {
          return;
        }
        if (event.status === "Sent" || event.status === "Acknowledged") {
          return;
        }
        const detail = event.detail?.trim() || "delivery failed";
        finish(() => reject(new Error(`[eam] ${expected.commandType} failed for ${formatPeerRoute(peer)}: ${detail}`)));
      });

      unsubscribePacket = nodeStore.onPacket((packet: PacketReceivedEvent) => {
        const missionSync = parseEamMissionSyncFields(packet.fieldsBase64);
        const matchingResult = responseMatchesExpectation(expected, missionSync?.result ?? null)
          ? missionSync?.result ?? null
          : null;
        const matchingEvent = eventMatchesExpectation(expected, missionSync?.event ?? null)
          ? missionSync?.event ?? null
          : null;
        if (!matchingResult && !matchingEvent) {
          return;
        }
        if (matchingResult?.status === "accepted") {
          return;
        }
        if (matchingResult?.status === "rejected") {
          const detail = responseDetail(matchingResult);
          finish(() => reject(new Error(`[eam] ${expected.commandType} rejected by ${formatPeerRoute(peer)}: ${detail}`)));
          return;
        }
        finish(resolve);
      });

      timeoutId = window.setTimeout(() => {
        finish(() => reject(new Error(`[eam] ${expected.commandType} timed out for ${formatPeerRoute(peer)} after ${LXMF_DELIVERY_WAIT_TIMEOUT_MS}ms.`)));
      }, LXMF_DELIVERY_WAIT_TIMEOUT_MS);
    });

    return { promise, cancel: () => finish() };
  }

  async function sendEamCommandAwaitingDelivery(
    peer: ReplicationPeer,
    command: EamCommandEnvelope,
    expected: EamTrackingExpectation,
  ): Promise<void> {
    const tracker = createEamDeliveryTracker(peer, expected);
    try {
      await sendEamCommand(peer.lxmfDestinationHex, command);
    } catch (error: unknown) {
      if (!isTimeoutError(error)) {
        tracker.cancel();
        throw error;
      }
      nodeStore.logUi(
        "Warn",
        `[eam] native send timed out for ${expected.commandType} to ${formatPeerRoute(peer)}; waiting for mission result/event before failing.`,
      );
    }
    await tracker.promise;
  }

  async function sendUpsertToPeer(peer: ReplicationPeer, message: ActionMessage): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      throw new Error("A local Reticulum identity is required before sending EAM messages.");
    }
    const args = commandArgsForMessage(message);
    const routeSuffix = peer.lxmfDestinationHex.slice(0, 8);
    const commandId = createTrackingId("eam-upsert-command", `${message.callsign}-${routeSuffix}`);
    const correlationId = createTrackingId("eam-upsert", `${message.callsign}-${routeSuffix}`);
    await sendEamCommandAwaitingDelivery(
      peer,
      createEamUpsertCommandEnvelope({
        commandId,
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args,
        correlationId,
        topics: eamTopics(args.team_uid),
      }),
      {
        commandId,
        correlationId,
        commandType: "mission.registry.eam.upsert",
        eamUid: args.eam_uid,
        callsign: args.callsign,
        teamUid: args.team_uid,
      },
    );
  }

  async function sendDeleteToPeer(peer: ReplicationPeer, message: ActionMessage): Promise<void> {
    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }
    const args: EamCommandArgsByType["mission.registry.eam.delete"] = {
      eam_uid: message.eamUid,
      callsign: message.callsign,
      team_uid: message.teamUid ?? fallbackTeamUid(message),
      team_member_uid: message.teamMemberUid ?? fallbackTeamMemberUid(message),
    };
    const routeSuffix = peer.lxmfDestinationHex.slice(0, 8);
    const commandId = createTrackingId("eam-delete-command", `${message.callsign}-${routeSuffix}`);
    const correlationId = createTrackingId("eam-delete", `${message.callsign}-${routeSuffix}`);
    await sendEamCommandAwaitingDelivery(
      peer,
      createEamDeleteCommandEnvelope({
        commandId,
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args,
        correlationId,
        topics: eamTopics(args.team_uid),
      }),
      {
        commandId,
        correlationId,
        commandType: "mission.registry.eam.delete",
        eamUid: args.eam_uid,
        callsign: args.callsign,
        teamUid: args.team_uid,
      },
    );
  }

  async function fanoutUpsert(message: ActionMessage): Promise<void> {
    const peers = replicationPeers(true);
    if (peers.length === 0) {
      throw new Error("No deliverable LXMF route is available for EAM sync.");
    }
    const payload = replicationPayload(message);
    await sendAcrossPeers(`mission.registry.eam.upsert for ${payload.callsign}`, peers, async (peer) => {
      await sendUpsertToPeer(peer, payload);
    });
  }

  async function fanoutDelete(message: ActionMessage): Promise<void> {
    const peers = replicationPeers(true);
    if (peers.length === 0) {
      return;
    }
    const payload = replicationPayload(message);
    await sendAcrossPeers(`mission.registry.eam.delete for ${payload.callsign}`, peers, async (peer) => {
      await sendDeleteToPeer(peer, payload);
    });
  }

  async function retryErroredMessages(): Promise<void> {
    if (recoveryInFlight.value) {
      return;
    }

    const erroredMessages = Object.values(byCallsign)
      .filter((message) => message.syncState === "error")
      .sort((a, b) => (a.draftCreatedAt ?? a.updatedAt) - (b.draftCreatedAt ?? b.updatedAt));

    if (erroredMessages.length === 0) {
      return;
    }

    const peers = replicationPeers(true);
    const sourceIdentity = localSourceIdentity();
    if (peers.length === 0 || !sourceIdentity) {
      nodeStore.logUi(
        "Debug",
        peers.length === 0
          ? "[eam] retry skipped; no deliverable LXMF routes are available yet."
          : "[eam] retry skipped; local Reticulum identity is not ready yet.",
      );
      return;
    }

    recoveryInFlight.value = true;
    try {
      nodeStore.logUi(
        "Debug",
        `[eam] retrying ${erroredMessages.length} failed item(s) across ${peers.length} route(s).`,
      );

      for (const message of erroredMessages) {
        markMessageState(message.callsign, "syncing");
        const payload = replicationPayload(message);

        try {
          if (payload.deletedAt) {
            await fanoutDelete(payload);
          } else {
            await fanoutUpsert(payload);
          }
          nodeStore.logUi("Info", `[eam] retry submitted for ${message.callsign}.`);
        } catch (error: unknown) {
          const detail = error instanceof Error ? error.message : String(error);
          markMessageState(message.callsign, "error", detail);
          nodeStore.logUi("Warn", `[eam] retry failed for ${message.callsign}: ${detail}`);
          break;
        }
      }
    } finally {
      recoveryInFlight.value = false;
    }
  }

  async function requestCommand<T extends EamCommandType>(
    commandType: T,
    args: EamCommandArgsByType[T],
  ): Promise<void> {
    const peers = replicationPeers(false);
    const sourceIdentity = localSourceIdentity();
    if (peers.length === 0 || !sourceIdentity) {
      return;
    }
    await sendAcrossPeers(commandType, peers, async (peer) => {
      const command = createRequestCommandForPeer(peer, commandType, args, sourceIdentity);
      if (command) {
        await sendEamCommandAwaitingDelivery(peer, command, {
          commandId: command.command_id,
          correlationId: command.correlation_id ?? command.command_id,
          commandType,
          eamUid: "eam_uid" in args ? asTrimmedString((args as Record<string, unknown>).eam_uid) : undefined,
          callsign: "callsign" in args ? asTrimmedString((args as Record<string, unknown>).callsign) : undefined,
          teamUid: "team_uid" in args ? asTrimmedString((args as Record<string, unknown>).team_uid) : undefined,
        });
      }
    });
  }

  function createRequestCommandForPeer<T extends EamCommandType>(
    peer: ReplicationPeer,
    commandType: T,
    args: EamCommandArgsByType[T],
    sourceIdentity = localSourceIdentity(),
  ): EamCommandEnvelope<T> | null {
    if (!sourceIdentity) {
      return null;
    }
    const suffix = `${commandType.split(".").at(-1) ?? "eam"}-${peer.lxmfDestinationHex.slice(0, 8)}`;
    if (commandType === "mission.registry.eam.list") {
      return createEamListCommandEnvelope({
        commandId: createTrackingId("eam-list-command", suffix),
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args: args as EamCommandArgsByType["mission.registry.eam.list"],
        correlationId: createTrackingId("eam-list", suffix),
        topics: eamTopics((args as { team_uid?: string }).team_uid),
      }) as EamCommandEnvelope<T>;
    }
    if (commandType === "mission.registry.eam.get") {
      return createEamGetCommandEnvelope({
        commandId: createTrackingId("eam-get-command", suffix),
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args: args as EamCommandArgsByType["mission.registry.eam.get"],
        correlationId: createTrackingId("eam-get", suffix),
        topics: eamTopics((args as { team_uid?: string }).team_uid),
      }) as EamCommandEnvelope<T>;
    }
    if (commandType === "mission.registry.eam.latest") {
      return createEamLatestCommandEnvelope({
        commandId: createTrackingId("eam-latest-command", suffix),
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args: args as EamCommandArgsByType["mission.registry.eam.latest"],
        correlationId: createTrackingId("eam-latest", suffix),
        topics: eamTopics((args as { team_uid?: string }).team_uid),
      }) as EamCommandEnvelope<T>;
    }
    if (commandType === "mission.registry.eam.team.summary") {
      return createEamTeamSummaryCommandEnvelope({
        commandId: createTrackingId("eam-team-summary-command", suffix),
        sourceIdentity,
        sourceDisplayName: localCallsign() || undefined,
        args: args as EamCommandArgsByType["mission.registry.eam.team.summary"],
        correlationId: createTrackingId("eam-team-summary", suffix),
        topics: eamTopics((args as { team_uid?: string }).team_uid),
      }) as EamCommandEnvelope<T>;
    }
    return null;
  }

  async function requestListFromPeer(peer: ReplicationPeer): Promise<void> {
    const linkage = nodeStore.hubRegistration.linkage;
    const command = createRequestCommandForPeer(peer, "mission.registry.eam.list", {
      team_uid: linkage?.teamUid,
      team_member_uid: linkage?.teamMemberUid,
      include_deleted: false,
    });
    if (command) {
      await sendEamCommandAwaitingDelivery(peer, command, {
        commandId: command.command_id,
        correlationId: command.correlation_id ?? command.command_id,
        commandType: "mission.registry.eam.list",
        teamUid: linkage?.teamUid,
      });
    }
  }

  async function syncPeerMessages(peer: ReplicationPeer): Promise<void> {
    const destination = normalizeHex(peer.lxmfDestinationHex);
    if (!destination || peerSyncInFlight.has(destination)) {
      return;
    }

    const sourceIdentity = localSourceIdentity();
    if (!sourceIdentity) {
      return;
    }

    peerSyncInFlight.add(destination);
    try {
      const localMessages = snapshotMessages()
        .filter((message) => message.syncState !== "draft")
        .sort((a, b) => a.updatedAt - b.updatedAt);
      if (localMessages.length > 0) {
        nodeStore.logUi(
          "Debug",
          `[eam] replaying ${localMessages.length} local item(s) to ${peer.label} before hydration.`,
        );
      }
      for (const message of localMessages) {
        const payload = replicationPayload(message);
        if (payload.deletedAt) {
          await sendDeleteToPeer(peer, payload);
          continue;
        }
        await sendUpsertToPeer(peer, payload);
      }
      await requestListFromPeer(peer);
    } finally {
      peerSyncInFlight.delete(destination);
    }
  }

  async function requestList(): Promise<void> {
    const linkage = nodeStore.hubRegistration.linkage;
    await requestCommand("mission.registry.eam.list", {
      team_uid: linkage?.teamUid,
      team_member_uid: linkage?.teamMemberUid,
      include_deleted: false,
    });
  }

  async function requestLatest(callsign?: string): Promise<void> {
    const linkage = nodeStore.hubRegistration.linkage;
    await requestCommand("mission.registry.eam.latest", {
      callsign: callsign?.trim() || undefined,
      team_uid: linkage?.teamUid,
      team_member_uid: linkage?.teamMemberUid,
    });
  }

  async function requestMessage(callsign: string): Promise<void> {
    const existing = byCallsign[keyFor(callsign)];
    const linkage = nodeStore.hubRegistration.linkage;
    await requestCommand("mission.registry.eam.get", {
      eam_uid: existing?.eamUid,
      callsign,
      team_uid: existing?.teamUid ?? linkage?.teamUid,
      team_member_uid: existing?.teamMemberUid ?? linkage?.teamMemberUid,
    });
  }

  async function requestTeamSummary(): Promise<void> {
    const linkage = nodeStore.hubRegistration.linkage;
    if (!linkage) {
      return;
    }
    await requestCommand("mission.registry.eam.team.summary", {
      team_uid: linkage.teamUid,
      team_member_uid: linkage.teamMemberUid,
      callsign: linkage.callsign,
    });
  }

  async function replayPendingDrafts(): Promise<void> {
    if (replayInFlight.value || !nodeStore.hubRegistrationReady) {
      return;
    }
    const drafts = Object.values(byCallsign)
      .filter((message) => message.syncState === "draft" && !message.deletedAt)
      .sort((a, b) => (a.draftCreatedAt ?? a.updatedAt) - (b.draftCreatedAt ?? b.updatedAt));
    if (drafts.length === 0) {
      return;
    }
    replayInFlight.value = true;
    try {
      for (const draft of drafts) {
        markMessageState(draft.callsign, "syncing");
        try {
          await fanoutUpsert(normalizeMessage({
            ...draft,
            teamMemberUid: draft.teamMemberUid ?? nodeStore.hubRegistration.linkage?.teamMemberUid,
            teamUid: draft.teamUid ?? nodeStore.hubRegistration.linkage?.teamUid,
            syncState: "syncing",
            syncError: undefined,
          }));
        } catch (error: unknown) {
          markMessageState(draft.callsign, "error", error instanceof Error ? error.message : String(error));
          break;
        }
      }
    } finally {
      replayInFlight.value = false;
    }
  }

  async function upsertLocal(
    next: Omit<ActionMessage, "updatedAt" | "deletedAt"> & { updatedAt?: number },
  ): Promise<void> {
    if (!isDraftModeActive()) {
      nodeStore.assertReadyForOutbound("send Emergency messages");
    }
    const message = localUpsertInput(next);
    applyUpsert(message);
    if (isDraftModeActive()) {
      return;
    }
    try {
      await fanoutUpsert(message);
    } catch (error: unknown) {
      markMessageState(message.callsign, "error", error instanceof Error ? error.message : String(error));
      throw error;
    }
  }

  async function deleteLocal(callsign: string): Promise<void> {
    const existing = byCallsign[keyFor(callsign)];
    if (!existing) {
      return;
    }
    const deletedAt = nowMs();
    applyDelete(callsign, deletedAt);
    if (existing.syncState === "draft" || isDraftModeActive()) {
      return;
    }
    try {
      await fanoutDelete(normalizeMessage({ ...existing, deletedAt, updatedAt: deletedAt }));
    } catch (error: unknown) {
      markMessageState(callsign, "error", error instanceof Error ? error.message : String(error));
      throw error;
    }
  }

  function rotateStatus(callsign: string, field: keyof ActionMessage): void {
    const current = byCallsign[keyFor(callsign)];
    if (!current || current.deletedAt) {
      return;
    }
    if (
      field !== "securityStatus"
      && field !== "capabilityStatus"
      && field !== "preparednessStatus"
      && field !== "medicalStatus"
      && field !== "mobilityStatus"
      && field !== "commsStatus"
    ) {
      return;
    }
    const idx = STATUS_ROTATION.indexOf(normalizeStatus(current[field]));
    const nextStatus = STATUS_ROTATION[(idx + 1) % STATUS_ROTATION.length];
    void upsertLocal({ ...current, [field]: nextStatus });
  }

  function messageMatchesFilters(message: ActionMessage, args: EamFilterArgs): boolean {
    if (!(args.include_deleted === true) && message.deletedAt) {
      return false;
    }
    if (args.eam_uid && message.eamUid !== args.eam_uid) {
      return false;
    }
    if (args.callsign && message.callsign.trim().toLowerCase() !== String(args.callsign).trim().toLowerCase()) {
      return false;
    }
    if (args.team_uid && message.teamUid !== args.team_uid) {
      return false;
    }
    if (args.team_member_uid && message.teamMemberUid !== args.team_member_uid) {
      return false;
    }
    return true;
  }

  async function handleMissionCommand(destination: string, command: EamCommandEnvelope): Promise<void> {
    const localIdentity = localSourceIdentity();
    const localDisplayName = localCallsign() || undefined;
    const accepted = createEamAcceptedPayload({
      commandId: command.command_id,
      correlationId: command.correlation_id,
      byIdentity: localIdentity || undefined,
    });

    if (command.command_type === "mission.registry.eam.upsert") {
      const args = command.args as EamCommandArgsByType["mission.registry.eam.upsert"];
      const incoming = messageFromEamRecord({
        ...args,
        source: args.source ?? {
          rns_identity: command.source.rns_identity,
          display_name: command.source.display_name,
        },
      });
      const outcome = applyUpsert(incoming);
      const stored = byCallsign[keyFor(incoming.callsign)];
      await sendEamResponse(destination, accepted);
      await sendEamResponse(
        destination,
        createEamUpsertResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          eam: stored ? toEamRecord(stored) : null,
        }),
        createEamEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.eam.upserted",
          payload: { eam: stored ? toEamRecord(stored) : null },
          topics: eamTopics(stored?.teamUid ?? args.team_uid),
          meta: { command_id: command.command_id, correlation_id: command.correlation_id },
        }),
      );
      if (outcome !== "ignored" && incoming.source?.rns_identity !== localIdentity) {
        notifyOperationalUpdate("Emergency update", summarizeMessage(incoming)).catch(() => undefined);
      }
      return;
    }

    if (command.command_type === "mission.registry.eam.delete") {
      const args = command.args as EamCommandArgsByType["mission.registry.eam.delete"];
      const target = args.callsign
        ? byCallsign[keyFor(args.callsign)]
        : Object.values(byCallsign).find((message) => message.eamUid === args.eam_uid);
      await sendEamResponse(destination, accepted);
      if (!target) {
        await sendEamResponse(
          destination,
          createEamDeleteResultPayload({
            commandId: command.command_id,
            correlationId: command.correlation_id,
            eam: null,
            status: "not_found",
            eamUid: args.eam_uid,
            callsign: args.callsign,
          }),
        );
        return;
      }
      const deletedAt = nowMs();
      applyDelete(target.callsign, deletedAt);
      const deleted = byCallsign[keyFor(target.callsign)];
      await sendEamResponse(
        destination,
        createEamDeleteResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          eam: deleted ? toEamRecord(deleted) : null,
          status: "deleted",
          eamUid: deleted?.eamUid,
          callsign: deleted?.callsign,
        }),
        createEamEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.eam.deleted",
          payload: {
            eam: deleted ? toEamRecord(deleted) : null,
            status: "deleted",
            eam_uid: deleted?.eamUid,
            callsign: deleted?.callsign,
          },
          topics: eamTopics(deleted?.teamUid ?? args.team_uid),
          meta: { command_id: command.command_id, correlation_id: command.correlation_id },
        }),
      );
      return;
    }

    if (command.command_type === "mission.registry.eam.list") {
      const args = command.args as EamCommandArgsByType["mission.registry.eam.list"];
      const matches = syncedSnapshot()
        .filter((message) => messageMatchesFilters(message, args))
        .sort((a, b) => b.updatedAt - a.updatedAt)
        .slice(
          Number(args.offset ?? 0),
          Number(args.offset ?? 0) + Number(args.limit ?? Number.MAX_SAFE_INTEGER),
        )
        .map((message) => toEamRecord(message));
      await sendEamResponse(destination, accepted);
      await sendEamResponse(
        destination,
        createEamListResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          eams: matches,
        }),
        createEamEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.eam.listed",
          payload: { eams: matches },
          topics: eamTopics(args.team_uid),
          meta: { command_id: command.command_id, correlation_id: command.correlation_id },
        }),
      );
      return;
    }

    if (command.command_type === "mission.registry.eam.get" || command.command_type === "mission.registry.eam.latest") {
      const args = command.args as EamFilterArgs;
      const matches = syncedSnapshot()
        .filter((message) => messageMatchesFilters(message, args))
        .sort((a, b) => b.updatedAt - a.updatedAt);
      const match = command.command_type === "mission.registry.eam.latest" ? matches[0] : matches[0];
      await sendEamResponse(destination, accepted);
      await sendEamResponse(
        destination,
        command.command_type === "mission.registry.eam.latest"
          ? createEamLatestResultPayload({
              commandId: command.command_id,
              correlationId: command.correlation_id,
              eam: match ? toEamRecord(match) : null,
            })
          : createEamGetResultPayload({
              commandId: command.command_id,
              correlationId: command.correlation_id,
              eam: match ? toEamRecord(match) : null,
            }),
        createEamEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType:
            command.command_type === "mission.registry.eam.latest"
              ? "mission.registry.eam.latest_retrieved"
              : "mission.registry.eam.retrieved",
          payload: { eam: match ? toEamRecord(match) : null },
          topics: eamTopics(args.team_uid),
          meta: { command_id: command.command_id, correlation_id: command.correlation_id },
        }),
      );
      return;
    }

    if (command.command_type === "mission.registry.eam.team.summary") {
      const args = command.args as EamCommandArgsByType["mission.registry.eam.team.summary"];
      const summary = computeTeamSummary(args.team_uid);
      await sendEamResponse(destination, accepted);
      await sendEamResponse(
        destination,
        createEamTeamSummaryResultPayload({
          commandId: command.command_id,
          correlationId: command.correlation_id,
          summary,
        }),
        createEamEventEnvelope({
          sourceIdentity: localIdentity || "mobile",
          sourceDisplayName: localDisplayName,
          eventType: "mission.registry.eam.team_summary.retrieved",
          payload: { summary },
          topics: eamTopics(args.team_uid),
          meta: { command_id: command.command_id, correlation_id: command.correlation_id },
        }),
      );
      return;
    }

    await sendEamResponse(
      destination,
      createEamRejectedPayload({
        commandId: command.command_id,
        correlationId: command.correlation_id,
        reasonCode: "unsupported_command",
        reason: `Unsupported EAM command ${command.command_type}.`,
      }),
    );
  }

  function applyMissionPayload(result: EamResponsePayload | null, event: EamEventEnvelope | null): void {
    const payloads = [
      event?.payload ?? null,
      result?.status === "result" ? result.result : null,
    ];
    for (const payload of payloads) {
      if (!payload) {
        continue;
      }
      if ("summary" in payload) {
        teamSummary.value = payload.summary;
      }
      if ("eams" in payload) {
        for (const record of payload.eams) {
          applyUpsert(messageFromEamRecord(record, byCallsign[keyFor(record.callsign)]));
        }
      } else if ("eam" in payload && payload.eam) {
        applyUpsert(messageFromEamRecord(payload.eam, byCallsign[keyFor(payload.eam.callsign)]));
      } else if (
        "status" in payload
        && (payload.status === "deleted" || payload.status === "not_found")
      ) {
        const deletion = payload as {
          eam?: EamRecord | null;
          callsign?: string;
        };
        const callsign = deletion.eam?.callsign ?? deletion.callsign;
        if (callsign) {
          applyDelete(callsign, nowMs());
        }
      }
    }
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;
    const decoder = new TextDecoder();

    nodeStore.onPacket((event: PacketReceivedEvent) => {
      const missionSync = parseEamMissionSyncFields(event.fieldsBase64);
      if (missionSync) {
        if (missionSync.commands.length > 0 && event.sourceHex) {
          for (const command of missionSync.commands) {
            void handleMissionCommand(event.sourceHex, command);
          }
          return;
        }
        applyMissionPayload(missionSync.result, missionSync.event);
        return;
      }

      const legacy = parseLegacyMessageReplication(decoder.decode(event.bytes));
      if (!legacy) {
        return;
      }
      if (legacy.kind === "snapshot_request") {
        void nodeStore.sendJson(event.destinationHex, {
          kind: "snapshot_response",
          requestedAt: legacy.requestedAt,
          messages: snapshotMessages(),
        } as ReplicationMessage);
        return;
      }
      if (legacy.kind === "snapshot_response") {
        for (const incoming of legacy.messages) {
          if (incoming.deletedAt) {
            applyDelete(incoming.callsign, incoming.deletedAt);
          } else {
            applyUpsert(incoming);
          }
        }
        return;
      }
      if (legacy.kind === "message_upsert") {
        const outcome = applyUpsert(legacy.message);
        if (outcome !== "ignored") {
          notifyOperationalUpdate("Emergency update", summarizeMessage(legacy.message)).catch(() => undefined);
        }
        return;
      }
      if (legacy.kind === "message_delete") {
        applyDelete(legacy.callsign, legacy.deletedAt);
      }
    });

    nodeStore.onLxmfDelivery((event: LxmfDeliveryEvent) => {
      if (!event.commandType?.startsWith("mission.registry.eam")) {
        return;
      }
      const detail = event.detail?.trim() || "delivery issue";
      if (event.status === "Sent" || event.status === "Acknowledged") {
        nodeStore.logUi("Info", `[eam] ${event.status.toLowerCase()} ${event.commandType} to ${event.destinationHex}.`);
        return;
      }
      nodeStore.logUi(event.status === "TimedOut" ? "Warn" : "Error", `[eam] ${event.status.toLowerCase()} ${event.commandType} to ${event.destinationHex} (${detail}).`);
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
        void retryErroredMessages().catch(() => undefined);
        const previousDestinations = previous?.identity
          ? new Set(previous.peers.map((peer) => peer.lxmfDestinationHex))
          : new Set<string>();
        for (const peer of replicationPeers(false)) {
          if (previousDestinations.has(peer.lxmfDestinationHex)) {
            continue;
          }
          void syncPeerMessages(peer).catch((error: unknown) => {
            nodeStore.logUi(
              "Warn",
              `[eam] peer sync failed for ${peer.label}: ${error instanceof Error ? error.message : String(error)}`,
            );
          });
        }
      },
      { immediate: true, deep: true },
    );

    watch(
      () => nodeStore.hubRegistrationReady,
      (ready) => {
        if (!ready) {
          return;
        }
        void replayPendingDrafts().catch(() => undefined);
        void requestList().catch(() => undefined);
        void requestTeamSummary().catch(() => undefined);
      },
      { immediate: true },
    );
  }

  const messages = computed(() =>
    Object.values(byCallsign)
      .filter((message) => !message.deletedAt)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const activeCount = computed(() => messages.value.length);
  const draftCount = computed(() => messages.value.filter((message) => message.syncState === "draft").length);
  const syncingCount = computed(() => messages.value.filter((message) => message.syncState === "syncing").length);
  const redCount = computed(
    () =>
      messages.value.filter(
        (message) =>
          message.securityStatus === "Red"
          || message.mobilityStatus === "Red"
          || message.medicalStatus === "Red",
      ).length,
  );

  return {
    byCallsign,
    teamSummary,
    messages,
    activeCount,
    draftCount,
    syncingCount,
    redCount,
    init,
    initReplication,
    upsertLocal,
    deleteLocal,
    rotateStatus,
    requestList,
    requestLatest,
    requestMessage,
    requestTeamSummary,
    replayPendingDrafts,
  };
});
