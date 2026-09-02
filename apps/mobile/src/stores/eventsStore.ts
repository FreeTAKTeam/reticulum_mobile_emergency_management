import type { CommunityStatusProjectionRecord, EventProjectionRecord, ProjectionInvalidationEvent } from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";

import {
  notifyOperationalUpdateOnce,
  primeOperationalNotificationScope,
  truncateNotificationBody,
} from "../services/operationalNotifications";
import {
  asTrimmedString,
  createEventUid,
  createTrackingId,
  encodeEventTypeKeywords,
  getEventContent,
  getEventUid,
  getEventUpdatedAt,
  isDeletedEvent,
  loadWebEvents,
  normalizeEvent,
  saveWebEvents,
  toTimelineRecord,
} from "../utils/eventProjection";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { createProjectionClientAccessor } from "../utils/projectionClient";
import { DEFAULT_R3AKT_MISSION_NAME, DEFAULT_R3AKT_MISSION_UID } from "../utils/r3akt";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

const getProjectionClient = createProjectionClientAccessor("events");

export const useEventsStore = defineStore("events", () => {
  const nodeStore = useNodeStore();
  const byUid = ref<Record<string, EventProjectionRecord>>({});
  const nativeCommunityRecords = ref<CommunityStatusProjectionRecord[]>([]);
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const notificationsPrimed = ref(false);
  const cleanups: Array<() => void> = [];

  function webPersist(): void {
    if (!supportsNativeNodeRuntime) saveWebEvents(byUid.value);
  }

  function eventNotificationKey(record: EventProjectionRecord): string {
    return `${getEventUid(record)}:${getEventUpdatedAt(record)}`;
  }

  function isLocalEventRecord(record: EventProjectionRecord): boolean {
    const localIdentity = asTrimmedString(nodeStore.status.identityHex).toLowerCase();
    const eventIdentity = asTrimmedString(
      record.args.source_identity ?? record.source.rns_identity,
    ).toLowerCase();
    if (localIdentity && eventIdentity) return localIdentity === eventIdentity;
    const localDisplayName = asTrimmedString(nodeStore.settings.displayName).toLowerCase();
    if (!localDisplayName) return false;
    const sourceDisplayName = asTrimmedString(
      record.args.source_display_name ?? record.source.display_name,
    ).toLowerCase();
    const callsign = asTrimmedString(record.args.callsign).toLowerCase();
    return sourceDisplayName === localDisplayName || callsign === localDisplayName;
  }

  async function notifyForInboundEvents(
    records: Record<string, EventProjectionRecord>,
  ): Promise<void> {
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
      if (isLocalEventRecord(record)) continue;
      await notifyOperationalUpdateOnce(
        "event",
        eventNotificationKey(record),
        `Event from ${asTrimmedString(record.args.callsign) || "Unknown"}`,
        truncateNotificationBody(getEventContent(record)),
      );
    }
  }

  async function refreshFromNative(): Promise<void> {
    if (!supportsNativeNodeRuntime || !nodeStore.status.running) return;
    await projectionRefreshCoordinator.run("events", async () => {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      const [records, communities] = await Promise.all([
        client.getEvents(),
        client.getCommunityStatuses(),
      ]);
      const next: Record<string, EventProjectionRecord> = {};
      for (const record of records) {
        const normalized = normalizeEvent(record);
        next[getEventUid(normalized)] = normalized;
      }
      byUid.value = next;
      nativeCommunityRecords.value = communities;
      await notifyForInboundEvents(next);
    });
  }

  function requestNativeRefresh(): void {
    void refreshFromNative().catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      nodeStore.setLastError(message);
      nodeStore.logUi("Error", `[events] native refresh failed: ${message}`);
    });
  }

  function init(): void {
    if (initialized.value) return;
    initialized.value = true;
    if (!supportsNativeNodeRuntime) {
      byUid.value = loadWebEvents();
      return;
    }
    requestNativeRefresh();
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Events") requestNativeRefresh();
  }

  function initReplication(): void {
    if (replicationInitialized.value) return;
    replicationInitialized.value = true;
    if (!supportsNativeNodeRuntime) return;
    const client = getProjectionClient(nodeStore.settings.clientMode);
    cleanups.push(client.on("projectionInvalidated", handleProjectionInvalidation));
    cleanups.push(client.on("statusChanged", requestNativeRefresh));
    // Projection clients are distinct from the primary node client. Watch the
    // shared runtime state so a one-time native status replay cannot be missed.
    cleanups.push(watch(
      () => nodeStore.status.running,
      requestNativeRefresh,
      { immediate: true },
    ));
  }

  async function persistNativeEventUpsert(
    nextRecord: EventProjectionRecord,
    previousRecord: EventProjectionRecord | undefined,
  ): Promise<void> {
    const entryUid = getEventUid(nextRecord);
    try {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      await client.upsertEvent(nextRecord);
      await refreshFromNative();
    } catch (error: unknown) {
      const currentRecord = byUid.value[entryUid];
      if (currentRecord?.updatedAt === nextRecord.updatedAt) {
        if (previousRecord) {
          byUid.value = { ...byUid.value, [entryUid]: previousRecord };
        } else {
          const { [entryUid]: _failedRecord, ...remainingRecords } = byUid.value;
          byUid.value = remainingRecords;
        }
      }
      const message = error instanceof Error ? error.message : String(error);
      nodeStore.setLastError(message);
      nodeStore.logUi("Error", `[events] native create failed: ${message}`);
    }
  }

  async function persistNativeEventDelete(
    entryUid: string,
    deletedAt: number,
    previousRecord: EventProjectionRecord,
  ): Promise<void> {
    try {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      await client.deleteEvent(entryUid, deletedAt);
      await refreshFromNative();
    } catch (error: unknown) {
      if (byUid.value[entryUid]?.deleted_at === deletedAt) {
        byUid.value = { ...byUid.value, [entryUid]: previousRecord };
      }
      const message = error instanceof Error ? error.message : String(error);
      nodeStore.setLastError(message);
      nodeStore.logUi("Error", `[events] native delete failed: ${message}`);
    }
  }

  async function upsertLocal(input: { type: string; summary: string; uid?: string }): Promise<void> {
    const localDisplayName = nodeStore.settings.displayName.trim() || "Unknown";
    const updatedAt = Date.now();
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
      byUid.value = { ...byUid.value, [entryUid]: nextRecord };
      webPersist();
      return;
    }
    const previousRecord = byUid.value[entryUid];
    byUid.value = { ...byUid.value, [entryUid]: nextRecord };
    void persistNativeEventUpsert(nextRecord, previousRecord);
  }

  async function deleteLocal(uid: string): Promise<void> {
    const normalizedUid = uid.trim();
    if (!normalizedUid) return;
    const deletedAt = Date.now();
    const existing = byUid.value[normalizedUid];
    if (!existing) return;
    byUid.value = {
      ...byUid.value,
      [normalizedUid]: { ...existing, deleted_at: deletedAt, updatedAt: deletedAt },
    };
    if (!supportsNativeNodeRuntime) {
      webPersist();
      return;
    }
    void persistNativeEventDelete(normalizedUid, deletedAt, existing);
  }

  const records = computed(() => Object.values(byUid.value)
    .filter((entry) => !isDeletedEvent(entry))
    .sort((left, right) => getEventUpdatedAt(right) - getEventUpdatedAt(left))
    .map((entry) => toTimelineRecord(entry)));

  const communityRecords = computed(() => nativeCommunityRecords.value
    .filter((entry) => Date.now() - entry.updatedAtMs <= 7 * 24 * 60 * 60_000)
    .sort((left, right) => right.updatedAtMs - left.updatedAtMs));

  return { records, communityRecords, init, initReplication, upsertLocal, deleteLocal };
});
