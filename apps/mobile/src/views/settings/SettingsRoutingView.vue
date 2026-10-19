<script setup lang="ts">
import { computed, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import RoutingFields from "../../components/settings/forms/RoutingFields.vue";
import { routingSlice } from "../../components/settings/settingsFormSlices";
import { SETTINGS_PAGE_COPY } from "../../components/settings/settingsPageCopy";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const form = useNodeSettingsForm(routingSlice);
const { draft } = form;
const actionFeedback = ref("");
const actionPending = ref(false);

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
  <SettingsPage :title="SETTINGS_PAGE_COPY.routing.title" :description="SETTINGS_PAGE_COPY.routing.description">
    <RoutingFields :draft="draft" />

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
