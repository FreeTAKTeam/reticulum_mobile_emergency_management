<script setup lang="ts">
import { computed, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { routingSlice, type RoutingDraft } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const form = useNodeSettingsForm(routingSlice);
const { draft } = form;
const actionFeedback = ref("");
const actionPending = ref(false);

const modes: Array<{ value: RoutingDraft["hubMode"]; label: string; description: string }> = [
  { value: "Autonomous", label: "Autonomous", description: "Find peers locally. No hub needed." },
  {
    value: "SemiAutonomous",
    label: "Semi-autonomous",
    description: "Use the hub's team directory, send directly. Falls back to local discovery.",
  },
  { value: "Connected", label: "Connected", description: "Send everything through the selected hub." },
];

const usesHub = computed(() => draft.hubMode !== "Autonomous");
const announceCandidates = computed(() => nodeStore.hubAnnounceCandidates);
const activePropagationNodeHex = computed(() => nodeStore.syncStatus.activePropagationNodeHex?.trim() ?? "");
const activePropagationNodeLabel = computed(() => {
  const hex = activePropagationNodeHex.value;
  if (!hex) return "None";
  const discovered = nodeStore.discoveredByDestination[hex];
  if (discovered) return discovered.announcedName || discovered.label || hex;
  return nodeStore.savedByDestination[hex]?.label || hex;
});
const propagationSelection = computed(() => {
  if (!activePropagationNodeHex.value) return "No propagation relay has been announced yet.";
  return nodeStore.bestPropagationNodeHex === activePropagationNodeHex.value
    ? "Auto-selected from announced hub-capable relays."
    : "Synced from runtime state.";
});
const directoryStatus = computed(() => {
  const override = draft.hubMode === "SemiAutonomous" && nodeStore.effectiveConnectedMode
    ? " · server forcing connected routing"
    : "";
  if (!draft.hubIdentityHash.trim()) {
    if (draft.hubMode === "Connected") return `No hub selected · outbound blocked${override}`;
    if (draft.hubMode === "SemiAutonomous") return `No hub selected · using local discovery${override}`;
    return `Local discovery${override}`;
  }
  return `${nodeStore.hubDirectoryPeers.length} cached peers${override}`;
});

function onCandidateSelected(event: Event): void {
  draft.hubIdentityHash = (event.target as HTMLSelectElement).value.trim();
}

async function runAction(action: () => Promise<void>, success: string): Promise<void> {
  if (actionPending.value) return;
  actionPending.value = true;
  actionFeedback.value = "";
  try {
    await action();
    actionFeedback.value = success;
  } catch (error: unknown) {
    actionFeedback.value = error instanceof Error ? error.message : String(error);
  } finally {
    actionPending.value = false;
  }
}
</script>

<template>
  <SettingsPage title="Routing & hubs" description="Whether messages travel peer to peer or through an RCH hub.">
    <section class="panel settings-card">
      <fieldset class="mode-options">
        <legend>Mode <span class="restart-badge">restart</span></legend>
        <label v-for="mode in modes" :key="mode.value" class="mode-option">
          <input v-model="draft.hubMode" type="radio" name="hub-mode" :value="mode.value" />
          <span>
            <strong>{{ mode.label }}</strong>
            <small>{{ mode.description }}</small>
          </span>
        </label>
      </fieldset>

      <div v-if="usesHub" class="grid">
        <label>
          Hub from announces
          <select :value="draft.hubIdentityHash" @change="onCandidateSelected">
            <option value="">Manual / none</option>
            <option v-for="candidate in announceCandidates" :key="candidate.destination" :value="candidate.destination">
              {{ candidate.label }} ({{ candidate.destination.slice(0, 10) }}…)
            </option>
          </select>
        </label>
        <label>
          <span>Hub identity hash <span class="restart-badge">restart</span></span>
          <input v-model="draft.hubIdentityHash" type="text" />
        </label>
        <label>
          Refresh interval (seconds)
          <input v-model.number="draft.hubRefreshIntervalSeconds" type="number" min="30" />
        </label>
      </div>
      <p v-if="usesHub && announceCandidates.length === 0" class="section-note">
        No RCH hub announces have been heard yet. Paste the hub identity hash instead.
      </p>
    </section>

    <section class="panel settings-card">
      <dl class="status-rows">
        <div class="status-row"><dt>Directory</dt><dd>{{ directoryStatus }}</dd></div>
        <div class="status-row"><dt>Hub registration</dt><dd>{{ nodeStore.hubRegistrationSummary }}</dd></div>
        <div class="status-row"><dt>Active propagation node</dt><dd>{{ activePropagationNodeLabel }}</dd></div>
        <div class="status-row"><dt>Selection mode</dt><dd>{{ propagationSelection }}</dd></div>
      </dl>
    </section>

    <section class="panel settings-card settings-actions-section" aria-labelledby="routing-actions-heading">
      <div class="config-section-header">
        <h3 id="routing-actions-heading">Actions</h3>
        <p>These run right away and do not change the form above.</p>
      </div>
      <div class="actions">
        <button type="button" :disabled="actionPending" @click="runAction(() => nodeStore.refreshHubDirectory(), 'Hub refresh requested.')">
          Refresh directory now
        </button>
        <button type="button" :disabled="actionPending" @click="runAction(() => nodeStore.bootstrapHubRegistration(true), 'Hub registration requested.')">
          Register team member
        </button>
        <button type="button" :disabled="actionPending" @click="runAction(() => nodeStore.forgetHubRegistryLinkage(), 'Hub registration cleared.')">
          Clear registration
        </button>
      </div>
      <p v-if="actionFeedback" class="feedback" role="status">{{ actionFeedback }}</p>
    </section>

    <details class="panel settings-advanced">
      <summary>Advanced</summary>
      <div class="settings-fields">
        <label>
          <span>Hub API base URL <span class="restart-badge">restart</span></span>
          <input v-model="draft.hubApiBaseUrl" type="url" placeholder="https://hub.example/" />
        </label>
        <label>
          <span>Hub API key <span class="restart-badge">restart</span></span>
          <input v-model="draft.hubApiKey" type="password" autocomplete="off" />
        </label>
      </div>
    </details>

    <template #footer>
      <SettingsSaveBar
        :dirty="form.dirty.value"
        :saving="form.saving.value"
        :feedback="form.feedback.value"
        @save="form.save"
        @discard="form.discard"
      />
      <SettingsLeaveDialog :open="form.leavePending.value" @answer="form.answerLeave" />
    </template>
  </SettingsPage>
</template>
