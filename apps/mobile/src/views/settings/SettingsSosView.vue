<script setup lang="ts">
import type { SosSettingsRecord } from "@reticulum/node-client";
import { PhCaretRight as CaretRight } from "@phosphor-icons/vue";
import { computed, onMounted, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsPermissionRow from "../../components/settings/SettingsPermissionRow.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import SettingsSosPinSheet from "../../components/settings/SettingsSosPinSheet.vue";
import { useSettingsDraft } from "../../components/settings/useSettingsDraft";
import { checkSetupPermissions, requestNotificationPermission } from "../../services/setupPermissions";
import { normalizeReleaseSosSettings, useSosStore } from "../../stores/sosStore";

const sosStore = useSosStore();

/** The PIN has its own sheet; keep it out of the form's save cycle. */
function withoutPin(settings: SosSettingsRecord): SosSettingsRecord {
  const { deactivationPinHash: _hash, deactivationPinSalt: _salt, ...rest } = normalizeReleaseSosSettings(settings);
  return rest as SosSettingsRecord;
}

const form = useSettingsDraft<SosSettingsRecord>({
  read: () => ({ ...normalizeReleaseSosSettings(sosStore.settings) }),
  fingerprint: (draft) => JSON.stringify(withoutPin(draft)),
  commit: async (draft) => {
    await sosStore.saveSettings({
      ...withoutPin(draft),
      deactivationPinHash: sosStore.settings.deactivationPinHash,
      deactivationPinSalt: sosStore.settings.deactivationPinSalt,
    });
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

async function checkNotifications() {
  return (await checkSetupPermissions()).notifications;
}

onMounted(() => {
  void sosStore.init().catch((error: unknown) => {
    actionFeedback.value = error instanceof Error ? error.message : String(error);
  });
});
</script>

<template>
  <SettingsPage title="SOS" description="How an emergency alert is raised and what it sends.">
    <section class="panel settings-card">
      <label class="checkbox">
        <input v-model="draft.enabled" type="checkbox" />
        Enable SOS
      </label>
      <div class="settings-fields">
        <label class="wide">
          Message template
          <textarea v-model="draft.messageTemplate" rows="3" maxlength="240" />
        </label>
        <label class="wide">
          Emergency end template
          <textarea v-model="draft.cancelMessageTemplate" rows="2" maxlength="180" />
        </label>
      </div>
      <label class="checkbox">
        <input v-model="draft.includeLocation" type="checkbox" />
        Include GPS and battery
      </label>
    </section>

    <section class="panel settings-card">
      <div class="config-section-header"><h3>Triggers</h3></div>
      <label class="checkbox">
        <input v-model="draft.triggerShake" type="checkbox" />
        Shake trigger
      </label>
      <label v-if="draft.triggerShake" class="settings-indented">
        Shake sensitivity
        <input v-model.number="draft.shakeSensitivity" min="1" max="6" step="0.1" type="number" />
      </label>
      <label class="checkbox">
        <input v-model="draft.triggerPowerButton" type="checkbox" />
        Power button trigger
      </label>
      <label class="checkbox">
        <input v-model="draft.floatingButton" type="checkbox" />
        Floating SOS button
      </label>
    </section>

    <section class="panel settings-card">
      <button type="button" class="settings-row task-row" @click="pinSheetOpen = true">
        <span class="settings-row-copy">
          <strong>Deactivation PIN</strong>
          <small>{{ pinSet ? "Set" : "Not set" }}</small>
        </span>
        <CaretRight :size="18" aria-hidden="true" />
      </button>
      <dl class="status-rows">
        <SettingsPermissionRow label="Notifications" :check="checkNotifications" :request="requestNotificationPermission" />
      </dl>
    </section>

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
