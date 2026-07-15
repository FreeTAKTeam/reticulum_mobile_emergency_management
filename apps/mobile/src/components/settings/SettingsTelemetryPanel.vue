<script setup lang="ts">
import { computed } from "vue";

import { useTelemetryStore } from "../../stores/telemetryStore";

interface TelemetrySettingsForm {
  telemetryEnabled: boolean;
  telemetryPublishIntervalSeconds: number;
  telemetryAccuracyThresholdMeters: number | undefined;
  telemetryStaleAfterMinutes: number | undefined;
  telemetryExpireAfterMinutes: number | undefined;
}

const props = defineProps<{ form: TelemetrySettingsForm }>();
const telemetryStore = useTelemetryStore();
const statusText = computed(() => {
  if (!props.form.telemetryEnabled) return "Disabled";
  if (telemetryStore.loopStatus === "permission_denied") return "Permission denied";
  if (telemetryStore.loopStatus === "gps_unavailable") return "GPS unavailable";
  if (telemetryStore.loopStatus === "running") return "Publishing";
  return "Idle";
});
const summary = computed(() =>
  props.form.telemetryEnabled
    ? `${statusText.value} | every ${props.form.telemetryPublishIntervalSeconds}s`
    : "Disabled",
);
</script>

<template>
    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path
                d="M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z"
              />
              <path d="M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z" />
            </svg>
          </span>
          <h2>Telemetry</h2>
          <p>{{ summary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <div class="grid">
          <label class="checkbox">
            <input v-model="form.telemetryEnabled" type="checkbox" />
            Enable telemetry sharing
          </label>
          <label>
            Telemetry publish interval (seconds)
            <input v-model.number="form.telemetryPublishIntervalSeconds" type="number" min="1" />
          </label>
          <label>
            Telemetry accuracy threshold (meters, optional)
            <input
              v-model.number="form.telemetryAccuracyThresholdMeters"
              type="number"
              min="0"
              placeholder="Unset"
            />
          </label>
          <label>
            Telemetry goes stale after (minutes)
            <input v-model.number="form.telemetryStaleAfterMinutes" type="number" min="1" />
          </label>
          <label>
            Telemetry disappears after (minutes)
            <input v-model.number="form.telemetryExpireAfterMinutes" type="number" min="1" />
          </label>
          <label>
            Telemetry status
            <input :value="statusText" class="readonly-input" type="text" readonly />
          </label>
        </div>
      </div>
    </details>
</template>
