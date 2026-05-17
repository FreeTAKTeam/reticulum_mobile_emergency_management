<script setup lang="ts">
import { computed, nextTick, reactive, ref, shallowRef, watch } from "vue";

import { useEventsStore } from "../stores/eventsStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  MECP_CATEGORIES,
  MECP_EVENT_CODES,
  MECP_SEVERITIES,
  encodeMecpMessage,
  type MecpCategoryCode,
  type MecpSeverity,
} from "../utils/mecp";

const eventsStore = useEventsStore();
const nodeStore = useNodeStore();

type EventSeverityFilter = "All" | "Mayday" | "Urgent" | "Safety" | "Routine";
type EventCategoryFilter = "All" | MecpCategoryCode;

const appReady = computed(() => nodeStore.ready);
const isCreateFormVisible = shallowRef(false);
const isSeverityMenuOpen = shallowRef(false);
const isEventMenuOpen = shallowRef(false);
const isFilterPanelOpen = shallowRef(false);
const categoryScroller = ref<HTMLElement | null>(null);
const categoryDrag = reactive({
  active: false,
  moved: false,
  pointerId: -1,
  startY: 0,
  startScrollTop: 0,
  suppressClickUntil: 0,
});
const configuredCallsign = computed(() => nodeStore.settings.displayName.trim() || "Unset");
const readinessHint = "Node is not ready yet. Wait for the top-right status to show Ready.";
const severityFilters: EventSeverityFilter[] = ["All", "Mayday", "Urgent", "Safety", "Routine"];
const filters = reactive<{
  severity: EventSeverityFilter;
  category: EventCategoryFilter;
}>({
  severity: "All",
  category: "All",
});

type CreateEventFormState = {
  severity: MecpSeverity;
  category: MecpCategoryCode;
  eventCode: string;
  details: string;
};

function createDefaultFormState(): CreateEventFormState {
  return {
    severity: 2,
    category: "P",
    eventCode: "P01",
    details: "",
  };
}

const createForm = reactive(createDefaultFormState());

const selectedSeverity = computed(() =>
  MECP_SEVERITIES.find((severity) => severity.value === createForm.severity) ?? MECP_SEVERITIES[2],
);
const selectedCategory = computed(() =>
  MECP_CATEGORIES.find((category) => category.code === createForm.category) ?? MECP_CATEGORIES[1],
);
const selectedEventOptions = computed(() => MECP_EVENT_CODES[createForm.category]);
const selectedEvent = computed(() =>
  selectedEventOptions.value.find((event) => event.code === createForm.eventCode)
    ?? selectedEventOptions.value[0],
);
const mecpPreview = computed(() =>
  encodeMecpMessage({
    severity: createForm.severity,
    codes: [selectedEvent.value.code],
    details: createForm.details,
  }),
);
const categoryFilters = computed<Array<{ value: EventCategoryFilter; label: string }>>(() => [
  { value: "All", label: "All categories" },
  ...MECP_CATEGORIES.map((category) => ({ value: category.code, label: category.label })),
]);
const filterSummary = computed(() => {
  const parts: string[] = [];
  if (filters.severity !== "All") {
    parts.push(filters.severity);
  }
  if (filters.category !== "All") {
    parts.push(MECP_CATEGORIES.find((category) => category.code === filters.category)?.label ?? filters.category);
  }
  return parts.length > 0 ? parts.join(" / ") : "All";
});
const events = computed(() =>
  eventsStore.records.filter((event) => {
    if (filters.severity !== "All" && event.mecp?.severity !== filters.severity) {
      return false;
    }
    if (filters.category !== "All") {
      if (event.mecp?.categoryCode !== filters.category) {
        return false;
      }
    }
    return true;
  }),
);

watch(
  () => createForm.category,
  (category) => {
    const options = MECP_EVENT_CODES[category];
    if (!options.some((option) => option.code === createForm.eventCode)) {
      createForm.eventCode = options[0].code;
    }
  },
);

function resetCreateForm(): void {
  Object.assign(createForm, createDefaultFormState());
  isSeverityMenuOpen.value = false;
  isEventMenuOpen.value = false;
  void nextTick(scrollSelectedCategoryIntoView);
}

function ensureReady(action: string): boolean {
  try {
    nodeStore.assertReadyForOutbound(action);
    return true;
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    nodeStore.setLastError(message);
    nodeStore.logUi("Error", `[events] ${action} blocked: ${message}`);
    return false;
  }
}

