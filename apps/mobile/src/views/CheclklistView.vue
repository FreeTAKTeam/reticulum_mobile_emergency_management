<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRouter } from "vue-router";

import { useNodeStore } from "../stores/nodeStore";
import {
  getChecklistRecords,
  type ChecklistFilter,
  type ChecklistRecord,
  type ChecklistSegment,
  type ChecklistStatus,
} from "../utils/checklists";

const nodeStore = useNodeStore();
const router = useRouter();
const activeSegment = ref<ChecklistSegment>("live");
const activeFilter = ref<ChecklistFilter>("all");
const expandedChecklistIds = ref<string[]>([]);
const isCreateFormVisible = ref(false);
const localChecklists = ref<ChecklistRecord[]>([]);

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
    scheduledAt: "",
  };
}

const createForm = reactive(createDefaultChecklistFormState());

const checklistRecords = computed(() => {
  const records = getChecklistRecords(activeSegment.value);
  if (activeSegment.value === "live") {
    return [...localChecklists.value, ...records];
  }
  return records;
});

const summary = computed(() => {
  const records = checklistRecords.value;
  return {
    total: records.length,
    active: records.filter((record) => record.status === "active").length,
    late: records.filter((record) => record.status === "late").length,
  };
});

const summaryMetrics = computed(() => [
  {
    key: "total",
    value: summary.value.total,
    label: "Total",
    alert: false,
  },
  {
    key: "active",
    value: summary.value.active,
    label: "Active",
    alert: false,
  },
  {
    key: "late",
    value: summary.value.late,
    label: "Late",
    alert: true,
  },
]);

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

function statusLabel(status: ChecklistStatus): string {
  if (status === "late") {
    return "Late";
  }
  if (status === "completed") {
    return "Completed";
  }
  return "Active";
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

function toggleCreateForm(): void {
  if (isCreateFormVisible.value) {
    resetCreateForm();
  }
  isCreateFormVisible.value = !isCreateFormVisible.value;
}

function createChecklist(): void {
  const title = createForm.title.trim();
  if (!title) {
    return;
  }

  localChecklists.value = [
    {
      id: `local-${Date.now()}`,
      title,
      subtitle: createForm.subtitle.trim() || "New task list",
      status: "active",
      progress: 0,
      statusCountLabel: "0 pending",
      scheduledAt: createForm.scheduledAt.trim() || new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }).format(new Date()),
      teamLabel: createForm.teamLabel.trim() || "Task Group",
      compatibilityLabel: "RCH compatible",
    },
    ...localChecklists.value,
  ];

  activeSegment.value = "live";
  resetCreateForm();
  isCreateFormVisible.value = false;
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

function openChecklist(checklistId: string): void {
  void router.push({ name: "checlklist-detail", params: { checklistId } });
}
</script>

