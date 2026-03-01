<script setup lang="ts">
import { computed } from "vue";

import { useMessagesStore } from "../stores/messagesStore";
import { getStatusScore } from "../utils/actionMessageStatus";

type GaugeField = "medicalStatus" | "commsStatus" | "mobilityStatus";

const GAUGE_CONFIG: Array<{
  key: string;
  label: string;
  field: GaugeField;
  color: string;
}> = [
  {
    key: "medical",
    label: "Medical",
    field: "medicalStatus",
    color: "#4aa3ff",
  },
  {
    key: "comms",
    label: "Comms",
    field: "commsStatus",
    color: "#18e5ff",
  },
  {
    key: "mobility",
    label: "Mobility",
    field: "mobilityStatus",
    color: "#ffc92e",
  },
];

const messagesStore = useMessagesStore();
messagesStore.init();

function averageScoreFor(field: GaugeField): number {
  const messages = messagesStore.messages;
  const totalMessages = messages.length;
  if (totalMessages === 0) {
    return 0;
  }

  const weightedTotal = messages.reduce((sum, message) => {
    return sum + getStatusScore(message[field]);
  }, 0);

  return Math.round(weightedTotal / totalMessages);
}

const ringMetrics = computed(() =>
  GAUGE_CONFIG.map((gauge) => ({
    key: gauge.key,
    label: gauge.label,
    color: gauge.color,
    pct: averageScoreFor(gauge.field),
  })),
);
</script>

<template>
  <section class="view">
    <header class="headline">
      <h1>Emergency Ops Dashboard</h1>
      <p>Status-weighted readiness from active action messages.</p>
    </header>

    <section class="panel">
      <h2>Operational Status</h2>
      <div class="rings">
        <div class="ring" v-for="ring in ringMetrics" :key="ring.key">
          <svg viewBox="0 0 120 120">
            <circle cx="60" cy="60" r="44" class="ring-bg" />
            <circle
              cx="60"
              cy="60"
              r="44"
              class="ring-fg"
              :style="{
                '--ring-color': ring.color,
                '--ring-pct': ring.pct,
              }"
            />
          </svg>
          <p class="ring-value">{{ ring.pct }}%</p>
          <p class="ring-label">{{ ring.label }}</p>
        </div>
      </div>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.headline h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.8rem, 3.4vw, 2.9rem);
  margin: 0;
}

.headline p {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0.28rem 0 0;
}

.panel {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
  padding: 0.9rem;
}

h2 {
  font-family: var(--font-headline);
  font-size: 1.56rem;
  margin: 0;
}

.rings {
  display: grid;
  gap: 0.7rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 0.75rem;
}

.ring {
  align-items: center;
  display: grid;
  justify-items: center;
}

svg {
  height: 110px;
  width: 110px;
}

.ring-bg {
  fill: none;
  opacity: 0.28;
  stroke: #234160;
  stroke-width: 12px;
}

.ring-fg {
  fill: none;
  stroke: var(--ring-color);
  stroke-dasharray: 276.46;
  stroke-dashoffset: calc(276.46 - (276.46 * var(--ring-pct) / 100));
  stroke-linecap: round;
  stroke-width: 12px;
  transform: rotate(-90deg);
  transform-origin: 50% 50%;
}

.ring-value {
  color: #deefff;
  font-family: var(--font-ui);
  font-size: 1rem;
  margin: -0.35rem 0 0;
}

.ring-label {
  color: #88a5cf;
  font-family: var(--font-ui);
  font-size: 0.75rem;
  letter-spacing: 0.09em;
  margin: 0.13rem 0 0;
  text-transform: uppercase;
}

@media (max-width: 700px) {
  .rings {
    grid-template-columns: 1fr;
  }
}
</style>
