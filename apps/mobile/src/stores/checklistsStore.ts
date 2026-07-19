import {
  type ChecklistRecord as RuntimeChecklistRecord,
  type ChecklistTemplateRecord as RuntimeChecklistTemplateRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";

import {
  runtimeChecklistToUi,
  runtimeTemplateToUi,
  type ChecklistRecord as UiChecklistRecord,
} from "../utils/checklists";
import { createChecklistNotificationCoordinator } from "../utils/checklistNotifications";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { createProjectionClientAccessor } from "../utils/projectionClient";
import { useNodeStore } from "./nodeStore";

const getProjectionClient = createProjectionClientAccessor("checklists");

type RuntimeChecklistDetailRecord = RuntimeChecklistRecord | RuntimeChecklistTemplateRecord;
const TASK_SUBMISSION_KEY_SEPARATOR = "::";

function normalizeMissionUid(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || `mission-${Date.now().toString(36)}`;
}

function taskSubmissionKey(checklistUid: string, taskUid: string): string {
  return `${checklistUid.trim()}${TASK_SUBMISSION_KEY_SEPARATOR}${taskUid.trim()}`;
}

export const useChecklistsStore = defineStore("checklists", () => {
  const nodeStore = useNodeStore();
  const live = ref<RuntimeChecklistRecord[]>([]);
  const templates = ref<RuntimeChecklistTemplateRecord[]>([]);
  const detailById = ref<Record<string, RuntimeChecklistDetailRecord | null>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const loadingLive = ref(false);
  const loadingTemplates = ref(false);
  const loadingDetailIds = ref<Record<string, boolean>>({});
  const submittedTaskKeys = ref<Record<string, true>>({});
  const trackedDetailIds = new Set<string>();
  const cleanups: Array<() => void> = [];

  const checklistNotifications = createChecklistNotificationCoordinator(
    () => nodeStore.status.identityHex,
  );

  function client(): ReticulumNodeClient {
    return getProjectionClient(nodeStore.settings.clientMode);
  }

  function markTaskSubmitted(checklistUid: string, taskUid: string): void {
    const key = taskSubmissionKey(checklistUid, taskUid);
    if (!key.includes(TASK_SUBMISSION_KEY_SEPARATOR) || key.endsWith(TASK_SUBMISSION_KEY_SEPARATOR)) {
      return;
    }
    submittedTaskKeys.value = {
      ...submittedTaskKeys.value,
      [key]: true,
    };
  }

  function clearTaskSubmitted(checklistUid: string, taskUid: string): void {
    const key = taskSubmissionKey(checklistUid, taskUid);
    if (!(key in submittedTaskKeys.value)) {
      return;
    }
    const next = { ...submittedTaskKeys.value };
    delete next[key];
    submittedTaskKeys.value = next;
  }

  function submittedTaskIdsForChecklist(checklistUid: string): ReadonlySet<string> {
    const normalizedUid = checklistUid.trim();
    if (!normalizedUid) {
      return new Set<string>();
    }
    const prefix = `${normalizedUid}${TASK_SUBMISSION_KEY_SEPARATOR}`;
    return new Set(
      Object.keys(submittedTaskKeys.value)
        .filter((key) => key.startsWith(prefix))
        .map((key) => key.slice(prefix.length))
        .filter((taskUid) => taskUid.length > 0),
    );
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

  async function refreshLive(): Promise<void> {
    await projectionRefreshCoordinator.run("checklists:live", async () => {
      loadingLive.value = true;
      try {
        const records = await client().listActiveChecklists();
        live.value = records;
        detailById.value = {
          ...detailById.value,
          ...Object.fromEntries(records.map((record) => [record.uid, record])),
        };
        await checklistNotifications.notifyForChanges(records);
      } finally {
        loadingLive.value = false;
      }
    }, { trailing: true });
  }

  async function refreshTemplates(): Promise<void> {
    await projectionRefreshCoordinator.run("checklists:templates", async () => {
      loadingTemplates.value = true;
      try {
        templates.value = await client().listChecklistTemplates();
      } finally {
        loadingTemplates.value = false;
      }
    }, { trailing: true });
  }

  async function refreshDetail(checklistUid: string): Promise<void> {
    const normalizedUid = checklistUid.trim();
    if (!normalizedUid) {
      return;
    }
    trackedDetailIds.add(normalizedUid);
    await projectionRefreshCoordinator.run(`checklists:detail:${normalizedUid}`, async () => {
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
    }, { trailing: true });
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
    cleanups.push(watch(
      () => nodeStore.status.running,
      (running) => {
        if (running) void refreshAll();
      },
      { immediate: true },
    ));
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
    const checklistUid = input.checklistUid?.trim() || `chk-${Date.now()}`;
    const description = input.description.trim();
    const startTime = input.startTime.trim() || new Date().toISOString();
    const projectionClient = client();
    await projectionClient.createChecklistFromTemplate({
      checklistUid,
      missionUid: normalizeMissionUid(input.missionUid?.trim() || input.name),
      templateUid: input.templateUid,
      name: input.name.trim(),
      description,
      startTime,
      createdByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
      createdByTeamMemberDisplayName: nodeStore.settings.displayName.trim() || undefined,
    });
    // The compact create envelope deliberately omits descriptive metadata so it
    // fits an RNode packet. Replicate those fields with the existing update
    // command instead of silently replacing them with receiver-side defaults.
    await projectionClient.updateChecklist({
      checklistUid,
      patch: {
        description,
        startTime,
      },
    });
    await refreshAfterMutation(checklistUid);
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
    const shouldShowSubmitted = input.userStatus === "COMPLETE";
    if (shouldShowSubmitted) {
      markTaskSubmitted(input.checklistUid, input.taskUid);
    }
    try {
      await ensureJoined(input.checklistUid);
      await client().setChecklistTaskStatus({
        checklistUid: input.checklistUid,
        taskUid: input.taskUid,
        userStatus: input.userStatus,
        changedByTeamMemberRnsIdentity: nodeStore.status.identityHex.trim() || undefined,
      });
      await refreshAfterMutation(input.checklistUid);
    } finally {
      if (shouldShowSubmitted) {
        clearTaskSubmitted(input.checklistUid, input.taskUid);
      }
    }
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
    submittedTaskIdsForChecklist,
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