function scrollSelectedCategoryIntoView(): void {
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  const selected = scroller.querySelector<HTMLElement>("[data-selected='true']");
  selected?.scrollIntoView({ block: "center", behavior: "smooth" });
}

function selectNearestVisibleCategory(): void {
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  const scrollerRect = scroller.getBoundingClientRect();
  const scrollerCenter = scrollerRect.top + scrollerRect.height / 2;
  const cards = Array.from(scroller.querySelectorAll<HTMLElement>("[data-category]"));
  let nearest: HTMLElement | null = null;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const card of cards) {
    const rect = card.getBoundingClientRect();
    const cardCenter = rect.top + rect.height / 2;
    const distance = Math.abs(cardCenter - scrollerCenter);
    if (distance < nearestDistance) {
      nearest = card;
      nearestDistance = distance;
    }
  }
  const category = nearest?.dataset.category ?? "";
  if (category && category !== createForm.category) {
    createForm.category = category as MecpCategoryCode;
  }
}

function toggleCreateForm(): void {
  if (!isCreateFormVisible.value && !ensureReady("send events")) {
    return;
  }
  isCreateFormVisible.value = !isCreateFormVisible.value;
  if (isCreateFormVisible.value) {
    void nextTick(scrollSelectedCategoryIntoView);
  }
}

function resetFilters(): void {
  filters.severity = "All";
  filters.category = "All";
}

function selectSeverity(severity: MecpSeverity): void {
  createForm.severity = severity;
  isSeverityMenuOpen.value = false;
}

function selectCategory(category: MecpCategoryCode): void {
  if (Date.now() < categoryDrag.suppressClickUntil) {
    return;
  }
  createForm.category = category;
  void nextTick(scrollSelectedCategoryIntoView);
}

function selectEvent(code: string): void {
  createForm.eventCode = code;
  isEventMenuOpen.value = false;
}

