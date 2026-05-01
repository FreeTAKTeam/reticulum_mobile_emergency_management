<script setup lang="ts">
import { computed, onMounted, shallowRef, watch } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";

import logoUrl from "./assets/rem-logo.png";
import SosOverlay from "./components/sos/SosOverlay.vue";
import { initAppNotifications, registerNotificationNavigationHandler } from "./services/notifications";
import { useChecklistsStore } from "./stores/checklistsStore";
import { useEventsStore } from "./stores/eventsStore";
import { useMessagingStore } from "./stores/messagingStore";
import { useMessagesStore } from "./stores/messagesStore";
import { useSosStore } from "./stores/sosStore";
import { useTelemetryStore } from "./stores/telemetryStore";
import { useNodeStore } from "./stores/nodeStore";
import { hasCompletedSetupWizard } from "./utils/setupWizardState";

const nodeStore = useNodeStore();
const messagingStore = useMessagingStore();
const messagesStore = useMessagesStore();
const eventsStore = useEventsStore();
const checklistsStore = useChecklistsStore();
const telemetryStore = useTelemetryStore();
const sosStore = useSosStore();
const route = useRoute();
const router = useRouter();

registerNotificationNavigationHandler(async (target) => {
  if (target.route && target.route !== "/inbox") {
    await router.push(target.route);
    return;
  }
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
    const setupCompleted = hasCompletedSetupWizard();
    if (setupCompleted) {
      await initAppNotifications();
    }
    await nodeStore.init();
    await messagingStore.init();
    await nodeStore.startNode();
    await messagingStore.hydrateStartupHistory();

    messagesStore.init();
    eventsStore.init();
    checklistsStore.init();
    telemetryStore.init();
    await sosStore.init();

    messagesStore.initReplication();
    eventsStore.initReplication();
    checklistsStore.initReplication();
    telemetryStore.initReplication();
    if (setupCompleted && nodeStore.settings.telemetry.enabled) {
      await telemetryStore.requestStartupPermission();
    }
    if (!setupCompleted && route.path !== "/setup") {
      await router.replace("/setup");
    }
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  }
});

type AppIcon =
  | "action-messages"
  | "chat"
  | "checklists"
  | "dashboard"
  | "events"
  | "map"
  | "more"
  | "peers"
  | "settings";

interface NavigationItem {
  path: string;
  label: string;
  icon: AppIcon;
}

const menuOpen = shallowRef(false);

const footerItems: NavigationItem[] = [
  { path: "/dashboard", label: "Dashboard", icon: "dashboard" },
  { path: "/inbox", label: "Chat", icon: "chat" },
  { path: "/checklists", label: "Tasks", icon: "checklists" },
  { path: "/telemetry", label: "Map", icon: "map" },
];

const menuItems: NavigationItem[] = [
  { path: "/inbox", label: "Chat", icon: "chat" },
  { path: "/messages", label: "Action Messages", icon: "action-messages" },
  { path: "/events", label: "Events", icon: "events" },
  { path: "/checklists", label: "Tasks", icon: "checklists" },
  { path: "/telemetry", label: "Map", icon: "map" },
  { path: "/peers", label: "Peers", icon: "peers" },
  { path: "/settings", label: "Settings", icon: "settings" },
];

const iconPaths: Record<AppIcon, string[]> = {
  "action-messages": [
    "M8 4.5h6l4 4v10a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
    "M14 4.5v4h4",
    "M9 12h6",
    "M9 15.5h6",
  ],
  chat: [
    "M6 7.5h12a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H11l-4 3v-3H6a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2Z",
    "M8 11h8",
    "M8 14h5",
  ],
  checklists: [
    "M8 5h8a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z",
    "M9.5 4h5a1 1 0 0 1 1 1v1h-7V5a1 1 0 0 1 1-1Z",
    "m9.5 11 1.5 1.5 3.5-3.5",
    "M9.5 16h5",
  ],
  dashboard: [
    "M5 5h5v5H5z",
    "M14 5h5v8h-5z",
    "M5 14h5v5H5z",
    "M14 16h5v3h-5z",
  ],
  events: [
    "M12 4.5 19 18.5H5z",
    "M12 9v4",
    "M12 16.2h.01",
  ],
  map: [
    "M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z",
    "M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z",
  ],
  more: [
    "M5 7h14",
    "M5 12h14",
    "M5 17h14",
  ],
  peers: [
    "M9.5 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    "M4.5 19a5 5 0 0 1 10 0",
    "M16.5 10.5a2.5 2.5 0 1 0 0-5",
    "M15.7 14.5a4.4 4.4 0 0 1 3.8 4.5",
  ],
  settings: [
    "M5 7h10",
    "M5 17h14",
    "M15 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
    "M9 19a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
  ],
};

