<script setup lang="ts">
import type { SettingsSaveField } from "../../../utils/settingsSaveErrors";
import SettingsRoleBadgesInput from "../SettingsRoleBadgesInput.vue";
import type { IdentityDraft } from "../settingsFormSlices";

/** Identity form body, shared by Settings > Identity and the setup wizard. */
const props = defineProps<{
  draft: IdentityDraft;
  errorField?: SettingsSaveField;
  validationError?: string;
}>();
const invalid = (field: SettingsSaveField): boolean => props.errorField === field;
</script>

<template>
  <section class="panel settings-card">
    <div class="settings-fields">
      <div class="settings-field">
        <label for="settings-callsign">Call sign <span class="restart-badge">restart</span></label>
        <input
          id="settings-callsign"
          v-model="draft.displayName"
          type="text"
          maxlength="64"
          autocomplete="off"
          :aria-invalid="Boolean(validationError)"
        />
        <small v-if="validationError" class="field-error" role="alert">{{ validationError }}</small>
      </div>
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
</template>
