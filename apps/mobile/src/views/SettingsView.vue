<script setup lang="ts">
import {
  PhArrowClockwise as ArrowClockwise,
  PhArrowCounterClockwise as ArrowCounterClockwise,
  PhCaretDown as CaretDown,
  PhCaretRight as CaretRight,
  PhInfo as Info,
} from "@phosphor-icons/vue";
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";

import { useSettingsIndexSections } from "../components/settings/useSettingsIndexSections";
import { useSettingsSectionFolds } from "../components/settings/useSettingsSectionFolds";
import { useNodeStore } from "../stores/nodeStore";
import { useSettingsRestartNotice } from "../stores/settingsRestartNotice";
import { useSosStore } from "../stores/sosStore";
import { appVersion } from "../utils/appVersion";
import { restartBannerText } from "../utils/settingsRestartReasons";

const nodeStore = useNodeStore();
const sosStore = useSosStore();
const restartNotice = useSettingsRestartNotice();
const router = useRouter();
const sections = useSettingsIndexSections();
const { isFolded, toggle } = useSettingsSectionFolds();
const restarting = ref(false);
const restartFeedback = ref("");

const restartPending = computed(() => nodeStore.status.running && nodeStore.nodeConfigRestartRequired);
const bannerText = computed(() => restartBannerText(restartNotice.reasons));

async function restartNode(): Promise<void> {
  if (restarting.value) return;
  restarting.value = true;
  restartFeedback.value = "";
  try {
    await nodeStore.restartNode();
    restartNotice.clear();
  } catch (error: unknown) {
    restartFeedback.value = error instanceof Error ? error.message : String(error);
  } finally {
    restarting.value = false;
  }
}

async function runSetupWizard(): Promise<void> {
  await router.push({ path: "/setup", query: { source: "settings" } });
}

onMounted(() => {
  void Promise.all([nodeStore.init(), sosStore.init()]).catch((error: unknown) => {
    const message = `Settings initialization failed: ${error instanceof Error ? error.message : String(error)}`;
    nodeStore.setLastError(message);
    nodeStore.logUi("Warn", message);
  });
});
</script>

<template>
  <section class="settings-view settings-index">
    <div v-if="restartPending" class="restart-banner" role="status">
      <ArrowClockwise :size="20" aria-hidden="true" />
      <p>{{ bannerText }}</p>
      <button type="button" :disabled="restarting" @click="restartNode">
        {{ restarting ? "Restarting" : "Restart" }}
      </button>
    </div>
    <p v-if="restartFeedback" class="feedback" role="alert">{{ restartFeedback }}</p>

    <section v-for="group in sections" :key="group.id" class="settings-group" :aria-labelledby="`settings-group-${group.id}`">
      <h2 :id="`settings-group-${group.id}`" class="settings-group-heading">
        <button
          type="button"
          class="settings-group-toggle"
          :aria-expanded="!isFolded(group.id)"
          :aria-controls="`settings-group-rows-${group.id}`"
          @click="toggle(group.id)"
        >
          <span class="settings-group-label">{{ group.label }}</span>
          <span v-if="isFolded(group.id)" class="settings-group-summary">{{ group.foldedSummary }}</span>
          <component :is="isFolded(group.id) ? CaretRight : CaretDown" :size="16" aria-hidden="true" />
        </button>
      </h2>
      <nav v-show="!isFolded(group.id)" :id="`settings-group-rows-${group.id}`" class="settings-rows" :aria-label="group.label">
        <RouterLink
          v-for="row in group.rows"
          :key="row.id"
          :to="row.to"
          class="settings-row"
          :class="{ 'task-row': row.task }"
        >
          <span class="settings-row-icon" aria-hidden="true"><component :is="row.icon" :size="20" /></span>
          <span class="settings-row-copy">
            <strong>{{ row.label }}</strong>
            <small>{{ row.summary }}</small>
          </span>
          <CaretRight :size="18" aria-hidden="true" />
        </RouterLink>
      </nav>
    </section>

    <nav class="settings-rows settings-footer" aria-label="About and setup">
      <RouterLink :to="{ name: 'settings-about' }" class="settings-row">
        <span class="settings-row-icon" aria-hidden="true"><Info :size="20" /></span>
        <span class="settings-row-copy">
          <strong>About</strong>
          <small>REM {{ appVersion }}</small>
        </span>
        <CaretRight :size="18" aria-hidden="true" />
      </RouterLink>
      <button type="button" class="settings-row task-row" aria-label="Run setup wizard" @click="runSetupWizard">
        <span class="settings-row-icon" aria-hidden="true"><ArrowCounterClockwise :size="20" /></span>
        <span class="settings-row-copy"><strong>Run setup wizard again</strong></span>
      </button>
    </nav>
  </section>
</template>

<style src="./styles/settings-layout.css"></style>
<style src="./styles/settings-content.css"></style>
<style src="./styles/settings-pages.css"></style>
