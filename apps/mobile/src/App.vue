<script setup lang="ts">
import { App, type BackButtonListenerEvent } from "@capacitor/app";
import { Capacitor } from "@capacitor/core";
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";

import { footerItems, iconPaths, menuItems } from "./appNavigation";
import logoUrl from "./assets/rem-logo.png";
import SplashScreen from "./components/SplashScreen.vue";
import SosOverlay from "./components/sos/SosOverlay.vue";
import { initAppNotifications, registerNotificationNavigationHandler } from "./services/notifications";
import {
  listPairedRnodeBluetoothDevices,
  type RnodeBleDeviceRecord,
} from "./services/rnodeBluetooth";
import { useChecklistsStore } from "./stores/checklistsStore";
import { useEventsStore } from "./stores/eventsStore";
import { useMessagingStore } from "./stores/messagingStore";
import { useMessagesStore } from "./stores/messagesStore";
import { useSosStore } from "./stores/sosStore";
import { useTelemetryStore } from "./stores/telemetryStore";
import { useNodeStore } from "./stores/nodeStore";
import {
  resolveAndroidRouteBackAction,
  runBackNavigationHandlers,
} from "./utils/androidBackNavigation";
import { appVersion } from "./utils/appVersion";
import { hasCompletedSetupWizard } from "./utils/setupWizardState";
import {
  STARTUP_INTERFACE_LOADING_DETAIL,
  STARTUP_INTERFACE_LOADING_SUMMARY,
  buildStartupInterfaceItems,
  statusHasRuntimeStartupReadiness,
  type StartupInterfaceItem,
} from "./utils/startupInterfaces";
import { reconcileStartupRuntime } from "./utils/startupRuntime";

const nodeStore = useNodeStore();
const messagingStore = useMessagingStore();
const messagesStore = useMessagesStore();
const eventsStore = useEventsStore();
const checklistsStore = useChecklistsStore();
const telemetryStore = useTelemetryStore();
const sosStore = useSosStore();
const route = useRoute();
const router = useRouter();
const startupInterfaceMockEnabled = computed(() => route.query.mock === "splash-interface-loading");
const startupSplashMockEnabled = computed(() => route.query.mock === "splash-screen");
const startupMockEnabled = computed(() => (
  startupSplashMockEnabled.value || startupInterfaceMockEnabled.value
));

function normalizedBluetoothId(value: string): string {
  return value
    .trim()
    .replace(/[:-]/g, "")
    .toLowerCase();
}

function deviceMatchesId(device: RnodeBleDeviceRecord, configuredId: string): boolean {
  const target = normalizedBluetoothId(configuredId);
  return [device.id, device.address]
    .some((value) => normalizedBluetoothId(value) === target);
}

function isRnodeDevice(device: RnodeBleDeviceRecord): boolean {
  return device.paired && /rnode/i.test(device.name);
}

async function repairStartupRnodeSelection(): Promise<void> {
  const rnode = nodeStore.settings.rnode;
  const configuredId = rnode.peripheralId.trim();
  if (!rnode.enabled || !configuredId) {
    return;
  }

  try {
    const pairedDevices = await listPairedRnodeBluetoothDevices();
    if (pairedDevices.some((device) => deviceMatchesId(device, configuredId))) {
      return;
    }

    const pairedRnodes = pairedDevices.filter(isRnodeDevice);
    if (pairedRnodes.length !== 1) {
      return;
    }

    const [device] = pairedRnodes;
    await nodeStore.updateSettings({
      rnode: {
        ...rnode,
        peripheralId: device.id || device.address,
        displayName: device.name || device.address,
      },
    });
  } catch {
    // RNode selection can still be fixed manually from Settings if Bluetooth is unavailable.
  }
}

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
  splashTimer = window.setTimeout(() => {
    splashMinimumElapsed.value = true;
  }, 1200);
  try {
    const setupCompleted = hasCompletedSetupWizard();
    if (setupCompleted) {
      await initAppNotifications();
    }
    await nodeStore.init();
    // Bind telemetry before fallible history hydration. Native startup may still be
    // pending here; telemetryStore watches runtime readiness and starts once ready.
    telemetryStore.init();
    await messagingStore.init();
    if (setupCompleted) {
      await repairStartupRnodeSelection();
      await reconcileStartupRuntime(
        {
          running: nodeStore.status.running,
          restartRequired: nodeStore.nodeConfigRestartRequired,
        },
        {
          start: () => nodeStore.startNode(),
          restart: () => nodeStore.restartNode(),
        },
      );
    }
    await messagingStore.hydrateStartupHistory();

    messagesStore.init();
    eventsStore.init();
    checklistsStore.init();
    await sosStore.init();

    messagesStore.initReplication();
    eventsStore.initReplication();
    checklistsStore.initReplication();
    telemetryStore.initReplication();
    if (setupCompleted && nodeStore.settings.telemetry.enabled) {
      await telemetryStore.requestStartupPermission();
    }
    if (!setupCompleted && route.path !== "/setup" && !startupMockEnabled.value) {
      await router.replace("/setup");
    }
  } catch (error: unknown) {
    nodeStore.lastError = error instanceof Error ? error.message : String(error);
  } finally {
    startupComplete.value = true;
  }
});

