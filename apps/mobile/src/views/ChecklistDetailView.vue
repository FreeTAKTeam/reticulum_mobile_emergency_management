<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";

import {
  getChecklistDetailById,
  getChecklistRecordById,
  type ChecklistTask,
  type ChecklistTaskMetaTone,
  type ChecklistTaskStatus,
} from "../utils/checklists";

const route = useRoute();

const checklistId = computed(() => String(route.params.checklistId ?? ""));
const checklistRecord = computed(() => getChecklistRecordById(checklistId.value));
const checklistDetail = computed(() => getChecklistDetailById(checklistId.value));
const visibleTasks = ref<ChecklistTask[]>([]);

watch(
  checklistDetail,
  (detail) => {
    visibleTasks.value = detail ? detail.tasks.map((task) => ({ ...task })) : [];
  },
  { immediate: true },
);

function taskStatusClass(status: ChecklistTaskStatus): string {
  return `task-${status}`;
}

function taskStatusLabel(status: ChecklistTaskStatus): string {
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

function completeTask(taskId: string): void {
  visibleTasks.value = visibleTasks.value.map((task) => {
    if (task.id !== taskId || task.status === "completed") {
      return task;
    }

    return {
      ...task,
      status: "completed",
      metaTone: "done",
      metaLabel: "Completed just now",
    };
  });
}
</script>

<template>
  <section class="view checklist-detail-view">
    <template v-if="checklistDetail && checklistRecord">
      <section class="detail-toolbar">
        <button type="button" class="detail-pill detail-pill-primary">
          <span>Sync</span>
        </button>
        <button type="button" class="detail-pill detail-pill-compact" aria-label="Add checklist task">
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
            <p>{{ checklistDetail.heroSubtitle }}</p>
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
            v-for="task in visibleTasks"
            :key="task.id"
            class="detail-panel task-card"
            :class="taskStatusClass(task.status)"
          >
            <div class="task-card-shell">
              <button
                type="button"
                class="task-toggle"
                :class="taskStatusClass(task.status)"
                :aria-label="`Mark ${task.title} as completed`"
                :disabled="task.status === 'completed'"
                @click="completeTask(task.id)"
              >
                <svg v-if="task.status === 'completed'" viewBox="0 0 24 24" fill="none">
                  <path d="m8 12 2.5 2.5L16 9" />
                </svg>
              </button>

              <div class="task-copy">
                <div class="task-copy-topline">
                  <div class="task-copy-heading">
                    <h3>{{ task.title }}</h3>
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
              </div>
            </div>
          </article>
        </div>
      </section>

    </template>

    <section v-else class="detail-panel empty-state detail-empty">
      <h1>Checklist not found.</h1>
      <p>The requested checklist could not be loaded from the current local dataset.</p>
      <RouterLink class="detail-back-link" to="/checlklist">Return to checklists</RouterLink>
    </section>
  </section>
</template>

<style scoped>
.checklist-detail-view {
  display: grid;
  gap: 1rem;
}

.detail-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  justify-content: flex-end;
}

.detail-panel,
.detail-back-link {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
  box-shadow: inset 0 1px 0 rgb(186 236 255 / 5%);
}

.hero-panel {
  display: grid;
  gap: 1rem;
  padding: 1.05rem 1.1rem 0.95rem;
}

.hero-topline {
  align-items: center;
  display: grid;
  gap: 1rem;
  grid-template-columns: auto minmax(0, 1fr) auto;
}

.hero-icon,
.hero-ornament {
  color: #3fa8ff;
  display: inline-flex;
}