<template>
  <section class="view checklist-view">
    <h1 class="sr-only">Checklists</h1>

    <section class="segment-strip">
      <h2 class="segment-title">Checklists</h2>
      <div class="segment-actions">
        <button
          type="button"
          class="sync-chip"
          :disabled="isSyncing"
          :class="{ busy: isSyncing }"
          @click="requestSync"
        >
          <span>{{ isSyncing ? "Syncing" : "Sync" }}</span>
        </button>
        <button
          type="button"
          class="create-toggle"
          aria-label="Create checklist"
          title="Create checklist"
          :aria-expanded="isCreateFormVisible"
          @click="toggleCreateForm"
        >
          +
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
          placeholder="Assigned team"
          aria-label="Assigned team"
        />
        <input
          v-model="createForm.scheduledAt"
          type="text"
          placeholder="Schedule"
          aria-label="Checklist schedule"
        />
        <div class="create-form-actions">
          <button
            type="button"
            class="template-chip"
            :class="{ selected: activeSegment === 'templates' }"
            @click="toggleTemplates"
          >
            Templates
          </button>
          <button type="button" class="upload-chip">
            Upload
          </button>
          <button type="submit" class="create-submit">
            Create checklist
          </button>
        </div>
      </div>
    </form>

    <section class="summary-panel">
      <div class="summary-grid">
        <article
          v-for="metric in summaryMetrics"
          :key="metric.key"
          class="summary-metric"
          :class="{ 'summary-metric-alert': metric.alert }"
        >
          <p class="summary-value">{{ metric.value }}</p>
          <p class="summary-label">{{ metric.label }}</p>
        </article>
      </div>
    </section>

    <section class="filter-row">
      <label class="filter-field">
        <span class="filter-label">Filter</span>
        <select
          v-model="activeFilter"
          class="filter-select"
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
      </label>
    </section>

    <section class="checklist-list">
      <article
        v-for="record in filteredRecords"
        :key="record.id"
        class="checklist-card"
        :class="statusCardClass(record.status)"
      >
        <button
          type="button"
          class="card-primary"
          :aria-label="`Open ${record.title}`"
          @click="openChecklist(record.id)"
        >
          <div class="card-topline">
            <div class="card-icon" aria-hidden="true">
              <svg v-if="record.status === 'completed'" viewBox="0 0 24 24" fill="none">
                <path d="M8 4.5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z" />
                <path d="M9 4h6a1 1 0 0 1 1 1v1H8V5a1 1 0 0 1 1-1Z" />
                <path d="m9.5 13 2 2 4-5" />
              </svg>
              <svg v-else viewBox="0 0 24 24" fill="none">
                <path d="M8 4.5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z" />
                <path d="M9 4h6a1 1 0 0 1 1 1v1H8V5a1 1 0 0 1 1-1Z" />
                <path d="M9.5 10h5" />
                <path d="M9.5 13.5h5" />
                <path d="M9.5 17h5" />
              </svg>
            </div>

            <div class="card-heading">
              <h2>{{ record.title }}</h2>
              <p>{{ record.subtitle }}</p>
            </div>

            <div class="card-top-actions">
              <span class="status-pill" :class="statusCardClass(record.status)">
                {{ statusLabel(record.status) }}
              </span>
              <span class="card-chevron" aria-hidden="true">&#8250;</span>
            </div>
          </div>

          <div class="progress-copy">
            <span>{{ record.progress }}% complete</span>
            <span>{{ record.statusCountLabel }}</span>
          </div>

          <div class="progress-track" aria-hidden="true">
            <div class="progress-fill" :style="{ width: `${record.progress}%` }"></div>
          </div>
        </button>

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
          <div class="card-metadata">
            <span class="metadata-item">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M7 4v3" />
                <path d="M17 4v3" />
                <path d="M5 8h14" />
                <path d="M6 6.5h12a1 1 0 0 1 1 1v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-10a1 1 0 0 1 1-1Z" />
              </svg>
              {{ record.scheduledAt }}
            </span>
            <span class="metadata-divider" aria-hidden="true"></span>
            <span class="metadata-item">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M12 12a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" />
                <path d="M5.5 19a6.5 6.5 0 0 1 13 0" />
              </svg>
              {{ record.teamLabel }}
            </span>
            <span class="metadata-divider" aria-hidden="true"></span>
            <span class="metadata-item metadata-compatibility">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path d="M4.5 12a7.5 7.5 0 0 1 15 0" />
                <path d="M8 12a4 4 0 0 1 8 0" />
                <circle cx="12" cy="12" r="1.5" />
              </svg>
              {{ record.compatibilityLabel }}
            </span>
          </div>
        </section>
      </article>

      <article v-if="filteredRecords.length === 0" class="empty-state">
        <h2>No checklist matches this filter.</h2>
        <p>Switch filters or open the template library to prepare a new checklist package.</p>
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
  display: flex;
  flex-shrink: 0;
  gap: 0.55rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.card-icon svg,
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

