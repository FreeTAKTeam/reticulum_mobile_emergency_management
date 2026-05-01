import {
  createReticulumNodeClient,
  type ChecklistRecord as RuntimeChecklistRecord,
  type ChecklistTemplateRecord as RuntimeChecklistTemplateRecord,
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
  runtimeChecklistToUi,
  runtimeTemplateToUi,
  type ChecklistRecord as UiChecklistRecord,
} from "../utils/checklists";
import { useNodeStore } from "./nodeStore";

type ProjectionClientCache = typeof globalThis & {
  __reticulumChecklistsProjectionClient?: ReticulumNodeClient;
};

type RuntimeChecklistDetailRecord = RuntimeChecklistRecord | RuntimeChecklistTemplateRecord;
type ChecklistNotificationWork = {
  key: string;
  title: string;
  body: string;
  route: string;
  timer: ReturnType<typeof setTimeout>;
};

const CHECKLIST_NOTIFICATION_DEBOUNCE_MS = 2_000;

function getProjectionClient(clientMode: "auto" | "capacitor"): ReticulumNodeClient {
  const cache = globalThis as ProjectionClientCache;
  if (!cache.__reticulumChecklistsProjectionClient) {
    cache.__reticulumChecklistsProjectionClient = createReticulumNodeClient({ mode: clientMode });
  }
  return cache.__reticulumChecklistsProjectionClient;
}

function normalizeMissionUid(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || `mission-${Date.now().toString(36)}`;
}

