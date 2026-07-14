import {
  type EamReadinessMessageRecord,
  type EamReadinessSummaryRecord,
  type ProjectionInvalidationEvent,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";

import {
  notifyOperationalUpdateOnce,
  primeOperationalNotificationScope,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import type { ActionMessage, EamTeamSummary } from "../types/domain";
import { applyActionMessageStatusCycle } from "../utils/actionMessageStatus";
import {
  cloneMessage,
  countRedStatuses,
  emptyEamReadinessSummary,
  keyFor,
  loadWebMessages,
  nextLocalUpdatedAt,
  normalizeIdentifier,
  normalizeMessage,
  optionalNumber,
  saveWebMessages,
  type StoredMessages,
  toProjectionRecord,
  toStoredMessages,
  toTeamSummary,
} from "../utils/eamProjection";
import { buildWebEamReadinessSummary, computeWebEamTeamSummary } from "../utils/eamReadiness";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { createProjectionClientAccessor } from "../utils/projectionClient";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

const getProjectionClient = createProjectionClientAccessor("messages");

export const useMessagesStore = defineStore("messages", () => {
  const nodeStore = useNodeStore();
  const byCallsign = ref<StoredMessages>({});
  const teamSummary = ref<EamTeamSummary | null>(null);
  const eamReadinessSummary = ref<EamReadinessSummaryRecord>(emptyEamReadinessSummary());
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const notificationsPrimed = ref(false);

  const cleanups: Array<() => void> = [];

  function webPersist(): void {
    if (!supportsNativeNodeRuntime) {
      saveWebMessages(byCallsign.value);
    }
  }

  function refreshWebReadiness(): void {
    eamReadinessSummary.value = buildWebEamReadinessSummary(Object.values(byCallsign.value));
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
    await projectionRefreshCoordinator.run("eams", async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      const [records, readinessSummary] = await Promise.all([
        client.getEams(),
        client.getEamReadinessSummary(),
      ]);
      const nextMessages = toStoredMessages(records);
      byCallsign.value = nextMessages;
      eamReadinessSummary.value = readinessSummary;
      await notifyForInboundMessages(nextMessages);
    }, { trailing: true });
  }

  async function refreshTeamSummary(): Promise<void> {
    const teamUid = nodeStore.hubRegistration.linkage?.teamUid?.trim() ?? "";
    if (!teamUid) {
      teamSummary.value = null;
      return;
    }

    if (!supportsNativeNodeRuntime) {
      teamSummary.value = computeWebEamTeamSummary(Object.values(byCallsign.value), teamUid);
      return;
    }

    await projectionRefreshCoordinator.run("eams:team-summary", async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      teamSummary.value = toTeamSummary(await client.getEamTeamSummary(teamUid));
    }, { trailing: true });
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
      refreshWebReadiness();
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
      updatedAt: nextLocalUpdatedAt(optionalNumber(next.updatedAt)),
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
      refreshWebReadiness();
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

    const key = keyFor(normalizedCallsign);
    const existing = byCallsign.value[key];
    const deletedAt = Date.now();
    const canReplicateDelete = !existing || canManageMessage(existing);

    if (existing) {
      byCallsign.value = {
        ...byCallsign.value,
        [key]: {
          ...existing,
          deletedAt,
          updatedAt: deletedAt,
        },
      };
    }

    if (!supportsNativeNodeRuntime) {
      if (!existing) {
        return;
      }
      webPersist();
      refreshWebReadiness();
      await refreshTeamSummary();
      return;
    }

    const client = getProjectionClient(nodeStore.settings.clientMode);
    try {
      if (canReplicateDelete) {
        await client.deleteEam(normalizedCallsign, deletedAt);
      } else {
        await client.deleteLocalEam(normalizedCallsign, deletedAt);
      }
      await refreshAll();
    } catch (error) {
      await refreshAll();
      throw error;
    }
  }

  function rotateStatus(callsign: string, field: keyof ActionMessage): void {
    const current = byCallsign.value[keyFor(callsign)];
    if (!current || current.deletedAt || !canManageMessage(current)) {
      return;
    }
    const updated = applyActionMessageStatusCycle(current, field, nextLocalUpdatedAt(current.updatedAt));
    if (!updated) {
      return;
    }
    byCallsign.value = {
      ...byCallsign.value,
      [keyFor(updated.callsign)]: cloneMessage(updated),
    };
    webPersist();
    void upsertLocal(updated);
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
  const eamReadinessByCallsign = computed(() => {
    const out: Record<string, EamReadinessMessageRecord> = {};
    for (const readiness of eamReadinessSummary.value.messages) {
      out[keyFor(readiness.callsign)] = readiness;
    }
    return out;
  });

  const activeCount = computed(() => messages.value.length);
  const draftCount = computed(() => messages.value.filter((message) => message.syncState === "draft").length);
  const syncingCount = computed(() => messages.value.filter((message) => message.syncState === "syncing").length);
  const redCount = computed(() =>
    messages.value.reduce((total, message) => total + countRedStatuses(message), 0),
  );

  function eamReadinessForCallsign(callsign: string): EamReadinessMessageRecord | undefined {
    return eamReadinessByCallsign.value[keyFor(callsign)];
  }

  return {
    byCallsign,
    teamSummary,
    eamReadinessSummary,
    messages,
    eamReadinessByCallsign,
    eamReadinessForCallsign,
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
