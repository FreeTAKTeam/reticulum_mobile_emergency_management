<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";

import ConversationList from "../components/messaging/ConversationList.vue";
import ConversationThread from "../components/messaging/ConversationThread.vue";
import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import { getMessageOverallScore, getOverallStatusBand } from "../utils/actionMessageStatus";
import { formatR3aktTeamColor } from "../utils/r3akt";

const messagingStore = useMessagingStore();
const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();
const telemetryStore = useTelemetryStore();
const mobilePane = shallowRef<"list" | "detail">("list");

onMounted(() => {
  messagingStore.init();
  messagesStore.init();
  messagesStore.initReplication();
});

const selectedConversation = computed(() => messagingStore.selectedConversation);
const activeConversationId = computed(() =>
  selectedConversation.value?.conversationId ?? messagingStore.selectedConversationId,
);
const selectedPeerDisplayName = computed(() =>
  selectedConversation.value?.displayName ?? "",
);

const selectedDestinationHex = computed(() =>
  selectedConversation.value?.destinationHex ?? "",
);
const hasSelectedConversation = computed(() => selectedConversation.value !== null);
const conversationCount = computed(() => messagingStore.conversations.length);
const selectedPeer = computed(() => {
  const destinationHex = selectedDestinationHex.value.trim().toLowerCase();
  if (!destinationHex) {
    return null;
  }
  return nodeStore.discoveredByDestination[destinationHex]
    ?? Object.values(nodeStore.discoveredByDestination).find((peer) =>
      peer.destination.trim().toLowerCase() === destinationHex
      || peer.lxmfDestinationHex?.trim().toLowerCase() === destinationHex,
    )
    ?? null;
});
const targetLookupNames = computed(() =>
  [...new Set([
    selectedPeerDisplayName.value,
    selectedPeer.value?.label ?? "",
    selectedPeer.value?.announcedName ?? "",
  ]
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
    .map((value) => value.toLowerCase()))],
);
const selectedTargetMessage = computed(() =>
  messagesStore.messages.find((message) => {
    const callsign = message.callsign.trim().toLowerCase();
    const sourceDisplayName = message.source?.display_name?.trim().toLowerCase() ?? "";
    return targetLookupNames.value.includes(callsign) || targetLookupNames.value.includes(sourceDisplayName);
  }) ?? null,
);
const targetStatusLabel = computed(() => {
  const message = selectedTargetMessage.value;
  if (!message) {
    return "Unknown";
  }
  return message.overallStatus ?? getOverallStatusBand(getMessageOverallScore(message));
});
const targetTeamLabel = computed(() => {
  const message = selectedTargetMessage.value;
  if (!message?.groupName) {
    return "";
  }
  return `${formatR3aktTeamColor(message.groupName)} Team`;
});
const targetTelemetryPosition = computed(() => {
  const lookupKeys = [
    selectedTargetMessage.value?.callsign ?? "",
    ...targetLookupNames.value,
  ]
    .map((value) => value.trim().toLowerCase())
    .filter((value) => value.length > 0);

  for (const key of lookupKeys) {
    const position = telemetryStore.byCallsign[key];
    if (position) {
      return position;
    }
  }
  return null;
});

const syncStatusLabel = computed(() => {
  const status = nodeStore.syncStatus;
  const detail = status.detail?.trim();
  return detail ? `${status.phase}: ${detail}` : status.phase;
});

function formatCoordinate(value: number, positiveLabel: string, negativeLabel: string): string {
  const hemisphere = value >= 0 ? positiveLabel : negativeLabel;
  return `${Math.abs(value).toFixed(2)}° ${hemisphere}`;
}

const targetLatitudeLabel = computed(() =>
  targetTelemetryPosition.value
    ? formatCoordinate(targetTelemetryPosition.value.lat, "N", "S")
    : "",
);
const targetLongitudeLabel = computed(() =>
  targetTelemetryPosition.value
    ? formatCoordinate(targetTelemetryPosition.value.lon, "E", "W")
    : "",
);

function handleSelectConversation(conversationId: string): void {
  messagingStore.selectConversation(conversationId);
  mobilePane.value = "detail";
}

function showConversationList(): void {
  mobilePane.value = "list";
}

