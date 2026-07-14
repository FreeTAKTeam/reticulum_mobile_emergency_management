<script setup lang="ts">
import { useChecklistList } from "../composables/useChecklistList";

const {
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
} = useChecklistList();
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

<style scoped src="./ChecklistViewControls.css"></style>
<style scoped src="./ChecklistViewDialogs.css"></style>
<style scoped src="./ChecklistViewCards.css"></style>
<style scoped src="./ChecklistViewResponsive.css"></style>
