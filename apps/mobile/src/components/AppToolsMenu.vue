<script setup lang="ts">
import { ref } from "vue";
import { RouterLink } from "vue-router";

import { iconPaths, menuItems } from "../appNavigation";
import { shutdownRem } from "../services/shutdownRem";

const props = defineProps<{
  open: boolean;
  currentPath: string;
  stopNode: () => Promise<void>;
}>();
const emit = defineEmits<{ close: [] }>();
const pending = ref(false);
const errorMessage = ref("");

function isTabActive(path: string): boolean {
  return props.currentPath === path || props.currentPath.startsWith(`${path}/`);
}

async function requestShutdown(): Promise<void> {
  if (pending.value || !window.confirm(
    "Quit REM? Messaging, location sharing and the background service will stop. Reopen REM to start it again.",
  )) {
    return;
  }
  pending.value = true;
  errorMessage.value = "";
  try {
    await shutdownRem(props.stopNode);
    emit("close");
  } catch (error: unknown) {
    errorMessage.value = `Could not quit REM: ${error instanceof Error ? error.message : String(error)}`;
  } finally {
    pending.value = false;
  }
}
</script>

<template>
  <div
    v-if="open"
    class="menu-backdrop"
    aria-hidden="true"
    @click="emit('close')"
  ></div>

  <aside
    v-if="open"
    class="tools-menu"
    aria-label="More tools"
  >
    <header class="tools-header">
      <h2>Tools</h2>
      <button
        type="button"
        class="tools-close"
        aria-label="Close more tools"
        @click="emit('close')"
      >
        <svg class="icon-svg" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M6 6l12 12" />
          <path d="M18 6 6 18" />
        </svg>
      </button>
    </header>

    <div class="tools-grid">
      <RouterLink
        v-for="item in menuItems"
        :key="`menu-${item.path}`"
        :to="item.path"
        class="tool-tile"
        :class="{ active: isTabActive(item.path) }"
        :aria-label="item.label"
        :title="item.label"
      >
        <span class="tool-tile-icon" aria-hidden="true">
          <svg class="icon-svg" viewBox="0 0 24 24" fill="none">
            <path
              v-for="path in iconPaths[item.icon]"
              :key="path"
              :d="path"
            />
          </svg>
        </span>
        <span class="tool-tile-label">{{ item.label }}</span>
      </RouterLink>
      <button
        type="button"
        class="tool-tile tool-shutdown"
        :disabled="pending"
        :aria-busy="pending"
        @click="requestShutdown"
      >
        <span class="tool-tile-icon" aria-hidden="true">
          <svg class="icon-svg" viewBox="0 0 24 24" fill="none">
            <path d="M12 3v9" />
            <path d="M6.5 5.5a9 9 0 1 0 11 0" />
          </svg>
        </span>
        <span class="tool-tile-label">{{ pending ? "Quitting…" : "Quit" }}</span>
      </button>
    </div>
    <p v-if="errorMessage" class="shutdown-error" role="alert">{{ errorMessage }}</p>
  </aside>
</template>

<style scoped src="./AppToolsMenu.css"></style>
