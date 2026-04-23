import {
  createReticulumNodeClient,
  type ChecklistRecord as RuntimeChecklistRecord,
  type ChecklistTemplateRecord as RuntimeChecklistTemplateRecord,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { ref } from "vue";

import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

type ProjectionClientCache = typeof globalThis & {
  __reticulumChecklistsProjectionClient?: ReticulumNodeClient;
};

type RuntimeChecklistDetailRecord = RuntimeChecklistRecord | RuntimeChecklistTemplateRecord;

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
  const loadingLive = ref(false);
  const loadingTemplates = ref(false);
  const loadingDetailIds = ref<Record<string, boolean>>({});
  const trackedDetailIds = new Set<string>();
  const cleanups: Array<() => void> = [];

  let refreshLivePromise: Promise<void> | null = null;
  let refreshTemplatesPromise: Promise<void> | null = null;
  const detailPromises = new Map<string, Promise<void>>();

  function client(): ReticulumNodeClient {
    return getProjectionClient(nodeStore.settings.clientMode);
  }

  function setDetailLoading(checklistUid: string, value: boolean): void {
    loadingDetailIds.value = {
      ...loadingDetailIds.value,
      [checklistUid]: value,
    };
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
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(checklistUid);
      await refreshLive();
    }
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
    if (!supportsNativeNodeRuntime) {
      return;
    }
    const projectionClient = client();
    cleanups.push(projectionClient.on("projectionInvalidated", handleProjectionInvalidation));
    cleanups.push(projectionClient.on("statusChanged", () => {
      void refreshAll();
    }));
  }

  async function importTemplateCsv(file: File, name?: string, description?: string): Promise<void> {
    const csvText = await file.text();
    await client().importChecklistTemplateCsv({
      name: (name?.trim() || file.name.replace(/\.csv$/i, "")).trim(),
      description: description?.trim() || "Imported CSV checklist template",
      csvText,
      sourceFilename: file.name,
    });
    await refreshTemplates();
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
    });
    await refreshLive();
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
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
    }
  }

  async function deleteChecklist(checklistUid: string): Promise<void> {
    await client().deleteChecklist(checklistUid);
    if (!supportsNativeNodeRuntime) {
      await refreshLive();
      detailById.value = {
        ...detailById.value,
        [checklistUid]: null,
      };
    }
  }

  async function uploadChecklist(checklistUid: string): Promise<void> {
    await client().uploadChecklist(checklistUid);
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(checklistUid);
      await refreshLive();
    }
  }

  async function joinChecklist(checklistUid: string): Promise<void> {
    await client().joinChecklist(checklistUid);
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(checklistUid);
      await refreshLive();
    }
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
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
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
    await client().addChecklistTaskRow(input);
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
    }
  }

  async function deleteTaskRow(input: {
    checklistUid: string;
    taskUid: string;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().deleteChecklistTaskRow(input);
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
    }
  }

  async function setTaskRowStyle(input: {
    checklistUid: string;
    taskUid: string;
    rowBackgroundColor?: string;
    lineBreakEnabled?: boolean;
  }): Promise<void> {
    await ensureJoined(input.checklistUid);
    await client().setChecklistTaskRowStyle(input);
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
    }
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
    if (!supportsNativeNodeRuntime) {
      await refreshDetail(input.checklistUid);
      await refreshLive();
    }
  }

  return {
    live,
    templates,
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
