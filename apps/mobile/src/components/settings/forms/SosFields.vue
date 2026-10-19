<script setup lang="ts">
import type { SosSettingsRecord } from "@reticulum/node-client";

import { checkSetupPermissions, requestNotificationPermission } from "../../../services/setupPermissions";
import SettingsPermissionRow from "../SettingsPermissionRow.vue";

/**
 * SOS form body and notification permission row, shared by Settings > SOS and the
 * setup wizard. The `access` slot adds rows (such as the PIN) above the permission row.
 */
defineProps<{ draft: SosSettingsRecord }>();

async function checkNotifications() {
  return (await checkSetupPermissions()).notifications;
}
</script>

<template>
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
    <slot name="access" />
    <dl class="status-rows">
      <SettingsPermissionRow label="Notifications" :check="checkNotifications" :request="requestNotificationPermission" />
    </dl>
  </section>
</template>
