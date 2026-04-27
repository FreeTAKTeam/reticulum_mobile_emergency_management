<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted } from "vue";

import { useChecklistsStore } from "../stores/checklistsStore";
import { useEventsStore } from "../stores/eventsStore";
import { useMessagesStore } from "../stores/messagesStore";
import { useNodeStore } from "../stores/nodeStore";
import {
  ACTION_MESSAGE_STATUS_CONFIG,
  getOverallRingColor,
  getOverallStatusBand,
  getStatusScore,
  type ActionMessageStatusField,
} from "../utils/actionMessageStatus";

const checklistsStore = useChecklistsStore();
const { dashboardSummary } = storeToRefs(checklistsStore);
const eventsStore = useEventsStore();
const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();

async function announceNow(): Promise<void> {
  try {
    await nodeStore.announceNow();
  } catch {
    // nodeStore already records the failure for the status surface
  }
}

async function requestSync(): Promise<void> {
  try {
    await nodeStore.requestLxmfSync();
  } catch {
    // current runtime reports sync failure through store state
  }
}

function averageScoreFor(field: ActionMessageStatusField): number {
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
  ACTION_MESSAGE_STATUS_CONFIG.map((status) => {
    const pct = averageScoreFor(status.field);
    return {
      key: status.field,
      label: status.label,
      color: getOverallRingColor(pct),
      band: getOverallStatusBand(pct),
      pct,
    };
  }),
);

const checklistSummaryMetrics = computed(() => [
  {
    key: "total",
    value: dashboardSummary.value.total,
    label: "Total",
    alert: false,
  },
  {
    key: "active",
    value: dashboardSummary.value.active,
    label: "Active",
    alert: false,
  },
  {
    key: "late",
    value: dashboardSummary.value.late,
    label: "Late",
    alert: true,
  },
]);

const activitySummaryMetrics = computed(() => [
  {
    key: "messages",
    value: messagesStore.activeCount,
    label: "MSG",
    alert: false,
  },
  {
    key: "events",
    value: eventsStore.records.length,
    label: "EVN",
    alert: false,
  },
]);

onMounted(() => {
  void checklistsStore.refreshLive();
});
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <button type="button" class="dashboard-chip action-chip" @click="announceNow">
          Announce
        </button>
        <button type="button" class="dashboard-chip action-chip" @click="requestSync">
          Sync
        </button>
      </div>
    </header>

    <section class="panel">
      <h2>Team Status</h2>
      <div class="rings">
        <article class="ring-card" v-for="ring in ringMetrics" :key="ring.key">
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
          <p class="ring-value" :style="{ color: ring.color }">{{ ring.pct }}%</p>
          <p class="ring-label">{{ ring.label }}</p>
          <p class="ring-band">{{ ring.band }}</p>
        </article>
      </div>
    </section>

    <section class="panel">
      <h2>Checklists</h2>
      <div class="summary-grid">
        <article
          v-for="metric in checklistSummaryMetrics"
          :key="metric.key"
          class="summary-metric"
          :class="{ 'summary-metric-alert': metric.alert }"
        >
          <p class="summary-value">{{ metric.value }}</p>
          <p class="summary-label">{{ metric.label }}</p>
        </article>
      </div>
    </section>

    <section class="panel">
      <h2>Activity</h2>
      <div class="summary-grid activity-grid">
        <article
          v-for="metric in activitySummaryMetrics"
          :key="metric.key"
          class="summary-metric"
          :class="{ 'summary-metric-alert': metric.alert }"
        >
          <p class="summary-value">{{ metric.value }}</p>
          <p class="summary-label">{{ metric.label }}</p>
        </article>
      </div>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.view-header {
  align-items: center;
  display: block;
}

.header-actions {
  align-items: center;
  display: grid;
  gap: 0.55rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.4rem, 3vw, 2.4rem);
  line-height: 1;
  margin: 0;
}

.view-header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(1rem, 1.6vw, 1.3rem);
  margin: 0.2rem 0 0;
}

.badge {
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  color: #64beff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.92rem;
  justify-content: center;
  letter-spacing: 0.08em;
  padding: 0.46rem 0.8rem;
  text-transform: uppercase;
}

.badge-button {
  --btn-bg: rgb(9 61 108 / 68%);
  --btn-bg-pressed: linear-gradient(180deg, rgb(199 241 255 / 96%), rgb(132 219 255 / 94%));
  --btn-border: rgb(73 173 255 / 62%);
  --btn-border-pressed: rgb(234 251 255 / 88%);
  --btn-shadow: inset 0 1px 0 rgb(186 236 255 / 8%), 0 8px 18px rgb(3 24 56 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 18 40 / 20%);
  --btn-color: #64beff;
  --btn-color-pressed: #063050;
  box-shadow:
    inset 0 1px 0 rgb(186 236 255 / 8%),
    0 8px 18px rgb(3 24 56 / 18%);
  cursor: pointer;
  min-height: 0;
}

