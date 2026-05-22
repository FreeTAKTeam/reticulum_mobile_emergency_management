<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";

import { notifyOperationalUpdate } from "../services/notifications";
import { useNodeStore } from "../stores/nodeStore";
import type { DiscoveredPeer, HubDirectoryPeerRecord, SavedPeer } from "../types/domain";

type PeerTab = "discovered" | "peers" | "hub";

const nodeStore = useNodeStore();
const route = useRoute();

const searchText = ref("");
const activeTab = ref<PeerTab>("discovered");
let visualMockRefreshInterval: number | undefined;

const mockNow = Date.now();
const mockPeerRecords: DiscoveredPeer[] = [
  {
    destination: "a13f6e2b94cd08ff31a92765db4e10c2",
    identityHex: "fdd5d08e476a4602bc51d0f37d72dd21",
    lxmfDestinationHex: "3ac7e918b5f1407bb759b0f3f4d41c9a",
    announceLastSeenAt: mockNow - 90_000,
    lxmfLastSeenAt: mockNow - 50_000,
    label: "Field Team Alpha",
    announcedName: "ALPHA-1",
    lastSeenAt: mockNow - 50_000,
    hops: 1,
    interfaceHex: "001a2b3c4d5e6f70",
    appData: "R3AKT,EmergencyMessages,Telemetry,LXMF",
    sources: ["announce", "import"],
    state: "connected",
    saved: true,
    stale: false,
    activeLink: true,
    lastResolutionAttemptAt: mockNow - 120_000,
  },
  {
    destination: "b06c8af91de44070983f6ec2a51b7d35",
    identityHex: "0e2bf871c1444ed197ed77df5f8632ae",
    lxmfDestinationHex: "967fd03e8c3245e7af2d7f691e86b580",
    announceLastSeenAt: mockNow - 8 * 60_000,
    lxmfLastSeenAt: mockNow - 7 * 60_000,
    label: "Medical relay",
    announcedName: "MED-RELAY",
    lastSeenAt: mockNow - 7 * 60_000,
    hops: 2,
    interfaceHex: "701f6e5d4c3b2a10",
    appData: "R3AKT,EmergencyMessages,TelemetryRelay",
    sources: ["announce", "hub", "import"],
    state: "connecting",
    saved: true,
    stale: false,
    activeLink: false,
    lastResolutionAttemptAt: mockNow - 30_000,
  },
  {
    destination: "c974de6aa1f8417a8c2e0bb5332ac01f",
    identityHex: "31397ec9c46d4caea5739f50821cecd7",
    lxmfDestinationHex: "18a738f903344a11a8c56695454da331",
    announceLastSeenAt: mockNow - 3 * 60 * 60_000,
    lxmfLastSeenAt: mockNow - 3 * 60 * 60_000,
    label: "North checkpoint",
    announcedName: "NORTH-CP",
    lastSeenAt: mockNow - 3 * 60 * 60_000,
    hops: 4,
    interfaceHex: "89abcdef01234567",
    appData: "R3AKT,EmergencyMessages,Checklists,GroupChat",
    sources: ["announce", "import"],
    state: "disconnected",
    saved: false,
    stale: true,
    activeLink: false,
    lastError: "Link closed by peer",
    lastResolutionError: "Path request timed out after 2 attempts",
    lastResolutionAttemptAt: mockNow - 15 * 60_000,
  },
  {
    destination: "f08ad9c21be64737a5bb68fd4434e912",
    identityHex: "e1b68f14e71d4cde8629ffbc5471459b",
    lxmfDestinationHex: "9b8fe7dc314446438d4ceab380208f6a",
    announceLastSeenAt: mockNow - 12 * 60_000,
    lxmfLastSeenAt: mockNow - 11 * 60_000,
    announcedName: "TRIAGE-2",
    lastSeenAt: mockNow - 11 * 60_000,
    hops: 2,
    interfaceHex: "445566778899aabb",
    appData: "R3AKT,EmergencyMessages,Medical",
    sources: ["announce"],
    state: "disconnected",
    saved: false,
    stale: false,
    activeLink: false,
    lastResolutionAttemptAt: mockNow - 10 * 60_000,
  },
  {
    destination: "d5b31a670cef4d3a99861177e6a00b8c",
    identityHex: "77f0de549fb84e189d1544f4f9d3d056",
    lxmfDestinationHex: "4192a8f785b34ee7b0685dd0a6ec4b29",
    announceLastSeenAt: mockNow - 35_000,
    lxmfLastSeenAt: mockNow - 35_000,
    label: "Drone operations",
    announcedName: "DRONE-OPS",
    lastSeenAt: mockNow - 35_000,
    hops: 1,
    interfaceHex: "0fedcba987654321",
    appData: "R3AKT,Telemetry,Imagery",
    sources: ["announce", "import"],
    state: "disconnected",
    saved: true,
    stale: false,
    activeLink: false,
    lastResolutionAttemptAt: mockNow - 20_000,
  },
];

