<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useTemplateRef, watch } from "vue";
import {
  PhBroadcast as Broadcast,
  PhCaretDown as CaretDown,
  PhCheck as Check,
  PhMagnifyingGlass as MagnifyingGlass,
  PhPlus as Plus,
} from "@phosphor-icons/vue";
import { useRoute, useRouter } from "vue-router";
import { YELLOW_TEAM_UID } from "@reticulum/node-client";

import ListWindowControls from "../components/ListWindowControls.vue";
import PeersTeamRoster from "../components/PeersTeamRoster.vue";
import { useListWindow } from "../composables/useListWindow";
import {
  teamColorHex,
  teamColorLabel,
  useTeamDirectory,
} from "../composables/useTeamDirectory";
import { notifyOperationalUpdate } from "../services/notifications";
import { useNodeStore } from "../stores/nodeStore";
import type { DiscoveredPeer, HubDirectoryPeerRecord } from "../types/domain";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";
import { discoveredPeerMatchesQuery } from "../utils/peerSearch";
import type { PeersVisualMockData } from "../utils/peersVisualMock";

type PeerTab = "discovered" | "peers" | "hub";
const BLUE_TEAM_UID = "43341e5c822d99857fa6e8641f2ca9c0";
const nodeStore = useNodeStore();
const route = useRoute();
const router = useRouter();
const searchText = ref("");
const activeTab = ref<PeerTab>("peers");
const teamMenu = useTemplateRef<HTMLDetailsElement>("teamMenu");
const { activeSection, selectableTeams, setActiveTeam, teamLabel } = useTeamDirectory();
const mockNow = Date.now();
let visualMockData: PeersVisualMockData | undefined;
let visualMockRefreshInterval: number | undefined;

async function applyVisualMockData(): Promise<void> {
  visualMockData ??= (await import("../utils/peersVisualMock")).createPeersVisualMockData(mockNow);
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
  const selectedTeam = nodeStore.settings.teams.activeTeamUid
    || nodeStore.hubDirectorySnapshot?.activeTeamUid;
  const activeTeamUid = selectedTeam === BLUE_TEAM_UID ? BLUE_TEAM_UID : YELLOW_TEAM_UID;
  nodeStore.hubDirectorySnapshot = {
    schemaVersion: 2,
    activeTeamUid,
    effectiveConnectedMode: true,
    teams: [
      { uid: YELLOW_TEAM_UID, color: "YELLOW", teamName: "Yellow" },
      { uid: BLUE_TEAM_UID, color: "BLUE", teamName: "Blue" },
    ],
    callerMemberships: [
      { teamUid: YELLOW_TEAM_UID, teamMemberUid: "mock-caller-yellow" },
      { teamUid: BLUE_TEAM_UID, teamMemberUid: "mock-caller-blue" },
    ],
    members: visualMockData.hubDirectoryPeers.flatMap((peer, index) => {
      const member = { ...peer, teamMemberUid: `mock-member-${index}` };
      return index === 0
        ? [{ ...member, teamUid: YELLOW_TEAM_UID }, { ...member, teamUid: BLUE_TEAM_UID }]
        : [{ ...member, teamUid: BLUE_TEAM_UID }];
    }),
    localTeams: nodeStore.settings.teams.localTeams.map((team) => ({
      ...team,
      memberDestinations: [...team.memberDestinations],
    })),
    items: visualMockData.hubDirectoryPeers,
    receivedAtMs: mockNow,
  };
  nodeStore.lastHubRefreshAt = mockNow;
}

function isVisualMockMode(): boolean {
  return import.meta.env.DEV && route.query.mockPeers === "1";
}

function scheduleVisualMockRefresh(): void {
  runDetachedStoreTask(nodeStore, "peers", "visual mock refresh", applyVisualMockData);
}

onMounted(() => {
  if (!isVisualMockMode()) return;
  scheduleVisualMockRefresh();
  window.setTimeout(scheduleVisualMockRefresh, 500);
  window.setTimeout(scheduleVisualMockRefresh, 1500);
  visualMockRefreshInterval = window.setInterval(scheduleVisualMockRefresh, 2000);
});

onUnmounted(() => {
  if (visualMockRefreshInterval !== undefined) window.clearInterval(visualMockRefreshInterval);
});

watch(activeTab, () => {
  if (isVisualMockMode()) window.setTimeout(scheduleVisualMockRefresh, 0);
});