.badge-button:focus-visible {
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 2px;
}

.dashboard-chip {
  align-items: center;
  background: rgb(7 25 54 / 84%);
  border: 1px solid rgb(73 173 255 / 48%);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 18px rgb(33 153 255 / 7%);
  color: #8fcaff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: clamp(0.76rem, 1.85vw, 0.95rem);
  font-weight: 700;
  gap: 0.48rem;
  justify-content: center;
  min-height: 2.85rem;
  min-width: 0;
  padding: 0.44rem 0.62rem;
  text-transform: none;
}

.dashboard-chip svg {
  flex: 0 0 auto;
  height: 1.08rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.08rem;
}

.dashboard-chip span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ready-chip {
  border-color: rgb(65 227 106 / 48%);
  color: #2fff73;
}

.ready-chip.offline {
  border-color: rgb(255 196 76 / 55%);
  color: #ffd36e;
}

.action-chip {
  --btn-bg: rgb(7 25 54 / 84%);
  --btn-border: rgb(73 173 255 / 48%);
  --btn-color: #8fcaff;
  cursor: pointer;
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
  font-size: clamp(1.2rem, 2.4vw, 1.56rem);
  margin: 0;
}

.rings {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 0.75rem;
}

.ring-card {
  align-items: center;
  display: grid;
  background:
    linear-gradient(145deg, rgb(18 35 68 / 92%), rgb(10 20 45 / 90%)),
    radial-gradient(circle at 72% 10%, rgb(69 235 255 / 14%), transparent 36%);
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 14px;
  gap: 0.12rem;
  justify-items: center;
  padding: 0.72rem 0.5rem 0.66rem;
}

svg {
  height: 94px;
  width: 94px;
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
  font-family: var(--font-ui);
  font-size: 1.05rem;
  font-weight: 700;
  margin: -0.08rem 0 0;
}

.ring-label {
  color: #88a5cf;
  font-family: var(--font-ui);
  font-size: 0.75rem;
  letter-spacing: 0.09em;
  margin: 0.13rem 0 0;
  text-transform: uppercase;
}

.ring-band {
  color: #9fb7d8;
  font-family: var(--font-ui);
  font-size: 0.69rem;
  letter-spacing: 0.08em;
  margin: 0.06rem 0 0;
  text-transform: uppercase;
}

.summary-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 0.75rem;
}

.activity-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.summary-metric {
  align-items: center;
  background:
    linear-gradient(145deg, rgb(18 35 68 / 92%), rgb(10 20 45 / 90%)),
    radial-gradient(circle at 72% 10%, rgb(69 235 255 / 14%), transparent 36%);
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 14px;
  display: grid;
  gap: 0.08rem;
  justify-items: center;
  min-height: 114px;
  padding: 0.85rem 0.45rem 0.72rem;
}

.summary-value {
  color: #f0f7ff;
  font-family: var(--font-ui);
  font-size: clamp(2.45rem, 4.6vw, 3.3rem);
  font-weight: 700;
  line-height: 1;
  margin: 0;
}

.summary-label {
  color: #88a5cf;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.09em;
  margin: 0.13rem 0 0;
  text-transform: uppercase;
}

.summary-metric-alert .summary-value,
.summary-metric-alert .summary-label {
  color: #ff6475;
}

@media (max-width: 720px) {
  h1 {
    font-size: 1.1rem;
  }

  .view-header {
    align-items: stretch;
  }

  .header-actions {
    gap: 0.5rem;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .action-chip {
    grid-column: auto;
  }

  .dashboard-chip {
    font-size: 0.62rem;
    gap: 0.24rem;
    min-height: 2.32rem;
    padding-inline: 0.26rem;
  }

  .dashboard-chip svg {
    height: 0.78rem;
    width: 0.78rem;
  }

  .ring-card {
    padding-inline: 0.32rem;
  }

  .summary-grid {
    gap: 0.5rem;
  }

  .summary-metric {
    min-height: 102px;
    padding-inline: 0.32rem;
  }

  .summary-value {
    font-size: clamp(2rem, 7vw, 2.5rem);
  }

  .summary-label {
    font-size: 0.68rem;
  }

  svg {
    height: 84px;
    width: 84px;
  }
}
</style>