const mockHubDirectoryPeers: HubDirectoryPeerRecord[] = [
  {
    identity: "fdd5d08e476a4602bc51d0f37d72dd21",
    destinationHash: "a13f6e2b94cd08ff31a92765db4e10c2",
    displayName: "ALPHA-1",
    announceCapabilities: ["r3akt", "emergency_messages", "telemetry", "lxmf"],
    clientType: "rem-mobile",
    registeredMode: "connected",
    lastSeen: new Date(mockNow - 50_000).toISOString(),
    status: "active",
  },
  {
    identity: "0e2bf871c1444ed197ed77df5f8632ae",
    destinationHash: "b06c8af91de44070983f6ec2a51b7d35",
    displayName: "MED-RELAY",
    announceCapabilities: ["r3akt", "emergency_messages", "telemetry_relay"],
    clientType: "rem-mobile",
    registeredMode: "semi_autonomous",
    lastSeen: new Date(mockNow - 7 * 60_000).toISOString(),
    status: "syncing",
  },
  {
    identity: "da0ca1b6e4c14a24bd139563a756e932",
    destinationHash: "ef4c2bb30a0d4ec588e797674e385119",
    displayName: "CACHE-TEAM",
    announceCapabilities: ["r3akt", "emergency_messages"],
    clientType: "rem-field",
    registeredMode: "autonomous",
    lastSeen: new Date(mockNow - 26 * 60 * 60_000).toISOString(),
    status: "stale",
  },
];

function applyVisualMockData(): void {
  for (const destination of Object.keys(nodeStore.discoveredByDestination)) {
    delete nodeStore.discoveredByDestination[destination];
  }
  for (const destination of Object.keys(nodeStore.savedByDestination)) {
    delete nodeStore.savedByDestination[destination];
  }

  for (const peer of mockPeerRecords) {
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
    items: mockHubDirectoryPeers,
    receivedAtMs: mockNow,
  };
  nodeStore.lastHubRefreshAt = mockNow;
}

function isVisualMockMode(): boolean {
  return import.meta.env.DEV && route.query.mockPeers === "1";
}

onMounted(() => {
  if (isVisualMockMode()) {
    applyVisualMockData();
    window.setTimeout(applyVisualMockData, 500);
    window.setTimeout(applyVisualMockData, 1500);
    visualMockRefreshInterval = window.setInterval(applyVisualMockData, 2000);
  }
});

onUnmounted(() => {
  if (visualMockRefreshInterval !== undefined) {
    window.clearInterval(visualMockRefreshInterval);
  }
});

