<script setup lang="ts">
import { computed, ref } from "vue";

import PeerRow from "../components/PeerRow.vue";
import { useNodeStore } from "../stores/nodeStore";
import type { DiscoveredPeer, SavedPeer } from "../types/domain";

const nodeStore = useNodeStore();

const searchText = ref("");
const feedback = ref("");
const isSavedSectionOpen = ref(true);

const filteredDiscovered = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.discoveredPeers.filter((peer: DiscoveredPeer) => {
    const requiresCapabilityVerification =
      peer.sources.includes("announce") && !peer.sources.includes("hub");

    if (
      nodeStore.settings.showOnlyCapabilityVerified &&
      requiresCapabilityVerification &&
      !peer.verifiedCapability
    ) {
      return false;
    }
    if (!query) {
      return true;
    }
    return (
      peer.destination.includes(query) ||
      (peer.label ?? "").toLowerCase().includes(query) ||
      (peer.announcedName ?? "").toLowerCase().includes(query) ||
      (peer.appData ?? "").toLowerCase().includes(query)
    );
  });
});

function announcedNameFor(destination: string): string | undefined {
  return nodeStore.discoveredByDestination[destination]?.announcedName;
}

const filteredSaved = computed(() => {
  const query = searchText.value.trim().toLowerCase();
  return nodeStore.savedPeers.filter((peer: SavedPeer) => {
    if (!query) {
      return true;
    }
    const announcedName = announcedNameFor(peer.destination)?.toLowerCase() ?? "";
    return (
      peer.destination.includes(query) ||
      (peer.label ?? "").toLowerCase().includes(query) ||
      announcedName.includes(query)
    );
  });
});

function savedPeerConnectionLabel(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  return peer?.activeLink || peer?.managementState === "managed" ? "Disconnect" : "Connect";
}

function savedPeerConnectionMessage(destination: string): string {
  const peer = nodeStore.discoveredByDestination[destination];
  if (peer?.activeLink) {
    return "Connected";
  }
  if (peer?.managementState === "managed" || peer?.state === "connecting") {
    return "Connecting";
  }
  return "Disconnected";
}

async function onSavedPeerConnectToggle(destination: string): Promise<void> {
  const disconnecting = savedPeerConnectionLabel(destination) === "Disconnect";
  await runNodeAction(
    () => (disconnecting ? nodeStore.disconnectPeer(destination) : nodeStore.connectPeer(destination)),
    disconnecting
      ? `Disconnect requested for ${destination}.`
      : `Connect requested for ${destination}.`,
  );
}

function isSaved(destination: string): boolean {
  return nodeStore.savedDestinations.has(destination);
}

async function onSaveToggle(destination: string, next: boolean): Promise<void> {
  try {
    if (next) {
      await nodeStore.savePeer(destination);
    } else {
      await nodeStore.unsavePeer(destination);
    }
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  }
}

async function onConnectToggle(destination: string, next: boolean): Promise<void> {
  try {
    if (next) {
      await nodeStore.connectPeer(destination);
    } else {
      await nodeStore.disconnectPeer(destination);
    }
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  }
}

async function runNodeAction(action: () => Promise<void>, successMessage: string): Promise<void> {
  try {
    await action();
    feedback.value = successMessage;
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  }
}
</script>

