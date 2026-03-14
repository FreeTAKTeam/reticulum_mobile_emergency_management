<script setup lang="ts">
import { computed, reactive, shallowRef } from "vue";

import { useEventsStore } from "../stores/eventsStore";
import { useNodeStore } from "../stores/nodeStore";

const eventsStore = useEventsStore();
const nodeStore = useNodeStore();
eventsStore.init();
eventsStore.initReplication();

const events = computed(() => eventsStore.records);
const appReady = computed(() => nodeStore.ready);
const isCreateFormVisible = shallowRef(false);
const configuredCallsign = computed(() => nodeStore.settings.displayName.trim() || "Unset");
const readinessHint = "Node is not ready yet. Wait for the top-right status to show Ready.";

const createForm = reactive({
  type: "Incident",
  summary: "",
});

function ensureReady(action: string): boolean {
  try {
    nodeStore.assertReadyForOutbound(action);
    return true;
  } catch {
    return false;
  }
}

function toggleCreateForm(): void {
  if (!isCreateFormVisible.value && !ensureReady("send events")) {
    return;
  }
  isCreateFormVisible.value = !isCreateFormVisible.value;
}

async function createEvent(): Promise<void> {
  if (!ensureReady("send events")) {
    return;
  }
  if (!createForm.summary.trim() || configuredCallsign.value === "Unset") {
    return;
  }
  try {
    await eventsStore.upsertLocal({
      type: createForm.type.trim() || "Incident",
      summary: createForm.summary.trim(),
    });
    createForm.summary = "";
    isCreateFormVisible.value = false;
  } catch (error: unknown) {
    nodeStore.setLastError(error instanceof Error ? error.message : String(error));
  }
}

async function deleteEvent(uid: string): Promise<void> {
  await eventsStore.deleteLocal(uid);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1>Events</h1>
        <p>Live replicated incident feed across connected peers.</p>
      </div>
      <div class="header-actions">
        <span class="badge"># {{ events.length }} EVT</span>
        <button
          class="create-toggle"
          type="button"
          aria-label="Add event"
          :aria-expanded="isCreateFormVisible"
          :aria-disabled="!appReady"
          :disabled="!appReady"
          :title="appReady ? 'Add event' : readinessHint"
          @click="toggleCreateForm"
        >
          +
        </button>
      </div>
    </header>

    <form v-show="isCreateFormVisible" class="create-form" @submit.prevent="createEvent">
      <input
        :value="configuredCallsign"
        type="text"
        placeholder="Configured call sign"
        aria-label="Configured call sign"
        :disabled="!appReady"
        readonly
      />
      <input
        v-model="createForm.type"
        type="text"
        placeholder="Type"
        aria-label="Type"
        :disabled="!appReady"
      />
      <input
        v-model="createForm.summary"
        type="text"
        placeholder="Event summary"
        aria-label="Event summary"
        :disabled="!appReady"
      />
      <button type="submit" :disabled="!appReady" :title="appReady ? 'Add event' : readinessHint">
        Add event
      </button>
    </form>

    <section class="timeline">
      <article class="event" v-for="event in events" :key="event.uid">
        <div class="event-head">
          <p class="event-type">{{ event.type }}</p>
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
        <h3>{{ event.summary }}</h3>
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
  display: flex;
  justify-content: space-between;
}

.header-actions {
  align-items: center;
  display: flex;
  gap: 0.55rem;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.4rem, 3vw, 2.4rem);
  line-height: 1;
  margin: 0;
}

.view-header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(1rem, 1.6vw, 1.3rem);
  margin: 0.2rem 0 0;
}

.badge {
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  color: #64beff;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  letter-spacing: 0.08em;
  padding: 0.46rem 0.8rem;
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

.create-toggle:disabled,
.create-form button:disabled,
.create-form input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.create-form {
  align-items: center;
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(150px, 190px) minmax(120px, 160px) 1fr auto;
}

.create-form input {
  background: rgb(8 22 50 / 82%);
  border: 1px solid rgb(75 118 185 / 44%);
  border-radius: 10px;
  color: #d1e9ff;
  font-family: var(--font-body);
  font-size: 1rem;
  padding: 0.5rem 0.6rem;
}

.create-form button {
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

.event-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
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

h3 {
  font-family: var(--font-body);
  font-size: 1.06rem;
  margin: 0.26rem 0 0;
}

.meta {
  color: #8da7cd;
  font-family: var(--font-body);
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

@media (max-width: 980px) {
  .create-form {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 720px) {
  h1 {
    font-size: 1.1rem;
  }

  .view-header {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.65rem;
  }

  .header-actions {
    align-self: stretch;
    justify-content: flex-end;
  }
}
</style>
