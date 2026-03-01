<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";

import ActionMessageList from "../components/ActionMessageList.vue";
import ActionMessageTable from "../components/ActionMessageTable.vue";
import { useMessagesStore } from "../stores/messagesStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  DEFAULT_R3AKT_TEAM_COLOR,
  R3AKT_TEAM_COLORS,
  formatR3aktTeamColorLabel,
  normalizeR3aktTeamColor,
} from "../utils/r3akt";

const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();

messagesStore.init();
messagesStore.initReplication();

const teamColorOptions = R3AKT_TEAM_COLORS.map((value) => ({
  value,
  label: formatR3aktTeamColorLabel(value),
}));
const teamColorPrompt = R3AKT_TEAM_COLORS.join(", ");
const defaultCallSign = computed(() => nodeStore.settings.displayName.trim());

const createForm = reactive({
  callsign: defaultCallSign.value,
  groupName: DEFAULT_R3AKT_TEAM_COLOR,
});
const isCreateFormVisible = shallowRef(false);

const messages = computed(() => messagesStore.messages);

watch(defaultCallSign, (next, previous) => {
  const current = createForm.callsign.trim();
  if (!current || current === previous) {
    createForm.callsign = next;
  }
});

function resetCreateForm(): void {
  createForm.callsign = defaultCallSign.value;
  createForm.groupName = DEFAULT_R3AKT_TEAM_COLOR;
}

function toggleCreateForm(): void {
  isCreateFormVisible.value = !isCreateFormVisible.value;
}

async function createMessage(): Promise<void> {
  const callsign = createForm.callsign.trim() || defaultCallSign.value;
  if (!callsign) {
    return;
  }
  await messagesStore.upsertLocal({
    callsign,
    groupName: normalizeR3aktTeamColor(createForm.groupName, DEFAULT_R3AKT_TEAM_COLOR),
    securityStatus: "Unknown",
    capabilityStatus: "Unknown",
    preparednessStatus: "Unknown",
    medicalStatus: "Unknown",
    mobilityStatus: "Unknown",
    commsStatus: "Unknown",
  });
  resetCreateForm();
  isCreateFormVisible.value = false;
}

function editMessage(callsign: string): void {
  const message = messages.value.find((item) => item.callsign === callsign);
  if (!message) {
    return;
  }
  const nextGroup = window.prompt(
    `Update team color (${teamColorPrompt})`,
    normalizeR3aktTeamColor(message.groupName),
  );
  if (nextGroup === null) {
    return;
  }
  messagesStore
    .upsertLocal({
      ...message,
      groupName: normalizeR3aktTeamColor(nextGroup, normalizeR3aktTeamColor(message.groupName)),
    })
    .catch(() => undefined);
}

function deleteMessage(callsign: string): void {
  messagesStore.deleteLocal(callsign).catch(() => undefined);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1>Emergency Action Messages</h1>
        <p>Monitor status updates from field teams and dispatch support.</p>
      </div>
      <div class="header-actions">
        <span class="badge"># {{ messagesStore.activeCount }} MSG</span>
        <button
          class="create-toggle"
          type="button"
          aria-label="Add message"
          :aria-expanded="isCreateFormVisible"
          @click="toggleCreateForm"
        >
          +
        </button>
      </div>
    </header>

    <form v-show="isCreateFormVisible" class="create-form" @submit.prevent="createMessage">
      <input
        v-model="createForm.callsign"
        type="text"
        placeholder="Call Sign"
        aria-label="Call Sign"
      />
      <select
        v-model="createForm.groupName"
        aria-label="Team color"
      >
        <option v-for="option in teamColorOptions" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <button type="submit">Add message</button>
    </form>

    <div class="desktop-only">
      <ActionMessageTable
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="messagesStore.rotateStatus"
      />
    </div>
    <div class="mobile-only">
      <ActionMessageList
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="messagesStore.rotateStatus"
      />
    </div>
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

p {
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

.create-form {
  align-items: center;
  display: grid;
  gap: 0.6rem;
  grid-template-columns: minmax(140px, 200px) minmax(160px, 220px) auto;
}

.create-form input,
.create-form select {
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
  font-size: 0.92rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.mobile-only {
  display: none;
}

@media (max-width: 980px) {
  .desktop-only {
    display: none;
  }

  .mobile-only {
    display: block;
  }

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
}
</style>