const pageTitle = computed(() => {
  switch (route.name) {
    case "dashboard":
      return "Dashboard";
    case "messages":
      return "Action Messages";
    case "events":
      return "Events";
    case "inbox":
      return "Chat";
    case "checklists":
      return "Tasks";
    case "checklist-detail":
      return "Checklist Detail";
    case "message-status-help":
      return "Status Help";
    case "peers":
      return "Peers";
    case "settings":
      return "Settings";
    case "setup":
      return "Setup";
    case "telemetry":
      return "Map";
    default:
      return "R.E.M.";
  }
});

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
  if (path === "/checklists") {
    return route.path === path
      || route.path.startsWith(`${path}/`)
      || route.path === "/checlklist"
      || route.path.startsWith("/checlklist/");
  }
  return route.path === path || route.path.startsWith(`${path}/`);
}

const moreRouteNames = new Set([
  "messages",
  "events",
  "message-status-help",
  "peers",
  "settings",
]);

const moreActive = computed(() => menuOpen.value || moreRouteNames.has(String(route.name ?? "")));
const setupActive = computed(() => route.name === "setup");

function toggleMenu(): void {
  menuOpen.value = !menuOpen.value;
}

function closeMenu(): void {
  menuOpen.value = false;
}

watch(
  () => route.fullPath,
  () => {
    closeMenu();
  },
);
</script>

<template>
  <div class="app-bg">
    <div class="app-shell" :class="{ 'setup-mode': setupActive }">
      <header v-if="!setupActive" class="masthead">
        <div class="brand">
          <div class="brand-mark-wrap">
            <img class="brand-mark" :src="logoUrl" alt="R.E.M. logo" />
          </div>
          <p class="title">R.E.M.</p>
        </div>
        <h1 class="page-title">{{ pageTitle }}</h1>
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

      <main class="content" :class="{ 'setup-content': setupActive }">
        <RouterView />
      </main>

      <div
        v-if="menuOpen && !setupActive"
        class="menu-backdrop"
        aria-hidden="true"
        @click="closeMenu"
      ></div>

      <aside
        v-if="menuOpen && !setupActive"
        class="tools-menu"
        aria-label="More tools"
      >
        <header class="tools-header">
          <h2>Tools</h2>
          <button
            type="button"
            class="tools-close"
            aria-label="Close more tools"
            @click="closeMenu"
          >
            <svg class="icon-svg" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M6 6l12 12" />
              <path d="M18 6 6 18" />
            </svg>
          </button>
        </header>

        <div class="tools-grid">
          <RouterLink
            v-for="item in menuItems"
            :key="`menu-${item.path}`"
            :to="item.path"
            class="tool-tile"
            :class="{ active: isTabActive(item.path) }"
            :aria-label="item.label"
            :title="item.label"
          >
            <span class="tool-tile-icon" aria-hidden="true">
              <svg class="icon-svg" viewBox="0 0 24 24" fill="none">
                <path
                  v-for="path in iconPaths[item.icon]"
                  :key="path"
                  :d="path"
                />
              </svg>
            </span>
            <span class="tool-tile-label">{{ item.label }}</span>
          </RouterLink>
        </div>
      </aside>

      <nav v-if="!setupActive" class="tabs" aria-label="Primary navigation">
        <RouterLink
          v-for="tab in footerItems"
          :key="tab.path"
          :to="tab.path"
          class="tab"
          :class="{ active: isTabActive(tab.path) }"
          :aria-label="tab.label"
          :title="tab.label"
        >
          <span class="tab-icon" aria-hidden="true">
            <svg class="icon-svg" viewBox="0 0 24 24" fill="none">
              <path
                v-for="path in iconPaths[tab.icon]"
                :key="path"
                :d="path"
              />
            </svg>
          </span>
          <span class="tab-label">{{ tab.label }}</span>
        </RouterLink>
        <button
          type="button"
          class="tab tab-more"
          :class="{ active: moreActive }"
          aria-label="More"
          :aria-expanded="menuOpen"
          title="More"
          @click="toggleMenu"
        >
          <span class="tab-icon" aria-hidden="true">
            <svg class="icon-svg" viewBox="0 0 24 24" fill="none">
              <path
                v-for="path in iconPaths.more"
                :key="path"
                :d="path"
              />
            </svg>
          </span>
          <span class="tab-label">More</span>
        </button>
      </nav>
      <SosOverlay v-if="!setupActive" />
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
  gap: 0.72rem;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: 100%;
  margin: 0 auto;
  max-width: 1600px;
  min-height: 0;
  position: relative;
  z-index: 1;
}

