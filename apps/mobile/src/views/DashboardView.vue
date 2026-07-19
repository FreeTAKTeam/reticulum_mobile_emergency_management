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
import { runDetachedStoreTask } from "../utils/detachedStoreTask";

const checklistsStore = useChecklistsStore();
const { dashboardSummary } = storeToRefs(checklistsStore);
const eventsStore = useEventsStore();
const messagesStore = useMessagesStore();
const messagingStore = useMessagingStore();
const nodeStore = useNodeStore();
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

const pluginSensorCards = computed(() => nodeStore.pluginSensors.map((sensor) => ({
  ...sensor,
  formattedValue: `${String(sensor.value)}${sensor.unit ? ` ${sensor.unit}` : ""}`,
  lastSeen: sensor.sampleAtMs > 0
    ? new Date(sensor.sampleAtMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : "-",
})));

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
  runDetachedStoreTask(nodeStore, "dashboard", "checklist refresh", checklistsStore.refreshLive);
  runDetachedStoreTask(nodeStore, "dashboard", "plugin sensor refresh", nodeStore.refreshPluginSensors);
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
      <h3 v-if="pluginSensorCards.length" class="activity-subheading">Plugin sensors</h3>
      <div v-if="pluginSensorCards.length" class="plugin-sensor-grid">
        <article
          v-for="sensor in pluginSensorCards"
          :key="`${sensor.pluginId}:${sensor.deviceId}:${sensor.sensorType}`"
          class="plugin-sensor-card"
        >
          <div>
            <strong>{{ sensor.displayName }}</strong>
            <span>{{ sensor.operatorRnsIdentity || sensor.sensorType }}</span>
          </div>
          <p>{{ sensor.formattedValue }}</p>
          <span>{{ sensor.status }} · {{ sensor.lastSeen }}</span>
        </article>
      </div>
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
  </section>
</template>

<style scoped src="./DashboardView.css"></style>