function showConversationDetail(): void {
  if (!hasSelectedConversation.value) {
    return;
  }
  mobilePane.value = "detail";
}

async function send(bodyUtf8: string): Promise<void> {
  const destinationHex = selectedDestinationHex.value;
  if (!destinationHex) {
    return;
  }
  await messagingStore.sendMessage(destinationHex, bodyUtf8);
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1 class="view-title">Inbox</h1>
      </div>
      <div class="view-actions">
        <p class="view-status">
          {{ nodeStore.ready ? "Node ready" : "Node not ready" }}
        </p>
      </div>
    </header>

    <section class="panel sync-panel">
      <p class="sync-line">
        Sync status: <strong>{{ syncStatusLabel }}</strong>
      </p>
      <button
        v-if="mobilePane === 'detail'"
        type="button"
        class="pane-toggle mobile-only sync-back-button"
        @click="showConversationList"
      >
        Back
      </button>
    </section>

    <section class="inbox-layout" :class="`pane-${mobilePane}`">
      <section class="panel inbox-panel list-panel">
        <header class="inbox-panel-header">
          <div>
            <p class="panel-kicker">Conversations</p>
            <h2 class="panel-title">Encrypted Inbox</h2>
          </div>
          <button
            v-if="hasSelectedConversation"
            type="button"
            class="pane-toggle mobile-only"
            @click="showConversationDetail"
          >
            Open Thread
          </button>
        </header>
        <p class="panel-copy">
          {{ conversationCount }} conversation{{ conversationCount === 1 ? "" : "s" }} available.
        </p>
        <ConversationList
          :items="messagingStore.conversations"
          :selected-conversation-id="activeConversationId"
          @select="handleSelectConversation"
        />
      </section>

      <section class="panel inbox-panel detail-panel">
        <ConversationThread
          :destination-hex="selectedDestinationHex"
          :display-name="selectedPeerDisplayName"
          :target-status="targetStatusLabel"
          :target-team="targetTeamLabel"
          :target-latitude="targetLatitudeLabel"
          :target-longitude="targetLongitudeLabel"
          :messages="messagingStore.activeMessages"
          @send="send"
        />
      </section>
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
.view-status {
  margin: 0;
}

.view-title {
  font-family: var(--font-headline);
  font-size: clamp(1.9rem, 3vw, 2.8rem);
}

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
  align-items: center;
  display: flex;
  gap: 0.75rem;
  justify-content: space-between;
}

.sync-line {
  color: #cfe5ff;
  font-family: var(--font-ui);
  margin: 0;
}

.sync-back-button {
  flex-shrink: 0;
}

.inbox-panel {
  display: grid;
  gap: 0.9rem;
}

.inbox-panel-header {
  align-items: start;
  display: flex;
  gap: 0.85rem;
  justify-content: space-between;
}

.panel-kicker,
.panel-title,
.panel-copy {
  margin: 0;
}

.panel-kicker {
  color: #60d8ff;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  letter-spacing: 0.18em;
  text-transform: uppercase;
}

.panel-title {
  color: #f1fbff;
  font-family: var(--font-headline);
  font-size: clamp(1.1rem, 2vw, 1.45rem);
}

.panel-copy {
  color: #8ea8d1;
  font-family: var(--font-body);
  font-size: 0.92rem;
}

.inbox-layout {
  display: grid;
  gap: 1rem;
  grid-template-columns: minmax(16rem, 22rem) minmax(0, 1fr);
}

.pane-toggle {
  background: linear-gradient(110deg, #00a8ff, #14f0ff);
  border: 0;
  border-radius: 11px;
  color: #032748;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  min-height: 38px;
  padding: 0 0.95rem;
  text-transform: uppercase;
}

.pane-toggle:active {
  background: linear-gradient(110deg, #0678bf, #10bbd8);
  transform: translateY(1px) scale(0.985);
}

.mobile-only {
  display: none;
}

@media (max-width: 900px) {
  .inbox-layout {
    grid-template-columns: 1fr;
  }

  .mobile-only {
    display: inline-flex;
  }

  .pane-list .detail-panel,
  .pane-detail .list-panel {
    display: none;
  }
}
</style>
