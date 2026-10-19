<script setup lang="ts">
import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { powerSlice } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import type { SettingsSaveField } from "../../utils/settingsSaveErrors";

const form = useNodeSettingsForm(powerSlice);
const { draft } = form;
const invalid = (field: SettingsSaveField): boolean => form.errorField.value === field;
</script>

<template>
  <SettingsPage title="Battery saver" description="Pause ordinary traffic when the battery runs low.">
    <section class="panel settings-card">
      <label class="checkbox">
        <input v-model="draft.enabled" type="checkbox" />
        Automatic power saver
      </label>
      <div class="settings-fields">
        <label>
          Activate at
          <select
            v-model.number="draft.thresholdPercent"
            :disabled="!draft.enabled"
            :aria-invalid="invalid('powerThresholdPercent')"
          >
            <option :value="10">10%</option>
            <option :value="20">20%</option>
            <option :value="30">30%</option>
          </select>
        </label>
      </div>
      <p class="section-note">
        Saver turns off again 3% above this level. Emergency traffic is always sent.
      </p>
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
    </template>
  </SettingsPage>
</template>
