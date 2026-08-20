<script setup lang="ts">
import { computed, ref } from "vue";

import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const runtimeFeedback = ref("");
const summary = computed(() => nodeStore.status.running ? "Node is running" : "Node is stopped");

function formatLogTimestamp(at: number): string {
  if (!Number.isFinite(at) || at <= 0) {
    return "--:--:--";
  }
  return new Date(at).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatInterfaceActivity(at: number): string {
  if (!Number.isFinite(at) || at <= 0) {
    return "No RX yet";
  }
  return new Date(at).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

async function runNodeAction(action: () => Promise<void>, success: string): Promise<void> {
  runtimeFeedback.value = "";
  try {
    await action();
    runtimeFeedback.value = success;
  } catch (error) {
    runtimeFeedback.value = error instanceof Error ? error.message : String(error);
  }
}
</script>

<template>
  <details class="panel fold-panel">
    <summary class="panel-summary">
      <div class="summary-copy">
        <span class="summary-icon" aria-hidden="true">
          <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
            <circle cx="6" cy="12" r="1.6" />
            <circle cx="12" cy="6" r="1.6" />
            <circle cx="18" cy="8" r="1.6" />
            <circle cx="18" cy="16" r="1.6" />
            <circle cx="10" cy="18" r="1.6" />
            <path d="M7.4 10.9 10.6 7.1" />
            <path d="M13.5 6.5 16.5 7.5" />
            <path d="M18 9.6v4.8" />
            <path d="M16.7 17.1 11.3 16.9" />
            <path d="M8.7 16.9 6.9 13.5" />
            <path d="M11.2 7.5 10.4 16.4" />
          </svg>
        </span>
        <h2>Node Control</h2>
        <p>{{ summary }}</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>
    <div class="panel-body">
      <div class="actions">
        <button type="button" @click="runNodeAction(() => nodeStore.startNode(), 'Node started.')">Start</button>
        <button type="button" @click="runNodeAction(() => nodeStore.stopNode(), 'Node stopped.')">Stop</button>
        <button type="button" @click="runNodeAction(() => nodeStore.reinitializeClient(), 'Node client recreated.')">Restart UI</button>
        <button type="button" @click="runNodeAction(() => nodeStore.restartNode(), 'Node restarted.')">Restart</button>
      </div>
      <p v-if="runtimeFeedback" class="feedback">{{ runtimeFeedback }}</p>
      <p v-if="nodeStore.lastError" class="feedback">{{ nodeStore.lastError }}</p>
      <div v-if="nodeStore.status.interfaces.length > 0" class="active-endpoints">
        <article
          v-for="iface in nodeStore.status.interfaces"
          :key="iface.interfaceHex"
          class="active-endpoint"
        >
          <span>
            {{ iface.label || iface.interfaceHex }}
            <small>{{ iface.kind }} | {{ iface.state }} | RX {{ iface.rxPackets }} / {{ iface.rxBytes }} bytes | {{ formatInterfaceActivity(iface.lastActivityMs) }}</small>
            <small v-if="iface.lastError">{{ iface.lastError }}</small>
          </span>
        </article>
      </div>
      <div class="log-list">
        <p v-for="entry in nodeStore.nodeControlEntries" :key="entry.at" class="log">
          <time :datetime="new Date(entry.at).toISOString()">{{ formatLogTimestamp(entry.at) }}</time>
          <span>{{ entry.level }}</span>
          <span>{{ entry.message }}</span>
        </p>
      </div>
    </div>
  </details>
</template>
