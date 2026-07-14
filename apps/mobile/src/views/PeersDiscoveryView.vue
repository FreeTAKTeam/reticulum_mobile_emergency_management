<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";

import { notifyOperationalUpdate } from "../services/notifications";
import { useNodeStore } from "../stores/nodeStore";
import type { DiscoveredPeer, HubDirectoryPeerRecord, SavedPeer } from "../types/domain";
import type { PeersVisualMockData } from "../utils/peersVisualMock";

type PeerTab = "discovered" | "peers" | "hub";

const nodeStore = useNodeStore();
const route = useRoute();

const searchText = ref("");
const activeTab = ref<PeerTab>("discovered");
let visualMockRefreshInterval: number | undefined;

const mockNow = Date.now();
let visualMockData: PeersVisualMockData | undefined;

async function applyVisualMockData(): Promise<void> {
  visualMockData ??= (await import("../utils/peersVisualMock"))
    .createPeersVisualMockData(mockNow);
  for (const destination of Object.keys(nodeStore.discoveredByDestination)) {
    delete nodeStore.discoveredByDestination[destination];
  }
  for (const destination of Object.keys(nodeStore.savedByDestination)) {
    delete nodeStore.savedByDestination[destination];
  }

  for (const peer of visualMockData.peers) {
    nodeStore.discoveredByDestination[peer.destination] = { ...peer };
    if (peer.saved) {
      nodeStore.savedByDestination[peer.destination] = {
        destination: peer.destination,
        label: peer.label,
        savedAt: mockNow - (peer.activeLink ? 2 * 60 * 60_000 : 14 * 60 * 60_000),
      };
    }
  }

  nodeStore.hubDirectorySnapshot = {
    effectiveConnectedMode: true,
    items: visualMockData.hubDirectoryPeers,
    receivedAtMs: mockNow,
  };
  nodeStore.lastHubRefreshAt = mockNow;
}

function scheduleVisualMockRefresh(): void {
  void applyVisualMockData();
}

function isVisualMockMode(): boolean {
  return import.meta.env.DEV && route.query.mockPeers === "1";
}

onMounted(() => {
  if (isVisualMockMode()) {
    scheduleVisualMockRefresh();
    window.setTimeout(scheduleVisualMockRefresh, 500);
    window.setTimeout(scheduleVisualMockRefresh, 1500);
    visualMockRefreshInterval = window.setInterval(scheduleVisualMockRefresh, 2000);
  }
});

onUnmounted(() => {
  if (visualMockRefreshInterval !== undefined) {
    window.clearInterval(visualMockRefreshInterval);
  }
});

watch(activeTab, () => {
  if (isVisualMockMode()) {
    window.setTimeout(scheduleVisualMockRefresh, 0);
  }
});

function isSaved(destination: string): boolean {
  return nodeStore.savedDestinations.has(destination);
}

function peerMatchesQuery(peer: DiscoveredPeer, query: string): boolean {
  return (
    peer.destination.includes(query) ||
    (peer.label ?? "").toLowerCase().includes(query) ||
    (peer.announcedName ?? "").toLowerCase().includes(query) ||
    (peer.appData ?? "").toLowerCase().includes(query)
  );
}

function announcedNameFor(destination: string): string | undefined {
  return nodeStore.discoveredByDestination[destination]?.announcedName;
}

function savedPeerMatchesQuery(peer: SavedPeer, query: string): boolean {
  const announcedName = announcedNameFor(peer.destination)?.toLowerCase() ?? "";
  return (
    peer.destination.includes(query) ||
    (peer.label ?? "").toLowerCase().includes(query) ||
    announcedName.includes(query)
  );
}

const filteredDiscovered = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.remAnnouncedPeers
    .filter((peer: DiscoveredPeer) => !query || peerMatchesQuery(peer, query))
    .sort((left, right) => right.lastSeenAt - left.lastSeenAt);
});

const filteredPeers = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.savedPeers.filter((peer: SavedPeer) => !query || savedPeerMatchesQuery(peer, query));
});

const filteredHubPeers = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.hubDirectoryPeers.filter((peer: HubDirectoryPeerRecord) => {
    if (!query) {
      return true;
    }
    return (
      peer.identity.toLowerCase().includes(query) ||
      peer.destinationHash.toLowerCase().includes(query) ||
      (peer.displayName ?? "").toLowerCase().includes(query) ||
      peer.announceCapabilities.some((capability) => capability.toLowerCase().includes(query))
    );
  });
});

function peerName(peer: Pick<DiscoveredPeer, "announcedName" | "label" | "destination">): string {
  return peer.announcedName || peer.label || peer.destination;
}

function savedPeerName(peer: SavedPeer): string {
  return announcedNameFor(peer.destination) || peer.label || "No label";
}

function peerNotificationName(destination: string): string {
  const discovered = nodeStore.discoveredByDestination[destination];
  const saved = nodeStore.savedByDestination[destination];
  return discovered?.announcedName || discovered?.label || saved?.label || `${destination.slice(0, 5)}...`;
}