function startCategoryDrag(event: PointerEvent): void {
  if (event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  categoryDrag.active = true;
  categoryDrag.moved = false;
  categoryDrag.pointerId = event.pointerId;
  categoryDrag.startY = event.clientY;
  categoryDrag.startScrollTop = scroller.scrollTop;
  scroller.setPointerCapture(event.pointerId);
}

function moveCategoryDrag(event: PointerEvent): void {
  if (!categoryDrag.active || event.pointerId !== categoryDrag.pointerId) {
    return;
  }
  const scroller = categoryScroller.value;
  if (!scroller) {
    return;
  }
  const deltaY = event.clientY - categoryDrag.startY;
  if (Math.abs(deltaY) > 4) {
    categoryDrag.moved = true;
    categoryDrag.suppressClickUntil = Date.now() + 250;
  }
  scroller.scrollTop = categoryDrag.startScrollTop - deltaY;
  event.preventDefault();
}

function stopCategoryDrag(event: PointerEvent): void {
  if (!categoryDrag.active || event.pointerId !== categoryDrag.pointerId) {
    return;
  }
  const scroller = categoryScroller.value;
  if (scroller?.hasPointerCapture(event.pointerId)) {
    scroller.releasePointerCapture(event.pointerId);
  }
  if (categoryDrag.moved) {
    selectNearestVisibleCategory();
  }
  categoryDrag.active = false;
  categoryDrag.moved = false;
  categoryDrag.pointerId = -1;
}

async function createEvent(): Promise<void> {
  if (!ensureReady("send events")) {
    return;
  }
  if (configuredCallsign.value === "Unset") {
    nodeStore.logUi("Warn", "[events] create blocked: callsign=Unset");
    return;
  }
  try {
    nodeStore.logUi("Info", `[events] creating MECP event body="${mecpPreview.value}".`);
    await eventsStore.upsertLocal({
      type: createForm.category,
      summary: mecpPreview.value,
    });
    resetCreateForm();
    isCreateFormVisible.value = false;
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    nodeStore.setLastError(message);
    nodeStore.logUi("Error", `[events] create failed: ${message}`);
  }
}

async function deleteEvent(uid: string): Promise<void> {
  await eventsStore.deleteLocal(uid);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <span class="utility-chip count-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 4 4 8l8 4 8-4-8-4Z" />
            <path d="M4 12l8 4 8-4" />
            <path d="M4 16l8 4 8-4" />
          </svg>
          <span>{{ events.length }} EVT</span>
        </span>
        <button
          class="utility-chip filter-chip"
          type="button"
          aria-label="Event filter status"
          :aria-expanded="isFilterPanelOpen"
          @click="isFilterPanelOpen = !isFilterPanelOpen"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />
          </svg>
          <span>Filter: {{ filterSummary }}</span>
        </button>
        <button
          class="create-toggle utility-new"
          type="button"
          aria-label="Add event"
          :aria-expanded="isCreateFormVisible"
          :aria-disabled="!appReady"
          :disabled="!appReady"
          :title="appReady ? 'Add event' : readinessHint"
          @click="toggleCreateForm"
        >
          <span aria-hidden="true">+</span>
        </button>
      </div>
    </header>

    <section v-if="isFilterPanelOpen" class="filter-panel" aria-label="Event filters">
      <label>
        <span>Severity</span>
        <select v-model="filters.severity" aria-label="Filter by severity">
          <option v-for="severity in severityFilters" :key="severity" :value="severity">
            {{ severity }}
          </option>
        </select>
      </label>
      <label>
        <span>Category</span>
        <select v-model="filters.category" aria-label="Filter by category">
          <option v-for="category in categoryFilters" :key="category.value" :value="category.value">
            {{ category.label }}
          </option>
        </select>
      </label>
      <button type="button" class="filter-reset" @click="resetFilters">Reset</button>
    </section>

    <form v-show="isCreateFormVisible" class="create-form" @submit.prevent="createEvent">
      <input
        :value="configuredCallsign"
        type="text"
        placeholder="Configured call sign"
        aria-label="Configured call sign"
        :disabled="!appReady"
        readonly
      />

      <section class="mecp-panel" aria-label="MECP event composer">
        <div class="field-block">
          <span class="field-label">Severity</span>
          <button
            class="menu-control severity-control"
            type="button"
            :aria-label="`Severity ${selectedSeverity.label}`"
            :aria-expanded="isSeverityMenuOpen"
            :disabled="!appReady"
            @click="isSeverityMenuOpen = !isSeverityMenuOpen"
          >
            <span :class="['severity-swatch', `severity-${selectedSeverity.status.toLowerCase()}`]" />
            <span class="menu-copy">
              <strong>{{ selectedSeverity.label }} - {{ selectedSeverity.meaning }}</strong>
            </span>
            <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="m6 9 6 6 6-6" />
            </svg>
          </button>
          <div v-if="isSeverityMenuOpen" class="dropdown severity-menu">
            <button
              v-for="severity in MECP_SEVERITIES"
              :key="severity.value"
              class="dropdown-row"
              type="button"
              @click="selectSeverity(severity.value)"
            >
              <span :class="['severity-swatch', `severity-${severity.status.toLowerCase()}`]" />
              <span>
                <strong>{{ severity.label }} - {{ severity.meaning }}</strong>
              </span>
            </button>
          </div>
        </div>

        <div class="field-block">
          <span class="field-label">Category</span>
          <div
            ref="categoryScroller"
            class="category-scroll"
            aria-label="MECP category selector"
            @pointerdown="startCategoryDrag"
            @pointermove="moveCategoryDrag"
            @pointerup="stopCategoryDrag"
            @pointercancel="stopCategoryDrag"
          >
            <button
              v-for="category in MECP_CATEGORIES"
              :key="category.code"
              :class="['category-card', { selected: category.code === createForm.category }]"
              :data-category="category.code"
              :aria-label="`${category.label} category`"
              :data-selected="category.code === createForm.category"
              type="button"
              @click="selectCategory(category.code)"
            >
              <span class="category-icon">
                <svg v-if="category.icon === 'medical'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 5v14" /><path d="M5 12h14" /><path d="M6 6h12v12H6z" />
                </svg>
                <svg v-else-if="category.icon === 'terrain'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M4 18 9 7l4 7 2-4 5 8" /><path d="M8 18h8" /><path d="M11 12h2" />
                </svg>
                <svg v-else-if="category.icon === 'weather'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M7 17h10a4 4 0 0 0 0-8 6 6 0 0 0-11.6 2" /><path d="M8 20l2-3" /><path d="M14 20l2-3" />
                </svg>
                <svg v-else-if="category.icon === 'supplies'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M5 9h14v10H5z" /><path d="M8 9V6h8v3" /><path d="M12 12v4" /><path d="M10 14h4" />
                </svg>
                <svg v-else-if="category.icon === 'position'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 21s6-5.2 6-11a6 6 0 0 0-12 0c0 5.8 6 11 6 11Z" /><path d="M12 10h.01" />
                </svg>
                <svg v-else-if="category.icon === 'coordination'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M6 8h6" /><path d="M12 8l5 5" /><path d="M7 17h10" /><circle cx="6" cy="8" r="2" /><circle cx="18" cy="14" r="2" />
                </svg>
                <svg v-else-if="category.icon === 'response'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="m5 13 4 4L19 7" /><path d="M4 20h16" />
                </svg>
                <svg v-else-if="category.icon === 'drill'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M5 8h14" /><path d="M7 8v10" /><path d="M17 8v10" /><path d="m8 6 2-2" /><path d="m14 6 2-2" />
                </svg>
                <svg v-else-if="category.icon === 'leisure'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M5 15h14" /><path d="M8 15v4" /><path d="M16 15v4" /><path d="M7 12a5 5 0 0 1 10 0" />
                </svg>
                <svg v-else-if="category.icon === 'threat'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 4 3 20h18L12 4Z" /><path d="M12 9v4" /><path d="M12 17h.01" />
                </svg>
                <svg v-else-if="category.icon === 'resources'" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M7 12 12 7l5 5" /><path d="M8 12v7h8v-7" /><path d="M10 19v-4h4v4" />
                </svg>
                <svg v-else viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="M12 3v4" /><path d="M12 17v4" /><path d="M4 12h4" /><path d="M16 12h4" /><circle cx="12" cy="12" r="4" />
                </svg>
              </span>
              <span class="category-copy">
                <strong>{{ category.label }}</strong>
                <small>{{ category.code }} category</small>
              </span>
            </button>
          </div>
        </div>

        <div class="field-block">
          <span class="field-label">Event</span>
          <button
            class="menu-control"
            type="button"
            :aria-label="`Event ${selectedEvent.code} ${selectedEvent.label}`"
            :aria-expanded="isEventMenuOpen"
            :disabled="!appReady"
            @click="isEventMenuOpen = !isEventMenuOpen"
          >
            <span class="code-badge">{{ selectedEvent.code }}</span>
            <span class="menu-copy">
              <strong>{{ selectedEvent.label }}</strong>
              <small>{{ selectedCategory.label }}</small>
            </span>
            <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="m6 9 6 6 6-6" />
            </svg>
          </button>
          <div v-if="isEventMenuOpen" class="dropdown event-menu">
            <button
              v-for="eventOption in selectedEventOptions"
              :key="eventOption.code"
              class="dropdown-row"
              type="button"
              :aria-label="`${eventOption.code} ${eventOption.label}`"
              @click="selectEvent(eventOption.code)"
            >
              <span class="code-badge">{{ eventOption.code }}</span>
              <span>{{ eventOption.label }}</span>
            </button>
          </div>
        </div>

        <input
          v-model="createForm.details"
          type="text"
          placeholder="Optional details"
          aria-label="Optional details"
          :disabled="!appReady"
        />

        <div class="mecp-preview" aria-label="MECP body preview">
          <span>Body</span>
          <strong>{{ mecpPreview }}</strong>
        </div>
      </section>

      <button type="submit" :disabled="!appReady" :title="appReady ? 'Add event' : readinessHint">
        Add event
      </button>
    </form>

    <section class="timeline">
      <article
        :class="[
          'event',
          {
            'mecp-event': event.mecp,
            [`mecp-event-${event.mecp?.severityStatus}`]: event.mecp,
          },
        ]"
        v-for="event in events"
        :key="event.uid"
      >
        <div class="event-head">
          <div class="event-heading">
            <span
              v-if="event.mecp"
              :class="['mecp-severity-chip', `mecp-severity-${event.mecp.severityStatus}`]"
            >
              {{ event.mecp.severity }}
            </span>
            <p class="event-type">{{ event.type }}</p>
          </div>
          <button
            class="action delete"
            type="button"
            :aria-label="`Delete ${event.callsign}`"
            title="Delete"
            @click="deleteEvent(event.uid)"
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
        <template v-if="event.mecp">
          <h3>{{ event.mecp.codeLabels.join(" + ") || event.summary }}</h3>
          <div v-if="event.mecp.extras.length > 0" class="mecp-extra-list" aria-label="Decoded MECP details">
            <span v-for="extra in event.mecp.extras" :key="extra">{{ extra }}</span>
          </div>
          <p v-if="event.mecp.details" class="mecp-details">{{ event.mecp.details }}</p>
          <p class="mecp-raw">{{ event.mecp.raw }}</p>
          <p v-if="event.mecp.warnings.length > 0" class="mecp-warning">
            {{ event.mecp.warnings.join(" ") }}
          </p>
        </template>
        <template v-else>
          <h3>{{ event.summary }}</h3>
        </template>
        <p class="meta">
          {{ event.callsign }} | {{ new Date(event.updatedAt).toLocaleTimeString() }}
        </p>
      </article>
      <p v-if="events.length === 0" class="empty">
        No events yet. Add one locally or wait for a peer snapshot.
      </p>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.view-header {
  align-items: center;
  display: block;
}

.header-actions {
  align-items: center;
  display: grid;
  gap: 0.8rem;
  grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.35fr) minmax(3.2rem, 0.32fr);
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.4rem, 3vw, 2.4rem);
  line-height: 1;
  margin: 0;
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
}

