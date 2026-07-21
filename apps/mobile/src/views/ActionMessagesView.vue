<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";

import ActionMessageList from "../components/ActionMessageList.vue";
import ActionMessageTable from "../components/ActionMessageTable.vue";
import ListWindowControls from "../components/ListWindowControls.vue";
import { useListWindow } from "../composables/useListWindow";
import type { ActionMessage } from "../types/domain";
import { useMessagesStore } from "../stores/messagesStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  ACTION_MESSAGE_STATUS_CONFIG,
  type ActionMessageStatusField,
} from "../utils/actionMessageStatus";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";
import {
  DEFAULT_R3AKT_TEAM_COLOR,
  R3AKT_TEAM_COLORS,
  type R3aktTeamColor,
  formatR3aktTeamColorLabel,
  normalizeR3aktTeamColor,
} from "../utils/r3akt";

const TEAM_COLOR_FILTER_ALL = "ALL";
type TeamColorFilter = typeof TEAM_COLOR_FILTER_ALL | R3aktTeamColor;

const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();
const route = useRoute();

const teamColorFilterOptions: Array<{ value: TeamColorFilter; label: string }> = [
  { value: TEAM_COLOR_FILTER_ALL, label: "All teams" },
  ...R3AKT_TEAM_COLORS.map((value) => ({
    value,
    label: formatR3aktTeamColorLabel(value),
  })),
];
const statusOptions = [
  { value: "Unknown", label: "Unknown" },
  { value: "Green", label: "Green" },
  { value: "Yellow", label: "Yellow" },
  { value: "Red", label: "Red" },
] as const;
const defaultCallSign = computed(() => nodeStore.settings.displayName.trim());
const activeTeamColor = computed<R3aktTeamColor>(() => {
  const activeTeamUid = nodeStore.hubDirectorySnapshot?.activeTeamUid
    || nodeStore.settings.teams.activeTeamUid;
  const team = nodeStore.hubDirectorySnapshot?.teams.find((entry) => entry.uid === activeTeamUid);
  return normalizeR3aktTeamColor(team?.color, DEFAULT_R3AKT_TEAM_COLOR);
});
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
const selectedTeamColorFilter = shallowRef<TeamColorFilter>(TEAM_COLOR_FILTER_ALL);

const messages = computed(() => messagesStore.messages);
const filteredMessages = computed(() => {
  if (selectedTeamColorFilter.value === TEAM_COLOR_FILTER_ALL) {
    return messages.value;
  }
  return messages.value.filter((message) =>
    normalizeR3aktTeamColor(message.groupName) === selectedTeamColorFilter.value,
  );
});
const messageWindow = useListWindow(filteredMessages, { resetKey: selectedTeamColorFilter });
const messageCountLabel = computed(() =>
  selectedTeamColorFilter.value === TEAM_COLOR_FILTER_ALL
    ? `${messagesStore.activeCount} MSG`
    : `${filteredMessages.value.length}/${messagesStore.activeCount} MSG`,
);
const filterStatusLabel = computed(() =>
  selectedTeamColorFilter.value === TEAM_COLOR_FILTER_ALL
    ? "All"
    : formatR3aktTeamColorLabel(selectedTeamColorFilter.value),
);
const selectedCallsign = computed(() => {
  const value = route.query.callsign;
  return Array.isArray(value) ? (value[0]?.trim() ?? "") : (value?.trim() ?? "");
});
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
  createForm.groupName = activeTeamColor.value;
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
  const normalizedGroupName = activeTeamColor.value;
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
  createForm.groupName = activeTeamColor.value;
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
  if (!message) {
    return;
  }
  runDetachedStoreTask(nodeStore, "eam", `delete ${callsign}`, () =>
    messagesStore.deleteLocal(callsign));
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
          <span>{{ messageCountLabel }}</span>
        </span>
        <label
          class="utility-chip filter-chip"
          for="team-color-filter"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />
          </svg>
          <span>Team: {{ filterStatusLabel }}</span>
          <select
            id="team-color-filter"
            v-model="selectedTeamColorFilter"
            aria-label="Team color filter"
          >
            <option
              v-for="option in teamColorFilterOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
        </label>
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
        <output class="active-team-output" aria-label="Active team">
          {{ formatR3aktTeamColorLabel(activeTeamColor) }} team
        </output>
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
        :messages="messageWindow.items.value"
        :editable-callsigns="editableCallsigns"
        :selected-callsign="selectedCallsign"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="cycleMessage"
      />
    </div>
    <div class="mobile-only">
      <ActionMessageList
        :messages="messageWindow.items.value"
        :editable-callsigns="editableCallsigns"
        :selected-callsign="selectedCallsign"
        @edit="editMessage"
        @delete="deleteMessage"
        @cycle="cycleMessage"
      />
    </div>
    <ListWindowControls
      :start="messageWindow.startIndex.value"
      :end="messageWindow.endIndex.value"
      :total="messageWindow.total.value"
      :has-previous="messageWindow.hasPrevious.value"
      :has-next="messageWindow.hasNext.value"
      @previous="messageWindow.previous"
      @next="messageWindow.next"
    />
  </section>
</template>

<style scoped src="./ActionMessagesView.css"></style>
