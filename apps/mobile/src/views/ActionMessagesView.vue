<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useRouter } from "vue-router";

import ActionMessageList from "../components/ActionMessageList.vue";
import ActionMessageTable from "../components/ActionMessageTable.vue";
import type { ActionMessage } from "../types/domain";
import { useMessagesStore } from "../stores/messagesStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  ACTION_MESSAGE_STATUS_CONFIG,
  type ActionMessageStatusField,
} from "../utils/actionMessageStatus";
import {
  DEFAULT_R3AKT_TEAM_COLOR,
  R3AKT_TEAM_COLORS,
  formatR3aktTeamColorLabel,
  normalizeR3aktTeamColor,
} from "../utils/r3akt";

const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();
const router = useRouter();

const teamColorOptions = R3AKT_TEAM_COLORS.map((value) => ({
  value,
  label: formatR3aktTeamColorLabel(value),
}));
const statusOptions = [
  { value: "Unknown", label: "Unknown" },
  { value: "Green", label: "Green" },
  { value: "Yellow", label: "Yellow" },
  { value: "Red", label: "Red" },
] as const;
const defaultCallSign = computed(() => nodeStore.settings.displayName.trim());
const appReady = computed(() => nodeStore.ready);
const draftModeActive = computed(() => nodeStore.settings.hub.mode !== "Disabled" && !nodeStore.hubRegistrationReady);
const canManageMessages = computed(() => true);
const localSaveHint = computed(() =>
  draftModeActive.value
    ? "Hub registration is still pending. Messages are saved locally and replay automatically once registration completes."
    : "Node is not ready yet. Message changes are saved locally and sync automatically once the node is ready.",
);
const showLocalSaveBanner = computed(() => draftModeActive.value || !appReady.value);

const createForm = reactive({
  callsign: defaultCallSign.value,
  groupName: DEFAULT_R3AKT_TEAM_COLOR,
  securityStatus: "Unknown" as ActionMessage["securityStatus"],
  capabilityStatus: "Unknown" as ActionMessage["capabilityStatus"],
  preparednessStatus: "Unknown" as ActionMessage["preparednessStatus"],
  medicalStatus: "Unknown" as ActionMessage["medicalStatus"],
  mobilityStatus: "Unknown" as ActionMessage["mobilityStatus"],
  commsStatus: "Unknown" as ActionMessage["commsStatus"],
});
const isCreateFormVisible = shallowRef(false);
const editingCallsign = shallowRef<string | null>(null);

const messages = computed(() => messagesStore.messages);
const submitLabel = computed(() => (editingCallsign.value ? "Save message" : "Add message"));
const submitTitle = computed(() => (editingCallsign.value ? "Save message" : "Add message"));

watch(defaultCallSign, (next, previous) => {
  if (editingCallsign.value) {
    return;
  }
  const current = createForm.callsign.trim();
  if (!current || current === previous) {
    createForm.callsign = next;
  }
});

function resetCreateForm(): void {
  createForm.callsign = defaultCallSign.value;
  createForm.groupName = DEFAULT_R3AKT_TEAM_COLOR;
  createForm.securityStatus = "Unknown";
  createForm.capabilityStatus = "Unknown";
  createForm.preparednessStatus = "Unknown";
  createForm.medicalStatus = "Unknown";
  createForm.mobilityStatus = "Unknown";
  createForm.commsStatus = "Unknown";
  editingCallsign.value = null;
}

function toggleCreateForm(): void {
  if (isCreateFormVisible.value) {
    resetCreateForm();
  }
  isCreateFormVisible.value = !isCreateFormVisible.value;
}

function openHelp(): void {
  router.push("/messages/help").catch(() => undefined);
}

function copyMessageStatuses(message: Pick<ActionMessage, ActionMessageStatusField>): void {
  createForm.securityStatus = message.securityStatus;
  createForm.capabilityStatus = message.capabilityStatus;
  createForm.preparednessStatus = message.preparednessStatus;
  createForm.medicalStatus = message.medicalStatus;
  createForm.mobilityStatus = message.mobilityStatus;
  createForm.commsStatus = message.commsStatus;
}

async function createMessage(): Promise<void> {
  const callsign = createForm.callsign.trim() || defaultCallSign.value;
  if (!callsign) {
    return;
  }
  const normalizedGroupName = normalizeR3aktTeamColor(
    createForm.groupName,
    DEFAULT_R3AKT_TEAM_COLOR,
  );
  const originalCallsign = editingCallsign.value;
  const existing = originalCallsign
    ? messages.value.find((message) => message.callsign === originalCallsign)
    : undefined;

  await messagesStore.upsertLocal(
    existing
      ? {
          ...existing,
          callsign,
          groupName: normalizedGroupName,
          securityStatus: createForm.securityStatus,
          capabilityStatus: createForm.capabilityStatus,
          preparednessStatus: createForm.preparednessStatus,
          medicalStatus: createForm.medicalStatus,
          mobilityStatus: createForm.mobilityStatus,
          commsStatus: createForm.commsStatus,
        }
      : {
          callsign,
          groupName: normalizedGroupName,
          securityStatus: createForm.securityStatus,
          capabilityStatus: createForm.capabilityStatus,
          preparednessStatus: createForm.preparednessStatus,
          medicalStatus: createForm.medicalStatus,
          mobilityStatus: createForm.mobilityStatus,
          commsStatus: createForm.commsStatus,
        },
  );
  if (existing && originalCallsign && originalCallsign !== callsign) {
    await messagesStore.deleteLocal(originalCallsign);
  }
  resetCreateForm();
  isCreateFormVisible.value = false;
}