.app-shell.setup-mode {
  grid-template-rows: minmax(0, 1fr);
  max-width: 980px;
}

.masthead {
  align-items: center;
  backdrop-filter: blur(16px);
  background: linear-gradient(135deg, rgb(5 21 43 / 88%), rgb(6 27 52 / 62%));
  border: 1px solid rgb(74 137 214 / 28%);
  border-radius: 15px;
  box-shadow:
    inset 0 1px 0 rgb(147 214 255 / 10%),
    0 18px 40px rgb(1 6 19 / 28%);
  display: grid;
  gap: 0.65rem;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  padding: 0.48rem 0.62rem;
}

.brand {
  align-items: center;
  display: flex;
  gap: 0.5rem;
  min-width: 0;
}

.brand-mark-wrap {
  align-items: center;
  background:
    radial-gradient(circle at 30% 30%, rgb(114 232 255 / 16%), transparent 48%),
    linear-gradient(145deg, rgb(8 31 59 / 90%), rgb(6 18 39 / 96%));
  border: 1px solid rgb(104 200 255 / 24%);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(188 241 255 / 10%),
    0 12px 30px rgb(1 8 22 / 36%);
  display: inline-flex;
  padding: 0.16rem;
}

.brand-mark {
  display: block;
  filter: drop-shadow(0 0 18px rgb(80 212 255 / 16%));
  height: 2.08rem;
  width: 2.08rem;
}

