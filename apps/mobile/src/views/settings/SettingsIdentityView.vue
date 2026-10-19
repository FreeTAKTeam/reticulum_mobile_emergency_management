<script setup lang="ts">
import { PhCopy as Copy } from "@phosphor-icons/vue";
import { computed, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import IdentityFields from "../../components/settings/forms/IdentityFields.vue";
import { identitySlice } from "../../components/settings/settingsFormSlices";
import { SETTINGS_PAGE_COPY } from "../../components/settings/settingsPageCopy";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const form = useNodeSettingsForm(identitySlice);
const { draft } = form;
const copyFeedback = ref("");
const ownAppHash = computed(() => nodeStore.status.appDestinationHex || "");

async function copyAppHash(): Promise<void> {
  try {
    await navigator.clipboard.writeText(ownAppHash.value);
    copyFeedback.value = "Copied.";
  } catch (error: unknown) {
    copyFeedback.value = error instanceof Error ? error.message : String(error);
  }
}
</script>

<template>
  <SettingsPage :title="SETTINGS_PAGE_COPY.identity.title" :description="SETTINGS_PAGE_COPY.identity.description">
    <IdentityFields
      :draft="draft"
      :error-field="form.errorField.value"
      :validation-error="form.validationError.value"
    />

    <section class="panel settings-card">
      <dl class="status-rows">
        <div class="status-row">
          <dt>Own app hash</dt>
          <dd>
            <code>{{ ownAppHash || "Start node to populate" }}</code>
            <button v-if="ownAppHash" type="button" class="icon-button" aria-label="Copy app hash" @click="copyAppHash">
              <Copy :size="16" aria-hidden="true" />
            </button>
          </dd>
        </div>
      </dl>
      <p v-if="copyFeedback" class="feedback" role="status">{{ copyFeedback }}</p>
    </section>

    <details class="panel settings-advanced">
      <summary>Advanced</summary>
      <div class="settings-fields">
        <label>
          <span>Announce interval (seconds) <span class="restart-badge">restart</span></span>
          <input v-model.number="draft.announceIntervalSeconds" type="number" min="60" />
        </label>
        <label class="wide">
          <span>Announce capabilities <span class="restart-badge">restart</span></span>
          <input v-model="draft.announceCapabilities" type="text" />
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
