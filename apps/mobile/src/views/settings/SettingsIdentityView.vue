<script setup lang="ts">
import { PhCopy as Copy } from "@phosphor-icons/vue";
import { computed, ref } from "vue";

import SettingsLeaveDialog from "../../components/settings/SettingsLeaveDialog.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import SettingsRoleBadgesInput from "../../components/settings/SettingsRoleBadgesInput.vue";
import SettingsSaveBar from "../../components/settings/SettingsSaveBar.vue";
import { identitySlice } from "../../components/settings/settingsFormSlices";
import { useNodeSettingsForm } from "../../components/settings/useNodeSettingsForm";
import { useNodeStore } from "../../stores/nodeStore";
import type { SettingsSaveField } from "../../utils/settingsSaveErrors";

const nodeStore = useNodeStore();
const form = useNodeSettingsForm(identitySlice);
const { draft } = form;
const copyFeedback = ref("");
const ownAppHash = computed(() => nodeStore.status.appDestinationHex || "");
const invalid = (field: SettingsSaveField): boolean => form.errorField.value === field;

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
  <SettingsPage title="Identity" description="Who this node is to others on the mesh.">
    <section class="panel settings-card">
      <div class="settings-fields">
        <label>
          <span>Call sign <span class="restart-badge">restart</span></span>
          <input v-model="draft.displayName" type="text" maxlength="64" />
        </label>
        <div class="wide settings-field">
          <label for="settings-household-name">Household name</label>
          <input
            id="settings-household-name"
            v-model="draft.householdName"
            maxlength="64"
            placeholder="Harbour House"
            aria-describedby="settings-household-name-hint"
            :aria-invalid="invalid('householdName')"
          />
          <small v-if="invalid('householdName')" id="settings-household-name-hint" class="field-error" role="alert">Household name must be 1 to 64 characters.</small>
          <small v-else id="settings-household-name-hint">{{ draft.householdName.trim() ? "Shown to peers in community status." : 'Leave empty to use "Household <call sign>".' }}</small>
        </div>
        <div class="settings-field">
          <label for="settings-household-id">Household ID</label>
          <input
            id="settings-household-id"
            v-model="draft.householdId"
            maxlength="16"
            pattern="[0-9a-fA-F]{16}"
            placeholder="16 hex characters"
            aria-describedby="settings-household-id-hint"
            :aria-invalid="invalid('householdId')"
          />
          <small v-if="invalid('householdId')" id="settings-household-id-hint" class="field-error" role="alert">Must be exactly 16 hex characters.</small>
        </div>
        <div class="settings-counts">
          <label><span>Adults</span><input v-model.number="draft.adults" type="number" min="0" max="20" :aria-invalid="invalid('householdCounts')" /></label>
          <label><span>Children</span><input v-model.number="draft.children" type="number" min="0" max="20" :aria-invalid="invalid('householdCounts')" /></label>
          <label><span>Pets</span><input v-model.number="draft.pets" type="number" min="0" max="20" :aria-invalid="invalid('householdCounts')" /></label>
        </div>
        <div class="wide settings-field">
          <label for="settings-role-badges">Role badges</label>
          <SettingsRoleBadgesInput
            v-model:badges="draft.roleBadges"
            v-model:text="draft.roleBadgeText"
            :invalid="invalid('roleBadges')"
          />
          <small id="settings-role-badges-hint" :class="{ 'field-error': invalid('roleBadges') }">Up to five. Public in community status.</small>
        </div>
      </div>
    </section>

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