function seenLabel(lastSeenAt?: number): string {
  if (!lastSeenAt) {
    return "never seen";
  }
  const elapsedMs = Math.max(0, Date.now() - lastSeenAt);
  const elapsedMinutes = Math.floor(elapsedMs / 60_000);
  if (elapsedMinutes < 60) {
    return `seen ${Math.max(1, elapsedMinutes)} min ago`;
  }
  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return `seen ${elapsedHours} hr ago`;
  }
  const elapsedDays = Math.floor(elapsedHours / 24);
  return `seen ${elapsedDays} day${elapsedDays === 1 ? "" : "s"} ago`;
}

function discoveredMeta(peer: DiscoveredPeer): string {
  const hops = typeof peer.hops === "number" ? ` | ${peer.hops} hops` : "";
  return `${seenLabel(peer.lastSeenAt)}${hops}`;
}

function savedPeerConnectionLabel(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  return peer?.activeLink ? "Disconnect" : "Connect";
}

function savedPeerStatusLabel(destination: string): "Connected" | "Reachable" | "Disconnected" {
  const peer = nodeStore.discoveredByDestination[destination];
  if (peer?.activeLink) {
    return "Connected";
  }
  return nodeStore.reachablePeers.some((reachablePeer) => reachablePeer.destination === destination)
    ? "Reachable"
    : "Disconnected";
}

function savedPeerMeta(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  return seenLabel(peer?.lastSeenAt);
}

function resolutionLabel(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  const error = peer?.lastResolutionError?.trim();
  if (error) {
    return `Resolution error: ${error}`;
  }
  if (peer?.lastResolutionAttemptAt) {
    return "Resolution attempted";
  }
  return "No resolution attempts";
}

async function onAddPeer(destination: string): Promise<void> {
  const peerName = peerNotificationName(destination);
  await runNodeAction(
    () => nodeStore.savePeer(destination),
    `added ${peerName}`,
    "Peer",
    `add ${peerName}`,
  );
}

async function onRemovePeer(destination: string): Promise<void> {
  const peerName = peerNotificationName(destination);
  await runNodeAction(
    () => nodeStore.removePeer(destination),
    `removed ${peerName}`,
    "Peer",
    `remove ${peerName}`,
  );
}

async function onSavedPeerConnectToggle(destination: string): Promise<void> {
  const disconnecting = savedPeerConnectionLabel(destination) === "Disconnect";
  const peerName = peerNotificationName(destination);
  await runNodeAction(
    () => (disconnecting ? nodeStore.disconnectPeer(destination) : nodeStore.connectPeer(destination)),
    disconnecting
      ? `disconnect requested ${peerName}`
      : `connect requested ${peerName}`,
    "Peer",
    `${disconnecting ? "disconnect" : "connect"} ${peerName}`,
  );
}

