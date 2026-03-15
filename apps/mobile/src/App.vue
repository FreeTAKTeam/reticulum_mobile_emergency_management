<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";

import { initAppNotifications } from "./services/notifications";
import { useEventsStore } from "./stores/eventsStore";
import { useMessagesStore } from "./stores/messagesStore";
import { useTelemetryStore } from "./stores/telemetryStore";
import { useNodeStore } from "./stores/nodeStore";

const nodeStore = useNodeStore();
const messagesStore = useMessagesStore();
const eventsStore = useEventsStore();
const telemetryStore = useTelemetryStore();
const route = useRoute();

onMounted(async () => {
  try {
    await initAppNotifications();
    messagesStore.init();
    messagesStore.initReplication();
    eventsStore.init();
    eventsStore.initReplication();
    telemetryStore.init();
    telemetryStore.initReplication();
    await telemetryStore.requestStartupPermission();

    await nodeStore.init();
    await nodeStore.startNode();
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  }
});

const tabItems = [
  { path: "/dashboard", label: "Dashboard", icon: "dashboard" },
  { path: "/messages", label: "Action Messages", icon: "messages" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/peers", label: "Peers", icon: "peers" },
  { path: "/telemetry", label: "Telemetry", icon: "telemetry" },
  { path: "/settings", label: "Settings", icon: "settings" },
];

const runningText = computed(() => (nodeStore.ready ? "Ready" : "Not Ready"));
const runningTitle = computed(() =>
  nodeStore.ready
    ? "App ready to send and receive events or messages."
    : "App is still starting. Sending stays blocked until the node is ready.",
);
const connectedPeerCount = computed(() => nodeStore.connectedDestinations.length);
const connectedPeerCountTitle = computed(() => {
  const count = connectedPeerCount.value;
  return count === 1 ? "1 connected peer" : `${count} connected peers`;
});

function isTabActive(path: string): boolean {
  return route.path === path || route.path.startsWith(`${path}/`);
}
</script>

<template>
  <div class="app-bg">
    <div class="app-shell">
      <header class="masthead">
        <div class="brand">
          <p class="asterisk">*</p>
          <div>
            <p class="title">Emergency Ops</p>
          </div>
        </div>
        <div class="mast-actions">
          <span
            class="peer-count"
            data-testid="connected-peer-count"
            aria-label="Connected peers"
            :title="connectedPeerCountTitle"
          >
            {{ connectedPeerCount }}
          </span>
          <span class="running" :class="{ pending: !nodeStore.ready }" :title="runningTitle">
            {{ runningText }}
          </span>
        </div>
      </header>

      <main class="content">
        <RouterView />
      </main>

      <nav class="tabs">
        <RouterLink
          v-for="tab in tabItems"
          :key="tab.path"
          :to="tab.path"
          class="tab"
          :class="{ active: isTabActive(tab.path) }"
          :aria-label="tab.label"
          :title="tab.label"
        >
          <span class="tab-icon" aria-hidden="true">
            <svg
              v-if="tab.icon === 'messages'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path
                d="M6 7.5h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H11l-4 3v-3H6a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2Z"
              />
              <path d="M8 11h8" />
              <path d="M8 14h5" />
            </svg>
            <svg
              v-else-if="tab.icon === 'events'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path
                d="M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z"
              />
              <path d="M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z" />
            </svg>
            <svg
              v-else-if="tab.icon === 'dashboard'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M5 5h5v5H5z" />
              <path d="M14 5h5v8h-5z" />
              <path d="M5 14h5v5H5z" />
              <path d="M14 16h5v3h-5z" />
            </svg>
            <svg
              v-else-if="tab.icon === 'telemetry'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M12 3.5a7 7 0 1 0 7 7" />
              <path d="M12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z" />
              <path d="M15.7 4.2l4.1.1-.1 4.1" />
              <path d="M19.7 4.3l-5.1 5.1" />
            </svg>
            <svg
              v-else-if="tab.icon === 'settings'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M5 7h10" />
              <path d="M5 17h14" />
              <path d="M15 7a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
              <path d="M9 17a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
            </svg>
            <svg
              v-else
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M12 5v4" />
              <path d="M12 15v4" />
              <path d="M5 12h4" />
              <path d="M15 12h4" />
              <path d="M7.8 7.8l2.8 2.8" />
              <path d="M13.4 13.4l2.8 2.8" />
              <path d="M16.2 7.8l-2.8 2.8" />
              <path d="M10.6 13.4l-2.8 2.8" />
              <circle cx="12" cy="12" r="2.2" />
            </svg>
          </span>
          <span class="sr-only">{{ tab.label }}</span>
        </RouterLink>
      </nav>
    </div>
  </div>
</template>

<style scoped>
.app-bg {
  background:
    radial-gradient(circle at 78% -15%, rgb(35 124 255 / 34%), transparent 42%),
    radial-gradient(circle at -5% 100%, rgb(10 164 255 / 20%), transparent 38%),
    linear-gradient(170deg, #030914, #091632 44%, #06142f 100%);
  height: 100dvh;
  min-height: 100dvh;
  overflow: hidden;
  padding-bottom: calc(env(safe-area-inset-bottom, 0px) + 0.8rem);
  padding-inline: 0.8rem;
  padding-top: calc(env(safe-area-inset-top, 0px) + 0.9rem);
}

.app-bg::before {
  background-image:
    linear-gradient(rgb(34 69 115 / 22%) 1px, transparent 1px),
    linear-gradient(90deg, rgb(34 69 115 / 22%) 1px, transparent 1px);
  background-size: 26px 26px;
  content: "";
  inset: 0;
  opacity: 0.5;
  pointer-events: none;
  position: fixed;
}

.app-shell {
  display: grid;
  gap: 0.95rem;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: 100%;
  margin: 0 auto;
  max-width: 1600px;
  min-height: 0;
  position: relative;
  z-index: 1;
}

.masthead {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.brand {
  align-items: center;
  display: flex;
  gap: 0.7rem;
}

.asterisk {
  color: #00d4ff;
  font-family: var(--font-headline);
  font-size: 2.8rem;
  line-height: 0.8;
  margin: 0;
  text-shadow: 0 0 20px rgb(0 212 255 / 48%);
}

.title {
  font-family: var(--font-headline);
  font-size: clamp(1rem, 1.5vw, 1.45rem);
  margin: 0;
  text-transform: uppercase;
}

.mast-actions {
  align-items: center;
  display: flex;
  gap: 0.65rem;
}

.peer-count {
  align-items: center;
  background: rgb(8 35 71 / 78%);
  border: 1px solid rgb(78 166 255 / 50%);
  border-radius: 999px;
  color: #d7efff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  font-variant-numeric: tabular-nums;
  justify-content: center;
  min-width: 2rem;
  padding: 0.28rem 0.52rem;
}

.running {
  background: rgb(7 47 84 / 74%);
  border: 1px solid rgb(73 171 255 / 54%);
  border-radius: 999px;
  color: #6ac1ff;
  font-family: var(--font-ui);
  font-size: 0.74rem;
  letter-spacing: 0.09em;
  padding: 0.3rem 0.62rem;
  text-transform: uppercase;
}

.running.pending {
  background: rgb(58 34 10 / 76%);
  border-color: rgb(255 178 72 / 54%);
  color: #ffcf7b;
}

.content {
  -webkit-overflow-scrolling: touch;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-bottom: 0.2rem;
  padding-right: 0.15rem;
  scrollbar-gutter: stable both-edges;
}

.tabs {
  align-self: end;
  backdrop-filter: blur(9px);
  background: rgb(3 13 33 / 84%);
  border: 1px solid rgb(63 99 157 / 37%);
  border-radius: 13px;
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  max-width: 100%;
}

.tab {
  align-items: center;
  border-right: 1px solid rgb(60 101 160 / 20%);
  color: #8ea5ca;
  display: grid;
  justify-items: center;
  min-height: 50px;
  padding: 0.35rem;
  position: relative;
  text-decoration: none;
}

.tab:last-child {
  border-right: 0;
}

.tab-icon {
  align-items: center;
  display: inline-flex;
  height: 1.45rem;
  justify-content: center;
  width: 1.45rem;
}

.icon-svg {
  height: 100%;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 100%;
}

.tab.active {
  color: #54ceff;
  text-shadow: 0 0 16px rgb(84 206 255 / 40%);
}

.sr-only {
  border: 0;
  clip: rect(0 0 0 0);
  height: 1px;
  margin: -1px;
  overflow: hidden;
  padding: 0;
  position: absolute;
  white-space: nowrap;
  width: 1px;
}

@media (max-width: 780px) {
  .app-bg {
    padding-bottom: calc(env(safe-area-inset-bottom, 0px) + 0.6rem);
    padding-inline: 0.6rem;
    padding-top: calc(env(safe-area-inset-top, 0px) + 0.6rem);
  }
}
</style>
