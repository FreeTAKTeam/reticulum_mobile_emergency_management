<script setup lang="ts">
import { computed, shallowRef } from "vue";

import StatusPill from "./StatusPill.vue";

import type { ActionMessage } from "../types/domain";
import {
  ACTION_MESSAGE_STATUS_CONFIG,
  type ActionMessageStatusField,
  getMessageOverallScore,
  getOverallRingColor,
  getOverallStatusBand,
} from "../utils/actionMessageStatus";
import { formatR3aktTeamColor } from "../utils/r3akt";

const props = defineProps<{
  message: ActionMessage;
}>();

const emit = defineEmits<{
  edit: [callsign: string];
  delete: [callsign: string];
  cycle: [callsign: string, field: keyof ActionMessage];
}>();

const isExpanded = shallowRef(false);

const formattedTeam = computed(() => formatR3aktTeamColor(props.message.groupName));
const overallScore = computed(() => getMessageOverallScore(props.message));
const overallColor = computed(() => getOverallRingColor(overallScore.value));
const overallBand = computed(() => getOverallStatusBand(overallScore.value));
const ringOffset = computed(() => 276.46 - ((276.46 * overallScore.value) / 100));
const toggleLabel = computed(() => (isExpanded.value ? "Hide statuses" : "Show statuses"));
const overallTitle = computed(() => `Overall readiness ${overallScore.value}% (${overallBand.value})`);

function toggleStatuses(): void {
  isExpanded.value = !isExpanded.value;
}

function cycleStatus(field: ActionMessageStatusField): void {
  emit("cycle", props.message.callsign, field);
}
</script>

<template>
  <article class="item">
    <header class="item-header">
      <div class="identity">
        <div class="identity-copy">
          <p class="eyebrow">Call Sign</p>
          <h3 class="callsign">{{ props.message.callsign }}</h3>
          <p class="group">Team: {{ formattedTeam }}</p>
        </div>

        <div class="overall" :style="{ '--overall-color': overallColor }" :title="overallTitle">
          <svg class="overall-chart" viewBox="0 0 120 120" aria-hidden="true">
            <circle class="overall-ring-bg" cx="60" cy="60" r="44" />
            <circle
              class="overall-ring-fg"
              cx="60"
              cy="60"
              r="44"
              :style="{ '--ring-offset': ringOffset }"
            />
          </svg>
          <div class="overall-copy">
            <p class="overall-label">Overall</p>
            <p class="overall-value">{{ overallScore }}%</p>
            <p class="overall-band">{{ overallBand }}</p>
          </div>
        </div>
      </div>

      <div class="controls">
        <button
          class="status-toggle"
          type="button"
          :aria-expanded="isExpanded"
          @click="toggleStatuses"
        >
          <span>{{ toggleLabel }}</span>
          <svg class="toggle-icon" :class="{ open: isExpanded }" viewBox="0 0 24 24" fill="none">
            <path d="M7 10.5 12 15.5 17 10.5" />
          </svg>
        </button>

        <div class="item-actions">
          <button class="action edit" type="button" @click="emit('edit', props.message.callsign)">
            Edit
          </button>
          <button class="action delete" type="button" @click="emit('delete', props.message.callsign)">
            Delete
          </button>
        </div>
      </div>
    </header>

    <section v-show="isExpanded" class="status-grid">
      <button
        v-for="status in ACTION_MESSAGE_STATUS_CONFIG"
        :key="status.field"
        type="button"
        class="pill-button"
        @click="cycleStatus(status.field)"
      >
        <StatusPill :label="status.label" :value="props.message[status.field]" />
      </button>
    </section>
  </article>
</template>

<style scoped>
.item {
  background:
    linear-gradient(145deg, rgb(18 35 68 / 92%), rgb(10 20 45 / 90%)),
    radial-gradient(circle at 72% 10%, rgb(69 235 255 / 16%), transparent 34%);
  border: 1px solid rgb(90 142 220 / 25%);
  border-radius: 16px;
  padding: 1rem;
}

.item-header {
  display: grid;
  gap: 0.95rem;
}

.identity {
  align-items: center;
  display: flex;
  gap: 0.95rem;
  justify-content: space-between;
}

.identity-copy {
  min-width: 0;
}

.eyebrow {
  color: #7ea6dc;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.12em;
  margin: 0;
  text-transform: uppercase;
}

.callsign {
  font-family: var(--font-headline);
  font-size: clamp(1.2rem, 2.3vw, 1.75rem);
  margin: 0.18rem 0 0;
}

.group {
  color: #9fb6d8;
  font-family: var(--font-body);
  font-size: 0.96rem;
  margin: 0.22rem 0 0;
}

.overall {
  align-items: center;
  display: inline-flex;
  flex-shrink: 0;
  gap: 0.7rem;
}

.overall-chart {
  height: 66px;
  width: 66px;
}

.overall-ring-bg {
  fill: none;
  opacity: 0.28;
  stroke: #234160;
  stroke-width: 12px;
}

.overall-ring-fg {
  fill: none;
  stroke: var(--overall-color);
  stroke-dasharray: 276.46;
  stroke-dashoffset: var(--ring-offset);
  stroke-linecap: round;
  stroke-width: 12px;
  transform: rotate(-90deg);
  transform-origin: 50% 50%;
}

.overall-copy {
  text-align: right;
}

.overall-label,
.overall-band {
  color: #93add4;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  margin: 0;
  text-transform: uppercase;
}

.overall-value {
  color: var(--overall-color);
  font-family: var(--font-headline);
  font-size: 1.36rem;
  line-height: 1;
  margin: 0.1rem 0;
}

.controls {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.7rem;
  justify-content: space-between;
}

.status-toggle,
.action {
  border-radius: 12px;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 38px;
  padding: 0 0.9rem;
  text-transform: uppercase;
}

.status-toggle {
  align-items: center;
  background: rgb(7 28 59 / 86%);
  border: 1px solid rgb(72 120 190 / 46%);
  color: #9bc2eb;
  display: inline-flex;
  gap: 0.45rem;
}

.toggle-icon {
  height: 1rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
  transform: rotate(0deg);
  transition: transform 0.18s ease;
  width: 1rem;
}

.toggle-icon.open {
  transform: rotate(180deg);
}

.item-actions {
  display: flex;
  gap: 0.65rem;
}

.action {
  border: 0;
}

.edit {
  background: rgb(11 39 84 / 80%);
  border: 1px solid rgb(66 169 255 / 80%);
  box-shadow: 0 0 16px rgb(66 169 255 / 24%);
  color: #61bbff;
}

.delete {
  background: rgb(53 15 25 / 70%);
  border: 1px solid rgb(255 70 91 / 84%);
  box-shadow: 0 0 16px rgb(255 72 104 / 24%);
  color: #ff7b89;
}

.status-grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 190px), 1fr));
  margin-top: 0.9rem;
}

.pill-button {
  background: transparent;
  border: 0;
  cursor: pointer;
  display: block;
  padding: 0;
  width: 100%;
}

.pill-button :deep(.pill) {
  box-sizing: border-box;
  margin: 0;
  min-height: 3rem;
  padding: 0.72rem 0.95rem;
  width: 100%;
}

@media (max-width: 640px) {
  .identity {
    align-items: flex-start;
    flex-direction: column;
  }

  .overall {
    width: 100%;
  }

  .overall-copy {
    text-align: left;
  }

  .controls,
  .item-actions {
    width: 100%;
  }

  .item-actions {
    justify-content: space-between;
  }

  .action,
  .status-toggle {
    flex: 1 1 auto;
    justify-content: center;
  }
}
</style>
