<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRouter } from "vue-router";

import { useChecklistsStore } from "../stores/checklistsStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  type ChecklistFilter,
  type ChecklistSegment,
  type ChecklistStatus,
} from "../utils/checklists";

const nodeStore = useNodeStore();
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

const isSyncing = ref(false);

async function requestSync(): Promise<void> {
  if (isSyncing.value) {
    return;
  }
  isSyncing.value = true;
  try {
    await nodeStore.requestLxmfSync();
  } catch {
    // nodeStore surfaces sync errors through existing app state.
  } finally {
    isSyncing.value = false;
  }
}

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

function openChecklistHelp(): void {
  isChecklistHelpVisible.value = true;
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
    name: "checlklist-detail",
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
</script>

<template>
  <section class="view checklist-view">
    <h1 class="sr-only">Checklists</h1>

    <section class="segment-strip">
      <div class="segment-actions">
        <span class="utility-chip count-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 4 4 8l8 4 8-4-8-4Z" />
            <path d="M4 12l8 4 8-4" />
            <path d="M4 16l8 4 8-4" />
          </svg>
          <span>{{ displayedTaskTotal }} Tasks</span>
        </span>
        <label class="utility-chip filter-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />
          </svg>
          <span>Filter:</span>
          <select
            v-model="activeFilter"
            class="header-filter-select"
            aria-label="Checklist status filter"
          >
            <option
              v-for="item in filterItems"
              :key="item.value"
              :value="item.value"
            >
              {{ item.label }}
            </option>
          </select>
          <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m7 10 5 5 5-5" />
          </svg>
        </label>
        <button
          type="button"
          class="create-toggle utility-new"
          aria-label="Create checklist"
          title="Create checklist"
          :aria-expanded="isCreateFormVisible"
          @click="toggleCreateForm"
        >
          <span aria-hidden="true">+</span>
        </button>
      </div>
    </section>

    <form v-show="isCreateFormVisible" class="create-form" @submit.prevent="createChecklist">
      <div class="create-form-top">
        <input
          v-model="createForm.title"
          type="text"
          placeholder="Checklist title"
          aria-label="Checklist title"
        />
        <input
          v-model="createForm.subtitle"
          type="text"
          placeholder="Checklist subtitle"
          aria-label="Checklist subtitle"
        />
      </div>
      <div class="create-form-bottom">
        <input
          v-model="createForm.teamLabel"
          type="text"
          placeholder="Assignment label (optional)"
          aria-label="Assignment label"
        />
        <input
          v-model="createForm.scheduledAt"
          type="datetime-local"
          aria-label="Checklist DTG"
        />
        <select v-model="selectedTemplateId" aria-label="Checklist template">
          <option value="" disabled>
            Select template
          </option>
          <option v-for="template in templateRecords" :key="template.id" :value="template.id">
            {{ template.title }}
          </option>
        </select>
        <div class="create-form-actions">
          <button
            type="button"
            class="template-chip"
            :class="{ selected: activeSegment === 'templates' }"
            @click="toggleTemplates"
          >
            Templates
          </button>
          <button type="button" class="upload-chip" :disabled="isMutating" @click="triggerTemplateUpload">
            Upload
          </button>
          <button type="submit" class="create-submit" :disabled="isMutating || !selectedTemplateId">
            Create checklist
          </button>
        </div>
      </div>
    </form>
    <input
      ref="importFileInput"
      type="file"
      accept=".csv,text/csv"
      class="sr-only"
      @change="handleTemplateUpload"
    />

    <div
      v-if="isChecklistHelpVisible"
      class="help-screen"
      role="dialog"
      aria-modal="true"
      aria-labelledby="checklist-help-title"
      @click.self="closeChecklistHelp"
    >
      <section class="help-panel">
        <div class="help-header">
          <h2 id="checklist-help-title">How checklists work</h2>
          <button
            type="button"
            class="help-close"
            aria-label="Close checklist help"
            @click="closeChecklistHelp"
          >
            x
          </button>
        </div>
        <p>
          Checklists are shared operational task lists. Creating one from a template publishes the checklist
          to nearby REM nodes and then synchronizes the full task list over LXMF.
        </p>
        <p>
          While a checklist is still receiving tasks, the card shows sync progress. Once every task is present,
          the normal completion bar is shown.
        </p>
        <p>
          Open a checklist to join, complete rows, edit task cells, add rows, or delete rows. Updates are saved
          through Rust first and then replicated to peers.
        </p>
      </section>
    </div>

    <div
      v-if="pendingDeleteChecklist"
      class="delete-confirm-screen"
      role="dialog"
      aria-modal="true"
      aria-labelledby="checklist-delete-title"
      @click.self="closeDeleteChecklistPrompt"
    >
      <section class="delete-confirm-panel">
        <div class="delete-confirm-header">
          <h2 id="checklist-delete-title">Delete checklist?</h2>
          <button
            type="button"
            class="help-close"
            aria-label="Cancel checklist deletion"
            @click="closeDeleteChecklistPrompt"
          >
            x
          </button>
        </div>
        <p>
          Delete "{{ pendingDeleteChecklist.title }}" from this device only, or also send an LXMF delete signal
          to connected saved devices?
        </p>
        <div class="delete-confirm-actions">
          <button type="button" class="delete-cancel" @click="closeDeleteChecklistPrompt">
            Cancel
          </button>
          <button type="button" class="delete-local" @click="confirmDeleteChecklist(false)">
            Delete locally
          </button>
          <button type="button" class="delete-remote" @click="confirmDeleteChecklist(true)">
            Delete locally + remote
          </button>
        </div>
      </section>
    </div>

    <section class="checklist-list">
      <article
        v-for="record in filteredRecords"
        :key="record.id"
        class="checklist-card"
        :class="statusCardClass(record.status)"
      >
        <div class="card-primary">
          <div class="card-topline">
            <button
              type="button"
              class="card-open card-heading-action"
              :aria-label="`Open ${record.title}`"
              @click="openChecklist(record.id)"
            >
              <div class="card-heading">
                <h2>{{ record.title }}</h2>
                <p>{{ record.subtitle }}</p>
              </div>
            </button>

            <div class="card-top-actions">
              <button
                class="action edit"
                type="button"
                :aria-label="`Edit ${record.title}`"
                title="Edit"
                @click="openChecklist(record.id, true)"
              >
                <svg class="action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 20h9" />
                  <path d="m16.5 3.5 4 4L8 20l-4 1 1-4z" />
                </svg>
              </button>
              <button
                v-if="activeSegment === 'live'"
                class="action delete"
                type="button"
                :aria-label="`Delete ${record.title}`"
                title="Delete"
                :disabled="isDeletingChecklist(record.id)"
                @click="requestDeleteChecklist(record.id, record.title)"
              >
                <svg class="action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M3 6h18" />
                  <path d="M8 6V4h8v2" />
                  <path d="M19 6l-1 14H6L5 6" />
                  <path d="M10 11v5" />
                  <path d="M14 11v5" />
                </svg>
              </button>
            </div>
          </div>

          <button
            type="button"
            class="card-open card-progress-action"
            :aria-label="`Open ${record.title}`"
            @click="openChecklist(record.id)"
          >
            <div v-if="record.taskSync" class="progress-copy task-sync-copy">
              <span>
                <span class="task-sync-pulse" aria-hidden="true"></span>
                {{ record.taskSync.label }}
              </span>
              <span>{{ record.taskSync.received }} / {{ record.taskSync.total }} tasks</span>
            </div>

            <div v-else class="progress-copy">
              <span>{{ record.progress }}% complete</span>
              <span>{{ record.statusCountLabel }}</span>
            </div>

            <div class="progress-track" aria-hidden="true">
              <div
                class="progress-fill"
                :class="{ 'task-sync-fill': record.taskSync }"
                :style="{ width: `${record.taskSync ? record.taskSync.progress : record.progress}%` }"
              ></div>
            </div>

            <p v-if="record.taskSync" class="task-sync-detail">
              {{ record.taskSync.detail }}
            </p>
          </button>
        </div>

        <div class="card-footer">
          <button
            type="button"
            class="metadata-toggle"
            :aria-expanded="isMetadataExpanded(record.id)"
            :aria-controls="`checklist-meta-${record.id}`"
            @click="toggleMetadata(record.id)"
          >
            <span>{{ isMetadataExpanded(record.id) ? "Hide details" : "Show details" }}</span>
            <svg
              class="toggle-icon"
              :class="{ open: isMetadataExpanded(record.id) }"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden="true"
            >
              <path d="M7 10.5 12 15.5 17 10.5" />
            </svg>
          </button>
        </div>

        <section
          class="card-details"
          v-show="isMetadataExpanded(record.id)"
          :id="`checklist-meta-${record.id}`"
        >
          <div class="card-metadata" aria-label="Checklist metadata">
            <span
              v-for="(line, index) in record.metadataLines"
              :key="`${record.id}-${index}-${line}`"
              class="metadata-item"
            >
              <svg v-if="index === 0" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M7 4v3" />
                <path d="M17 4v3" />
                <path d="M5 8h14" />
                <path d="M6 6.5h12a1 1 0 0 1 1 1v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-10a1 1 0 0 1 1-1Z" />
              </svg>
              <svg v-else-if="index === 1" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M12 8v4l2.5 1.5" />
                <path d="M20 12a8 8 0 1 1-2.35-5.65" />
                <path d="M20 5v4h-4" />
              </svg>
              <svg v-else viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M12 12a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" />
                <path d="M5.5 19a6.5 6.5 0 0 1 13 0" />
              </svg>
              {{ line }}
            </span>
          </div>
        </section>
      </article>

      <article v-if="filteredRecords.length === 0" class="empty-state">
        <h2>{{ hasChecklistRecords ? "No checklist matches this filter." : emptyStateTitle }}</h2>
        <p>
          {{
            hasChecklistRecords
              ? "Switch filters or open the template library to prepare a new checklist package."
              : emptyStateCopy
          }}
        </p>
      </article>
    </section>
  </section>
</template>

<style scoped>
.checklist-view {
  display: grid;
  gap: 1rem;
}

.sr-only {
  border: 0;
  clip: rect(0 0 0 0);
  height: 1px;
  margin: -1px;
  overflow: hidden;
  padding: 0;
  position: absolute;
  white-space: nowrap;
  width: 1px;
}

.summary-panel,
.checklist-card,
.empty-state {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
  box-shadow: inset 0 1px 0 rgb(186 236 255 / 5%);
}

.segment-strip {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  justify-content: flex-end;
}

.segment-title {
  color: #d5eaff;
  font-family: var(--font-headline);
  font-size: 1.3rem;
  margin: 0;
}

.segment-actions {
  align-items: center;
  display: grid;
  flex-shrink: 0;
  gap: 0.8rem;
  grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.35fr) minmax(3.2rem, 0.32fr);
}

