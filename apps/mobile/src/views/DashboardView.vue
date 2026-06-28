<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";

import {
  listNotificationActivity,
  subscribeNotificationActivity,
  type NotificationActivityRecord,
} from "../services/notifications";
import { useChecklistsStore } from "../stores/checklistsStore";
import { useEventsStore } from "../stores/eventsStore";
import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import { useWearablesStore } from "../stores/wearablesStore";

const checklistsStore = useChecklistsStore();
const { dashboardSummary } = storeToRefs(checklistsStore);
const eventsStore = useEventsStore();
const messagesStore = useMessagesStore();
const messagingStore = useMessagingStore();
const nodeStore = useNodeStore();
const wearablesStore = useWearablesStore();
const notificationActivities = shallowRef<NotificationActivityRecord[]>(listNotificationActivity());
let unsubscribeNotificationActivity: (() => void) | null = null;
const dashboardActionTitle = computed(() =>
  nodeStore.ready
    ? "Send runtime command"
    : "Node is not ready yet. Wait for the top-right status to show Ready.",
);

const announceIconPaths = [
  "m3 11 14-6v14L3 13v-2Z",
  "M17 9.5h2a2 2 0 0 1 0 4h-2",
  "M6 13v5",
];

const syncIconPaths = [
  "M21 12a9 9 0 0 1-15.4 6.36L3 16",
  "M3 21v-5h5",
  "M3 12A9 9 0 0 1 18.4 5.64L21 8",
  "M21 3v5h-5",
];

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

const ringMetrics = computed(() =>
  messagesStore.eamReadinessSummary.statusMetrics.map((metric) => ({
    key: metric.field,
    label: metric.label,
    color: metric.ringColor,
    pct: metric.score,
    href: "/messages",
  })),
);

const checklistSummaryMetrics = computed(() => [
  {
    key: "total",
    value: dashboardSummary.value.total,
    label: "Total",
    href: "/checklists",
    alert: false,
  },
  {
    key: "active",
    value: dashboardSummary.value.active,
    label: "Active",
    href: "/checklists",
    alert: false,
  },
  {
    key: "late",
    value: dashboardSummary.value.late,
    label: "Late",
    href: "/checklists",
    alert: true,
  },
]);

const activitySummaryMetrics = computed(() => [
  {
    key: "messages",
    value: messagesStore.activeCount,
    label: "EAM",
    href: "/messages",
    alert: false,
  },
  {
    key: "events",
    value: eventsStore.records.length,
    label: "EVN",
    href: "/events",
    alert: false,
  },
  {
    key: "threads",
    value: messagingStore.conversations.length,
    label: "Threads",
    href: "/inbox",
    alert: false,
  },
]);

const wearableMetrics = computed(() =>
  wearablesStore.wearableStatus.map((status) => ({
    key: `${status.deviceId}:${status.sensorType}`,
    name: status.deviceName || "Generic BLE Heart Rate Device",
    bpm: status.sensorType === "heart_rate_bpm" ? status.value : "-",
    status: status.status,
    operator: status.operatorRnsIdentity || "Unassigned",
    lastSeen: status.lastSeenTimestampMs > 0
      ? new Date(status.lastSeenTimestampMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      : "-",
  })),
);

function activityHref(activity: NotificationActivityRecord): string {
  const route = activity.route?.trim();
  if (!route) {
    return "";
  }
  const params = new URLSearchParams();
  if (activity.conversationId) {
    params.set("conversation", activity.conversationId);
  }
  if (activity.messageIdHex) {
    params.set("message", activity.messageIdHex);
  }
  const query = params.toString();
  return query ? `${route}?${query}` : route;
}

function activityTone(activity: NotificationActivityRecord): string {
  const route = activity.route?.trim().toLowerCase() ?? "";
  if (route.startsWith("/inbox")) {
    return "chat";
  }
  if (route.startsWith("/events")) {
    return "event";
  }
  if (route.startsWith("/checklists")) {
    return "checklist";
  }
  if (route.startsWith("/messages")) {
    return "eam";
  }
  return "default";
}

function formatActivityTime(timestamp: number): string {
  if (!timestamp) {
    return "";
  }
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
  }).format(timestamp);
}

