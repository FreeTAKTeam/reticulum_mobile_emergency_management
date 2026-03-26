import {
  createReticulumNodeClient,
  type EamProjectionRecord,
  type EamTeamSummaryRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";

import {
  notifyOperationalUpdateOnce,
  primeOperationalNotificationScope,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import type { ActionMessage, EamStatus, EamTeamSummary, EamWireStatus } from "../types/domain";
import { DEFAULT_R3AKT_TEAM_COLOR, normalizeR3aktTeamColor } from "../utils/r3akt";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

const MESSAGE_STORAGE_KEY = "reticulum.mobile.messages.v1";
const STATUS_ROTATION: EamStatus[] = ["Unknown", "Green", "Yellow", "Red"];

type StoredMessages = Record<string, ActionMessage>;
type TeamStatusBuckets = Partial<Record<EamWireStatus, number>>;

type ProjectionClientCache = typeof globalThis & {
  __reticulumMessagesProjectionClient?: ReticulumNodeClient;
};

function nowMs(): number {
  return Date.now();
}

function normalizeStatus(value: unknown): EamStatus {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : "Unknown";
}

function normalizeWireStatus(value: unknown): EamWireStatus | undefined {
  return value === "Green" || value === "Yellow" || value === "Red" ? value : undefined;
}

function normalizeSyncState(
  value: unknown,
): ActionMessage["syncState"] {
  return value === "draft" || value === "syncing" || value === "synced" || value === "error"
    ? value
    : undefined;
}

function optionalNumber(value: unknown): number | undefined {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function asTrimmedString(value: unknown): string | undefined {
  return typeof value === "string" ? value.trim() || undefined : undefined;
}

function normalizeIdentifier(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function keyFor(callsign: string): string {
  return callsign.trim().toLowerCase();
}

function cloneMessage(message: ActionMessage): ActionMessage {
  return {
    ...message,
    source: message.source ? { ...message.source } : undefined,
  };
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
    overallStatus: normalizeWireStatus(entry.overallStatus),
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

function loadWebMessages(): StoredMessages {
  try {
    const raw = localStorage.getItem(MESSAGE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Array<Partial<ActionMessage>>;
    const out: StoredMessages = {};
    for (const entry of parsed) {
      const normalized = normalizeMessage(entry);
      if (!normalized.callsign) {
        continue;
      }
      out[keyFor(normalized.callsign)] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

function saveWebMessages(messages: StoredMessages): void {
  localStorage.setItem(MESSAGE_STORAGE_KEY, JSON.stringify(Object.values(messages)));
}

function toProjectionRecord(message: ActionMessage): EamProjectionRecord {
  return {
    callsign: message.callsign,
    groupName: normalizeR3aktTeamColor(message.groupName, DEFAULT_R3AKT_TEAM_COLOR),
    securityStatus: message.securityStatus,
    capabilityStatus: message.capabilityStatus,
    preparednessStatus: message.preparednessStatus,
    medicalStatus: message.medicalStatus,
    mobilityStatus: message.mobilityStatus,
    commsStatus: message.commsStatus,
    notes: message.notes,
    updatedAt: message.updatedAt,
    deletedAt: message.deletedAt,
    eamUid: message.eamUid,
    teamMemberUid: message.teamMemberUid,
    teamUid: message.teamUid,
    reportedAt: message.reportedAt,
    reportedBy: message.reportedBy,
    overallStatus: message.overallStatus,
    confidence: message.confidence,
    ttlSeconds: message.ttlSeconds,
    source: message.source,
    syncState: message.syncState,
    syncError: message.syncError,
    draftCreatedAt: message.draftCreatedAt,
    lastSyncedAt: message.lastSyncedAt,
  };
}

function fromProjectionRecord(record: EamProjectionRecord): ActionMessage {
  return normalizeMessage({
    callsign: record.callsign,
    groupName: record.groupName,
    securityStatus: normalizeStatus(record.securityStatus),
    capabilityStatus: normalizeStatus(record.capabilityStatus),
    preparednessStatus: normalizeStatus(record.preparednessStatus),
    medicalStatus: normalizeStatus(record.medicalStatus),
    mobilityStatus: normalizeStatus(record.mobilityStatus),
    commsStatus: normalizeStatus(record.commsStatus),
    notes: record.notes,
    updatedAt: record.updatedAt,
    deletedAt: record.deletedAt,
    eamUid: record.eamUid,
    teamMemberUid: record.teamMemberUid,
    teamUid: record.teamUid,
    reportedAt: record.reportedAt,
    reportedBy: record.reportedBy,
    overallStatus: normalizeWireStatus(record.overallStatus),
    confidence: record.confidence,
    ttlSeconds: record.ttlSeconds,
    source: record.source,
    syncState: normalizeSyncState(record.syncState),
    syncError: record.syncError,
    draftCreatedAt: record.draftCreatedAt,
    lastSyncedAt: record.lastSyncedAt,
  });
}

function toStoredMessages(records: EamProjectionRecord[]): StoredMessages {
  const out: StoredMessages = {};
  for (const record of records) {
    const message = fromProjectionRecord(record);
    if (!message.callsign) {
      continue;
    }
    out[keyFor(message.callsign)] = message;
  }
  return out;
}

function toTeamSummary(record: EamTeamSummaryRecord | null): EamTeamSummary | null {
  if (!record) {
    return null;
  }
  const byStatus: TeamStatusBuckets = {};
  if (record.greenTotal > 0) {
    byStatus.Green = record.greenTotal;
  }
  if (record.yellowTotal > 0) {
    byStatus.Yellow = record.yellowTotal;
  }
  if (record.redTotal > 0) {
    byStatus.Red = record.redTotal;
  }
  return {
    team_uid: record.teamUid,
    total: record.total,
    active_total: record.activeTotal,
    deleted_total: record.deletedTotal,
    overall_status: normalizeWireStatus(record.overallStatus),
    by_status: byStatus,
    updated_at: new Date(record.updatedAt).toISOString(),
  };
}

function computeWebTeamSummary(messages: ActionMessage[], teamUid: string): EamTeamSummary {
  const teamMessages = messages.filter(
    (message) => message.teamUid === teamUid && !message.deletedAt,
  );
  const byStatus: TeamStatusBuckets = {};
  for (const message of teamMessages) {
    const status = message.overallStatus;
    if (!status) {
      continue;
    }
    byStatus[status] = (byStatus[status] ?? 0) + 1;
  }
  const overallStatus = byStatus.Red
    ? "Red"
    : byStatus.Yellow
      ? "Yellow"
      : byStatus.Green
        ? "Green"
        : undefined;
  return {
    team_uid: teamUid,
    total: teamMessages.length,
    active_total: teamMessages.length,
    deleted_total: messages.filter((message) => message.teamUid === teamUid && Boolean(message.deletedAt)).length,
    overall_status: overallStatus,
    by_status: byStatus,
    updated_at: new Date().toISOString(),
  };
}

function getProjectionClient(mode: "auto" | "capacitor"): ReticulumNodeClient {
  const cache = globalThis as ProjectionClientCache;
  if (!cache.__reticulumMessagesProjectionClient) {
    cache.__reticulumMessagesProjectionClient = createReticulumNodeClient({ mode });
  }
  return cache.__reticulumMessagesProjectionClient;
}

function countRedStatuses(message: ActionMessage): number {
  return [
    message.securityStatus,
    message.capabilityStatus,
    message.preparednessStatus,
    message.medicalStatus,
    message.mobilityStatus,
    message.commsStatus,
  ].filter((status) => status === "Red").length;
}

export const useMessagesStore = defineStore("messages", () => {
  const nodeStore = useNodeStore();
  const byCallsign = ref<StoredMessages>({});
  const teamSummary = ref<EamTeamSummary | null>(null);
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const notificationsPrimed = ref(false);

  let refreshPromise: Promise<void> | null = null;
  let refreshQueued = false;
  let teamSummaryPromise: Promise<void> | null = null;
  let teamSummaryQueued = false;
  const cleanups: Array<() => void> = [];

  function webPersist(): void {
    if (!supportsNativeNodeRuntime) {
      saveWebMessages(byCallsign.value);
    }
  }

  function canManageMessage(message: ActionMessage): boolean {
    const localAppDestination = normalizeIdentifier(nodeStore.status.appDestinationHex);
    const localIdentity = normalizeIdentifier(nodeStore.status.identityHex);
    const localDisplayName = normalizeIdentifier(nodeStore.settings.displayName);
    const messageTeamMemberUid = normalizeIdentifier(message.teamMemberUid);
    const messageSourceIdentity = normalizeIdentifier(message.source?.rns_identity);
    const messageReportedBy = normalizeIdentifier(message.reportedBy);
    const messageCallsign = normalizeIdentifier(message.callsign);
    const hasRemoteIdentity = Boolean(messageTeamMemberUid || messageSourceIdentity || message.lastSyncedAt);

    if (message.syncState === "draft") {
      return true;
    }
    if (!hasRemoteIdentity) {
      return true;
    }
    if (localAppDestination && messageTeamMemberUid && messageTeamMemberUid === localAppDestination) {
      return true;
    }
    if (localIdentity && messageSourceIdentity && messageSourceIdentity === localIdentity) {
      return true;
    }
    if (localDisplayName && (messageCallsign === localDisplayName || messageReportedBy === localDisplayName)) {
      return true;
    }
    return false;
  }

  function eamNotificationKey(message: ActionMessage): string {
    return `${keyFor(message.callsign)}:${message.updatedAt}`;
  }

  async function notifyForInboundMessages(messages: StoredMessages): Promise<void> {
    const activeMessages = Object.values(messages).filter((message) => !message.deletedAt);
    if (!notificationsPrimed.value) {
      primeOperationalNotificationScope(
        "eam",
        activeMessages.map((message) => eamNotificationKey(message)),
      );
      notificationsPrimed.value = true;
      return;
    }

    for (const message of activeMessages) {
      if (canManageMessage(message)) {
        continue;
      }
      const title = `EAM from ${message.reportedBy?.trim() || message.callsign}`;
      const body = message.notes?.trim()
        || `${message.groupName} status ${message.overallStatus ?? "updated"}`;
      await notifyOperationalUpdateOnce(
        "eam",
        eamNotificationKey(message),
        title,
        truncateNotificationBody(body),
      );
    }
  }

  async function refreshFromNative(): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      return;
    }
    if (refreshPromise) {
      refreshQueued = true;
      await refreshPromise;
      return;
    }
    const promise = (async () => {
      do {
        refreshQueued = false;
        const client = getProjectionClient(nodeStore.settings.clientMode);
        const records = await client.getEams();
        const nextMessages = toStoredMessages(records);
        byCallsign.value = nextMessages;
        await notifyForInboundMessages(nextMessages);
      } while (refreshQueued);
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

  async function refreshTeamSummary(): Promise<void> {
    const teamUid = nodeStore.hubRegistration.linkage?.teamUid?.trim() ?? "";
    if (!teamUid) {
      teamSummary.value = null;
      return;
    }

    if (!supportsNativeNodeRuntime) {
      teamSummary.value = computeWebTeamSummary(Object.values(byCallsign.value), teamUid);
      return;
    }

    if (teamSummaryPromise) {
      teamSummaryQueued = true;
      await teamSummaryPromise;
      return;
    }

    const promise = (async () => {
      do {
        teamSummaryQueued = false;
        const client = getProjectionClient(nodeStore.settings.clientMode);
        teamSummary.value = toTeamSummary(await client.getEamTeamSummary(teamUid));
      } while (teamSummaryQueued);
    })();
    teamSummaryPromise = promise;
    try {
      await promise;
    } finally {
      if (teamSummaryPromise === promise) {
        teamSummaryPromise = null;
      }
    }
  }

  async function refreshAll(): Promise<void> {
    await refreshFromNative();
    await refreshTeamSummary();
  }

  function init(): void {
    if (initialized.value) {
      if (supportsNativeNodeRuntime) {
        void refreshAll();
      }
      return;
    }
    initialized.value = true;

    if (!supportsNativeNodeRuntime) {
      byCallsign.value = loadWebMessages();
      void refreshTeamSummary();
      return;
    }

    void refreshAll();
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Eams") {
      void refreshAll();
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
      void refreshAll();
    }));

    watch(
      () => nodeStore.hubRegistration.linkage?.teamUid ?? "",
      () => {
        void refreshTeamSummary();
      },
      { immediate: true },
    );
  }

  async function upsertLocal(
    next: Omit<ActionMessage, "updatedAt" | "deletedAt"> & { updatedAt?: number },
  ): Promise<void> {
    const normalized = normalizeMessage({
      ...next,
      updatedAt: optionalNumber(next.updatedAt) ?? nowMs(),
    });
    if (!normalized.callsign) {
      return;
    }
    const existing = byCallsign.value[keyFor(normalized.callsign)];
    if (existing && !canManageMessage(existing)) {
      return;
    }

    if (!supportsNativeNodeRuntime) {
      byCallsign.value = {
        ...byCallsign.value,
        [keyFor(normalized.callsign)]: cloneMessage(normalized),
      };
      webPersist();
      await refreshTeamSummary();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await client.upsertEam(toProjectionRecord(normalized));
    await refreshAll();
  }

  async function deleteLocal(callsign: string): Promise<void> {
    const normalizedCallsign = callsign.trim();
    if (!normalizedCallsign) {
      return;
    }
    const existing = byCallsign.value[keyFor(normalizedCallsign)];
    if (existing && !canManageMessage(existing)) {
      return;
    }

    if (!supportsNativeNodeRuntime) {
      const key = keyFor(normalizedCallsign);
      const existing = byCallsign.value[key];
      if (!existing) {
        return;
      }
      byCallsign.value = {
        ...byCallsign.value,
        [key]: {
          ...existing,
          deletedAt: nowMs(),
          updatedAt: nowMs(),
        },
      };
      webPersist();
      await refreshTeamSummary();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    await client.deleteEam(normalizedCallsign, nowMs());
    await refreshAll();
  }

  function rotateStatus(callsign: string, field: keyof ActionMessage): void {
    const current = byCallsign.value[keyFor(callsign)];
    if (!current || current.deletedAt || !canManageMessage(current)) {
      return;
    }
    const nextStatusKey = String(field);
    if (!nextStatusKey.endsWith("Status")) {
      return;
    }
    const currentStatus = normalizeStatus(current[field]);
    const currentIndex = STATUS_ROTATION.indexOf(currentStatus);
    const nextStatus = STATUS_ROTATION[(currentIndex + 1) % STATUS_ROTATION.length];
    void upsertLocal({
      ...current,
      [field]: nextStatus,
    });
  }

  async function requestList(): Promise<void> {
    await refreshAll();
  }

  async function requestLatest(_callsign?: string): Promise<void> {
    await refreshAll();
  }

  async function requestMessage(_callsign: string): Promise<void> {
    await refreshAll();
  }

  async function requestTeamSummary(): Promise<void> {
    await refreshTeamSummary();
  }

  async function replayPendingDrafts(): Promise<void> {
    await refreshAll();
  }

  const messages = computed(() =>
    Object.values(byCallsign.value)
      .filter((message) => !message.deletedAt)
      .sort((left, right) => right.updatedAt - left.updatedAt),
  );

  const activeCount = computed(() => messages.value.length);
  const draftCount = computed(() => messages.value.filter((message) => message.syncState === "draft").length);
  const syncingCount = computed(() => messages.value.filter((message) => message.syncState === "syncing").length);
  const redCount = computed(() =>
    messages.value.reduce((total, message) => total + countRedStatuses(message), 0),
  );

  return {
    byCallsign,
    teamSummary,
    messages,
    activeCount,
    draftCount,
    syncingCount,
    redCount,
    canManageMessage,
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
