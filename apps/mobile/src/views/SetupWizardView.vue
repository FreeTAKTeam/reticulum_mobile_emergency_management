<script setup lang="ts">
import { computed, onMounted } from "vue";

import IdentityFields from "../components/settings/forms/IdentityFields.vue";
import LocationFields from "../components/settings/forms/LocationFields.vue";
import NetworkFields from "../components/settings/forms/NetworkFields.vue";
import RnodePairingFields from "../components/settings/forms/RnodePairingFields.vue";
import RoutingFields from "../components/settings/forms/RoutingFields.vue";
import SosFields from "../components/settings/forms/SosFields.vue";
import { SETTINGS_PAGE_COPY, type SettingsPageCopy } from "../components/settings/settingsPageCopy";
import SetupWelcomeStep from "../components/setup/SetupWelcomeStep.vue";
import WizardProgress from "../components/setup/WizardProgress.vue";
import { useSetupWizard } from "../composables/useSetupWizard";
import logoUrl from "../assets/rem-logo.png";

const wizard = useSetupWizard();
const stepId = computed(() => wizard.activeStep.value.id);
const REVIEW_COPY: SettingsPageCopy = {
  title: "Review setup",
  description: "Finish saves these choices and starts the node. Change any of them later in Settings.",
};
const heading = computed<SettingsPageCopy>(() => {
  switch (stepId.value) {
    case "identity":
      return SETTINGS_PAGE_COPY.identity;
    case "network":
      return wizard.pairingRnode.value ? SETTINGS_PAGE_COPY.rnodePairing : SETTINGS_PAGE_COPY.network;
    case "routing":
      return SETTINGS_PAGE_COPY.routing;
    case "location":
      return SETTINGS_PAGE_COPY.location;
    case "sos":
      return SETTINGS_PAGE_COPY.sos;
    default:
      return REVIEW_COPY;
  }
});
onMounted(() => { wizard.open(); });
</script>
<template>
  <section class="setup-view" data-testid="setup-wizard">
    <div class="setup-console">
      <header class="console-header">
        <div class="brand-lockup">
          <img class="brand-symbol" :src="logoUrl" alt="Reticulum Emergency Manager logo" />
          <div class="brand-copy">
            <p>Reticulum</p>
            <span>Emergency Manager</span>
          </div>
        </div>
        <div class="node-state" aria-label="Node readiness preview">
          <span class="state-dot"></span>
          <span>Node<br /><strong>Ready</strong></span>
        </div>
      </header>
      <div class="step-band">
        <div>
          <p class="band-label">Setup Wizard</p>
          <span class="band-underline"></span>
        </div>
        <span class="band-step">Step {{ wizard.activeIndex.value + 1 }} of {{ wizard.steps.length }}</span>
      </div>
      <WizardProgress :steps="wizard.steps" :active-index="wizard.activeIndex.value" />

      <article class="wizard-panel" :class="`step-${stepId}`">
        <SetupWelcomeStep v-if="stepId === 'welcome'" :logo-url="logoUrl" />

        <section v-else class="wizard-section">
          <div class="section-heading">
            <p v-if="wizard.activeStep.value.optional" class="eyebrow">Optional</p>
            <h1>{{ heading.title }}</h1>
            <p>{{ heading.description }}</p>
          </div>

          <div class="settings-view wizard-form">
            <IdentityFields
              v-if="stepId === 'identity'"
              :draft="wizard.identity"
              :error-field="wizard.errorField.value"
              :validation-error="wizard.activeStepError.value"
            />
            <template v-else-if="stepId === 'network'">
              <RnodePairingFields
                v-if="wizard.pairingRnode.value"
                :draft="wizard.network"
                @done="wizard.pairingRnode.value = false"
              />
              <NetworkFields
                v-else
                :draft="wizard.network"
                @feedback="wizard.feedback.value = $event"
                @pair="wizard.pairingRnode.value = true"
              />
            </template>
            <RoutingFields v-else-if="stepId === 'routing'" :draft="wizard.routing" />
            <LocationFields
              v-else-if="stepId === 'location'"
              :draft="wizard.location"
              :validation-error="wizard.activeStepError.value"
            />
            <SosFields v-else-if="stepId === 'sos'" :draft="wizard.sos" />
            <dl v-else class="review-grid">
              <div v-for="row in wizard.reviewRows.value" :key="row.label">
                <dt>{{ row.label }}</dt>
                <dd>{{ row.value }}</dd>
              </div>
            </dl>
          </div>
        </section>
      </article>

      <p v-if="wizard.feedback.value" class="feedback" role="status">{{ wizard.feedback.value }}</p>

      <footer class="wizard-actions">
        <button
          type="button"
          class="secondary-action"
          :disabled="wizard.activeIndex.value === 0 || wizard.saving.value"
          @click="wizard.back"
        >
          Back
        </button>
        <button
          v-if="wizard.activeStep.value.id !== 'review'"
          type="button"
          class="primary-action"
          :disabled="!wizard.canGoNext.value"
          @click="wizard.next"
        >
          {{ wizard.activeStep.value.id === "welcome" ? "Start Setup" : "Next" }}
        </button>
        <button
          v-else
          type="button"
          class="primary-action"
          :disabled="wizard.saving.value || !wizard.canGoNext.value"
          data-testid="setup-finish"
          @click="wizard.finish"
        >
          {{ wizard.saving.value ? "Saving" : "Finish" }}
        </button>
      </footer>
    </div>
  </section>
</template>

<style scoped src="./SetupWizardLayout.css"></style>
<style scoped src="./SetupWizardControls.css"></style>
<style src="./styles/settings-layout.css"></style>
<style src="./styles/settings-content.css"></style>
<style src="./styles/settings-pages.css"></style>