.hero-icon svg,
.hero-ornament svg,
.task-toggle svg,
.task-meta svg {
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

.hero-icon {
  height: 2.85rem;
  width: 2.85rem;
}

.hero-icon svg {
  height: 100%;
  width: 100%;
}

.hero-copy h1 {
  color: #f2f8ff;
  font-family: var(--font-headline);
  font-size: clamp(0.98rem, 1.65vw, 1.24rem);
  margin: 0;
  text-transform: uppercase;
}

.hero-copy p {
  color: #2fa5ff;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  letter-spacing: 0.12em;
  margin: 0.28rem 0 0;
  text-transform: uppercase;
}

.hero-ornament {
  height: 4.2rem;
  width: 4.2rem;
}

.hero-ornament svg {
  height: 100%;
  width: 100%;
}

.hero-divider {
  background: rgb(92 126 176 / 28%);
  height: 1px;
  width: 100%;
}

.hero-progress-copy {
  align-items: center;
  color: #f2f8ff;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  gap: 0.85rem;
  justify-content: center;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.hero-progress-copy span:first-child,
.hero-progress-copy span:last-child {
  color: #45abff;
}

.hero-progress-separator {
  background: rgb(92 126 176 / 42%);
  height: 1.5rem;
  width: 1px;
}

.hero-progress-track {
  background: rgb(36 56 91 / 88%);
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 999px;
  height: 1.1rem;
  overflow: hidden;
}

.hero-progress-fill {
  background: linear-gradient(90deg, #45abff, #3b89d9);
  border-radius: inherit;
  height: 100%;
}

.tasks-section {
  display: grid;
  gap: 1rem;
}

.tasks-heading {
  align-items: center;
  display: flex;
  gap: 0.8rem;
}

.tasks-heading h2 {
  color: #33a8ff;
  font-family: var(--font-ui);
  font-size: 0.9rem;
  letter-spacing: 0.1em;
  margin: 0;
  text-transform: uppercase;
}

.tasks-heading-mark {
  background:
    repeating-linear-gradient(
      45deg,
      rgb(44 123 212 / 60%),
      rgb(44 123 212 / 60%) 5px,
      transparent 5px,
      transparent 9px
    );
  border-radius: 3px;
  display: inline-block;
  height: 1rem;
  width: 1.3rem;
}

.tasks-heading-line {
  background: rgb(31 100 176 / 46%);
  flex: 1;
  height: 1px;
}

.task-list {
  display: grid;
  gap: 1rem;
}

.task-card {
  overflow: hidden;
  position: relative;
}

.task-card::before {
  background: #45abff;
  content: "";
  inset: 0 auto 0 0;
  position: absolute;
  width: 10px;
}

.task-card.task-late::before {
  background: #ef4e60;
}

.task-card.task-completed::before {
  background: #2ebf7c;
}

.task-card-shell {
  display: grid;
  gap: 0.85rem;
  grid-template-columns: auto minmax(0, 1fr);
  padding: 0.95rem 0.95rem 0.85rem 1.2rem;
}

.task-toggle {
  align-items: center;
  background: transparent;
  border: 1px solid currentColor;
  border-radius: 10px;
  color: #45abff;
  cursor: pointer;
  display: inline-flex;
  height: 2.2rem;
  justify-content: center;
  padding: 0;
  width: 2.2rem;
}

.task-toggle.task-late {
  color: #ff6475;
}

.task-toggle.task-completed {
  background: rgb(14 67 42 / 82%);
  border-color: rgb(71 214 145 / 40%);
  color: #8df3c1;
}

.task-toggle svg {
  height: 0.92rem;
  width: 0.92rem;
}

.task-toggle:disabled {
  cursor: default;
}

.task-copy {
  display: grid;
  gap: 0.7rem;
}

.task-copy-topline {
  align-items: start;
  display: grid;
  gap: 0.75rem;
  grid-template-columns: minmax(0, 1fr) auto;
}

.task-copy-heading h3 {
  color: #f1f8ff;
  font-family: var(--font-headline);
  font-size: clamp(0.98rem, 1.65vw, 1.24rem);
  margin: 0;
  min-width: 0;
  text-transform: uppercase;
}

.task-copy-heading p {
  color: #c2d7f6;
  font-family: var(--font-body);
  font-size: 0.9rem;
  line-height: 1.38;
  margin: 0.3rem 0 0;
  max-width: 32rem;
}

.task-status-pill {
  align-items: center;
  background: rgb(8 29 61 / 88%);
  border: 1px solid rgb(74 133 207 / 45%);
  border-radius: 12px;
  color: #64beff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  justify-content: center;
  letter-spacing: 0.08em;
  min-height: 0;
  padding: 0.38rem 0.82rem;
  text-transform: uppercase;
}

.task-status-pill.task-late {
  border-color: rgb(255 100 117 / 58%);
  color: #ff6475;
}

.task-status-pill.task-completed {
  background: rgb(14 67 42 / 82%);
  border-color: rgb(71 214 145 / 40%);
  color: #8df3c1;
}

.task-divider {
  background: rgb(92 126 176 / 24%);
  height: 1px;
  width: 100%;
}

.task-meta {
  align-items: center;
  color: #7ea6dc;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  gap: 0.5rem;
  letter-spacing: 0.04em;
  margin: 0;
  text-transform: uppercase;
}

.task-meta svg {
  height: 0.92rem;
  width: 0.92rem;
}

.task-meta-clock {
  color: #7ea6dc;
}

.task-meta-alert {
  color: #ff6475;
}

.task-meta-done {
  color: #8df3c1;
}

.detail-pill {
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

.detail-pill-primary {
  min-width: 8rem;
}

.detail-pill-compact {
  font-family: var(--font-headline);
  font-size: 1.35rem;
  font-weight: 700;
  line-height: 1;
  min-width: 2.6rem;
  padding: 0.3rem 0.82rem;
}

.detail-empty {
  justify-items: start;
  padding: 1.25rem;
}

.detail-empty h1 {
  color: #f1f8ff;
  font-family: var(--font-headline);
  font-size: 1.45rem;
  margin: 0;
}

.detail-empty p {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0.55rem 0 0;
}

.detail-back-link {
  align-items: center;
  color: #64beff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.9rem;
  letter-spacing: 0.08em;
  margin-top: 1rem;
  padding: 0.7rem 1rem;
  text-decoration: none;
  text-transform: uppercase;
}

@media (max-width: 720px) {
  .detail-toolbar {
    justify-content: flex-end;
  }

  .hero-panel {
    padding: 0.95rem 0.9rem 0.9rem;
  }

  .hero-topline {
    grid-template-columns: auto 1fr;
  }

  .hero-ornament {
    display: none;
  }

  .hero-copy h1 {
    font-size: 0.98rem;
  }

  .hero-copy p {
    font-size: 0.76rem;
    letter-spacing: 0.08em;
  }

  .hero-progress-copy {
    gap: 0.8rem;
    justify-content: space-between;
  }

  .hero-progress-separator {
    display: none;
  }

  .task-card-shell {
    grid-template-columns: auto 1fr;
    padding: 0.9rem 0.85rem 0.82rem 1.05rem;
  }

  .task-copy-topline {
    grid-template-columns: 1fr;
  }

  .task-status-pill {
    justify-self: start;
  }

  .detail-pill {
    min-width: 0;
  }
}
</style>
