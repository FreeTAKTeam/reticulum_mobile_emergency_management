<script setup lang="ts">
import type { PluginLxmfMessageLogEntry } from "../../stores/nodeStore";

defineProps<{
  messages: PluginLxmfMessageLogEntry[];
}>();

function formatReceivedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return "pending";
  }
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatReceivedDatetime(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return "";
  }
  return new Date(timestamp).toISOString();
}
</script>

<template>
  <section class="plugin-lxmf-log" aria-label="Recent plug-in LXMF messages">
    <header class="plugin-lxmf-log-header">
      <h3>Recent LXMF</h3>
      <span>{{ messages.length }}</span>
    </header>

    <ul class="plugin-lxmf-log-list">
      <li
        v-for="(message, index) in messages"
        :key="`${message.receivedAt}:${message.pluginId}:${message.messageName}:${index}`"
        class="plugin-lxmf-log-entry"
      >
        <div>
          <strong>{{ message.messageName }}</strong>
          <span>{{ message.pluginId }}</span>
        </div>
        <time :datetime="formatReceivedDatetime(message.receivedAt)">
          {{ formatReceivedAt(message.receivedAt) }}
        </time>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.plugin-lxmf-log {
  background: rgb(7 20 44 / 76%);
  border: 1px solid rgb(67 106 165 / 35%);
  border-radius: 8px;
  display: grid;
  gap: 0.62rem;
  padding: 0.72rem;
}

.plugin-lxmf-log-header,
.plugin-lxmf-log-entry {
  align-items: center;
  display: grid;
  gap: 0.7rem;
  grid-template-columns: 1fr auto;
}

.plugin-lxmf-log-header h3 {
  color: #d5eaff;
  font-family: var(--font-headline);
  font-size: 1rem;
  margin: 0;
}

.plugin-lxmf-log-header span,
.plugin-lxmf-log-entry time {
  color: #8fe3ff;
  font-family: var(--font-ui);
  font-size: 0.72rem;
}

.plugin-lxmf-log-list {
  display: grid;
  gap: 0.5rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.plugin-lxmf-log-entry {
  background: rgb(6 17 38 / 68%);
  border: 1px solid rgb(70 110 174 / 30%);
  border-radius: 8px;
  padding: 0.5rem 0.58rem;
}

.plugin-lxmf-log-entry div {
  display: grid;
  gap: 0.16rem;
  min-width: 0;
}

.plugin-lxmf-log-entry strong,
.plugin-lxmf-log-entry span {
  overflow-wrap: anywhere;
}

.plugin-lxmf-log-entry strong {
  color: #d5eaff;
  font-family: var(--font-body);
  font-size: 0.86rem;
}

.plugin-lxmf-log-entry span {
  color: #8fa9d1;
  font-family: var(--font-body);
  font-size: 0.76rem;
}
</style>