<template>
  <section class="view">
    <header>
      <h1>Peers &amp; Discovery</h1>
      <p>
        Select destination allowlist per device. New discoveries never auto-save.
      </p>
      <div class="header-actions">
        <button
          type="button"
          class="badge badge-button"
          @click="
            runNodeAction(() => nodeStore.announceNow(), 'Announce requested.')
          "
        >
          Announce
        </button>
        <button
          type="button"
          class="badge badge-button"
          @click="
            runNodeAction(() => nodeStore.requestLxmfSync(), 'Sync requested.')
          "
        >
          Sync
        </button>
      </div>
    </header>

    <section class="panel controls">
      <input
        v-model="searchText"
        type="search"
        placeholder="Search destination, label, or announced name"
      />
      <label class="checkbox">
        <input
          :checked="nodeStore.settings.showOnlyCapabilityVerified"
          type="checkbox"
          @change="
            nodeStore.updateSettings({
              showOnlyCapabilityVerified: ($event.target as HTMLInputElement).checked,
            })
          "
        />
        Show only capability-verified peers
      </label>
    </section>

    <section class="panel">
      <h2>Discovered</h2>
      <p class="section-meta">
        {{ nodeStore.communicationReadyPeerCount }}/{{ nodeStore.connectedLinkDestinations.length }} possible/connected |
        {{ nodeStore.missionReadyPeerCount }} mission-ready |
        {{ nodeStore.relayEligiblePeerCount }} relay-eligible |
        {{ filteredDiscovered.length }} peers visible
      </p>
      <div class="rows">
        <PeerRow
          v-for="peer in filteredDiscovered"
          :key="peer.destination"
          :peer="peer"
          :is-saved="isSaved(peer.destination)"
          @save-toggle="onSaveToggle"
          @connect-toggle="onConnectToggle"
          @label-change="nodeStore.setPeerLabel"
        />
      </div>
    </section>

    <section class="panel saved-panel">
      <button
        type="button"
        class="saved-toggle"
        :aria-expanded="isSavedSectionOpen"
        @click="isSavedSectionOpen = !isSavedSectionOpen"
      >
        <div class="saved-toggle-copy">
          <h2>Saved</h2>
          <p class="section-meta">{{ filteredSaved.length }} peers saved locally</p>
        </div>
        <span class="saved-toggle-icon" :class="{ open: isSavedSectionOpen }" aria-hidden="true">
          ▾
        </span>
      </button>
      <div v-show="isSavedSectionOpen" class="saved-section-body">
        <div class="actions saved-actions">
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
        <div v-if="filteredSaved.length > 0" class="saved-list">
          <article v-for="peer in filteredSaved" :key="peer.destination" class="saved-item">
            <div>
              <p class="dest">{{ peer.destination }}</p>
              <p class="saved-label">{{ peer.label || "No label" }}</p>
              <p class="saved-state">{{ savedPeerConnectionMessage(peer.destination) }}</p>
            </div>
            <div class="actions">
              <button type="button" @click="onSavedPeerConnectToggle(peer.destination)">
                {{ savedPeerConnectionLabel(peer.destination) }}
              </button>
              <button type="button" @click="nodeStore.unsavePeer(peer.destination)">Remove</button>
            </div>
          </article>
        </div>
        <p v-else class="saved-empty">No saved peers yet.</p>
        <p v-if="feedback" class="feedback">{{ feedback }}</p>
      </div>
    </section>

    <section class="panel">
      <div class="section-header">
        <h2>Directory (Hub)</h2>
        <p>
          Mode: {{ nodeStore.settings.hub.mode }} | Last refresh:
          {{
            nodeStore.lastHubRefreshAt
              ? new Date(nodeStore.lastHubRefreshAt).toLocaleTimeString()
              : "never"
          }}
        </p>
      </div>
      <div class="actions">
        <button
          type="button"
          @click="
            runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub directory refreshed.')
          "
        >
          Refresh hub list
        </button>
      </div>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.8rem, 3.4vw, 2.9rem);
  margin: 0;
}

header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0.25rem 0 0;
}

.header-actions {
  align-items: center;
  display: flex;
  gap: 0.55rem;
  margin-top: 0.7rem;
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
  box-shadow:
    inset 0 1px 0 rgb(186 236 255 / 8%),
    0 8px 18px rgb(3 24 56 / 18%);
  cursor: pointer;
  min-height: 0;
  touch-action: manipulation;
  transition:
    background 120ms ease,
    border-color 120ms ease,
    box-shadow 120ms ease,
    color 120ms ease,
    transform 120ms ease;
}