function editMessage(callsign: string): void {
  const message = messages.value.find((item) => item.callsign === callsign);
  if (!message) {
    return;
  }
  createForm.callsign = message.callsign;
  createForm.groupName = normalizeR3aktTeamColor(message.groupName);
  copyMessageStatuses(message);
  editingCallsign.value = message.callsign;
  isCreateFormVisible.value = true;
}

function cycleMessage(callsign: string, field: keyof ActionMessage | string): void {
  messagesStore.rotateStatus(callsign, field as keyof ActionMessage);
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
        <p>Status updates from field members.</p>
      </div>
      <div class="header-actions">
        <span class="badge"># {{ messagesStore.activeCount }} MSG</span>
        <span v-if="messagesStore.draftCount > 0" class="badge badge-warning">
          {{ messagesStore.draftCount }} Draft
        </span>
        <button
          class="help-trigger"
          type="button"
          aria-label="Open status color help"
          @click="openHelp"
        >
          ?
        </button>
        <button
          class="create-toggle"
          type="button"
          aria-label="Add message"
          :aria-expanded="isCreateFormVisible"
          :aria-disabled="!canManageMessages"
          :disabled="!canManageMessages"
          :title="canManageMessages ? 'Add message' : localSaveHint"
          @click="toggleCreateForm"
        >
          +
        </button>
      </div>
    </header>

    <p v-if="showLocalSaveBanner" class="sync-banner">
      <template v-if="draftModeActive">
        {{ nodeStore.hubRegistrationSummary }} Pending drafts replay automatically in creation order.
      </template>
      <template v-else>
        {{ localSaveHint }}
      </template>
    </p>

    <form v-show="isCreateFormVisible" class="create-form" @submit.prevent="createMessage">
      <div class="create-form-top">
        <input
          v-model="createForm.callsign"
          type="text"
          placeholder="Call Sign"
          aria-label="Call Sign"
          :disabled="!canManageMessages"
        />
        <select
          v-model="createForm.groupName"
          aria-label="Team color"
          :disabled="!canManageMessages"
        >
          <option v-for="option in teamColorOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select>
        <button
          type="submit"
          :disabled="!canManageMessages"
          :title="canManageMessages ? submitTitle : localSaveHint"
        >
          {{ submitLabel }}
        </button>
      </div>

      <div class="status-edit-grid">
        <label
          v-for="status in ACTION_MESSAGE_STATUS_CONFIG"
          :key="status.field"
          class="status-edit-field"
        >
          <span class="status-edit-label">{{ status.label }}</span>
          <select
            v-model="createForm[status.field]"
            :aria-label="`${status.label} status`"
            :disabled="!canManageMessages"
          >
            <option v-for="option in statusOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>
      </div>
    </form>

    <div class="desktop-only">
      <ActionMessageTable
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="cycleMessage"
      />
    </div>
    <div class="mobile-only">
      <ActionMessageList
        :messages="messages"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="cycleMessage"
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

.badge-warning {
  background: rgb(82 56 5 / 82%);
  border-color: rgb(255 196 76 / 65%);
  color: #ffd36e;
}

.sync-banner {
  background: rgb(34 45 77 / 62%);
  border: 1px solid rgb(105 141 214 / 35%);
  border-radius: 12px;
  color: #bbd3ff;
  font-family: var(--font-body);
  margin: 0;
  padding: 0.8rem 0.95rem;
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
.create-form input:disabled,
.create-form select:disabled {
  cursor: not-allowed;
  opacity: 0.55;
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
  min-width: 2.3rem;
  padding: 0;
}

.help-trigger:hover,
.help-trigger:focus-visible {
  border-color: rgb(102 219 255 / 76%);
  box-shadow: 0 0 0 1px rgb(9 55 95 / 75%), 0 0 20px rgb(40 178 255 / 18%);
  color: #d8f8ff;
}

.create-form {
  display: grid;
  gap: 0.6rem;
}

.create-form-top {
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

.status-edit-grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.status-edit-field {
  display: grid;
  gap: 0.28rem;
}

.status-edit-label {
  color: #9cb3d6;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.08em;
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
    gap: 0.75rem;
  }

  .create-form-top {
    grid-template-columns: 1fr;
  }

  .status-edit-grid {
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
