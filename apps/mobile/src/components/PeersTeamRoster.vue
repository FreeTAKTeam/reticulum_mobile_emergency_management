<script setup lang="ts">
import { computed } from "vue";
import {
  PhMagnifyingGlass as MagnifyingGlass,
  PhUsersThree as UsersThree,
} from "@phosphor-icons/vue";
import { useRouter } from "vue-router";

import { useTeamDirectory } from "../composables/useTeamDirectory";
import { notifyOperationalUpdate } from "../services/notifications";
import { useNodeStore } from "../stores/nodeStore";

const props = defineProps<{ searchText: string }>();
const emit = defineEmits<{ "update:searchText": [value: string] }>();
const router = useRouter();
const nodeStore = useNodeStore();
const {
  activeSection,
  connectedDestinations,
  connectionStatus,
} = useTeamDirectory();

const filteredRows = computed(() => {
  const query = props.searchText.trim().toLowerCase();
  const rows = activeSection.value?.rows ?? [];
  if (!query) return rows;
  return rows.filter((row) => [
    row.destination,
    row.displayName,
    row.member?.identity ?? "",
    ...(row.member?.announceCapabilities ?? []),
  ].some((value) => value.toLowerCase().includes(query)));
});

function seenLabel(destination: string): string {
  const lastSeenAt = nodeStore.discoveredByDestination[destination]?.lastSeenAt;
  if (!lastSeenAt) return "not recently announced";
  const minutes = Math.floor(Math.max(0, Date.now() - lastSeenAt) / 60_000);
  if (minutes < 60) return `seen ${Math.max(1, minutes)} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `seen ${hours} hr ago`;
  const days = Math.floor(hours / 24);
  return `seen ${days} day${days === 1 ? "" : "s"} ago`;
}

function routeLabel(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  const hops = typeof peer?.hops === "number"
    ? `${peer.hops} hop${peer.hops === 1 ? "" : "s"}`
    : "route unknown";
  return `${destination.slice(0, 8)}… · ${hops}`;
}

async function runAction(
  action: () => Promise<void>,
  successMessage: string,
  failureAction: string,
): Promise<void> {
  try {
    await action();
    await notifyOperationalUpdate("Peers", successMessage, { route: "/peers" });
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    nodeStore.setLastError(detail);
    await notifyOperationalUpdate(
      "Peers",
      `${failureAction} failed - ${detail}`,
      { route: "/peers" },
    );
  }
}

async function toggleConnection(destination: string): Promise<void> {
  const disconnecting = connectedDestinations.value.has(destination.toLowerCase());
  await runAction(
    () => disconnecting
      ? nodeStore.disconnectPeer(destination)
      : nodeStore.connectPeer(destination),
    disconnecting ? "Disconnect requested." : "Connect requested.",
    disconnecting ? "disconnect peer" : "connect peer",
  );
}

async function openManageTeams(): Promise<void> {
  await router.push({
    name: "manage-teams",
    query: { from: "peers" },
  });
}
</script>

<template>
  <section class="active-roster-panel">
    <header class="roster-header">
      <div>
        <h2>Peers in {{ activeSection?.label || "Yellow" }}</h2>
        <p>
          {{ activeSection?.connected ?? 0 }} connected
          <span aria-hidden="true">·</span>
          {{ activeSection?.reachable ?? 0 }} reachable
        </p>
      </div>
      <button type="button" class="manage-teams-button" @click="openManageTeams">
        Manage teams
      </button>
    </header>

    <label class="roster-search">
      <MagnifyingGlass :size="22" aria-hidden="true" />
      <input
        :value="searchText"
        type="search"
        placeholder="Search peers"
        aria-label="Search peers"
        @input="emit('update:searchText', ($event.target as HTMLInputElement).value)"
      />
    </label>

    <div v-if="filteredRows.length > 0" class="roster-list">
      <article v-for="peer in filteredRows" :key="peer.destination" class="roster-row">
        <div class="peer-avatar" :class="connectionStatus(peer.destination).toLowerCase()">
          <UsersThree :size="24" weight="light" aria-hidden="true" />
        </div>
        <div class="roster-copy">
          <h3>{{ peer.displayName }}</h3>
          <p>{{ routeLabel(peer.destination) }}</p>
          <p class="last-seen">{{ seenLabel(peer.destination) }}</p>
        </div>
        <div class="roster-state">
          <span :class="connectionStatus(peer.destination).toLowerCase()">
            {{ connectionStatus(peer.destination) }}
          </span>
        </div>
        <button
          type="button"
          class="connection-button"
          :class="{ connected: connectedDestinations.has(peer.destination.toLowerCase()) }"
          @click="toggleConnection(peer.destination)"
        >
          {{ connectedDestinations.has(peer.destination.toLowerCase()) ? "Disconnect" : "Connect" }}
        </button>
      </article>
    </div>
    <p v-else class="empty-copy">
      {{ searchText.trim() ? "No active-team peers match this search." : "No peers are assigned to the active team." }}
    </p>
  </section>
</template>

<style scoped src="./PeersTeamRoster.css"></style>