.badge-button:active {
  background: linear-gradient(118deg, #0b4d7d, #106f90);
  border-color: rgb(86 197 255 / 72%);
  box-shadow:
    inset 0 1px 0 rgb(224 248 255 / 14%),
    0 4px 10px rgb(3 18 40 / 20%);
  color: #dff8ff;
  transform: translateY(1px) scale(0.985);
}

.badge-button:focus-visible {
  outline: 2px solid rgb(111 219 255 / 70%);
  outline-offset: 2px;
}

.panel {
  background: rgb(9 24 52 / 86%);
  border: 1px solid rgb(72 114 184 / 33%);
  border-radius: 15px;
  padding: 0.9rem;
}

.controls input[type="search"] {
  width: min(420px, 100%);
}

h2 {
  font-family: var(--font-headline);
  font-size: 1.52rem;
  margin: 0;
}

.section-meta {
  color: #90a9d2;
  font-family: var(--font-body);
  margin: 0.25rem 0 0.65rem;
}

.section-header p {
  color: #90a9d2;
  font-family: var(--font-body);
  margin: 0.2rem 0 0;
}

.rows {
  display: grid;
  gap: 0.56rem;
}

.saved-list {
  display: grid;
  gap: 0.5rem;
}

.saved-panel {
  gap: 0.75rem;
}

.saved-toggle {
  align-items: center;
  background: transparent;
  border: 0;
  color: inherit;
  display: flex;
  justify-content: space-between;
  padding: 0;
  text-align: left;
  width: 100%;
}

.saved-toggle-copy {
  min-width: 0;
}

.saved-toggle-copy .section-meta {
  margin-bottom: 0;
}

.saved-toggle-icon {
  color: #7fd8ff;
  font-size: 1.1rem;
  line-height: 1;
  transform: rotate(-90deg);
  transition: transform 160ms ease;
}

.saved-toggle-icon.open {
  transform: rotate(0deg);
}

.saved-section-body {
  border-top: 1px solid rgb(71 112 176 / 22%);
  margin-top: 0.75rem;
  padding-top: 0.75rem;
}

.saved-actions {
  margin-top: 0;
}

.saved-item {
  align-items: center;
  background: rgb(9 24 50 / 70%);
  border: 1px solid rgb(71 112 176 / 29%);
  border-radius: 11px;
  display: flex;
  justify-content: space-between;
  padding: 0.6rem 0.74rem;
}

.dest {
  color: #d5eaff;
  font-family: var(--font-ui);
  font-size: 0.89rem;
  letter-spacing: 0.06em;
  margin: 0;
}

.saved-label {
  color: #8aa5d1;
  font-family: var(--font-body);
  margin: 0.15rem 0 0;
}

.saved-state {
  color: #7fd8ff;
  font-family: var(--font-body);
  font-size: 0.84rem;
  margin: 0.12rem 0 0;
}

.saved-empty {
  color: #8aa5d1;
  font-family: var(--font-body);
  margin: 0;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 0.65rem;
}

button:not(.saved-toggle):not(.badge-button) {
  background: linear-gradient(118deg, #0b9fff, #20ecff);
  border: 0;
  border-radius: 10px;
  box-shadow: 0 10px 22px rgb(3 32 75 / 22%);
  color: #03284b;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 34px;
  padding: 0 0.76rem;
  touch-action: manipulation;
  transition:
    background 120ms ease,
    box-shadow 120ms ease,
    color 120ms ease,
    transform 120ms ease;
  text-transform: uppercase;
}

button:not(.saved-toggle):not(.badge-button):active {
  background: linear-gradient(118deg, #046aa8, #0ea9cb);
  box-shadow:
    inset 0 1px 0 rgb(220 248 255 / 16%),
    0 4px 10px rgb(3 21 47 / 24%);
  color: #e8fbff;
  transform: translateY(1px) scale(0.985);
}

.checkbox {
  align-items: center;
  color: #9bb3d7;
  display: flex;
  font-family: var(--font-body);
  gap: 0.45rem;
  margin-top: 0.7rem;
}

input,
textarea {
  background: rgb(6 18 39 / 82%);
  border: 1px solid rgb(70 110 172 / 43%);
  border-radius: 10px;
  color: #d8ecff;
  font-family: var(--font-body);
  font-size: 0.96rem;
  padding: 0.5rem 0.56rem;
}

textarea {
  margin-top: 0.6rem;
  resize: vertical;
  width: 100%;
}

.feedback {
  color: #96afd5;
  font-family: var(--font-body);
  margin: 0.58rem 0 0;
}

@media (max-width: 760px) {
  .header-actions {
    flex-wrap: wrap;
  }

  .saved-item {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.55rem;
  }
}
</style>
