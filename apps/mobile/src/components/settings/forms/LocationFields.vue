<script setup lang="ts">
import { checkSetupPermissions, requestLocationPermission } from "../../../services/setupPermissions";
import SettingsPermissionRow from "../SettingsPermissionRow.vue";
import type { LocationDraft } from "../settingsFormSlices";

/**
 * Location sharing form body and permission row, shared by Settings > Location and
 * the setup wizard. The `status` slot adds rows above the permission row.
 */
defineProps<{ draft: LocationDraft; validationError?: string }>();

async function checkLocation() {
  return (await checkSetupPermissions()).location;
}
</script>

<template>
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
    <label>
      Share with saved LXMF contacts (Sideband standard)
      <textarea v-model="draft.lxmfContacts" aria-label="Sideband telemetry contacts" placeholder="One LXMF destination hash per line" />
    </label>
    <label>
      Share with saved Columba 2.2.6 contacts
      <textarea v-model="draft.columbaContacts" aria-label="Columba telemetry contacts" placeholder="One LXMF destination hash per line" />
    </label>
    <p class="muted">These saved contacts receive exact positions while sharing is enabled. Remove a hash to revoke permission; unsaving a contact stops delivery. Columba 2.2.6 uses its own speed profile.</p>
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
          :aria-invalid="Boolean(validationError) && Number(draft.expireAfterMinutes) < Number(draft.staleAfterMinutes)"
        />
        minutes.
      </p>
    </fieldset>
    <small v-if="validationError" class="field-error" role="alert">{{ validationError }}</small>
  </section>

  <section class="panel settings-card">
    <dl class="status-rows">
      <slot name="status" />
      <SettingsPermissionRow label="Location permission" :check="checkLocation" :request="requestLocationPermission" />
    </dl>
    <slot name="after-status" />
  </section>
</template>
