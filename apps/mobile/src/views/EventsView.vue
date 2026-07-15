<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { RouterLink } from "vue-router";

import MecpCategorySelector from "../components/events/MecpCategorySelector.vue";
import ListWindowControls from "../components/ListWindowControls.vue";
import { useListWindow } from "../composables/useListWindow";
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
const isCreatingEvent = shallowRef(false);
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
const eventWindow = useListWindow(events, {
  resetKey: () => `${filters.severity}:${filters.category}`,
});

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

function toggleCreateForm(): void {
  if (!isCreateFormVisible.value && !ensureReady("send events")) {
    return;
  }
  isCreateFormVisible.value = !isCreateFormVisible.value;
}

function resetFilters(): void {
  filters.severity = "All";
  filters.category = "All";
}

function selectSeverity(severity: MecpSeverity): void {
  createForm.severity = severity;
  isSeverityMenuOpen.value = false;
}

function selectEvent(code: string): void {
  createForm.eventCode = code;
  isEventMenuOpen.value = false;
}

async function createEvent(): Promise<void> {
  if (isCreatingEvent.value) {
    return;
  }
  if (!ensureReady("send events")) {
    return;
  }
  if (configuredCallsign.value === "Unset") {
    nodeStore.logUi("Warn", "[events] create blocked: callsign=Unset");
    return;
  }
  isCreatingEvent.value = true;
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
  } finally {
    isCreatingEvent.value = false;
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
        <RouterLink
          class="utility-chip help-trigger"
          to="/events/help"
          aria-label="Open MECP event help"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle cx="12" cy="12" r="9" />
            <path d="M9.75 9a2.25 2.25 0 0 1 4.13 1.25c0 1.5-1.88 1.88-1.88 3.25" />
            <path d="M12 17h.01" />
          </svg>
          <span>MECP Help</span>
        </RouterLink>
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

        <MecpCategorySelector
          v-model="createForm.category"
          :active="isCreateFormVisible"
        />

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

      <button type="submit" :disabled="!appReady || isCreatingEvent" :title="appReady ? 'Add event' : readinessHint">
        {{ isCreatingEvent ? "Adding event..." : "Add event" }}
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
        v-for="event in eventWindow.items.value"
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
      <ListWindowControls
        :start="eventWindow.startIndex.value"
        :end="eventWindow.endIndex.value"
        :total="eventWindow.total.value"
        :has-previous="eventWindow.hasPrevious.value"
        :has-next="eventWindow.hasNext.value"
        @previous="eventWindow.previous"
        @next="eventWindow.next"
      />
    </section>
  </section>
</template>

<style scoped src="./EventsView.css"></style>
<style scoped src="./EventsTimeline.css"></style>
