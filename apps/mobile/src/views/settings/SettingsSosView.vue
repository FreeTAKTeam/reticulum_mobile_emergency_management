<script setup lang="ts">
import type { SosSettingsRecord } from "@reticulum/node-client";
import { PhCaretRight as CaretRight } from "@phosphor-icons/vue";
import { computed, onMounted, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import SettingsSosPinSheet from "../../components/settings/SettingsSosPinSheet.vue";
import SosFields from "../../components/settings/forms/SosFields.vue";
import { SETTINGS_PAGE_COPY } from "../../components/settings/settingsPageCopy";
import { readSosDraft, sosSettingsForSave, sosSettingsWithoutPin } from "../../components/settings/sosFormDraft";
import { useSettingsDraft } from "../../components/settings/useSettingsDraft";
import { useSosStore } from "../../stores/sosStore";

const sosStore = useSosStore();

const form = useSettingsDraft<SosSettingsRecord>({
  read: () => readSosDraft(sosStore.settings),
  fingerprint: (draft) => JSON.stringify(sosSettingsWithoutPin(draft)),
  commit: async (draft) => {
    await sosStore.saveSettings(sosSettingsForSave(draft, sosStore.settings));
    return "SOS settings saved.";
  },
});
const { draft } = form;

const pinSheetOpen = ref(false);
const pinSet = computed(() => Boolean(sosStore.settings.deactivationPinHash));
const actionFeedback = ref("");
const confirmingTrigger = ref(false);

async function runAction(action: () => Promise<void>, success: string): Promise<void> {
  actionFeedback.value = "";
  try {
    await action();
    actionFeedback.value = success;
  } catch (error: unknown) {
    actionFeedback.value = error instanceof Error ? error.message : String(error);
  }
}

async function setPin(pin: string): Promise<void> {
  pinSheetOpen.value = false;
  await runAction(() => sosStore.setPin(pin), "SOS PIN updated.");
}

async function clearPin(): Promise<void> {
  pinSheetOpen.value = false;
  await runAction(() => sosStore.setPin(undefined), "SOS PIN cleared.");
}

async function triggerSos(): Promise<void> {
  confirmingTrigger.value = false;
  await runAction(() => sosStore.trigger("Manual"), "SOS triggered.");
}

onMounted(() => {
  void sosStore.init().catch((error: unknown) => {
    actionFeedback.value = error instanceof Error ? error.message : String(error);
  });
});
</script>

<template>
  <SettingsPage :title="SETTINGS_PAGE_COPY.sos.title" :description="SETTINGS_PAGE_COPY.sos.description">
    <SosFields :draft="draft">
      <template #access>
        <button type="button" class="settings-row task-row" @click="pinSheetOpen = true">
          <span class="settings-row-copy">
            <strong>Deactivation PIN</strong>
            <small>{{ pinSet ? "Set" : "Not set" }}</small>
          </span>
          <CaretRight :size="18" aria-hidden="true" />
        </button>
      </template>
    </SosFields>

    <section class="panel settings-card danger-zone" aria-labelledby="sos-danger-heading">
      <div class="config-section-header">
        <h3 id="sos-danger-heading">Trigger SOS now</h3>
        <p>Sends a real SOS alert to your peers using the saved settings.</p>
      </div>
      <div class="actions">
        <button v-if="!confirmingTrigger" type="button" class="danger-button" @click="confirmingTrigger = true">
          Trigger SOS
        </button>
        <template v-else>
          <button type="button" class="danger-button" @click="triggerSos">Send SOS to peers</button>
          <button type="button" @click="confirmingTrigger = false">Cancel</button>
        </template>
      </div>
      <p v-if="actionFeedback" class="feedback" role="status">{{ actionFeedback }}</p>
      <p v-if="sosStore.lastError" class="feedback">{{ sosStore.lastError }}</p>
    </section>

    <template #footer>
      <SettingsSaveBar
        :dirty="form.dirty.value"
        :saving="form.saving.value"
        :feedback="form.feedback.value"
        @save="form.save"
        @discard="form.discard"
      />
      <SettingsLeaveDialog :open="form.leavePending.value" @answer="form.answerLeave" />
      <SettingsSosPinSheet :open="pinSheetOpen" :pin-set="pinSet" @close="pinSheetOpen = false" @set="setPin" @clear="clearPin" />
    </template>
  </SettingsPage>
</template>
