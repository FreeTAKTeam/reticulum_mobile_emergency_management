import type {
  ChecklistRecord as RuntimeChecklistRecord,
  ChecklistTemplateRecord as RuntimeChecklistTemplateRecord,
} from "@reticulum/node-client";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";

import { useChecklistsStore } from "../stores/checklistsStore";
import { useNodeStore } from "../stores/nodeStore";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";
import {
  runtimeChecklistDetailToUi,
  runtimeChecklistToUi,
  runtimeTemplateDetailToUi,
  runtimeTemplateToUi,
  type ChecklistTask,
  type ChecklistTaskCell,
  type ChecklistTaskMetaTone,
  type ChecklistTaskStatus,
} from "../utils/checklists";

export function useChecklistDetail() {
  const route = useRoute();
  const checklistsStore = useChecklistsStore();
  const nodeStore = useNodeStore();

  const checklistId = computed(() => String(route.params.checklistId ?? ""));
  const checklistRuntimeRecord = computed(() => checklistsStore.getChecklistDetailById(checklistId.value));

  function isLiveChecklistRecord(
    record: RuntimeChecklistRecord | RuntimeChecklistTemplateRecord | null,
  ): record is RuntimeChecklistRecord {
    return Boolean(record && "syncState" in record);
  }

  const liveChecklistRuntimeRecord = computed(() =>
    isLiveChecklistRecord(checklistRuntimeRecord.value) ? checklistRuntimeRecord.value : null,
  );
  const isTemplatePreview = computed(() => Boolean(checklistRuntimeRecord.value && !liveChecklistRuntimeRecord.value));
  const checklistRecord = computed(() => {
    const record = checklistRuntimeRecord.value;
    if (!record) {
      return undefined;
    }
    return isLiveChecklistRecord(record) ? runtimeChecklistToUi(record) : runtimeTemplateToUi(record);
  });
  const submittedTaskIds = computed(() => checklistsStore.submittedTaskIdsForChecklist(checklistId.value));
  const checklistDetail = computed(() => {
    const record = checklistRuntimeRecord.value;
    if (!record) {
      return undefined;
    }
    return isLiveChecklistRecord(record)
      ? runtimeChecklistDetailToUi(record, { submittedTaskIds: submittedTaskIds.value })
      : runtimeTemplateDetailToUi(record);
  });
  const visibleTasks = computed<ChecklistTask[]>(() => checklistDetail.value?.tasks ?? []);
  const isCurrentParticipant = computed(() => {
    const record = liveChecklistRuntimeRecord.value;
    const identity = nodeStore.status.identityHex.trim().toLowerCase();
    if (!record || !identity) {
      return true;
    }
    return record.participantRnsIdentities.some((participant) => participant.toLowerCase() === identity);
  });
  const shouldShowJoin = computed(() => Boolean(liveChecklistRuntimeRecord.value && !isCurrentParticipant.value));
  const isMutating = ref(false);
  const editingTaskId = ref<string | null>(null);
  const editingTaskValue = ref("");
  const cellDrafts = ref<Record<string, string>>({});
  const routeEditConsumed = ref(false);

  function refreshDetailDetached(value: string): void {
    runDetachedStoreTask(nodeStore, "checklists", "detail refresh", () =>
      checklistsStore.refreshDetail(value));
  }

  function taskStatusClass(status: ChecklistTaskStatus): string {
    return `task-${status}`;
  }

  function taskStatusLabel(status: ChecklistTaskStatus): string {
    if (status === "submitted") {
      return "Submitted";
    }
    if (status === "late") {
      return "Late";
    }
    if (status === "completed") {
      return "Completed";
    }
    return "Pending";
  }

  function taskMetaClass(tone: ChecklistTaskMetaTone): string {
    return `task-meta-${tone}`;
  }

  async function completeTask(taskId: string): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    isMutating.value = true;
    try {
      await checklistsStore.setTaskStatus({
        checklistUid: checklistId.value,
        taskUid: taskId,
        userStatus: "COMPLETE",
      });
    } finally {
      isMutating.value = false;
    }
  }

  async function joinChecklist(): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    isMutating.value = true;
    try {
      await checklistsStore.joinChecklist(checklistId.value);
    } finally {
      isMutating.value = false;
    }
  }

  async function uploadChecklist(): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    isMutating.value = true;
    try {
      await checklistsStore.uploadChecklist(checklistId.value);
    } finally {
      isMutating.value = false;
    }
  }

  function startTaskEdit(task: ChecklistTask): void {
    if (isTemplatePreview.value || isMutating.value) {
      return;
    }
    editingTaskId.value = task.id;
    editingTaskValue.value = task.title;
    cellDrafts.value = Object.fromEntries(
      task.cells
        .filter((cell) => cell.editable)
        .map((cell) => [taskCellDraftKey(task, cell), cell.value]),
    );
  }

  function cancelTaskEdit(): void {
    editingTaskId.value = null;
    editingTaskValue.value = "";
    cellDrafts.value = {};
  }

  function taskCellDraftKey(task: ChecklistTask, cell: ChecklistTaskCell): string {
    return `${task.id}:${cell.columnUid}`;
  }

  function taskCellDraftValue(task: ChecklistTask, cell: ChecklistTaskCell): string {
    return cellDrafts.value[taskCellDraftKey(task, cell)] ?? cell.value;
  }

  function updateTaskCellDraft(task: ChecklistTask, cell: ChecklistTaskCell, event: Event): void {
    const target = event.target as HTMLInputElement;
    cellDrafts.value = {
      ...cellDrafts.value,
      [taskCellDraftKey(task, cell)]: target.value,
    };
  }

  function taskCellHasDraft(task: ChecklistTask, cell: ChecklistTaskCell): boolean {
    return taskCellDraftValue(task, cell) !== cell.value;
  }

  function isTaskEditing(task: ChecklistTask): boolean {
    return editingTaskId.value === task.id;
  }

  function editableTaskCells(task: ChecklistTask): ChecklistTaskCell[] {
    return task.cells.filter((cell) => cell.columnUid !== task.primaryColumnUid);
  }

  function taskHasDraft(task: ChecklistTask): boolean {
    if (editingTaskId.value !== task.id) {
      return false;
    }
    if (editingTaskValue.value.trim() !== task.title) {
      return true;
    }
    return task.cells.some((cell) => cell.editable && taskCellHasDraft(task, cell));
  }

  async function saveTaskRow(task: ChecklistTask): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    const titleValue = editingTaskValue.value.trim();
    if (!titleValue) {
      return;
    }
    isMutating.value = true;
    try {
      if (titleValue !== task.title) {
        await checklistsStore.setTaskCell({
          checklistUid: checklistId.value,
          taskUid: task.id,
          columnUid: task.primaryColumnUid,
          value: titleValue,
        });
      }
      for (const cell of task.cells) {
        if (!cell.editable || cell.columnUid === task.primaryColumnUid || !taskCellHasDraft(task, cell)) {
          continue;
        }
        await checklistsStore.setTaskCell({
          checklistUid: checklistId.value,
          taskUid: task.id,
          columnUid: cell.columnUid,
          value: taskCellDraftValue(task, cell),
        });
      }
      cancelTaskEdit();
    } finally {
      isMutating.value = false;
    }
  }

  async function toggleTaskEdit(task: ChecklistTask): Promise<void> {
    if (isTaskEditing(task)) {
      await saveTaskRow(task);
      return;
    }
    startTaskEdit(task);
  }

  async function deleteTaskRow(task: ChecklistTask): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    if (!window.confirm(`Delete checklist row "${task.title}"?`)) {
      return;
    }
    isMutating.value = true;
    try {
      await checklistsStore.deleteTaskRow({
        checklistUid: checklistId.value,
        taskUid: task.id,
      });
      if (editingTaskId.value === task.id) {
        cancelTaskEdit();
      }
    } finally {
      isMutating.value = false;
    }
  }

  async function addTaskRow(): Promise<void> {
    if (!checklistId.value || !liveChecklistRuntimeRecord.value || isMutating.value) {
      return;
    }
    const nextNumber = liveChecklistRuntimeRecord.value.tasks.length + 1;
    isMutating.value = true;
    try {
      await checklistsStore.addTaskRow({
        checklistUid: checklistId.value,
        number: nextNumber,
        legacyValue: `Task ${nextNumber}`,
      });
    } finally {
      isMutating.value = false;
    }
  }

  watch(checklistId, (value) => {
    if (!value) {
      return;
    }
    refreshDetailDetached(value);
  }, { immediate: true });

  watch([visibleTasks, () => route.query.edit], ([tasks, edit]) => {
    if (routeEditConsumed.value || edit !== "1" || isTemplatePreview.value || tasks.length === 0) {
      return;
    }
    routeEditConsumed.value = true;
    startTaskEdit(tasks[0]);
  }, { immediate: true });

  onMounted(() => {
    if (checklistId.value) {
      refreshDetailDetached(checklistId.value);
    }
  });

  return {
    checklistDetail,
    checklistRecord,
    isTemplatePreview,
    shouldShowJoin,
    isMutating,
    joinChecklist,
    uploadChecklist,
    addTaskRow,
    visibleTasks,
    taskStatusClass,
    completeTask,
    isTaskEditing,
    editingTaskValue,
    saveTaskRow,
    cancelTaskEdit,
    taskStatusLabel,
    taskMetaClass,
    editableTaskCells,
    taskCellDraftValue,
    updateTaskCellDraft,
    taskHasDraft,
    toggleTaskEdit,
    deleteTaskRow,
  };
}
