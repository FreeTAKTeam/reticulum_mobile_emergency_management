<script setup lang="ts">
import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { mapSlice } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";

const form = useNodeSettingsForm(mapSlice);
const { draft } = form;
</script>

<template>
  <SettingsPage title="Map" description="How the map looks on this phone.">
    <section class="panel settings-card">
      <fieldset class="mode-options">
        <legend>Preferred map</legend>
        <label class="mode-option">
          <input v-model="draft.preferredMapLayer" type="radio" name="map-layer" value="base" />
          <span><strong>Base</strong><small>Streets and labels.</small></span>
        </label>
        <label class="mode-option">
          <input v-model="draft.preferredMapLayer" type="radio" name="map-layer" value="satellite" />
          <span><strong>Satellite</strong><small>Aerial imagery.</small></span>
        </label>
      </fieldset>
      <p class="section-note">Offline map downloads will appear here.</p>
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