export const useChecklistsStore = defineStore("checklists", () => {
  const nodeStore = useNodeStore();
  const live = ref<RuntimeChecklistRecord[]>([]);
  const templates = ref<RuntimeChecklistTemplateRecord[]>([]);
  const detailById = ref<Record<string, RuntimeChecklistDetailRecord | null>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const notificationsPrimed = ref(false);
  const loadingLive = ref(false);
  const loadingTemplates = ref(false);
  const loadingDetailIds = ref<Record<string, boolean>>({});
  const trackedDetailIds = new Set<string>();
  const cleanups: Array<() => void> = [];

  let refreshLivePromise: Promise<void> | null = null;
  let refreshTemplatesPromise: Promise<void> | null = null;
  const pendingChecklistNotifications = new Map<string, ChecklistNotificationWork>();
  const detailPromises = new Map<string, Promise<void>>();

  function client(): ReticulumNodeClient {
    return getProjectionClient(nodeStore.settings.clientMode);
  }

  function activeTaskCount(record: Pick<RuntimeChecklistRecord, "tasks">): number {
    return record.tasks.filter((task) => !task.deletedAt && task.number > 0).length;
  }

  function projectedTaskTotal(record: RuntimeChecklistRecord): number {
    const countedTasks =
      record.counts.pendingCount + record.counts.lateCount + record.counts.completeCount;
    const expectedTasks = typeof record.expectedTaskCount === "number" ? record.expectedTaskCount : 0;
    const highestTaskNumber = record.tasks.reduce((highest, task) => Math.max(highest, task.number), 0);
    return Math.max(activeTaskCount(record), countedTasks, expectedTasks, highestTaskNumber);
  }

  const liveUiRecords = computed<UiChecklistRecord[]>(() => live.value.map(runtimeChecklistToUi));
  const templateUiRecords = computed<UiChecklistRecord[]>(() => templates.value.map(runtimeTemplateToUi));
  const liveTaskTotal = computed(() =>
    live.value
      .filter((record) => !record.deletedAt)
      .reduce((total, record) => total + projectedTaskTotal(record), 0),
  );
  const templateTaskTotal = computed(() =>
    templates.value.reduce((total, record) => total + activeTaskCount(record), 0),
  );
  const dashboardSummary = computed(() => ({
    total: liveUiRecords.value.length,
    active: liveUiRecords.value.filter((record) => record.status === "active").length,
    late: liveUiRecords.value.filter((record) => record.status === "late").length,
  }));

  function setDetailLoading(checklistUid: string, value: boolean): void {
    loadingDetailIds.value = {
      ...loadingDetailIds.value,
      [checklistUid]: value,
    };
  }

  function latestChecklistChangeStamp(record: RuntimeChecklistRecord): string {
    const stamps = [
      record.updatedAt,
      record.uploadedAt,
      record.deletedAt,
    ].filter((value): value is string => Boolean(value?.trim()));
    return stamps.reduce((latest, value) => (value > latest ? value : latest), "");
  }

  function checklistNotificationKey(record: RuntimeChecklistRecord): string {
    return `${record.uid}:${latestChecklistChangeStamp(record)}`;
  }

  function normalizedIdentity(value: string | undefined): string {
    return (value ?? "").trim().toLowerCase();
  }

  function isLocalChecklistRecord(record: RuntimeChecklistRecord): boolean {
    const localIdentity = normalizedIdentity(nodeStore.status.identityHex);
    if (!localIdentity) {
      return false;
    }
    const changedBy = normalizedIdentity(record.lastChangedByTeamMemberRnsIdentity);
    if (changedBy) {
      return changedBy === localIdentity;
    }
    return normalizedIdentity(record.createdByTeamMemberRnsIdentity) === localIdentity;
  }

  function checklistNotificationBody(record: RuntimeChecklistRecord): string {
    const counts = record.counts ?? { pendingCount: 0, completeCount: 0, lateCount: 0 };
    const tasks = Array.isArray(record.tasks) ? record.tasks : [];
    const summary = `${counts.pendingCount} pending, ${counts.completeCount} complete`;
    const late = counts.lateCount > 0 ? `, ${counts.lateCount} late` : "";
    const taskCount = tasks.length === 1 ? "1 task" : `${tasks.length} tasks`;
    return truncateNotificationBody(`${summary}${late} across ${taskCount}`);
  }

  function queueChecklistNotification(record: RuntimeChecklistRecord): void {
    const key = checklistNotificationKey(record);
    if (!key.trim()) {
      return;
    }
    const existing = pendingChecklistNotifications.get(record.uid);
    if (existing) {
      clearTimeout(existing.timer);
    }
    const work: ChecklistNotificationWork = {
      key,
      title: `Checklist updated: ${record.name || "Checklist"}`,
      body: checklistNotificationBody(record),
    route: `/checklists/${record.uid}`,
      timer: setTimeout(() => {
        pendingChecklistNotifications.delete(record.uid);
        void notifyOperationalUpdateOnce(
          "checklist",
          work.key,
          work.title,
          work.body,
          { route: work.route },
        );
      }, CHECKLIST_NOTIFICATION_DEBOUNCE_MS),
    };
    pendingChecklistNotifications.set(record.uid, work);
  }

  async function notifyForChecklistChanges(records: RuntimeChecklistRecord[]): Promise<void> {
    const activeRecords = records.filter((record) => record && !record.deletedAt);
    if (!notificationsPrimed.value) {
      primeOperationalNotificationScope(
        "checklist",
        activeRecords.map((record) => checklistNotificationKey(record)),
      );
      notificationsPrimed.value = true;
      return;
    }

    for (const record of activeRecords) {
      if (isLocalChecklistRecord(record)) {
        continue;
      }
      queueChecklistNotification(record);
    }
  }

  async function refreshLive(): Promise<void> {
    if (refreshLivePromise) {
      await refreshLivePromise;
      return;
    }
    const promise = (async () => {
      loadingLive.value = true;
      try {
        const records = await client().listActiveChecklists();
        live.value = records;
        detailById.value = {
          ...detailById.value,
          ...Object.fromEntries(records.map((record) => [record.uid, record])),
        };
        await notifyForChecklistChanges(records);
      } finally {
        loadingLive.value = false;
      }
    })();
    refreshLivePromise = promise;
    try {
      await promise;
    } finally {
      if (refreshLivePromise === promise) {
        refreshLivePromise = null;
      }
    }
  }

  async function refreshTemplates(): Promise<void> {
    if (refreshTemplatesPromise) {
      await refreshTemplatesPromise;
      return;
    }
    const promise = (async () => {
      loadingTemplates.value = true;
      try {
        templates.value = await client().listChecklistTemplates();
      } finally {
        loadingTemplates.value = false;
      }
    })();
    refreshTemplatesPromise = promise;
    try {
      await promise;
    } finally {
      if (refreshTemplatesPromise === promise) {
        refreshTemplatesPromise = null;
      }
    }
  }

  async function refreshDetail(checklistUid: string): Promise<void> {
    const normalizedUid = checklistUid.trim();
    if (!normalizedUid) {
      return;
    }
    trackedDetailIds.add(normalizedUid);
    const existing = detailPromises.get(normalizedUid);
    if (existing) {
      await existing;
      return;
    }
    const promise = (async () => {
      setDetailLoading(normalizedUid, true);
      try {
        let record: RuntimeChecklistDetailRecord | null = await client().getChecklist(normalizedUid);
        if (!record) {
          record = getTemplateById(normalizedUid);
          if (!record) {
            await refreshTemplates();
            record = getTemplateById(normalizedUid);
          }
        }
        detailById.value = {
          ...detailById.value,
          [normalizedUid]: record,
        };
      } finally {
        setDetailLoading(normalizedUid, false);
      }
    })();
    detailPromises.set(normalizedUid, promise);
    try {
      await promise;
    } finally {
      detailPromises.delete(normalizedUid);
    }
  }

  async function refreshAll(): Promise<void> {
    await Promise.all([refreshLive(), refreshTemplates()]);
    if (trackedDetailIds.size === 0) {
      return;
    }
    await Promise.all([...trackedDetailIds].map((checklistUid) => refreshDetail(checklistUid)));
  }

  async function refreshAfterMutation(checklistUid?: string): Promise<void> {
    const normalizedUid = checklistUid?.trim();
    await Promise.all([
      refreshLive(),
      normalizedUid ? refreshDetail(normalizedUid) : Promise.resolve(),
    ]);
  }

  function isRuntimeChecklistRecord(
    record: RuntimeChecklistDetailRecord | null | undefined,
  ): record is RuntimeChecklistRecord {
    return Boolean(record && "syncState" in record);
  }

  function getChecklistById(checklistUid: string): RuntimeChecklistRecord | null {
    const normalizedUid = checklistUid.trim();
    if (!normalizedUid) {
      return null;
    }
    if (normalizedUid in detailById.value) {
      const record = detailById.value[normalizedUid];
      return isRuntimeChecklistRecord(record) ? record : null;
    }
    return live.value.find((record) => record.uid === normalizedUid) ?? null;
  }

  function getChecklistDetailById(checklistUid: string): RuntimeChecklistDetailRecord | null {
    const normalizedUid = checklistUid.trim();
    if (!normalizedUid) {
      return null;
    }
    if (normalizedUid in detailById.value) {
      return detailById.value[normalizedUid] ?? null;
    }
    return live.value.find((record) => record.uid === normalizedUid)
      ?? templates.value.find((record) => record.uid === normalizedUid)
      ?? null;
  }

  function getTemplateById(templateUid: string): RuntimeChecklistTemplateRecord | null {
    const normalizedUid = templateUid.trim();
    if (!normalizedUid) {
      return null;
    }
    return templates.value.find((record) => record.uid === normalizedUid) ?? null;
  }

  async function ensureJoined(checklistUid: string): Promise<void> {
    const identityHex = nodeStore.status.identityHex.trim();
    if (!identityHex) {
      return;
    }
    let checklist = getChecklistById(checklistUid);
    if (!checklist) {
      await refreshDetail(checklistUid);
      checklist = getChecklistById(checklistUid);
    }
    if (!checklist) {
      return;
    }
    if (checklist.participantRnsIdentities.includes(identityHex)) {
      return;
    }
    await client().joinChecklist(checklistUid);
    await refreshAfterMutation(checklistUid);
  }

  function init(): void {
    if (initialized.value) {
      void refreshAll();
      return;
    }
    initialized.value = true;
    void refreshAll();
  }

  function handleProjectionInvalidation(event: ProjectionInvalidationEvent): void {
    if (event.scope === "Checklists") {
      void refreshLive();
      return;
    }
    if (event.scope === "ChecklistDetail" && typeof event.key === "string" && event.key.trim()) {
      void refreshDetail(event.key);
    }
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;
    const projectionClient = client();
    cleanups.push(projectionClient.on("projectionInvalidated", handleProjectionInvalidation));
    cleanups.push(projectionClient.on("statusChanged", () => {
      void refreshAll();
    }));
  }

  async function importTemplateCsv(file: File, name?: string, description?: string): Promise<RuntimeChecklistTemplateRecord> {
    const csvText = await file.text();
    const template = await client().importChecklistTemplateCsv({
      name: (name?.trim() || file.name.replace(/\.csv$/i, "")).trim(),
      description: description?.trim() || "Imported CSV checklist template",
      csvText,
      sourceFilename: file.name,
    });
    await refreshTemplates();
    return template;
  }

  async function createFromTemplate(input: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
  }): Promise<void> {
    await client().createChecklistFromTemplate({
      checklistUid: input.checklistUid,
      missionUid: normalizeMissionUid(input.missionUid?.trim() || input.name),
      templateUid: input.templateUid,
      name: input.name.trim(),
      description: input.description.trim(),
      startTime: input.startTime.trim() || new Date().toISOString(),
      createdByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
      createdByTeamMemberDisplayName: nodeStore.settings.displayName.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  async function updateChecklist(input: {
    checklistUid: string;
    patch: {
      missionUid?: string;
      templateUid?: string;
      name?: string;
      description?: string;
      startTime?: string;
    };
  }): Promise<void> {
    await client().updateChecklist(input);
    await refreshAfterMutation(input.checklistUid);
  }

  async function deleteChecklist(
    checklistUid: string,
    options: { deleteRemote?: boolean } = {},
  ): Promise<void> {
    await client().deleteChecklist(checklistUid, {
      deleteRemote: options.deleteRemote ?? false,
    });
    await refreshLive();
    detailById.value = {
      ...detailById.value,
      [checklistUid]: null,
    };
  }

  async function uploadChecklist(checklistUid: string): Promise<void> {
    await client().uploadChecklist(checklistUid);
    await refreshAfterMutation(checklistUid);
  }

  async function joinChecklist(checklistUid: string): Promise<void> {
    await client().joinChecklist(checklistUid);
    await refreshAfterMutation(checklistUid);
  }

  async function setTaskStatus(input: {
    checklistUid: string;
    taskUid: string;
    userStatus: "PENDING" | "COMPLETE";
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().setChecklistTaskStatus({
      checklistUid: input.checklistUid,
      taskUid: input.taskUid,
      userStatus: input.userStatus,
      changedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  async function addTaskRow(input: {
    checklistUid: string;
    taskUid?: string;
    number: number;
    dueRelativeMinutes?: number;
    legacyValue?: string;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().addChecklistTaskRow({
      ...input,
      changedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  async function deleteTaskRow(input: {
    checklistUid: string;
    taskUid: string;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().deleteChecklistTaskRow({
      ...input,
      changedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  async function setTaskRowStyle(input: {
    checklistUid: string;
    taskUid: string;
    rowBackgroundColor?: string;
    lineBreakEnabled?: boolean;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().setChecklistTaskRowStyle({
      ...input,
      changedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  async function setTaskCell(input: {
    checklistUid: string;
    taskUid: string;
    columnUid: string;
    value?: string;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().setChecklistTaskCell({
      ...input,
      updatedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
    });
    await refreshAfterMutation(input.checklistUid);
  }

  return {
    live,
    templates,
    liveUiRecords,
    templateUiRecords,
    liveTaskTotal,
    templateTaskTotal,
    dashboardSummary,
    detailById,
    initialized,
    replicationInitialized,
    loadingLive,
    loadingTemplates,
    loadingDetailIds,
    init,
    initReplication,
    refreshLive,
    refreshTemplates,
    refreshDetail,
    refreshAll,
    getChecklistById,
    getChecklistDetailById,
    getTemplateById,
    importTemplateCsv,
    createFromTemplate,
    updateChecklist,
    deleteChecklist,
    joinChecklist,
    uploadChecklist,
    setTaskStatus,
    addTaskRow,
    deleteTaskRow,
    setTaskRowStyle,
    setTaskCell,
  };
});