function refreshNotificationActivities(): void {
  notificationActivities.value = listNotificationActivity();
}

onMounted(() => {
  void checklistsStore.refreshLive();
  void wearablesStore.init();
  refreshNotificationActivities();
  unsubscribeNotificationActivity = subscribeNotificationActivity(refreshNotificationActivities);
});

onUnmounted(() => {
  unsubscribeNotificationActivity?.();
  unsubscribeNotificationActivity = null;
});
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <button
          type="button"
          class="dashboard-chip action-chip"
          :disabled="!nodeStore.ready"
          :title="dashboardActionTitle"
          @click="announceNow"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path v-for="path in announceIconPaths" :key="path" :d="path" />
          </svg>
          <span>Announce</span>
        </button>
        <button
          type="button"
          class="dashboard-chip action-chip"
          :disabled="!nodeStore.ready"
          :title="dashboardActionTitle"
          @click="requestSync"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path v-for="path in syncIconPaths" :key="path" :d="path" />
          </svg>
          <span>Sync</span>
        </button>
      </div>
    </header>

    <section class="panel">
      <h2>Team Status</h2>
      <div class="rings">
        <RouterLink class="ring-card" v-for="ring in ringMetrics" :key="ring.key" :to="ring.href">
          <div class="ring-visual">
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
          </div>
          <p class="ring-label">{{ ring.label }}</p>
        </RouterLink>
      </div>
    </section>

    <section class="panel">
      <h2>Activity</h2>
      <div class="summary-grid activity-grid">
        <RouterLink
          v-for="metric in activitySummaryMetrics"
          :key="metric.key"
          class="summary-metric"
          :class="{ 'summary-metric-alert': metric.alert }"
          :to="metric.href"
        >
          <p class="summary-value">{{ metric.value }}</p>
          <p class="summary-label">{{ metric.label }}</p>
        </RouterLink>
      </div>
      <h3 class="activity-subheading">Checklists</h3>
      <div class="summary-grid checklist-grid">
        <RouterLink
          v-for="metric in checklistSummaryMetrics"
          :key="metric.key"
          class="summary-metric"
          :class="{ 'summary-metric-alert': metric.alert }"
          :to="metric.href"
        >
          <p class="summary-value">{{ metric.value }}</p>
          <p class="summary-label">{{ metric.label }}</p>
        </RouterLink>
      </div>
      <h3 class="activity-subheading">Logs</h3>
      <div class="activity-list" aria-label="Logs">
        <component
          :is="activityHref(activity) ? 'RouterLink' : 'article'"
          v-for="activity in notificationActivities.slice(0, 5)"
          :key="activity.id"
          class="activity-item"
          :class="`activity-${activityTone(activity)}`"
          v-bind="activityHref(activity) ? { to: activityHref(activity) } : {}"
        >
          <span class="activity-dot" aria-hidden="true" />
          <span class="activity-copy">
            <strong>{{ activity.title }}</strong>
            <span>{{ activity.body }}</span>
          </span>
          <time :datetime="new Date(activity.at).toISOString()">{{ formatActivityTime(activity.at) }}</time>
        </component>
        <p v-if="notificationActivities.length === 0" class="activity-empty">
          No logs yet.
        </p>
      </div>
    </section>

    <section class="panel">
      <h2>Wearables</h2>
      <div v-if="wearableMetrics.length" class="wearable-list">
        <article v-for="metric in wearableMetrics" :key="metric.key" class="wearable-row">
          <div>
            <strong>{{ metric.name }}</strong>
            <span>{{ metric.operator }}</span>
          </div>
          <p>{{ metric.bpm }} bpm</p>
          <span>{{ metric.status }} | {{ metric.lastSeen }}</span>
        </article>
      </div>
      <p v-else class="empty-copy">No wearable heart-rate data.</p>
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
  text-decoration: none;
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

.action-chip:disabled {
  cursor: not-allowed;
  opacity: 0.56;
}

