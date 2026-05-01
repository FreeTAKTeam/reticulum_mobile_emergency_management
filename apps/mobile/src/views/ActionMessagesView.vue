<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";

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
const draftModeActive = computed(
  () => nodeStore.settings.hub.mode !== "Autonomous" && !nodeStore.hubRegistrationReady,
);
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
const editableCallsigns = computed(() =>
  messages.value
    .filter((message) => messagesStore.canManageMessage(message))
    .map((message) => message.callsign),
);
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
  if (!message || !messagesStore.canManageMessage(message)) {
    return;
  }
  createForm.callsign = message.callsign;
  createForm.groupName = normalizeR3aktTeamColor(message.groupName);
  copyMessageStatuses(message);
  editingCallsign.value = message.callsign;
  isCreateFormVisible.value = true;
}

function cycleMessage(callsign: string, field: keyof ActionMessage | string): void {
  const message = messages.value.find((item) => item.callsign === callsign);
  if (!message || !messagesStore.canManageMessage(message)) {
    return;
  }
  messagesStore.rotateStatus(callsign, field as keyof ActionMessage);
}

function deleteMessage(callsign: string): void {
  const message = messages.value.find((item) => item.callsign === callsign);
  if (!message || !messagesStore.canManageMessage(message)) {
    return;
  }
  messagesStore.deleteLocal(callsign).catch(() => undefined);
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
          <span>{{ messagesStore.activeCount }} MSG</span>
        </span>
        <span
          class="utility-chip filter-chip"
          aria-label="Action message filter status"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />
          </svg>
          <span>Filter: All</span>
        </span>
        <RouterLink
          class="utility-chip help-trigger"
          to="/messages/help"
          aria-label="Open status color help"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle cx="12" cy="12" r="9" />
            <path d="M9.75 9a2.25 2.25 0 0 1 4.13 1.25c0 1.5-1.88 1.88-1.88 3.25" />
            <path d="M12 17h.01" />
          </svg>
          <span>Status Help</span>
        </RouterLink>
        <button
          class="create-toggle utility-new"
          type="button"
          aria-label="Add message"
          :aria-expanded="isCreateFormVisible"
          :aria-disabled="!canManageMessages"
          :disabled="!canManageMessages"
          :title="canManageMessages ? 'Add message' : localSaveHint"
          @click="toggleCreateForm"
        >
          <span aria-hidden="true">+</span>
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
        :editable-callsigns="editableCallsigns"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="cycleMessage"
      />
    </div>
    <div class="mobile-only">
      <ActionMessageList
        :messages="messages"
        :editable-callsigns="editableCallsigns"
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
  display: block;
}

.header-actions {
  align-items: center;
  display: grid;
  gap: 0.8rem;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.15fr) minmax(0, 1fr) minmax(3.2rem, 0.32fr);
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
  font-size: clamp(0.82rem, 2.1vw, 1rem);
  font-weight: 700;
  justify-content: center;
  line-height: 1;
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
    align-items: stretch;
  }

  .header-actions {
    gap: 0.55rem;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.18fr) minmax(2.8rem, 0.42fr) minmax(2.8rem, 0.35fr);
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

  .help-trigger span {
    display: none;
  }
}
</style>
