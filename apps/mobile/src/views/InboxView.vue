<script setup lang="ts">
import { computed, onMounted } from "vue";

import ConversationList from "../components/messaging/ConversationList.vue";
import ConversationThread from "../components/messaging/ConversationThread.vue";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";

const messagingStore = useMessagingStore();
const nodeStore = useNodeStore();

onMounted(() => {
  messagingStore.init();
});

const selectedPeerDisplayName = computed(() =>
  messagingStore.selectedConversation?.displayName ?? "",
);

const selectedDestinationHex = computed(() =>
  messagingStore.selectedConversation?.destinationHex ?? "",
);

const syncStatusLabel = computed(() => {
  const status = nodeStore.syncStatus;
  const detail = status.detail?.trim();
  return detail ? `${status.phase}: ${detail}` : status.phase;
});

async function send(bodyUtf8: string): Promise<void> {
  const destinationHex = selectedDestinationHex.value;
  if (!destinationHex) {
    return;
  }
  await messagingStore.sendMessage(destinationHex, bodyUtf8);
}

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

async function useSelectedAsPropagationNode(): Promise<void> {
  if (!selectedDestinationHex.value) {
    return;
  }
  try {
    await nodeStore.setActivePropagationNode(selectedDestinationHex.value);
  } catch {
    // nodeStore already records the failure for the status surface
  }
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1 class="view-title">Inbox</h1>
        <p class="view-copy">
          Sideband-style peer messaging over LXMF with transport-backed delivery updates.
        </p>
      </div>
      <div class="view-actions">
        <p class="view-status">
          {{ nodeStore.ready ? "Node ready" : "Node not ready" }}
        </p>
        <button type="button" class="action-button" @click="announceNow">Announce</button>
        <button type="button" class="action-button" @click="requestSync">Sync</button>
        <button
          type="button"
          class="action-button"
          :disabled="!selectedDestinationHex"
          @click="useSelectedAsPropagationNode"
        >
          Set Propagation Peer
        </button>
      </div>
    </header>

    <section class="panel sync-panel">
      <p class="sync-line">
        Sync status: <strong>{{ syncStatusLabel }}</strong>
      </p>
      <p class="sync-line">
        Active propagation node:
        <strong>{{ nodeStore.syncStatus.activePropagationNodeHex || "none" }}</strong>
      </p>
    </section>

    <section class="panel inbox-layout">
      <ConversationList
        :items="messagingStore.conversations"
        :selected-conversation-id="messagingStore.selectedConversationId"
        @select="messagingStore.selectConversation"
      />
      <ConversationThread
        :destination-hex="selectedDestinationHex"
        :display-name="selectedPeerDisplayName"
        :messages="messagingStore.activeMessages"
        @send="send"
      />
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.view-header {
  align-items: end;
  display: flex;
  gap: 1rem;
  justify-content: space-between;
}

.view-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.65rem;
  justify-content: flex-end;
}

.view-title,
.view-copy,
.view-status {
  margin: 0;
}

.view-title {
  font-family: var(--font-headline);
  font-size: clamp(1.9rem, 3vw, 2.8rem);
}

.view-copy,
.view-status {
  color: #94add3;
  font-family: var(--font-body);
}

.panel {
  background: rgb(9 24 52 / 86%);
  border: 1px solid rgb(72 114 184 / 33%);
  border-radius: 15px;
  padding: 0.95rem;
}

.sync-panel {
  display: grid;
  gap: 0.35rem;
}

.sync-line {
  color: #cfe5ff;
  font-family: var(--font-ui);
  margin: 0;
}

.action-button {
  background: linear-gradient(135deg, rgb(16 75 135 / 90%), rgb(24 125 170 / 82%));
  border: 1px solid rgb(120 227 255 / 35%);
  border-radius: 999px;
  color: #f5fbff;
  cursor: pointer;
  font-family: var(--font-ui);
  padding: 0.55rem 0.9rem;
}

.action-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.inbox-layout {
  display: grid;
  gap: 1rem;
  grid-template-columns: minmax(16rem, 22rem) minmax(0, 1fr);
}

@media (max-width: 900px) {
  .inbox-layout {
    grid-template-columns: 1fr;
  }
}
</style>
