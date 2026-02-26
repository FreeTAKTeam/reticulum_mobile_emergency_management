<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";

import { useMessagesStore } from "../stores/messagesStore";
import { useNodeStore } from "../stores/nodeStore";

const nodeStore = useNodeStore();
const messagesStore = useMessagesStore();
messagesStore.init();

const ringMetrics = computed(() => [
  {
    key: "medical",
    label: "Medical",
    pct: Math.max(20, 100 - messagesStore.redCount * 5),
    color: "#4aa3ff",
  },
  {
    key: "comms",
    label: "Comms",
    pct: Math.max(20, 82 - messagesStore.redCount * 3),
    color: "#18e5ff",
  },
  {
    key: "mobility",
    label: "Mobility",
    pct: Math.max(20, 76 - messagesStore.redCount * 4),
    color: "#ffc92e",
  },
]);
</script>

<template>
  <section class="view">
    <header class="headline">
      <h1>Emergency Ops Dashboard</h1>
      <p>Mesh readiness, resource load, and peer control.</p>
    </header>

    <section class="panel">
      <h2>Mesh Network Connectivity</h2>
      <div class="wave-preview"></div>
      <div class="ticks">
        <span>12 AM</span>
        <span>6 AM</span>
        <span>12 PM</span>
        <span>6 PM</span>
      </div>
    </section>

    <section class="panel">
      <h2>Resource Allocation</h2>
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

    <section class="panel peers-entry">
      <div>
        <h2>Peers &amp; Discovery</h2>
        <p>
          Discovered: {{ nodeStore.discoveredPeers.length }} | Saved:
          {{ nodeStore.savedPeers.length }} | Connected:
          {{ nodeStore.connectedDestinations.length }}
        </p>
      </div>
      <RouterLink to="/peers" class="open-btn">Open</RouterLink>
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

.wave-preview {
  background:
    radial-gradient(circle at 50% 36%, rgb(66 234 255 / 44%), transparent 44%),
    linear-gradient(180deg, rgb(4 13 31 / 96%), rgb(8 25 54 / 96%));
  border: 1px solid rgb(85 132 196 / 31%);
  border-radius: 10px;
  height: 190px;
  margin-top: 0.75rem;
  position: relative;
}

.wave-preview::before {
  animation: drift 5.4s linear infinite;
  background:
    radial-gradient(circle at 26% 50%, rgb(58 238 255 / 80%), transparent 34%),
    radial-gradient(circle at 66% 52%, rgb(84 196 255 / 80%), transparent 34%);
  content: "";
  inset: 0;
  mix-blend-mode: screen;
  position: absolute;
}

@keyframes drift {
  0% {
    transform: translateX(-6%);
  }
  50% {
    transform: translateX(6%);
  }
  100% {
    transform: translateX(-6%);
  }
}

.ticks {
  color: #7d9bc3;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  justify-content: space-between;
  letter-spacing: 0.08em;
  margin-top: 0.44rem;
  text-transform: uppercase;
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

.peers-entry {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.peers-entry p {
  color: #95acd0;
  font-family: var(--font-body);
  margin: 0.2rem 0 0;
}

.open-btn {
  background: linear-gradient(115deg, #00a2ff, #2df2ff);
  border-radius: 11px;
  color: #03274d;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  font-weight: 700;
  letter-spacing: 0.1em;
  min-width: 92px;
  padding: 0.58rem 0.85rem;
  text-align: center;
  text-decoration: none;
  text-transform: uppercase;
}

@media (max-width: 700px) {
  .rings {
    grid-template-columns: 1fr;
  }

  .peers-entry {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.7rem;
  }
}
</style>