async function runNodeAction(
  action: () => Promise<void>,
  successMessage: string,
  title = "Peers",
  failureAction = "action",
): Promise<void> {
  try {
    await action();
    await notifyOperationalUpdate(title, successMessage, { route: "/peers" });
  } catch (error: unknown) {
    await notifyOperationalUpdate(
      "Peer",
      `${failureAction} failed - ${error instanceof Error ? error.message : String(error)}`,
      { route: "/peers" },
    );
  }
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <span class="utility-chip stat-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M16 21v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2" />
            <circle cx="9.5" cy="7" r="3" />
            <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
            <path d="M16 3.13a3 3 0 0 1 0 5.74" />
          </svg>
          <span>{{ nodeStore.savedPeerCount }} Peers</span>
        </span>
        <span class="utility-chip stat-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M10 13a5 5 0 0 0 7.07 0l2.12-2.12a5 5 0 0 0-7.07-7.07L11 4.93" />
            <path d="M14 11a5 5 0 0 0-7.07 0L4.81 13.12a5 5 0 0 0 7.07 7.07L13 19.07" />
          </svg>
          <span>{{ nodeStore.connectedPeerCount }} Connected</span>
        </span>
        <span class="utility-chip stat-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 20h.01" />
            <path d="M8.5 16.5a5 5 0 0 1 7 0" />
            <path d="M5 13a10 10 0 0 1 14 0" />
            <path d="M2 9.5a15 15 0 0 1 20 0" />
          </svg>
          <span>{{ nodeStore.reachablePeerCount }} Reachable</span>
        </span>
        <label class="utility-chip search-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle cx="11" cy="11" r="7" />
            <path d="m16 16 4 4" />
          </svg>
          <input
            v-model="searchText"
            type="search"
            placeholder="Search Peers"
            aria-label="Search peers"
          />
        </label>
        <button
          type="button"
          class="utility-chip announce-chip"
          @click="
            runNodeAction(() => nodeStore.announceNow(), 'Announce requested.')
          "
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m3 11 14-6v14L3 13v-2Z" />
            <path d="M17 9.5h2a2 2 0 0 1 0 4h-2" />
            <path d="M6 13v5" />
          </svg>
          Announce
        </button>
      </div>
    </header>

    <nav class="peer-tabs" aria-label="Peer sections">
      <button
        type="button"
        class="tab-button"
        :class="{ active: activeTab === 'discovered' }"
        @click="activeTab = 'discovered'"
      >
        <span>Discovered</span>
        <strong>{{ filteredDiscovered.length }}</strong>
      </button>
      <button
        type="button"
        class="tab-button"
        :class="{ active: activeTab === 'peers' }"
        @click="activeTab = 'peers'"
      >
        <span>Peers</span>
        <strong>{{ filteredPeers.length }}</strong>
      </button>
      <button
        type="button"
        class="tab-button"
        :class="{ active: activeTab === 'hub' }"
        @click="activeTab = 'hub'"
      >
        <span>Hub</span>
        <strong>{{ filteredHubPeers.length }}</strong>
      </button>
    </nav>

    <section v-if="activeTab === 'discovered'" class="panel">
      <div class="section-header">
        <h2>Discovered</h2>
        <p>{{ filteredDiscovered.length }} REM clients heard through announces</p>
      </div>
      <div v-if="filteredDiscovered.length > 0" class="peer-list">
        <article
          v-for="peer in filteredDiscovered"
          :key="peer.destination"
          class="peer-item compact"
        >
          <div class="peer-copy">
            <p class="dest">{{ peer.destination }}</p>
            <div class="peer-name-line">
              <span v-if="isSaved(peer.destination)" class="peer-state">Peer</span>
              <p class="peer-name">{{ peerName(peer) }}</p>
            </div>
            <p class="peer-meta">{{ discoveredMeta(peer) }}</p>
          </div>
          <div v-if="!isSaved(peer.destination)" class="actions inline-actions">
            <button
              type="button"
              @click="onAddPeer(peer.destination)"
            >
              Add
            </button>
            <button type="button" @click="onRemovePeer(peer.destination)">Remove</button>
          </div>
        </article>
      </div>
      <p v-else class="empty-copy">No REM-capable announces match this search.</p>
    </section>

    <section v-else-if="activeTab === 'peers'" class="panel">
      <div class="section-header split-header">
        <div>
          <h2>Peers</h2>
          <p>{{ filteredPeers.length }} saved peers by last seen | {{ nodeStore.connectedPeerCount }} Connected | {{ nodeStore.reachablePeerCount }} Reachable</p>
        </div>
        <div class="actions header-inline-actions">
          <button
            type="button"
            @click="
              runNodeAction(() => nodeStore.connectAllSaved(), 'Connect requested for saved peers.')
            "
          >
            Connect all
          </button>
          <button
            type="button"
            @click="
              runNodeAction(() => nodeStore.disconnectAllSaved(), 'Disconnected all saved peers.')
            "
          >
            Disconnect all
          </button>
        </div>
      </div>
      <div v-if="filteredPeers.length > 0" class="peer-list">
        <article v-for="peer in filteredPeers" :key="peer.destination" class="peer-item">
          <div class="peer-copy">
            <p class="dest">{{ peer.destination }}</p>
            <div class="peer-name-line">
              <span
                class="peer-connection-pill"
                :class="savedPeerStatusLabel(peer.destination).toLowerCase()"
              >
                {{ savedPeerStatusLabel(peer.destination) }}
              </span>
              <p class="peer-name">{{ savedPeerName(peer) }}</p>
            </div>
            <p class="peer-meta">{{ savedPeerMeta(peer.destination) }}</p>
            <p class="peer-resolution">{{ resolutionLabel(peer.destination) }}</p>
          </div>
          <div class="actions inline-actions">
            <button type="button" @click="onSavedPeerConnectToggle(peer.destination)">
              {{ savedPeerConnectionLabel(peer.destination) }}
            </button>
            <button type="button" @click="onRemovePeer(peer.destination)">Remove</button>
          </div>
        </article>
      </div>
      <p v-else class="empty-copy">No peers saved locally.</p>
    </section>

    <section v-else class="panel">
      <div class="section-header split-header">
        <div>
          <h2>Hub</h2>
          <p>
            Mode: {{ nodeStore.settings.hub.mode }} | Last refresh:
            {{
              nodeStore.lastHubRefreshAt
                ? new Date(nodeStore.lastHubRefreshAt).toLocaleTimeString()
                : "never"
            }}
          </p>
        </div>
        <div class="actions header-inline-actions">
          <button
            type="button"
            @click="
              runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub directory refreshed.')
            "
          >
            Refresh hub list
          </button>
        </div>
      </div>
      <div v-if="filteredHubPeers.length > 0" class="peer-list">
        <article
          v-for="peer in filteredHubPeers"
          :key="peer.destinationHash"
          class="peer-item hub-item"
        >
          <div class="peer-copy">
            <p class="dest">{{ peer.destinationHash }}</p>
            <p class="peer-name">{{ peer.displayName || peer.identity }}</p>
            <p class="peer-meta">
              {{ peer.status || "unknown" }} | {{ peer.registeredMode || "unregistered" }} |
              {{ peer.clientType || "unknown client" }}
            </p>
            <p class="peer-resolution">{{ peer.announceCapabilities.join(", ") }}</p>
          </div>
        </article>
      </div>
      <p v-else class="empty-copy">No hub peers cached.</p>
    </section>
  </section>
</template>

<style scoped src="./PeersDiscoveryView.css"></style>