const startupInterfaceItems = computed<StartupInterfaceItem[]>(() => {
  if (startupInterfaceMockEnabled.value) {
    return [
      { id: "rnode", label: "LoRa", detail: "Starting radio interface", state: "loading" },
      { id: "tcp", label: "TCP community", detail: "Starting TCP interface", state: "loading" },
      { id: "local", label: "Reticulum Net", detail: "Starting runtime", state: "loading" },
    ];
  }
  return buildStartupInterfaceItems(nodeStore.status, nodeStore.settings);
});
const startupConfiguredInterfaceItems = computed(() =>
  startupInterfaceItems.value.filter((item) => item.id !== "local" && item.state !== "disabled"),
);
const startupInterfacesNeedGrace = computed(() =>
  startupConfiguredInterfaceItems.value.length > 0
  && !statusHasRuntimeStartupReadiness(nodeStore.status),
);
const menuOpen = shallowRef(false);
const splashMinimumElapsed = shallowRef(false);
const startupComplete = shallowRef(false);
const showSplash = computed(() => (
  startupMockEnabled.value
  || !splashMinimumElapsed.value
  || !startupComplete.value
  || startupInterfacesNeedGrace.value
));
let splashTimer: number | undefined;
let androidBackButtonListener: { remove: () => Promise<void> } | undefined;

const pageTitle = computed(() => {
  switch (route.name) {
    case "dashboard":
      return "Dashboard";
    case "messages":
      return "EAM";
    case "events":
      return "Events";
    case "event-mecp-help":
      return "MECP Help";
    case "inbox":
      return "Chat";
    case "checklists":
      return "Checklists";
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

const readinessError = computed(() => nodeStore.readinessError.trim());
const runningText = computed(() => (nodeStore.ready ? "Ready" : "Not Ready"));
const runningTitle = computed(() => {
  if (nodeStore.ready) {
    return "App ready to send and receive events or messages.";
  }
  if (readinessError.value) {
    return `Node is not ready: ${readinessError.value}`;
  }
  return "App is still starting. Sending stays blocked until the node is ready.";
});
const possiblePeerCount = computed(() => nodeStore.savedPeerCount);
const connectedPeerCount = computed(() => nodeStore.connectedPeerCount);
const reachablePeerCount = computed(() => nodeStore.reachablePeerCount);
const peerCountLabel = computed(
  () => `${possiblePeerCount.value}/${connectedPeerCount.value}/${reachablePeerCount.value}`,
);
const connectedPeerCountTitle = computed(() => {
  const possible = possiblePeerCount.value;
  const connected = connectedPeerCount.value;
  const reachable = reachablePeerCount.value;
  const possibleLabel = possible === 1 ? "1 saved peer" : `${possible} saved peers`;
  const connectedLabel = connected === 1 ? "1 live link" : `${connected} live links`;
  const reachableLabel = reachable === 1 ? "1 recently seen peer" : `${reachable} recently seen peers`;
  return `${possibleLabel}, ${connectedLabel}, ${reachableLabel}`;
});

function isTabActive(path: string): boolean {
  if (path === "/checklists") {
    return route.path === path
      || route.path.startsWith(`${path}/`);
  }
  return route.path === path || route.path.startsWith(`${path}/`);
}

const moreRouteNames = new Set([
  "messages",
  "events",
  "event-mecp-help",
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

async function handleAndroidBackButton(event: BackButtonListenerEvent): Promise<void> {
  if (menuOpen.value) {
    closeMenu();
    return;
  }
  if (await runBackNavigationHandlers()) {
    return;
  }

  const action = resolveAndroidRouteBackAction({
    canGoBack: event.canGoBack,
    currentPath: route.path,
  });
  if (action === "back") {
    router.back();
    return;
  }
  if (action === "dashboard") {
    await router.replace("/dashboard");
  }
}

async function registerAndroidBackButtonHandler(): Promise<void> {
  if (Capacitor.getPlatform() !== "android") {
    return;
  }
  androidBackButtonListener = await App.addListener("backButton", (event) => {
    void handleAndroidBackButton(event);
  });
}

watch(
  () => route.fullPath,
  () => {
    closeMenu();
  },
);

void registerAndroidBackButtonHandler();

onUnmounted(() => {
  if (splashTimer !== undefined) {
    window.clearTimeout(splashTimer);
  }
  void androidBackButtonListener?.remove();
  androidBackButtonListener = undefined;
});
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
            aria-label="Saved peers, live links, and recently seen saved peers"
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
      <SplashScreen
        v-if="showSplash"
        :version="appVersion"
        :interface-loading="startupInterfaceMockEnabled || showSplash"
        :interface-items="startupInterfaceItems"
        :loading-message="STARTUP_INTERFACE_LOADING_SUMMARY"
        :loading-detail="STARTUP_INTERFACE_LOADING_DETAIL"
      />
    </div>
  </div>
</template>

<style scoped src="./App.css"></style>