const filteredDiscovered = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.remAnnouncedPeers
    .filter((peer: DiscoveredPeer) => !query || discoveredPeerMatchesQuery(peer, query))
    .sort((left, right) => right.lastSeenAt - left.lastSeenAt);
});

const filteredHubPeers = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.hubDirectoryPeers.filter((peer: HubDirectoryPeerRecord) => !query || [
    peer.identity,
    peer.destinationHash,
    peer.displayName ?? "",
    ...peer.announceCapabilities,
  ].some((value) => value.toLowerCase().includes(query)));
});
const discoveredWindow = useListWindow(filteredDiscovered, { resetKey: searchText });
const hubWindow = useListWindow(filteredHubPeers, { resetKey: searchText });

function peerName(peer: Pick<DiscoveredPeer, "announcedName" | "label" | "destination">): string {
  return peer.announcedName || peer.label || peer.destination;
}

function seenLabel(lastSeenAt?: number): string {
  if (!lastSeenAt) return "never seen";
  const minutes = Math.floor(Math.max(0, Date.now() - lastSeenAt) / 60_000);
  if (minutes < 60) return `seen ${Math.max(1, minutes)} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `seen ${hours} hr ago`;
  const days = Math.floor(hours / 24);
  return `seen ${days} day${days === 1 ? "" : "s"} ago`;
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
    const detail = error instanceof Error ? error.message : String(error);
    await notifyOperationalUpdate("Peer", `${failureAction} failed - ${detail}`, { route: "/peers" });
  }
}

async function changeSavedState(destination: string, save: boolean): Promise<void> {
  const peer = nodeStore.discoveredByDestination[destination];
  const name = peer?.announcedName || peer?.label || `${destination.slice(0, 5)}...`;
  await runNodeAction(
    () => save ? nodeStore.savePeer(destination) : nodeStore.removePeer(destination),
    `${save ? "added" : "removed"} ${name}`,
    "Peer",
    `${save ? "add" : "remove"} ${name}`,
  );
}

function selectorLabel(teamUid: string): string {
  const team = selectableTeams.value.find((entry) => entry.uid === teamUid);
  if (!team) return "Yellow";
  const color = teamColorLabel(team.color);
  const label = teamLabel(team.uid);
  return label.toLowerCase() === color.toLowerCase() ? color : `${color} · ${label}`;
}

async function chooseActiveTeam(teamUid: string): Promise<void> {
  teamMenu.value?.removeAttribute("open");
  await runNodeAction(
    () => setActiveTeam(teamUid),
    `${teamLabel(teamUid)} is now the active team.`,
    "Teams",
    "select active team",
  );
}

async function openManageTeams(action?: "add"): Promise<void> {
  teamMenu.value?.removeAttribute("open");
  await router.push({
    name: "manage-teams",
    query: {
      from: "peers",
      ...(action ? { action } : {}),
    },
  });
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <details ref="teamMenu" class="active-team-menu">
          <summary aria-label="Active team">
            <span
              class="team-color-dot"
              :style="{ '--team-color': teamColorHex(activeSection?.team.color || 'YELLOW') }"
              aria-hidden="true"
            ></span>
            <span class="active-team-copy">
              <strong>{{ selectorLabel(activeSection?.team.uid || YELLOW_TEAM_UID) }}</strong>
              <small>Active team</small>
            </span>
            <CaretDown class="team-menu-caret" :size="20" aria-hidden="true" />
          </summary>
          <div class="team-menu-popover" role="menu" aria-label="Choose active team">
            <button
              v-for="team in selectableTeams"
              :key="team.uid"
              type="button"
              class="team-menu-item"
              role="menuitemradio"
              :aria-checked="team.uid === activeSection?.team.uid"
              @click="chooseActiveTeam(team.uid)"
            >
              <span
                class="team-color-dot"
                :style="{ '--team-color': teamColorHex(team.color) }"
                aria-hidden="true"
              ></span>
              <span>{{ selectorLabel(team.uid) }}</span>
              <Check v-if="team.uid === activeSection?.team.uid" :size="19" aria-hidden="true" />
            </button>
            <button type="button" class="team-menu-item add-team-option" role="menuitem" @click="openManageTeams('add')">
              <Plus :size="20" aria-hidden="true" />
              <span>Add team</span>
            </button>
          </div>
        </details>
        <button type="button" class="utility-chip announce-chip" @click="runNodeAction(() => nodeStore.announceNow(), 'Announce requested.')">
          <Broadcast :size="20" aria-hidden="true" />
          Announce
        </button>
      </div>
    </header>

    <nav class="peer-tabs" aria-label="Peer sections">
      <button type="button" class="tab-button" :class="{ active: activeTab === 'discovered' }" @click="activeTab = 'discovered'">
        <span>Discovered</span><strong>{{ filteredDiscovered.length }}</strong>
      </button>
      <button type="button" class="tab-button" :class="{ active: activeTab === 'peers' }" @click="activeTab = 'peers'">
        <span>Peers</span><strong>{{ nodeStore.savedPeerCount }}</strong>
      </button>
      <button type="button" class="tab-button" :class="{ active: activeTab === 'hub' }" @click="activeTab = 'hub'">
        <span>Hub</span><strong>{{ filteredHubPeers.length }}</strong>
      </button>
    </nav>

    <section v-if="activeTab === 'discovered'" class="panel">
      <div class="section-header"><h2>Discovered</h2><p>{{ filteredDiscovered.length }} REM clients heard through announces</p></div>
      <label class="panel-search">
        <MagnifyingGlass :size="21" aria-hidden="true" />
        <input v-model="searchText" type="search" placeholder="Search discovered peers" aria-label="Search discovered peers" />
      </label>
      <div v-if="filteredDiscovered.length > 0" class="peer-list">
        <article v-for="peer in discoveredWindow.items.value" :key="peer.destination" class="peer-item compact">
          <div class="peer-copy">
            <p class="dest">{{ peer.destination }}</p>
            <div class="peer-name-line"><span v-if="nodeStore.savedDestinations.has(peer.destination)" class="peer-state">Peer</span><p class="peer-name">{{ peerName(peer) }}</p></div>
            <p class="peer-meta">{{ seenLabel(peer.lastSeenAt) }}{{ typeof peer.hops === "number" ? ` | ${peer.hops} hops` : "" }}</p>
          </div>
          <div class="actions inline-actions">
            <button v-if="!nodeStore.savedDestinations.has(peer.destination)" type="button" @click="changeSavedState(peer.destination, true)">Add</button>
            <button v-else type="button" @click="changeSavedState(peer.destination, false)">Remove</button>
          </div>
        </article>
        <ListWindowControls :start="discoveredWindow.startIndex.value" :end="discoveredWindow.endIndex.value" :total="discoveredWindow.total.value" :has-previous="discoveredWindow.hasPrevious.value" :has-next="discoveredWindow.hasNext.value" @previous="discoveredWindow.previous" @next="discoveredWindow.next" />
      </div>
      <p v-else class="empty-copy">No REM-capable announces match this search.</p>
    </section>

    <PeersTeamRoster v-else-if="activeTab === 'peers'" v-model:search-text="searchText" />

    <section v-else class="panel">
      <div class="section-header split-header">
        <div><h2>Hub</h2><p>Mode: {{ nodeStore.settings.hub.mode }} | Last refresh: {{ nodeStore.lastHubRefreshAt ? new Date(nodeStore.lastHubRefreshAt).toLocaleTimeString() : "never" }}</p></div>
        <div class="actions header-inline-actions"><button type="button" @click="runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub directory refreshed.')">Refresh hub list</button></div>
      </div>
      <label class="panel-search">
        <MagnifyingGlass :size="21" aria-hidden="true" />
        <input v-model="searchText" type="search" placeholder="Search hub peers" aria-label="Search hub peers" />
      </label>
      <div v-if="filteredHubPeers.length > 0" class="peer-list">
        <article v-for="peer in hubWindow.items.value" :key="peer.destinationHash" class="peer-item hub-item">
          <div class="peer-copy"><p class="dest">{{ peer.destinationHash }}</p><p class="peer-name">{{ peer.displayName || peer.identity }}</p><p class="peer-meta">{{ peer.status || "unknown" }} | {{ peer.registeredMode || "unregistered" }} | {{ peer.clientType || "unknown client" }}</p><p class="peer-resolution">{{ peer.announceCapabilities.join(", ") }}</p></div>
        </article>
        <ListWindowControls :start="hubWindow.startIndex.value" :end="hubWindow.endIndex.value" :total="hubWindow.total.value" :has-previous="hubWindow.hasPrevious.value" :has-next="hubWindow.hasNext.value" @previous="hubWindow.previous" @next="hubWindow.next" />
      </div>
      <p v-else class="empty-copy">No hub peers cached.</p>
    </section>
  </section>
</template>

<style scoped src="./PeersDiscoveryView.css"></style>