.create-form input {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 1rem;
  min-height: 44px;
  padding: 0.58rem 0.7rem;
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

.summary-panel {
  padding: 0.78rem 0.82rem;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.75rem;
}

.summary-metric {
  align-items: center;
  display: grid;
  background:
    linear-gradient(145deg, rgb(18 35 68 / 92%), rgb(10 20 45 / 90%)),
    radial-gradient(circle at 72% 10%, rgb(69 235 255 / 14%), transparent 36%);
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 14px;
  gap: 0.08rem;
  justify-items: center;
  min-height: 114px;
  padding: 0.85rem 0.45rem 0.72rem;
}

.summary-value {
  color: #f0f7ff;
  font-family: var(--font-ui);
  font-size: clamp(2.45rem, 4.6vw, 3.3rem);
  font-weight: 700;
  line-height: 1;
  margin: 0;
}

.summary-label {
  color: #88a5cf;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.09em;
  margin: 0.13rem 0 0;
  text-transform: uppercase;
}

.summary-metric-alert .summary-value,
.summary-metric-alert .summary-label {
  color: #ff6475;
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

.status-pill {
  align-items: center;
  background: rgb(8 29 61 / 88%);
  border: 1px solid rgb(74 133 207 / 45%);
  border-radius: 15px;
  color: #91a8cf;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  justify-content: center;
  letter-spacing: 0.08em;
  min-height: 0;
  padding: 0.5rem 0.95rem;
  text-transform: uppercase;
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
  background: none;
  border: 0;
  color: inherit;
  cursor: pointer;
  display: grid;
  gap: 1rem;
  padding: 0;
  text-align: left;
  width: 100%;
}

.card-primary:focus-visible {
  border-radius: 14px;
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 3px;
}

.card-topline {
  display: grid;
  gap: 1rem;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: start;
}

.card-icon {
  align-items: center;
  border: 1px solid rgb(74 190 255 / 90%);
  border-radius: 12px;
  color: #64beff;
  display: inline-flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}

.card-icon svg {
  height: 1.4rem;
  width: 1.4rem;
}

.checklist-card.status-late .card-icon {
  border-color: rgb(255 100 117 / 88%);
  color: #ff6475;
}

.card-heading h2 {
  color: #f1f8ff;
  font-family: var(--font-headline);
  font-size: clamp(1.2rem, 2.3vw, 1.75rem);
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
  gap: 0.9rem;
}

.status-pill.status-active,
.checklist-card.status-active .progress-fill {
  color: #64beff;
}

.status-pill.status-late,
.checklist-card.status-late .progress-fill {
  border-color: rgb(255 100 117 / 58%);
  color: #ff6475;
}

.status-pill.status-completed,
.checklist-card.status-completed .progress-fill {
  color: #8df3c1;
}

.status-pill.status-completed {
  background: rgb(14 67 42 / 82%);
  border-color: rgb(71 214 145 / 40%);
}

.card-chevron {
  color: #839fc9;
  font-family: var(--font-ui);
  font-size: 2.25rem;
  line-height: 1;
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

.metadata-divider {
  background: rgb(92 126 176 / 32%);
  height: 1.25rem;
  width: 1px;
}

.metadata-compatibility {
  color: #2db7ff;
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
    align-self: flex-end;
    justify-content: flex-end;
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

  .summary-grid {
    gap: 0.5rem;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .summary-metric {
    min-height: 102px;
    padding-inline: 0.32rem;
  }

  .summary-value {
    font-size: clamp(2rem, 7vw, 2.5rem);
  }

  .summary-label {
    font-size: 0.68rem;
  }

  .filter-field {
    max-width: none;
  }

  .card-topline {
    grid-template-columns: auto 1fr;
  }

  .card-top-actions {
    grid-column: 1 / -1;
    justify-content: space-between;
  }

  .card-metadata {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.7rem;
  }

  .card-footer {
    justify-content: flex-start;
  }

  .metadata-divider {
    display: none;
  }
}
</style>