watch(activeTab, () => {
  if (isVisualMockMode()) {
    window.setTimeout(applyVisualMockData, 0);
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

function savedPeerConnectionMessage(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  if (peer?.activeLink) {
    return "Connected";
  }
  if (peer?.state === "connecting") {
    return "Connecting";
  }
  return "Disconnected";
}

function savedPeerStatusLabel(destination: string): "Connected" | "Disconnected" {
  return nodeStore.discoveredByDestination[destination]?.activeLink ? "Connected" : "Disconnected";
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
          </div>
        </article>
      </div>
      <p v-else class="empty-copy">No REM-capable announces match this search.</p>
    </section>

    <section v-else-if="activeTab === 'peers'" class="panel">
      <div class="section-header split-header">
        <div>
          <h2>Peers</h2>
          <p>{{ filteredPeers.length }} managed peers | {{ nodeStore.connectedPeerCount }} online</p>
        </div>
        <div class="actions header-inline-actions">
          <button
            type="button"
            @click="
              runNodeAction(() => nodeStore.connectAllSaved(), 'Connected all saved peers.')
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
            <button type="button" @click="nodeStore.unsavePeer(peer.destination)">Remove</button>
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

<style scoped>
.view {
  display: grid;
  gap: 0.82rem;
}

.view-header {
  display: block;
}

.header-actions {
  align-items: center;
  display: grid;
  gap: 0.55rem;
  grid-template-columns: minmax(0, 0.82fr) minmax(0, 0.92fr) minmax(0, 1.62fr) minmax(6.6rem, 0.86fr);
}

.utility-chip {
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
  font-size: clamp(0.74rem, 1.8vw, 0.92rem);
  font-weight: 700;
  gap: 0.46rem;
  justify-content: flex-start;
  min-height: 2.75rem;
  min-width: 0;
  padding: 0.42rem 0.62rem;
  text-decoration: none;
}

.utility-chip svg {
  flex: 0 0 auto;
  height: 1.08rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.08rem;
}

.utility-chip span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.search-chip {
  color: #7fa7d2;
}

.search-chip input {
  background: transparent;
  border: 0;
  color: #d8ecff;
  flex: 1 1 auto;
  font: inherit;
  min-width: 0;
  outline: 0;
  padding: 0;
}

.search-chip input::placeholder {
  color: #7d92b5;
}

.announce-chip {
  color: #29b9ff;
  cursor: pointer;
  justify-content: center;
}

.peer-tabs {
  background: rgb(5 17 39 / 76%);
  border: 1px solid rgb(73 173 255 / 34%);
  border-radius: 14px;
  display: grid;
  gap: 0.35rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  padding: 0.34rem;
}

.tab-button {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 10px;
  box-shadow: none;
  color: #8aa5d1;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  font-weight: 700;
  justify-content: space-between;
  letter-spacing: 0.08em;
  min-height: 2.45rem;
  padding: 0 0.72rem;
  text-transform: uppercase;
}

.tab-button.active {
  background: linear-gradient(180deg, rgb(12 43 88 / 92%), rgb(7 27 63 / 96%));
  border-color: rgb(82 180 255 / 58%);
  box-shadow:
    inset 0 1px 0 rgb(213 245 255 / 12%),
    0 0 20px rgb(35 159 255 / 12%);
  color: #d8f3ff;
}

.tab-button strong {
  color: #7fd8ff;
  font-size: 0.82rem;
}

.panel {
  background: rgb(9 24 52 / 86%);
  border: 1px solid rgb(72 114 184 / 33%);
  border-radius: 15px;
  padding: 0.9rem;
}

h2 {
  font-family: var(--font-headline);
  font-size: 1.52rem;
  margin: 0;
}

.section-header {
  margin-bottom: 0.75rem;
}

.section-header p {
  color: #90a9d2;
  font-family: var(--font-body);
  margin: 0.2rem 0 0;
}

.split-header {
  align-items: flex-start;
  display: flex;
  gap: 0.8rem;
  justify-content: space-between;
}

.peer-list {
  display: grid;
  gap: 0.56rem;
}

.peer-item {
  align-items: center;
  background: rgb(12 27 58 / 74%);
  border: 1px solid rgb(78 123 196 / 26%);
  border-radius: 13px;
  display: grid;
  gap: 0.75rem;
  grid-template-columns: minmax(0, 1fr) auto;
  padding: 0.74rem 0.86rem;
}

.peer-item.compact {
  min-height: 6rem;
}

.peer-copy {
  min-width: 0;
}

.dest {
  color: #ddf1ff;
  font-family: var(--font-ui);
  font-size: 0.9rem;
  letter-spacing: 0.06em;
  margin: 0;
  overflow-wrap: anywhere;
}

.peer-name {
  color: #7be4ff;
  font-family: var(--font-headline);
  font-size: 1.02rem;
  font-weight: 700;
  margin: 0.22rem 0 0;
}

.peer-name-line {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.48rem;
  margin-top: 0.22rem;
}

.peer-name-line .peer-name {
  margin-top: 0;
}

.peer-connection-pill {
  align-items: center;
  border: 1px solid transparent;
  border-radius: 999px;
  display: inline-block;
  flex: 0 0 auto;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  padding: 0.1rem 0.45rem;
  text-transform: uppercase;
}

.peer-connection-pill.connected {
  background: rgb(14 67 42 / 82%);
  border-color: rgb(71 214 145 / 40%);
  color: #8df3c1;
}

.peer-connection-pill.disconnected {
  background: rgb(82 25 35 / 82%);
  border-color: rgb(248 113 113 / 42%);
  color: #fecaca;
}

.peer-meta,
.peer-resolution {
  color: #8ea8d1;
  font-family: var(--font-body);
  font-size: 0.9rem;
  margin: 0.15rem 0 0;
}

.peer-resolution {
  color: #9db9e1;
}

.peer-state {
  align-items: center;
  background: rgb(14 67 42 / 82%);
  border: 1px solid rgb(71 214 145 / 40%);
  border-radius: 999px;
  color: #8df3c1;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 1.6rem;
  padding: 0.1rem 0.55rem;
  text-transform: uppercase;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 0;
}

.inline-actions {
  justify-content: flex-end;
}

.header-inline-actions {
  flex: 0 0 auto;
}

button:not(.tab-button) {
  --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%));
  --btn-border: rgb(74 133 207 / 45%);
  --btn-border-pressed: rgb(224 248 255 / 86%);
  --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #042541;
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 999px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 32px;
  padding: 0 0.82rem;
  text-transform: uppercase;
}

.empty-copy {
  color: #96afd5;
  font-family: var(--font-body);
  margin: 0;
}

@media (max-width: 760px) {
  .header-actions {
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1fr);
  }

  .utility-chip {
    font-size: 0.72rem;
    gap: 0.34rem;
    min-height: 2.55rem;
    padding-inline: 0.42rem;
  }

  .utility-chip svg {
    height: 0.95rem;
    width: 0.95rem;
  }

  .peer-tabs {
    gap: 0.25rem;
  }

  .tab-button {
    font-size: 0.72rem;
    padding-inline: 0.5rem;
  }

  .split-header,
  .peer-item {
    align-items: stretch;
    grid-template-columns: 1fr;
  }

  .split-header {
    flex-direction: column;
  }

  .inline-actions,
  .header-inline-actions {
    justify-content: flex-start;
  }
}
</style>
