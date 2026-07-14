import { storeToRefs } from "pinia";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRouter } from "vue-router";

import { useChecklistsStore } from "../stores/checklistsStore";
import {
  type ChecklistFilter,
  type ChecklistSegment,
  type ChecklistStatus,
} from "../utils/checklists";

export function useChecklistList() {
  const checklistsStore = useChecklistsStore();
  const {
    liveUiRecords,
    templateUiRecords,
    liveTaskTotal,
    templateTaskTotal,
  } = storeToRefs(checklistsStore);
  const router = useRouter();
  const activeSegment = ref<ChecklistSegment>("live");
  const activeFilter = ref<ChecklistFilter>("all");
  const expandedChecklistIds = ref<string[]>([]);
  const isCreateFormVisible = ref(false);
  const selectedTemplateId = ref("");
  const importFileInput = ref<HTMLInputElement | null>(null);
  const isMutating = ref(false);
  const deletingChecklistIds = ref<string[]>([]);
  const isChecklistHelpVisible = ref(false);
  const DEFAULT_TARGET_DAYS = 30;

  type PendingChecklistDelete = {
    id: string;
    title: string;
  };

  const pendingDeleteChecklist = ref<PendingChecklistDelete | null>(null);

  function toDatetimeLocalValue(date: Date): string {
    const offsetMs = date.getTimezoneOffset() * 60_000;
    return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
  }

  function defaultChecklistTargetDtg(): string {
    const target = new Date();
    target.setDate(target.getDate() + DEFAULT_TARGET_DAYS);
    return toDatetimeLocalValue(target);
  }

  function createDefaultChecklistFormState(): {
    title: string;
    subtitle: string;
    teamLabel: string;
    scheduledAt: string;
  } {
    return {
      title: "",
      subtitle: "",
      teamLabel: "",
      scheduledAt: defaultChecklistTargetDtg(),
    };
  }

  const createForm = reactive(createDefaultChecklistFormState());
  const checklistRecords = computed(() =>
    activeSegment.value === "templates"
      ? templateUiRecords.value
      : liveUiRecords.value,
  );
  const templateRecords = computed(() => templateUiRecords.value);
  const displayedTaskTotal = computed(() =>
    activeSegment.value === "templates"
      ? templateTaskTotal.value
      : liveTaskTotal.value,
  );
  const hasChecklistRecords = computed(() => checklistRecords.value.length > 0);
  const emptyStateTitle = computed(() =>
    activeSegment.value === "templates" ? "No checklist templates available." : "No checklists available.",
  );
  const emptyStateCopy = computed(() =>
    activeSegment.value === "templates"
      ? "The runtime has not loaded any checklist templates yet."
      : "The runtime has not loaded any checklist data yet.",
  );

  const filteredRecords = computed(() => {
    if (activeFilter.value === "all") {
      return checklistRecords.value;
    }
    return checklistRecords.value.filter((record) => record.status === activeFilter.value);
  });

  const filterItems: Array<{ value: ChecklistFilter; label: string }> = [
    { value: "all", label: "All" },
    { value: "active", label: "Active" },
    { value: "late", label: "Late" },
    { value: "completed", label: "Completed" },
  ];

  function statusCardClass(status: ChecklistStatus): string {
    return `status-${status}`;
  }

  function toggleTemplates(): void {
    activeSegment.value = activeSegment.value === "templates" ? "live" : "templates";
  }

  function resetCreateForm(): void {
    Object.assign(createForm, createDefaultChecklistFormState());
  }

  function checklistStartTimeIso(): string {
    const scheduledAt = createForm.scheduledAt.trim();
    if (!scheduledAt) {
      return new Date().toISOString();
    }
    const parsed = new Date(scheduledAt);
    return Number.isNaN(parsed.getTime()) ? new Date().toISOString() : parsed.toISOString();
  }

  function toggleCreateForm(): void {
    if (isCreateFormVisible.value) {
      resetCreateForm();
    }
    isCreateFormVisible.value = !isCreateFormVisible.value;
  }

  function closeChecklistHelp(): void {
    isChecklistHelpVisible.value = false;
  }

  async function ensureChecklistData(segment?: ChecklistSegment): Promise<void> {
    if (!segment || segment === "live") {
      await checklistsStore.refreshLive();
    }
    if (!segment || segment === "templates") {
      await checklistsStore.refreshTemplates();
    }
    if (!selectedTemplateId.value && templateRecords.value.length > 0) {
      selectedTemplateId.value = templateRecords.value[0]?.id ?? "";
    }
  }

  async function createChecklist(): Promise<void> {
    const title = createForm.title.trim();
    if (!title || !selectedTemplateId.value || isMutating.value) {
      return;
    }
    isMutating.value = true;
    try {
      await checklistsStore.createFromTemplate({
        templateUid: selectedTemplateId.value,
        missionUid: createForm.teamLabel.trim() || undefined,
        name: title,
        description: createForm.subtitle.trim() || "Emergency preparedness checklist",
        startTime: checklistStartTimeIso(),
      });
      activeSegment.value = "live";
      resetCreateForm();
      isCreateFormVisible.value = false;
    } finally {
      isMutating.value = false;
    }
  }

  function isMetadataExpanded(checklistId: string): boolean {
    return expandedChecklistIds.value.includes(checklistId);
  }

  function toggleMetadata(checklistId: string): void {
    if (isMetadataExpanded(checklistId)) {
      expandedChecklistIds.value = expandedChecklistIds.value.filter((id) => id !== checklistId);
      return;
    }
    expandedChecklistIds.value = [...expandedChecklistIds.value, checklistId];
  }

  function openChecklist(checklistId: string, edit = false): void {
    void router.push({
      name: "checklist-detail",
      params: { checklistId },
      query: edit ? { edit: "1" } : undefined,
    });
  }

  function isDeletingChecklist(checklistId: string): boolean {
    return deletingChecklistIds.value.includes(checklistId);
  }

  function requestDeleteChecklist(checklistId: string, title: string): void {
    if (activeSegment.value !== "live" || isDeletingChecklist(checklistId)) {
      return;
    }
    pendingDeleteChecklist.value = {
      id: checklistId,
      title,
    };
  }

  function closeDeleteChecklistPrompt(): void {
    pendingDeleteChecklist.value = null;
  }

  async function confirmDeleteChecklist(deleteRemote: boolean): Promise<void> {
    const pending = pendingDeleteChecklist.value;
    if (!pending || activeSegment.value !== "live" || isDeletingChecklist(pending.id)) {
      return;
    }
    pendingDeleteChecklist.value = null;
    deletingChecklistIds.value = [...deletingChecklistIds.value, pending.id];
    try {
      await checklistsStore.deleteChecklist(pending.id, { deleteRemote });
      expandedChecklistIds.value = expandedChecklistIds.value.filter((id) => id !== pending.id);
    } finally {
      deletingChecklistIds.value = deletingChecklistIds.value.filter((id) => id !== pending.id);
    }
  }

  function triggerTemplateUpload(): void {
    importFileInput.value?.click();
  }

  async function handleTemplateUpload(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || isMutating.value) {
      return;
    }
    isMutating.value = true;
    try {
      const importedTemplate = await checklistsStore.importTemplateCsv(file);
      activeSegment.value = "templates";
      await ensureChecklistData("templates");
      selectedTemplateId.value = importedTemplate.uid;
    } finally {
      input.value = "";
      isMutating.value = false;
    }
  }

  watch(activeSegment, (segment) => {
    void ensureChecklistData(segment);
  });

  onMounted(() => {
    void ensureChecklistData();
  });

  return {
    displayedTaskTotal,
    activeFilter,
    filterItems,
    isCreateFormVisible,
    toggleCreateForm,
    createChecklist,
    createForm,
    selectedTemplateId,
    templateRecords,
    isMutating,
    toggleTemplates,
    activeSegment,
    triggerTemplateUpload,
    importFileInput,
    handleTemplateUpload,
    isChecklistHelpVisible,
    closeChecklistHelp,
    pendingDeleteChecklist,
    closeDeleteChecklistPrompt,
    confirmDeleteChecklist,
    filteredRecords,
    statusCardClass,
    openChecklist,
    isDeletingChecklist,
    requestDeleteChecklist,
    isMetadataExpanded,
    toggleMetadata,
    hasChecklistRecords,
    emptyStateTitle,
    emptyStateCopy,
  };
}
