<script setup lang="ts">
import { computed } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import TelemetryDeliveryStatus from "../../components/settings/TelemetryDeliveryStatus.vue";
import LocationFields from "../../components/settings/forms/LocationFields.vue";
import { locationSlice } from "../../components/settings/settingsFormSlices";
import { SETTINGS_PAGE_COPY } from "../../components/settings/settingsPageCopy";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
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
</script>

<template>
  <SettingsPage :title="SETTINGS_PAGE_COPY.location.title" :description="SETTINGS_PAGE_COPY.location.description">
    <LocationFields :draft="draft" :validation-error="form.validationError.value">
      <template #status>
        <div class="status-row"><dt>Local location</dt><dd>{{ status }}</dd></div>
      </template>
      <template #after-status>
        <TelemetryDeliveryStatus :publication="telemetryStore.publication" />
        <p v-if="telemetryStore.telemetryError" class="feedback">{{ telemetryStore.telemetryError }}</p>
      </template>
    </LocationFields>

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