.utility-chip svg,
.chevron,
.category-icon svg {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.utility-chip svg {
  flex: 0 0 auto;
  height: 1.22rem;
  stroke-width: 1.8;
  width: 1.22rem;
}

.utility-chip span {
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
  border-style: solid;
  cursor: pointer;
}

.filter-panel {
  align-items: end;
  background: rgb(6 18 43 / 82%);
  border: 1px solid rgb(73 173 255 / 36%);
  border-radius: 12px;
  display: grid;
  gap: 0.65rem;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
  padding: 0.72rem;
}

.filter-panel label {
  display: grid;
  gap: 0.28rem;
}

.filter-panel label span {
  color: #8da7cd;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.filter-panel select {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 0.92rem;
  min-height: 2.4rem;
  min-width: 0;
  padding: 0.42rem 0.55rem;
}

.filter-reset {
  background: rgb(8 39 74 / 84%);
  border: 1px solid rgb(92 205 255 / 50%);
  border-radius: 10px;
  color: #8ee6ff;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 2.4rem;
  padding: 0 0.72rem;
  text-transform: uppercase;
}

.create-toggle {
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 0;
  border-radius: 12px;
  color: #032748;
  cursor: pointer;
  font-family: var(--font-headline);
  font-size: 1.5rem;
  font-weight: 700;
  height: 2.3rem;
  line-height: 1;
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

.create-toggle:disabled,
.create-form button:disabled,
.create-form input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.create-form {
  display: grid;
  gap: 0.7rem;
}

.create-form input,
.menu-control {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 1rem;
  min-height: 2.55rem;
  padding: 0.5rem 0.6rem;
}

.create-form > button {
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 0;
  border-radius: 11px;
  color: #032748;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.85rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.mecp-panel {
  display: grid;
  gap: 0.7rem;
}

.field-block {
  display: grid;
  gap: 0.36rem;
  position: relative;
}

.field-label {
  color: #8da7cd;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.11em;
  text-transform: uppercase;
}

.menu-control {
  align-items: center;
  cursor: pointer;
  display: grid;
  gap: 0.62rem;
  grid-template-columns: auto minmax(0, 1fr) auto;
  text-align: left;
  width: 100%;
}

.menu-copy {
  display: grid;
  gap: 0.14rem;
  min-width: 0;
}

.category-copy {
  display: grid;
  gap: 0.16rem;
  min-width: 0;
}

.menu-copy strong,
.category-copy strong {
  color: #e6f8ff;
  font-family: var(--font-body);
  font-size: 0.98rem;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu-copy small,
.category-copy small {
  color: #8ea8d1;
  font-family: var(--font-ui);
  font-size: 0.7rem;
  letter-spacing: 0.05em;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.chevron {
  color: #8fcaff;
  height: 1rem;
  stroke-width: 2;
  width: 1rem;
}

.severity-swatch {
  border-radius: 999px;
  box-shadow: 0 0 14px currentColor;
  height: 1rem;
  width: 1rem;
}

.severity-red {
  background: linear-gradient(120deg, #8f1d28, #ff3648);
  color: rgb(255 54 72 / 36%);
}

.severity-yellow {
  background: linear-gradient(120deg, #a07b00, #f5cc19);
  color: rgb(245 204 25 / 34%);
}

.severity-green {
  background: linear-gradient(120deg, #0f8b5f, #16ce79);
  color: rgb(22 206 121 / 34%);
}

.severity-unknown {
  background: linear-gradient(120deg, #2d3f66, #4f6f9f);
  color: rgb(79 111 159 / 34%);
}

.dropdown {
  background: rgb(4 15 34 / 98%);
  border: 1px solid rgb(73 173 255 / 42%);
  border-radius: 12px;
  box-shadow: 0 14px 30px rgb(0 0 0 / 38%);
  display: grid;
  gap: 0.25rem;
  padding: 0.35rem;
  z-index: 8;
}

.severity-menu {
  position: absolute;
  top: calc(100% + 0.28rem);
  width: 100%;
}

.event-menu {
  max-height: 10.8rem;
  overflow-y: auto;
}

.structured-fields {
  display: grid;
  gap: 0.55rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.dropdown-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 9px;
  color: #d9ecff;
  cursor: pointer;
  display: grid;
  font-family: var(--font-body);
  gap: 0.62rem;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 2.45rem;
  padding: 0.38rem 0.5rem;
  text-align: left;
}

.dropdown-row:hover,
.dropdown-row:focus-visible {
  background: rgb(13 120 195 / 25%);
}

.category-scroll {
  -webkit-overflow-scrolling: touch;
  cursor: grab;
  display: grid;
  gap: 0.48rem;
  max-height: 5.9rem;
  overscroll-behavior-y: contain;
  overflow-y: auto;
  padding: 0.12rem 0.28rem 0.12rem 0;
  scrollbar-color: #37c9ff rgb(7 25 54 / 84%);
  touch-action: pan-y;
  user-select: none;
}

.category-scroll:active {
  cursor: grabbing;
}

.category-card {
  align-items: center;
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 12px;
  color: #91b2df;
  cursor: pointer;
  display: grid;
  gap: 0.72rem;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 4.8rem;
  padding: 0.7rem 0.78rem;
  text-align: left;
}

.category-card.selected {
  background:
    radial-gradient(circle at 18% 22%, rgb(35 159 255 / 20%), transparent 44%),
    rgb(8 22 50 / 92%);
  border-color: rgb(102 219 255 / 78%);
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 20px rgb(40 178 255 / 16%);
  color: #8fe3ff;
}

.category-icon {
  align-items: center;
  background: rgb(5 18 40 / 88%);
  border: 1px solid rgb(93 171 255 / 28%);
  border-radius: 10px;
  display: inline-flex;
  height: 2.5rem;
  justify-content: center;
  width: 2.5rem;
}

.category-icon svg {
  height: 1.35rem;
  stroke-width: 1.75;
  width: 1.35rem;
}

.code-badge {
  align-items: center;
  background: rgb(13 120 195 / 26%);
  border: 1px solid rgb(102 219 255 / 42%);
  border-radius: 8px;
  color: #8fe3ff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  justify-content: center;
  min-width: 3.05rem;
  padding: 0.32rem 0.45rem;
}

.mecp-preview {
  align-items: center;
  background: rgb(4 17 39 / 86%);
  border: 1px solid rgb(43 217 178 / 34%);
  border-radius: 10px;
  color: #7af4d3;
  display: flex;
  gap: 0.6rem;
  justify-content: space-between;
  min-width: 0;
  padding: 0.58rem 0.68rem;
}

.mecp-preview span {
  color: #8da7cd;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.11em;
  text-transform: uppercase;
}

.mecp-preview strong,
.mecp-raw {
  font-family: var(--font-ui);
  letter-spacing: 0.04em;
}

.timeline {
  display: grid;
  gap: 0.8rem;
}

.event {
  background:
    radial-gradient(circle at 18% 20%, rgb(33 115 255 / 17%), transparent 46%),
    linear-gradient(130deg, rgb(13 32 65 / 92%), rgb(9 19 43 / 90%));
  border: 1px solid rgb(73 112 170 / 28%);
  border-radius: 14px;
  padding: 0.8rem 1rem;
}

.mecp-event {
  border-color: rgb(43 217 178 / 26%);
  position: relative;
}

.mecp-event::before {
  border-radius: 14px 0 0 14px;
  content: "";
  inset: -1px auto -1px -1px;
  position: absolute;
  width: 4px;
}

.mecp-event-red {
  border-color: rgb(255 54 72 / 42%);
  box-shadow: inset 0 0 0 1px rgb(255 54 72 / 9%);
}

.mecp-event-red::before {
  background: linear-gradient(180deg, #8f1d28, #ff3648);
}

.mecp-event-yellow {
  border-color: rgb(245 204 25 / 42%);
  box-shadow: inset 0 0 0 1px rgb(245 204 25 / 9%);
}

.mecp-event-yellow::before {
  background: linear-gradient(180deg, #a07b00, #f5cc19);
}

.mecp-event-green {
  border-color: rgb(22 206 121 / 38%);
  box-shadow: inset 0 0 0 1px rgb(22 206 121 / 8%);
}

.mecp-event-green::before {
  background: linear-gradient(180deg, #0f8b5f, #16ce79);
}

.mecp-event-unknown {
  border-color: rgb(79 111 159 / 38%);
  box-shadow: inset 0 0 0 1px rgb(79 111 159 / 8%);
}

.mecp-event-unknown::before {
  background: linear-gradient(180deg, #2d3f66, #4f6f9f);
}

.event-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.event-heading {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.42rem;
  min-width: 0;
}

.event-type {
  color: #74beff;
  font-family: var(--font-ui);
  font-size: 0.76rem;
  font-weight: 700;
  letter-spacing: 0.13em;
  margin: 0;
  text-transform: uppercase;
}

.mecp-severity-chip {
  border: 1px solid rgb(73 173 255 / 38%);
  border-radius: 999px;
  box-shadow: inset 0 1px 0 rgb(186 236 255 / 6%);
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  line-height: 1;
  padding: 0.24rem 0.52rem 0.27rem;
  text-transform: uppercase;
}

.mecp-severity-red {
  background: rgb(53 15 25 / 70%);
  border-color: rgb(255 100 117 / 48%);
  color: #ff8190;
}

.mecp-severity-yellow {
  background: rgb(82 56 5 / 82%);
  border-color: rgb(255 196 76 / 65%);
  color: #ffd36e;
}

.mecp-severity-green {
  background: rgb(14 67 42 / 82%);
  border-color: rgb(71 214 145 / 40%);
  color: #8df3c1;
}

.mecp-severity-unknown {
  background: rgb(35 46 76 / 82%);
  border-color: rgb(126 166 220 / 24%);
  color: #b5c7e9;
}

h3 {
  font-family: var(--font-body);
  font-size: 1.06rem;
  margin: 0.26rem 0 0;
}

.mecp-details,
.meta {
  color: #8da7cd;
  font-family: var(--font-body);
  margin: 0.3rem 0 0;
}

.mecp-extra-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.36rem;
  margin-top: 0.48rem;
}

.mecp-extra-list span {
  background: rgb(122 244 211 / 8%);
  border: 1px solid rgb(122 244 211 / 22%);
  border-radius: 6px;
  color: #aeeedf;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  padding: 0.18rem 0.36rem;
  text-transform: uppercase;
}

.mecp-raw {
  color: #7af4d3;
  margin: 0.38rem 0 0;
}

.mecp-warning {
  color: #ffd66e;
  font-family: var(--font-body);
  font-size: 0.74rem;
  margin: 0.3rem 0 0;
}

.action {
  align-items: center;
  border: 0;
  border-radius: 10px;
  cursor: pointer;
  display: inline-flex;
  flex-shrink: 0;
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

.empty {
  color: #8da7cd;
  font-family: var(--font-body);
  margin: 0;
}

@media (min-width: 981px) {
  .create-form {
    align-items: start;
    grid-template-columns: minmax(150px, 190px) minmax(0, 1fr) auto;
  }

  .create-form > button {
    min-height: 2.55rem;
  }
}

@media (max-width: 720px) {
  h1 {
    font-size: 1.1rem;
  }

  .view-header {
    align-items: stretch;
  }

  .header-actions {
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

  .filter-panel {
    align-items: stretch;
    grid-template-columns: 1fr;
  }
}
</style>
