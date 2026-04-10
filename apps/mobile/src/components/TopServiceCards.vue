<script setup lang="ts">
import { computed } from "vue";

import type { NodeUiSettings } from "../types/domain";

const props = defineProps<{
  running: boolean;
  connectedCount: number;
  settings: NodeUiSettings;
}>();

const gatewayStatus = computed(() => {
  if (props.settings.hub.mode === "Disabled") {
    return "Hub disabled";
  }
  if (props.settings.hub.mode === "RchHttp") {
    return "Legacy HTTP";
  }
  if (props.settings.hub.identityHash) {
    return `RCH ${props.settings.hub.identityHash.slice(0, 8)}...`;
  }
  return "RCH ListClients";
});

const cards = computed(() => [
  {
    key: "mesh",
    title: "Mesh Network",
    value: props.running ? "Active" : "Awaiting updates...",
  },
  {
    key: "gateway",
    title: "Gateway",
    value: gatewayStatus.value,
  },
  {
    key: "package",
    title: "Data Package",
    value: props.running ? "Awaiting sync" : "Standby",
  },
  {
    key: "api",
    title: "API & Federations",
    value: props.running ? "Listening" : "Idle",
  },
  {
    key: "cot",
    title: "COT Network",
    value: props.running ? "Standby" : "Offline",
  },
  {
    key: "clients",
    title: "Connected Clients",
    value: String(props.connectedCount),
  },
]);
</script>

<template>
  <section class="cards">
    <article v-for="card in cards" :key="card.key" class="card">
      <p class="card-title">{{ card.title }}</p>
      <p class="card-value">{{ card.value }}</p>
    </article>
  </section>
</template>

<style scoped>
.cards {
  display: grid;
  gap: 0.8rem;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
}

.card {
  backdrop-filter: blur(5px);
  background:
    radial-gradient(circle at 85% 15%, rgb(52 211 255 / 20%), transparent 45%),
    linear-gradient(135deg, rgb(20 37 72 / 95%), rgb(13 23 47 / 95%));
  border: 1px solid rgb(75 130 209 / 28%);
  border-radius: 16px;
  min-height: 78px;
  padding: 0.8rem 0.95rem;
}

.card-title {
  color: #7da0d6;
  font-family: var(--font-ui);
  font-size: 0.73rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  margin: 0;
  text-transform: uppercase;
}

.card-value {
  color: #dff3ff;
  font-family: var(--font-body);
  font-size: 1.04rem;
  font-weight: 700;
  margin: 0.18rem 0 0;
}
</style>
