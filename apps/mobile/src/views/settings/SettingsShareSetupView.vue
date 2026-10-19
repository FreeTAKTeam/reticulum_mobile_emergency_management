<script setup lang="ts">
import SettingsBlockOnboardingPanel from "../../components/settings/SettingsBlockOnboardingPanel.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
</script>

<template>
  <SettingsPage
    title="Share this setup"
    description="Create or import a Signed Block Code: an expiring, signed copy of network settings and trusted peers."
  >
    <SettingsBlockOnboardingPanel />
  </SettingsPage>
</template>
