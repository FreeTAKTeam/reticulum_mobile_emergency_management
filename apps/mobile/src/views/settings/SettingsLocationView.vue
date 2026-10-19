<script setup lang="ts">
import { computed } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsPermissionRow from "../../components/settings/SettingsPermissionRow.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { locationSlice } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { checkSetupPermissions, requestLocationPermission } from "../../services/setupPermissions";
import { useNodeStore } from "../../stores/nodeStore";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { telemetryStatusLabel } from "../../utils/settingsIndexSummaries";

const nodeStore = useNodeStore();
const telemetryStore = useTelemetryStore();
const form = useNodeSettingsForm(locationSlice);
const { draft } = form;
const status = computed(() =>
  telemetryStatusLabel(nodeStore.settings.telemetry.enabled, telemetryStore.loopStatus),
);

async function checkLocation() {
  return (await checkSetupPermissions()).location;
}
</script>

<template>
  <SettingsPage title="Location sharing" description="Share this phone's position with your peers on the map.">
    <section class="panel settings-card">
      <label class="checkbox">
        <input v-model="draft.enabled" type="checkbox" />
        Enable telemetry sharing
      </label>
      <div class="settings-fields">
        <label>
          Telemetry publish interval (seconds)
          <input v-model.number="draft.publishIntervalSeconds" type="number" min="1" />
        </label>
        <label>
          Accuracy threshold (meters, optional)
          <input v-model.number="draft.accuracyThresholdMeters" type="number" min="0" placeholder="Unset" />
        </label>
      </div>
      <fieldset class="settings-pair">
        <legend>Peer positions on the map</legend>
        <p>
          Marked stale after
          <input
            v-model.number="draft.staleAfterMinutes"
            aria-label="Telemetry goes stale after (minutes)"
            type="number"
            min="1"
          />
          minutes <span class="restart-badge">restart</span>
          and hidden after
          <input
            v-model.number="draft.expireAfterMinutes"
            aria-label="Telemetry disappears after (minutes)"
            type="number"
            min="1"
            :aria-invalid="Boolean(form.validationError.value)"
          />
          minutes.
        </p>
        <small v-if="form.validationError.value" class="field-error" role="alert">{{ form.validationError.value }}</small>
      </fieldset>
    </section>

    <section class="panel settings-card">
      <dl class="status-rows">
        <div class="status-row"><dt>Telemetry status</dt><dd>{{ status }}</dd></div>
        <SettingsPermissionRow label="Location permission" :check="checkLocation" :request="requestLocationPermission" />
      </dl>
      <p v-if="telemetryStore.telemetryError" class="feedback">{{ telemetryStore.telemetryError }}</p>
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
