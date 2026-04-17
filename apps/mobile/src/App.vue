<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";

import logoUrl from "./assets/rem-logo.png";
import SosOverlay from "./components/sos/SosOverlay.vue";
import { initAppNotifications, registerNotificationNavigationHandler } from "./services/notifications";
import { useEventsStore } from "./stores/eventsStore";
import { useMessagingStore } from "./stores/messagingStore";
import { useMessagesStore } from "./stores/messagesStore";
import { useSosStore } from "./stores/sosStore";
import { useTelemetryStore } from "./stores/telemetryStore";
import { useNodeStore } from "./stores/nodeStore";

const nodeStore = useNodeStore();
const messagingStore = useMessagingStore();
const messagesStore = useMessagesStore();
const eventsStore = useEventsStore();
const telemetryStore = useTelemetryStore();
const sosStore = useSosStore();
const route = useRoute();
const router = useRouter();

registerNotificationNavigationHandler(async (target) => {
  if (target.route !== "/inbox" && !target.conversationId) {
    return;
  }
  await router.push({
    path: "/inbox",
    query: {
      ...(target.conversationId ? { conversation: target.conversationId } : {}),
      ...(target.messageIdHex ? { message: target.messageIdHex } : {}),
    },
  });
});

onMounted(async () => {
  try {
    await initAppNotifications();
    await nodeStore.init();
    await messagingStore.init();
    await nodeStore.startNode();
    await messagingStore.hydrateStartupHistory();

    messagesStore.init();
    eventsStore.init();
    telemetryStore.init();
    await sosStore.init();

    messagesStore.initReplication();
    eventsStore.initReplication();
    telemetryStore.initReplication();
    await telemetryStore.requestStartupPermission();
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  }
});

const tabItems = [
  { path: "/dashboard", label: "Dashboard", icon: "dashboard" },
  { path: "/inbox", label: "Inbox", icon: "inbox" },
  { path: "/telemetry", label: "Telemetry", icon: "telemetry" },
  { path: "/messages", label: "Action Messages", icon: "messages" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/peers", label: "Peers", icon: "peers" },
  { path: "/settings", label: "Settings", icon: "settings" },
];

const runningText = computed(() => (nodeStore.ready ? "Ready" : "Not Ready"));
const runningTitle = computed(() =>
  nodeStore.ready
    ? "App ready to send and receive events or messages."
    : "App is still starting. Sending stays blocked until the node is ready.",
);
const possiblePeerCount = computed(() => nodeStore.savedPeerCount);
const connectedPeerCount = computed(() => nodeStore.connectedPeerCount);
const peerCountLabel = computed(
  () => `${possiblePeerCount.value}/${connectedPeerCount.value}`,
);
const connectedPeerCountTitle = computed(() => {
  const possible = possiblePeerCount.value;
  const connected = connectedPeerCount.value;
  const possibleLabel = possible === 1 ? "1 saved peer" : `${possible} saved peers`;
  const connectedLabel = connected === 1 ? "1 saved peer connected" : `${connected} saved peers connected`;
  return `${possibleLabel}, ${connectedLabel}`;
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
          <div class="brand-mark-wrap">
            <img class="brand-mark" :src="logoUrl" alt="R.E.M. logo" />
          </div>
          <p class="title">R.E.M.</p>
        </div>
        <div class="mast-actions">
            <span
              class="peer-count"
              data-testid="connected-peer-count"
              aria-label="Saved peers and connected saved peers"
              :title="connectedPeerCountTitle"
            >
            {{ peerCountLabel }}
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
              v-if="tab.icon === 'inbox'"
              class="icon-svg"
              viewBox="0 0 24 24"
              fill="none"
            >
              <path d="M5 6.5h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2Z" />
              <path d="M3 10.5h5l1.8 2h4.4l1.8-2H21" />
            </svg>
            <svg
              v-else-if="tab.icon === 'messages'"
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
              <path d="M8 4.5h6l4 4v10a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z" />
              <path d="M14 4.5v4h4" />
              <path d="M9 12h6" />
              <path d="M9 15.5h6" />
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
              <path
                d="M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z"
              />
              <path d="M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z" />
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
      <SosOverlay />
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
  backdrop-filter: blur(16px);
  background: linear-gradient(135deg, rgb(5 21 43 / 88%), rgb(6 27 52 / 62%));
  border: 1px solid rgb(74 137 214 / 28%);
  border-radius: 18px;
  box-shadow:
    inset 0 1px 0 rgb(147 214 255 / 10%),
    0 18px 40px rgb(1 6 19 / 28%);
  display: flex;
  justify-content: space-between;
  padding: 0.72rem 0.9rem;
}

.brand {
  align-items: center;
  display: flex;
  gap: 0.82rem;
}

.brand-mark-wrap {
  align-items: center;
  background:
    radial-gradient(circle at 30% 30%, rgb(114 232 255 / 16%), transparent 48%),
    linear-gradient(145deg, rgb(8 31 59 / 90%), rgb(6 18 39 / 96%));
  border: 1px solid rgb(104 200 255 / 24%);
  border-radius: 16px;
  box-shadow:
    inset 0 1px 0 rgb(188 241 255 / 10%),
    0 12px 30px rgb(1 8 22 / 36%);
  display: inline-flex;
  padding: 0.24rem;
}

.brand-mark {
  display: block;
  filter: drop-shadow(0 0 18px rgb(80 212 255 / 16%));
  height: 3.05rem;
  width: 3.05rem;
}

.title {
  color: #f3fbff;
  font-family: var(--font-headline);
  font-size: clamp(1.28rem, 1.8vw, 1.72rem);
  letter-spacing: 0.22em;
  margin: 0;
  text-transform: uppercase;
}

.mast-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.65rem;
  justify-content: flex-end;
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
  box-shadow: inset 0 1px 0 rgb(211 241 255 / 8%);
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
  box-shadow: inset 0 1px 0 rgb(211 241 255 / 8%);
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
  grid-template-columns: repeat(7, minmax(0, 1fr));
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

  .masthead {
    gap: 0.6rem;
    padding: 0.62rem 0.72rem;
  }

  .brand {
    flex: 1;
    gap: 0.55rem;
    min-width: 0;
  }

  .brand-mark {
    height: 2.5rem;
    width: 2.5rem;
  }

  .title {
    font-size: 1.12rem;
    letter-spacing: 0.16em;
  }

  .masthead {
    align-items: center;
    flex-direction: row;
  }

  .mast-actions {
    flex-shrink: 0;
    flex-wrap: nowrap;
    gap: 0.45rem;
    width: auto;
  }

  .peer-count {
    min-width: 1.8rem;
    padding: 0.25rem 0.48rem;
  }

  .running {
    font-size: 0.68rem;
    padding: 0.28rem 0.52rem;
  }
}
</style>