.panel {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
  padding: 0.9rem;
}

.ring-card:focus-visible,
.summary-metric:focus-visible,
.activity-item:focus-visible,
.dashboard-chip:focus-visible {
  outline: 2px solid rgb(111 219 255 / 72%);
  outline-offset: 2px;
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
  text-decoration: none;
}

.ring-visual {
  display: grid;
  place-items: center;
  position: relative;
}

.ring-visual svg {
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
  font-size: 1.02rem;
  font-weight: 700;
  left: 50%;
  margin: 0;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
}

.ring-label {
  color: #88a5cf;
  font-family: var(--font-ui);
  font-size: 0.75rem;
  letter-spacing: 0.09em;
  margin: 0.13rem 0 0;
  text-transform: uppercase;
}

.summary-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 0.75rem;
}

.activity-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.checklist-grid {
  margin-top: 0.55rem;
}

.activity-subheading {
  color: #a8d7ff;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.11em;
  margin: 0.9rem 0 0;
  text-transform: uppercase;
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
  text-decoration: none;
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

.activity-list {
  display: grid;
  gap: 0.52rem;
  margin-top: 0.55rem;
  max-height: 18.25rem;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-right: 0.18rem;
  scrollbar-color: rgb(88 187 255 / 55%) rgb(7 20 45 / 72%);
  scrollbar-width: thin;
}

.activity-item {
  align-items: center;
  background: rgb(5 19 43 / 72%);
  border: 1px solid rgb(85 136 205 / 25%);
  border-radius: 8px;
  color: #dceeff;
  display: grid;
  gap: 0.58rem;
  grid-template-columns: auto minmax(0, 1fr) auto;
  min-height: 3.25rem;
  padding: 0.58rem 0.68rem;
  text-decoration: none;
}

.activity-dot {
  background: #66d9ff;
  border-radius: 999px;
  box-shadow: 0 0 12px rgb(102 217 255 / 38%);
  height: 0.52rem;
  width: 0.52rem;
}

.activity-chat .activity-dot {
  background: #66d9ff;
}

.activity-event .activity-dot {
  background: #ffd36e;
}

.activity-checklist .activity-dot {
  background: #8df3c1;
}

.activity-eam .activity-dot {
  background: #ff8fa0;
}

.activity-copy {
  display: grid;
  gap: 0.12rem;
  min-width: 0;
}

.activity-copy strong,
.activity-copy span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.activity-copy strong {
  color: #f3fbff;
  font-family: var(--font-headline);
  font-size: 0.88rem;
}

.activity-copy span,
.activity-empty {
  color: #8ea8d1;
  font-family: var(--font-body);
  font-size: 0.78rem;
}

.activity-item time {
  color: #74a6d5;
  font-family: var(--font-ui);
  font-size: 0.68rem;
}

.activity-empty {
  margin: 0.2rem 0 0;
}

.wearable-list {
  display: grid;
  gap: 0.6rem;
  margin-top: 0.75rem;
}

.wearable-row {
  align-items: center;
  background: rgb(7 20 44 / 72%);
  border: 1px solid rgb(67 106 165 / 30%);
  border-radius: 8px;
  display: grid;
  gap: 0.65rem;
  grid-template-columns: minmax(0, 1fr) auto auto;
  padding: 0.7rem 0.8rem;
}

.wearable-row strong,
.wearable-row span,
.wearable-row p,
.empty-copy {
  font-family: var(--font-body);
}

.wearable-row strong,
.wearable-row span {
  display: block;
  overflow-wrap: anywhere;
}

.wearable-row strong,
.wearable-row p {
  color: #d5eaff;
}

.wearable-row p {
  font-family: var(--font-ui);
  font-weight: 700;
  margin: 0;
}

.wearable-row span,
.empty-copy {
  color: #96afd5;
}

.empty-copy {
  margin: 0.75rem 0 0;
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

  .ring-visual svg {
    height: 84px;
    width: 84px;
  }

  .wearable-row {
    grid-template-columns: 1fr;
  }
}
</style>