.title {
  color: #f3fbff;
  font-family: var(--font-headline);
  font-size: clamp(0.98rem, 1.3vw, 1.22rem);
  letter-spacing: 0.15em;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.page-title {
  color: #f5fbff;
  font-family: var(--font-headline);
  font-size: clamp(1.16rem, 2.3vw, 1.75rem);
  font-weight: 700;
  letter-spacing: 0;
  line-height: 1;
  margin: 0;
  max-width: min(38vw, 28rem);
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mast-actions {
  align-items: center;
  display: flex;
  flex-wrap: nowrap;
  gap: 0.42rem;
  justify-content: flex-end;
  min-width: 0;
}

.peer-count {
  align-items: center;
  background: rgb(8 35 71 / 78%);
  border: 1px solid rgb(78 166 255 / 50%);
  border-radius: 999px;
  color: #d7efff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  justify-content: center;
  min-width: 1.8rem;
  padding: 0.22rem 0.45rem;
  box-shadow: inset 0 1px 0 rgb(211 241 255 / 8%);
}

.running {
  background: rgb(7 47 84 / 74%);
  border: 1px solid rgb(73 171 255 / 54%);
  border-radius: 999px;
  color: #6ac1ff;
  font-family: var(--font-ui);
  font-size: 0.66rem;
  letter-spacing: 0.09em;
  padding: 0.23rem 0.5rem;
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

.content.setup-content {
  padding: 0;
  scrollbar-gutter: stable;
}

.menu-backdrop {
  background: rgb(0 4 12 / 36%);
  inset: 0;
  position: fixed;
  z-index: 8;
}

.tools-menu {
  backdrop-filter: blur(14px);
  background:
    linear-gradient(145deg, rgb(3 11 26 / 95%), rgb(4 17 37 / 94%)),
    radial-gradient(circle at 22% 0%, rgb(59 177 255 / 20%), transparent 42%);
  border: 1px solid rgb(83 138 205 / 34%);
  border-radius: 16px;
  bottom: calc(62px + env(safe-area-inset-bottom, 0px) + 0.75rem);
  box-shadow:
    inset 0 1px 0 rgb(177 229 255 / 8%),
    0 22px 48px rgb(0 0 0 / 46%);
  max-height: min(70dvh, 560px);
  overflow: hidden;
  position: absolute;
  right: 0;
  width: min(27rem, calc(100vw - 1.2rem));
  z-index: 12;
}

.tools-close {
  align-items: center;
  color: #dff3ff;
  display: inline-flex;
  height: 3.2rem;
  justify-content: center;
  width: 3.2rem;
}

.tools-close .icon-svg {
  height: 1.65rem;
  width: 1.65rem;
}

.tools-close {
  --btn-bg: transparent;
  --btn-bg-pressed: rgb(22 52 83 / 92%);
  --btn-border: transparent;
  --btn-border-pressed: rgb(122 210 255 / 42%);
  --btn-shadow: none;
  --btn-shadow-pressed: inset 0 0 0 1px rgb(122 210 255 / 20%);
  --btn-color: #dff3ff;
  --btn-color-pressed: #f4fbff;
  background: transparent;
  border: 0;
  cursor: pointer;
  padding: 0;
}

.tools-header {
  align-items: center;
  background: rgb(2 8 18 / 58%);
  border-bottom: 1px solid rgb(89 126 181 / 26%);
  display: flex;
  justify-content: space-between;
  min-height: 3.4rem;
  padding: 0 1rem;
}

.tools-header h2 {
  color: #f2f8ff;
  font-family: var(--font-headline);
  font-size: 1.45rem;
  letter-spacing: 0;
  margin: 0;
}

.tools-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.tool-tile {
  align-items: center;
  aspect-ratio: 1 / 0.88;
  border-bottom: 1px solid rgb(165 207 255 / 22%);
  border-right: 1px solid rgb(165 207 255 / 22%);
  color: #d8ecff;
  display: grid;
  gap: 0.38rem;
  justify-items: center;
  min-height: 6.4rem;
  padding: 0.76rem 0.5rem;
  text-align: center;
  text-decoration: none;
}

.tool-tile:nth-child(3n) {
  border-right: 0;
}

.tool-tile.active {
  background:
    linear-gradient(180deg, rgb(32 140 219 / 28%), rgb(5 28 61 / 56%));
  color: #70d8ff;
}

.tool-tile:focus-visible {
  outline: 2px solid rgb(116 218 255 / 72%);
  outline-offset: -3px;
}

.tool-tile-icon {
  align-items: center;
  color: currentColor;
  display: inline-flex;
  height: 2.25rem;
  justify-content: center;
  width: 2.25rem;
}

.tool-tile-label {
  color: currentColor;
  font-family: var(--font-ui);
  font-size: clamp(0.82rem, 2.8vw, 1.02rem);
  font-weight: 600;
  line-height: 1.05;
  overflow-wrap: anywhere;
}

.tabs {
  align-self: end;
  backdrop-filter: blur(9px);
  background: rgb(3 13 33 / 84%);
  border: 1px solid rgb(63 99 157 / 37%);
  border-radius: 15px;
  display: grid;
  gap: 0.18rem;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  max-width: 100%;
  padding: 0.28rem;
}

.tab {
  align-items: center;
  background: transparent;
  border: 0;
  color: #8ea5ca;
  cursor: pointer;
  display: grid;
  font: inherit;
  gap: 0.18rem;
  justify-items: center;
  min-height: 54px;
  padding: 0.34rem 0.22rem;
  position: relative;
  text-decoration: none;
}

.tab-icon {
  align-items: center;
  display: inline-flex;
  height: 1.35rem;
  justify-content: center;
  width: 1.35rem;
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
  background: linear-gradient(180deg, #32baff, #1597ee);
  border: 1px solid rgb(191 238 255 / 62%);
  border-radius: 13px;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 34%),
    0 0 18px rgb(50 186 255 / 28%);
  color: #03192f;
  text-shadow: none;
}

.tab-label {
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 600;
  line-height: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
    gap: 0.42rem;
    grid-template-columns: minmax(0, 0.72fr) minmax(0, auto) minmax(0, 0.86fr);
    padding: 0.42rem 0.5rem;
  }

  .brand {
    gap: 0.38rem;
    min-width: 0;
  }

  .brand-mark {
    height: 1.82rem;
    width: 1.82rem;
  }

  .title {
    font-size: 0.86rem;
    letter-spacing: 0.12em;
  }

  .page-title {
    font-size: 1.12rem;
    max-width: 34vw;
  }

  .mast-actions {
    gap: 0.34rem;
  }

  .peer-count {
    font-size: 0.66rem;
    min-width: 1.62rem;
    padding: 0.19rem 0.38rem;
  }

  .running {
    font-size: 0.61rem;
    padding: 0.2rem 0.42rem;
  }

  .tools-menu {
    bottom: calc(61px + env(safe-area-inset-bottom, 0px) + 0.55rem);
    width: min(25.5rem, calc(100vw - 1.2rem));
  }

  .tool-tile {
    min-height: 5.9rem;
  }

  .tabs {
    padding: 0.24rem;
  }

  .tab {
    min-height: 52px;
  }

  .tab-label {
    font-size: 0.68rem;
  }
}
</style>
