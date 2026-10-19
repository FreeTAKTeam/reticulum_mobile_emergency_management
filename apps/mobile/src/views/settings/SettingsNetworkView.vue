<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { networkSlice } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { useNodeStore } from "../../stores/nodeStore";
import type { NodeUiSettings } from "../../types/domain";
import { normalizeRnodeSettings } from "../../utils/rnodeProfiles";

const nodeStore = useNodeStore();
const route = useRoute();
const pairing = computed(() => route.name === "settings-rnode-pairing");

/** A changed LoRa interface is applied right away, as the single Settings form did. */
async function applyRnodeChange(before: NodeUiSettings): Promise<string | undefined> {
  const rnodeChanged = JSON.stringify(normalizeRnodeSettings(before.rnode))
    !== JSON.stringify(normalizeRnodeSettings(nodeStore.settings.rnode));
  if (!rnodeChanged) {
    return undefined;
  }
  try {
    if (nodeStore.status.running) {
      await nodeStore.restartNode();
    } else {
      await nodeStore.startNode();
    }
    return "RNode settings saved and applied to the LoRa interface.";
  } catch (error: unknown) {
    return `RNode settings saved, but node start/restart failed: ${error instanceof Error ? error.message : String(error)}`;
  }
}

const form = useNodeSettingsForm(networkSlice, { afterSave: applyRnodeChange });
</script>

<template>
  <SettingsPage
    :title="pairing ? 'Pair RNode device' : 'Network interfaces'"
    :description="pairing ? 'Find a LoRa radio over Bluetooth or USB and use it for this node.' : 'How this node reaches the mesh: TCP servers and LoRa radio.'"
    :back="pairing ? { name: 'settings-network' } : '/settings'"
    :back-label="pairing ? 'Back to network' : 'Back to settings'"
  >
    <RouterView v-slot="{ Component }">
      <component :is="Component" :draft="form.draft" @feedback="form.feedback.value = $event" />
    </RouterView>

    <template #footer>
      <SettingsSaveBar
        v-if="!pairing"
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
