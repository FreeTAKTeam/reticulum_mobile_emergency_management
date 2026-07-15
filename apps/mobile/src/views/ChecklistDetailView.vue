<script setup lang="ts">
import ListWindowControls from "../components/ListWindowControls.vue";
import { useChecklistDetail } from "../composables/useChecklistDetail";
import { useListWindow } from "../composables/useListWindow";

const {
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
} = useChecklistDetail();
const taskWindow = useListWindow(visibleTasks);
</script>

<template>
  <section class="view checklist-detail-view">
    <template v-if="checklistDetail && checklistRecord">
      <section v-if="!isTemplatePreview" class="detail-toolbar">
        <button
          v-if="shouldShowJoin"
          type="button"
          class="detail-pill"
          :disabled="isMutating"
          @click="joinChecklist"
        >
          <span>Join</span>
        </button>
        <button
          type="button"
          class="detail-pill detail-pill-primary"
          :disabled="isMutating"
          @click="uploadChecklist"
        >
          <span>Sync</span>
        </button>
        <button
          type="button"
          class="detail-pill detail-pill-compact"
          aria-label="Add checklist task"
          :disabled="isMutating"
          @click="addTaskRow"
        >
          +
        </button>
      </section>

      <section class="detail-panel hero-panel">
        <div class="hero-topline">
          <div class="hero-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none">
              <path d="M8 4.5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z" />
              <path d="M9 4h6a1 1 0 0 1 1 1v1H8V5a1 1 0 0 1 1-1Z" />
              <path d="m9.2 10 1 1 2-2" />
              <path d="m9.2 13.5 1 1 2-2" />
              <path d="m9.2 17 1 1 2-2" />
              <path d="M13.6 9.1h2.2" />
              <path d="M13.6 12.6h2.2" />
              <path d="M13.6 16.1h2.2" />
            </svg>
          </div>

          <div class="hero-copy">
            <h1>{{ checklistDetail.heroTitle }}</h1>
            <div class="hero-meta" aria-label="Checklist metadata">
              <p
                v-for="line in checklistDetail.heroMetaLines"
                :key="line"
              >
                {{ line }}
              </p>
            </div>
          </div>

          <div class="hero-ornament" aria-hidden="true">
            <svg viewBox="0 0 64 64" fill="none">
              <path d="M10 18V10h8" />
              <path d="M54 18V10h-8" />
              <path d="M10 46v8h8" />
              <path d="M54 46v8h-8" />
              <circle cx="32" cy="32" r="15" />
              <circle cx="32" cy="32" r="5" />
              <path d="M32 11v13" />
              <path d="M32 40v13" />
              <path d="M11 32h13" />
              <path d="M40 32h13" />
              <path d="m32 17 3.5 11.5L47 32l-11.5 3.5L32 47l-3.5-11.5L17 32l11.5-3.5Z" />
            </svg>
          </div>
        </div>

        <div class="hero-divider"></div>

        <div class="hero-progress-copy">
          <span>{{ checklistDetail.progressLabel }}</span>
          <span class="hero-progress-separator" aria-hidden="true"></span>
          <span>{{ checklistDetail.pendingLabel }}</span>
        </div>

        <div class="hero-progress-track" aria-hidden="true">
          <div class="hero-progress-fill" :style="{ width: `${checklistDetail.progress}%` }"></div>
        </div>
      </section>

      <section class="tasks-section">
        <div class="tasks-heading">
          <span class="tasks-heading-mark" aria-hidden="true"></span>
          <h2>{{ checklistDetail.tasksHeading }}</h2>
          <span class="tasks-heading-line" aria-hidden="true"></span>
        </div>

        <div class="task-list">
          <article
            v-for="task in taskWindow.items.value"
            :key="task.id"
            class="detail-panel task-card"
            :class="[taskStatusClass(task.status), { 'task-line-break': task.lineBreakEnabled }]"
            :style="task.rowBackgroundColor ? { '--task-accent': task.rowBackgroundColor } : undefined"
          >
            <div class="task-card-shell">
              <button
                type="button"
                class="task-toggle"
                :class="taskStatusClass(task.status)"
                :aria-label="`Mark ${task.title} as completed`"
                :disabled="isTemplatePreview || task.status === 'completed' || task.status === 'submitted' || isMutating"
                @click="completeTask(task.id)"
              >
                <svg v-if="task.status === 'completed'" viewBox="0 0 24 24" fill="none">
                  <path d="m8 12 2.5 2.5L16 9" />
                </svg>
              </button>

              <div class="task-copy">
                <div class="task-copy-topline">
                  <div class="task-copy-heading">
                    <template v-if="isTaskEditing(task)">
                      <label class="task-edit-label" :for="`task-edit-${task.id}`">Task text</label>
                      <input
                        :id="`task-edit-${task.id}`"
                        v-model="editingTaskValue"
                        class="task-edit-input"
                        type="text"
                        :disabled="isMutating"
                        @keyup.enter="saveTaskRow(task)"
                        @keyup.esc="cancelTaskEdit"
                      />
                    </template>
                    <h3 v-else>{{ task.title }}</h3>
                    <p>{{ task.description }}</p>
                  </div>

                  <span class="task-status-pill" :class="taskStatusClass(task.status)">
                    {{ taskStatusLabel(task.status) }}
                  </span>
                </div>

                <div class="task-divider"></div>

                <p class="task-meta" :class="taskMetaClass(task.metaTone)">
                  <svg v-if="task.metaTone === 'alert'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <path d="M12 4.5 20 19.5H4z" />
                    <path d="M12 9v4.5" />
                    <circle cx="12" cy="16.8" r=".8" fill="currentColor" stroke="none" />
                  </svg>
                  <svg v-else-if="task.metaTone === 'done'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <circle cx="12" cy="12" r="8" />
                    <path d="m8.5 12 2.3 2.3 4.7-4.7" />
                  </svg>
                  <svg v-else viewBox="0 0 24 24" fill="none" aria-hidden="true">
                    <circle cx="12" cy="12" r="8" />
                    <path d="M12 8v4.5l3 1.8" />
                  </svg>
                  <span>{{ task.metaLabel }}</span>
                </p>

                <div
                  v-if="!isTemplatePreview && editableTaskCells(task).length > 0"
                  class="task-cells"
                >
                  <div
                    v-for="cell in editableTaskCells(task)"
                    :key="cell.columnUid"
                    class="task-cell"
                    :class="{ 'task-cell-readonly': !cell.editable }"
                  >
                    <label
                      class="task-cell-label"
                      :for="isTaskEditing(task) ? `task-cell-${task.id}-${cell.columnUid}` : undefined"
                    >
                      {{ cell.label }}
                    </label>
                    <div v-if="isTaskEditing(task)" class="task-cell-control">
                      <input
                        :id="`task-cell-${task.id}-${cell.columnUid}`"
                        class="task-cell-input"
                        type="text"
                        :value="taskCellDraftValue(task, cell)"
                        :disabled="isMutating || !cell.editable"
                        @input="updateTaskCellDraft(task, cell, $event)"
                        @keyup.enter="saveTaskRow(task)"
                        @keyup.esc="cancelTaskEdit"
                      />
                    </div>
                    <p v-else class="task-cell-value">{{ cell.value || "Not set" }}</p>
                  </div>
                </div>

                <div v-if="!isTemplatePreview" class="task-actions" aria-label="Task row actions">
                  <button
                    type="button"
                    class="task-icon-button task-icon-edit"
                    :class="{ active: isTaskEditing(task), dirty: taskHasDraft(task) }"
                    :aria-label="isTaskEditing(task) ? `Save ${task.title}` : `Edit ${task.title}`"
                    :title="isTaskEditing(task) ? 'Save' : 'Edit'"
                    :disabled="isMutating || (isTaskEditing(task) && !editingTaskValue.trim())"
                    @click="toggleTaskEdit(task)"
                  >
                    <svg class="task-action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                      <path d="M12 20h9" />
                      <path d="m16.5 3.5 4 4L8 20l-4 1 1-4z" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="task-icon-button task-icon-delete"
                    :aria-label="`Delete ${task.title}`"
                    title="Delete"
                    :disabled="isMutating"
                    @click="deleteTaskRow(task)"
                  >
                    <svg class="task-action-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                      <path d="M9 4h6" />
                      <path d="M5 7h14" />
                      <path d="M8 7v12a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V7" />
                      <path d="M10 10v7" />
                      <path d="M14 10v7" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </article>
          <ListWindowControls
            :start="taskWindow.startIndex.value"
            :end="taskWindow.endIndex.value"
            :total="taskWindow.total.value"
            :has-previous="taskWindow.hasPrevious.value"
            :has-next="taskWindow.hasNext.value"
            @previous="taskWindow.previous"
            @next="taskWindow.next"
          />
        </div>
      </section>

    </template>

    <section v-else class="detail-panel empty-state detail-empty">
      <h1>Checklist not found.</h1>
      <p>The requested checklist could not be loaded from the current local dataset.</p>
    <RouterLink class="detail-back-link" to="/checklists">Return to checklists</RouterLink>
    </section>
  </section>
</template>

<style scoped src="./ChecklistDetailHero.css"></style>
<style scoped src="./ChecklistDetailTasks.css"></style>
<style scoped src="./ChecklistDetailActions.css"></style>