.metadata-item svg,
.utility-toggle svg {
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

.sync-chip,
.template-chip {
  --btn-bg: rgb(9 61 108 / 68%);
  --btn-bg-pressed: linear-gradient(180deg, rgb(199 241 255 / 96%), rgb(132 219 255 / 94%));
  --btn-border: rgb(73 173 255 / 62%);
  --btn-border-pressed: rgb(234 251 255 / 88%);
  --btn-shadow: inset 0 1px 0 rgb(186 236 255 / 8%), 0 8px 18px rgb(3 24 56 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 18 40 / 20%);
  --btn-color: #64beff;
  --btn-color-pressed: #063050;
  align-items: center;
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  box-shadow:
    inset 0 1px 0 rgb(186 236 255 / 8%),
    0 8px 18px rgb(3 24 56 / 18%);
  color: #64beff;
  cursor: pointer;
  display: inline-flex;
  flex-shrink: 0;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  gap: 0.5rem;
  justify-content: center;
  letter-spacing: 0.08em;
  min-height: 0;
  min-width: 8rem;
  padding: 0.46rem 0.95rem;
  text-transform: uppercase;
}

.sync-chip:focus-visible,
.template-chip:focus-visible {
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 2px;
}

.sync-chip.busy {
  color: #d7efff;
}

.badge {
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  color: #64beff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  justify-content: center;
  letter-spacing: 0.08em;
  padding: 0.46rem 0.8rem;
  text-transform: uppercase;
}

.utility-chip {
  align-items: center;
  background: rgb(7 25 54 / 84%);
  border: 1px solid rgb(73 173 255 / 58%);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 20px rgb(33 153 255 / 8%);
  color: #8fcaff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: clamp(0.82rem, 2.1vw, 1rem);
  font-weight: 700;
  gap: 0.58rem;
  justify-content: center;
  min-height: 3rem;
  min-width: 0;
  padding: 0.48rem 0.74rem;
  text-decoration: none;
}

.utility-chip svg {
  flex: 0 0 auto;
  height: 1.22rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.22rem;
}

.utility-chip span {
  flex: 0 0 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.count-chip,
.filter-chip {
  justify-content: flex-start;
}

.filter-chip {
  cursor: pointer;
}

.filter-chip .chevron {
  margin-left: auto;
}

.header-filter-select {
  appearance: none;
  background: transparent;
  border: 0;
  color: #c7dcff;
  cursor: pointer;
  flex: 1 1 auto;
  font: inherit;
  min-width: 0;
  outline: 0;
  padding: 0;
}

.header-filter-select option {
  background: #061833;
  color: #d1e9ff;
}

.utility-toggle {
  --btn-bg: rgb(7 20 44 / 84%);
  --btn-bg-pressed: linear-gradient(180deg, rgb(22 58 107 / 82%), rgb(8 24 52 / 96%));
  --btn-border: rgb(73 173 255 / 40%);
  --btn-border-pressed: rgb(226 248 255 / 84%);
  --btn-shadow: inset 0 1px 0 rgb(186 236 255 / 6%), 0 8px 18px rgb(3 24 56 / 14%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 26%), 0 4px 10px rgb(3 18 40 / 18%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #eafaff;
  align-items: center;
  background: rgb(7 20 44 / 84%);
  border: 1px solid rgb(73 173 255 / 40%);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(186 236 255 / 6%),
    0 8px 18px rgb(3 24 56 / 14%);
  color: #8fdbff;
  cursor: pointer;
  display: inline-flex;
  flex-shrink: 0;
  height: 2.3rem;
  justify-content: center;
  min-height: 0;
  min-width: 2.3rem;
  padding: 0;
  width: 2.3rem;
}

.utility-toggle svg {
  height: 1.1rem;
  width: 1.1rem;
}

.create-toggle {
  --btn-bg: linear-gradient(110deg, #00a8ff, #14f0ff);
  --btn-bg-pressed: linear-gradient(110deg, #d8f6ff, #94ebff);
  --btn-border: rgb(20 240 255 / 35%);
  --btn-border-pressed: rgb(238 252 255 / 88%);
  --btn-shadow: 0 8px 18px rgb(3 24 56 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 72%), 0 4px 10px rgb(3 21 47 / 24%);
  --btn-color: #032748;
  --btn-color-pressed: #053057;
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 1px solid var(--btn-border);
  border-radius: 12px;
  color: #032748;
  cursor: pointer;
  font-family: var(--font-headline);
  font-size: 1.5rem;
  font-weight: 700;
  height: 2.3rem;
  line-height: 1;
  min-height: 0;
  min-width: 2.3rem;
  padding: 0;
}

.utility-new {
  align-items: center;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: clamp(0.9rem, 2.35vw, 1.05rem);
  gap: 0.58rem;
  height: auto;
  justify-content: center;
  min-height: 3rem;
  min-width: 3.2rem;
  padding: 0.48rem;
}

.create-form {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
  box-shadow: inset 0 1px 0 rgb(186 236 255 / 5%);
  display: grid;
  gap: 0.65rem;
  padding: 1rem;
}

.create-form-top,
.create-form-bottom {
  display: grid;
  gap: 0.65rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.create-form input,
.create-form select {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 1rem;
  min-height: 44px;
  padding: 0.58rem 0.7rem;
}

.create-form select {
  appearance: none;
  background:
    linear-gradient(135deg, rgb(8 22 50 / 92%), rgb(4 16 38 / 94%)),
    radial-gradient(circle at 100% 0%, rgb(0 168 255 / 18%), transparent 38%);
  cursor: pointer;
  padding-right: 2.1rem;
}

.create-form select option {
  background: #061833;
  color: #d1e9ff;
}

.create-form-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  justify-content: flex-end;
  grid-column: 1 / -1;
}

.upload-chip,
.create-submit {
  align-items: center;
  border: 1px solid rgb(73 173 255 / 40%);
  border-radius: 12px;
  cursor: pointer;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.85rem;
  font-weight: 700;
  justify-content: center;
  letter-spacing: 0.08em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.upload-chip {
  background: rgb(7 20 44 / 84%);
  box-shadow:
    inset 0 1px 0 rgb(186 236 255 / 6%),
    0 8px 18px rgb(3 24 56 / 14%);
  color: #8fdbff;
}

.create-submit {
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border-color: rgb(20 240 255 / 35%);
  color: #032748;
}

.help-close {
  align-items: center;
  background: rgb(8 38 72 / 78%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  box-shadow: 0 0 16px rgb(66 169 255 / 18%);
  color: #8fdbff;
  cursor: pointer;
  display: inline-flex;
  font-family: var(--font-ui);
  font-weight: 700;
  justify-content: center;
  min-height: 0;
  padding: 0;
}

.help-trigger {
  align-items: center;
  background: rgb(8 28 58 / 92%);
  border: 1px solid rgb(93 171 255 / 42%);
  border-radius: 12px;
  color: #8fdbff;
  cursor: pointer;
  display: inline-flex;
  font-family: var(--font-headline);
  font-size: 1.2rem;
  font-weight: 700;
  height: 2.3rem;
  justify-content: center;
  line-height: 1;
  min-height: 0;
  width: 2.3rem;
}

.help-trigger:hover,
.help-trigger:focus-visible {
  border-color: rgb(102 219 255 / 76%);
  box-shadow: 0 0 0 1px rgb(9 55 95 / 75%), 0 0 20px rgb(40 178 255 / 18%);
  color: #d8f8ff;
}

.help-close {
  font-size: 1.25rem;
  height: 2.15rem;
  line-height: 1;
  width: 2.15rem;
}

.help-screen,
.delete-confirm-screen {
  align-items: center;
  background: rgb(2 9 24 / 72%);
  display: flex;
  inset: 0;
  justify-content: center;
  padding: 1rem;
  position: fixed;
  z-index: 20;
}

.help-panel,
.delete-confirm-panel {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 96%), rgb(7 16 37 / 98%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 18%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 44%);
  border-radius: 18px;
  box-shadow: 0 18px 40px rgb(0 0 0 / 36%);
  color: #cfe2ff;
  display: grid;
  gap: 0.85rem;
  max-width: 34rem;
  padding: 1.05rem;
  width: min(100%, 34rem);
}

.help-header,
.delete-confirm-header {
  align-items: center;
  display: flex;
  gap: 1rem;
  justify-content: space-between;
}

.help-panel h2,
.delete-confirm-panel h2 {
  color: #f1f8ff;
  font-family: var(--font-headline);
  font-size: 1.3rem;
  margin: 0;
}

.help-panel p,
.delete-confirm-panel p {
  color: #b8cdec;
  font-family: var(--font-body);
  line-height: 1.45;
  margin: 0;
}

.delete-confirm-actions {
  display: grid;
  gap: 0.7rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.delete-confirm-actions button {
  min-height: 2.5rem;
  padding: 0.72rem 0.8rem;
  width: 100%;
}

.delete-cancel {
  background: rgb(9 21 45 / 88%);
  border: 1px solid rgb(93 127 181 / 48%);
  color: #cfe2ff;
}

.delete-local {
  background: rgb(23 42 72 / 88%);
  border: 1px solid rgb(112 154 215 / 58%);
  color: #e8f2ff;
}

.delete-remote {
  background: rgb(72 20 35 / 88%);
  border: 1px solid rgb(255 83 105 / 78%);
  color: #ffd7dd;
}

.filter-row {
  display: grid;
  gap: 0.8rem;
}

.filter-field {
  align-items: center;
  display: flex;
  gap: 0.65rem;
  max-width: 24rem;
}

.filter-label {
  color: #9cb3d6;
  flex: 0 0 auto;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.filter-select {
  background: rgb(6 17 38 / 82%);
  border: 1px solid rgb(70 110 174 / 42%);
  border-radius: 10px;
  box-shadow: inset 0 1px 0 rgb(186 236 255 / 5%);
  color: #daecff;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 0.95rem;
  flex: 1 1 auto;
  min-height: 44px;
  padding: 0.58rem 0.7rem;
  width: auto;
}

.filter-select:active {
  border-color: rgb(120 227 255 / 42%);
  transform: translateY(1px) scale(0.99);
}

.checklist-list {
  display: grid;
  gap: 1rem;
}

.checklist-card {
  display: grid;
  gap: 1rem;
  padding: 1rem 1rem 0;
}

.card-primary {
  color: inherit;
  display: grid;
  gap: 1rem;
  width: 100%;
}

.card-open {
  background: none;
  border: 0;
  color: inherit;
  cursor: pointer;
  padding: 0;
  text-align: left;
  width: 100%;
}

.card-open:focus-visible {
  border-radius: 14px;
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 3px;
}

.card-topline {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: start;
}

.card-heading-action {
  display: block;
  min-width: 0;
}

.card-progress-action {
  display: grid;
  gap: 1rem;
}

.card-heading h2 {
  color: #f1f8ff;
  font-family: var(--font-headline);
  font-size: clamp(1.2rem, 2.3vw, 1.75rem);
  line-height: 1.08;
  margin: 0;
  min-width: 0;
}

.card-heading p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(1rem, 2vw, 1.16rem);
  margin: 0.55rem 0 0;
}

.card-top-actions {
  align-items: center;
  display: inline-flex;
  flex-shrink: 0;
  gap: 0.5rem;
  justify-content: flex-end;
}

.action {
  align-items: center;
  border: 0;
  border-radius: 10px;
  cursor: pointer;
  display: inline-flex;
  height: 2.2rem;
  justify-content: center;
  padding: 0;
  width: 2.2rem;
}

.action-icon {
  fill: none;
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1rem;
}

.delete {
  background: rgb(53 15 25 / 70%);
  border: 1px solid rgb(255 70 91 / 84%);
  box-shadow: 0 0 16px rgb(255 72 104 / 24%);
  color: #ff7b89;
}

.action:disabled {
  cursor: wait;
  opacity: 0.55;
}

.edit {
  background: rgb(11 39 84 / 80%);
  border: 1px solid rgb(66 169 255 / 80%);
  box-shadow: 0 0 16px rgb(66 169 255 / 24%);
  color: #61bbff;
}

.checklist-card.status-active .progress-fill {
  color: #64beff;
}

.checklist-card.status-late .progress-fill {
  color: #ff6475;
}

.checklist-card.status-completed .progress-fill {
  color: #8df3c1;
}

.progress-copy {
  color: #8da8d3;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  justify-content: space-between;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.progress-copy span:last-child {
  color: #64beff;
}

.checklist-card.status-late .progress-copy span:last-child {
  color: #ff6475;
}

.checklist-card.status-completed .progress-copy span:last-child {
  color: #8df3c1;
}

.progress-track {
  background: rgb(47 68 103 / 82%);
  border-radius: 999px;
  height: 16px;
  overflow: hidden;
}

.progress-fill {
  background: linear-gradient(90deg, #57b8ff, #3f8fe4);
  border-radius: inherit;
  height: 100%;
}

.checklist-card.status-late .progress-fill {
  background: linear-gradient(90deg, #ff6475, #ef4e60);
}

.checklist-card.status-completed .progress-fill {
  background: linear-gradient(90deg, #52dc9c, #2ebf7c);
}

.task-sync-copy span:first-child {
  align-items: center;
  color: #9ddcff;
  display: inline-flex;
  gap: 0.45rem;
}

.task-sync-pulse {
  background: #14f0ff;
  border-radius: 999px;
  box-shadow: 0 0 0 0 rgb(20 240 255 / 44%);
  height: 0.62rem;
  width: 0.62rem;
}

.task-sync-pulse {
  animation: task-sync-pulse 1.35s ease-out infinite;
}

.task-sync-fill {
  background: linear-gradient(90deg, #16d5ff, #5de9ff);
}

.task-sync-detail {
  color: #7f9fc8;
  font-family: var(--font-body);
  font-size: 0.88rem;
  margin: 0;
}

.card-footer {
  display: flex;
  justify-content: flex-end;
}

.metadata-toggle {
  align-items: center;
  background: rgb(7 28 59 / 86%);
  border: 1px solid rgb(72 120 190 / 46%);
  border-radius: 12px;
  color: #9bc2eb;
  cursor: pointer;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  gap: 0.45rem;
  letter-spacing: 0.08em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.metadata-toggle:focus-visible {
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 2px;
}

.toggle-icon {
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
  transform: rotate(0deg);
  transition: transform 0.18s ease;
  width: 1rem;
}

.toggle-icon.open {
  transform: rotate(180deg);
}

.card-metadata {
  align-items: center;
  border-top: 1px solid rgb(92 126 176 / 28%);
  color: #8da8d3;
  display: flex;
  flex-wrap: wrap;
  gap: 0.9rem;
  padding: 1rem 0;
}

.metadata-item {
  align-items: center;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  gap: 0.55rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.metadata-item svg {
  height: 0.92rem;
  width: 0.92rem;
}

.empty-state {
  padding: 1.4rem;
}

.empty-state h2 {
  font-family: var(--font-headline);
  font-size: 1.4rem;
  margin: 0;
}

.empty-state p {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0.5rem 0 0;
}

@media (max-width: 720px) {
  .segment-strip {
    justify-content: flex-start;
  }

  .segment-actions {
    gap: 0.55rem;
    grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.34fr) minmax(2.8rem, 0.35fr);
  }

  .utility-chip,
  .utility-new {
    font-size: 0.78rem;
    gap: 0.38rem;
    min-height: 2.7rem;
    padding-inline: 0.46rem;
  }

  .utility-chip svg {
    height: 1rem;
    width: 1rem;
  }

  .sync-chip {
    min-width: 0;
  }

  .create-form-top,
  .create-form-bottom {
    grid-template-columns: 1fr;
  }

  .create-form-actions {
    justify-content: flex-start;
  }

  .filter-field {
    max-width: none;
  }

  .delete-confirm-actions {
    grid-template-columns: 1fr;
  }

  .card-top-actions {
    justify-content: flex-end;
  }

  .card-metadata {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.7rem;
  }

  .card-footer {
    justify-content: flex-start;
  }

}

@keyframes task-sync-pulse {
  0% {
    box-shadow: 0 0 0 0 rgb(20 240 255 / 44%);
  }

  70% {
    box-shadow: 0 0 0 0.45rem rgb(20 240 255 / 0%);
  }

  100% {
    box-shadow: 0 0 0 0 rgb(20 240 255 / 0%);
  }
}
</style>
