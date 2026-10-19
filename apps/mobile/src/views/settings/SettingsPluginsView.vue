<script setup lang="ts">
import { PhCaretRight as CaretRight } from "@phosphor-icons/vue";
import { ref } from "vue";

import SettingsPage from "../../components/settings/SettingsPage.vue";
import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const refreshing = ref(false);
const feedback = ref("");

async function refresh(): Promise<void> {
  if (refreshing.value) return;
  refreshing.value = true;
  feedback.value = "";
  try {
    await nodeStore.refreshPluginProjection(true);
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  } finally {
    refreshing.value = false;
  }
}
</script>

<template>
  <SettingsPage title="Plugins" description="Android plugin apps that extend REM.">
    <section class="panel settings-card" data-testid="plugin-settings-panel">
      <p class="section-note">
        Plugin APKs are installed separately. Android permissions belong to the plugin app;
        each plugin's page controls what it may do inside REM.
      </p>
      <div class="actions">
        <button type="button" :disabled="refreshing" @click="refresh">Refresh installed plugins</button>
      </div>
      <p v-if="feedback" class="feedback" role="status">{{ feedback }}</p>
      <nav v-if="nodeStore.plugins.length" class="settings-rows" aria-label="Installed plugins">
        <RouterLink
          v-for="plugin in nodeStore.plugins"
          :key="plugin.pluginId"
          :to="{ name: 'settings-plugin-detail', params: { pluginId: plugin.pluginId } }"
          class="settings-row"
        >
          <span class="settings-row-copy">
            <strong>{{ plugin.displayName }}</strong>
            <small>
              Version {{ plugin.version }} · {{ !plugin.trusted ? "publisher not approved" : plugin.enabled ? "enabled" : "disabled" }}
            </small>
          </span>
          <span class="status-pill">{{ plugin.state }}</span>
          <CaretRight :size="18" aria-hidden="true" />
        </RouterLink>
      </nav>
      <p v-else class="section-note">No plugin APKs were discovered.</p>
    </section>
  </SettingsPage>
</template>
