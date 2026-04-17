<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import ConversationList from "../components/messaging/ConversationList.vue";
import ConversationThread from "../components/messaging/ConversationThread.vue";
import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import type { DiscoveredPeer } from "../types/domain";
import { getMessageOverallScore, getOverallStatusBand } from "../utils/actionMessageStatus";
import { formatR3aktTeamColor } from "../utils/r3akt";

const messagingStore = useMessagingStore();
const messagesStore = useMessagesStore();
const nodeStore = useNodeStore();
const sosStore = useSosStore();
const telemetryStore = useTelemetryStore();
const route = useRoute();
const router = useRouter();
const mobilePane = shallowRef<"list" | "detail">("list");
const selectedThreadDestinationHex = shallowRef("");

interface ConnectedPeerOption {
  value: string;
  displayName: string;
}

interface SosMessageMapTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
}

function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function safeLower(value: unknown): string {
  return safeTrim(value).toLowerCase();
}

function routeQueryString(value: unknown): string {
  return Array.isArray(value) ? safeTrim(value[0]) : safeTrim(value);
}

function destinationsMatch(left: unknown, right: unknown): boolean {
  const normalizedLeft = safeLower(left);
  const normalizedRight = safeLower(right);
  return normalizedLeft.length > 0 && normalizedLeft === normalizedRight;
}

function isDraftConversationId(value: string): boolean {
  return safeLower(value).startsWith("draft:");
}

const selectedConversation = computed(() => messagingStore.selectedConversation);
const activeConversationId = computed(() =>
  selectedConversation.value?.conversationId ?? messagingStore.selectedConversationId,
);
const connectedPeerOptions = computed<ConnectedPeerOption[]>(() => {
  const seen = new Set<string>();
  return nodeStore.discoveredPeers
    .filter((peer) => peer.activeLink)
    .filter((peer) => peer.saved || nodeStore.savedDestinations.has(peer.destination))
    .map((peer) => {
      const value = safeTrim(peer.lxmfDestinationHex) || safeTrim(peer.destination);
      const displayName = safeTrim(peer.announcedName) || safeTrim(peer.label) || value;
      return { value, displayName };
    })
    .filter((option) => {
      const normalizedValue = safeLower(option.value);
      if (!normalizedValue || seen.has(normalizedValue)) {
        return false;
      }
      seen.add(normalizedValue);
      return true;
    })
    .sort((left, right) => left.displayName.localeCompare(right.displayName));
});
const selectedConversationOption = computed<ConnectedPeerOption | null>(() => {
  const value = safeTrim(selectedConversation.value?.destinationHex);
  if (!safeTrim(value)) {
    return null;
  }
  return {
    value,
    displayName: safeTrim(selectedConversation.value?.displayName) || value,
  };
});
const threadDestinationOptions = computed<ConnectedPeerOption[]>(() => {
  const next = [...connectedPeerOptions.value];
  const current = selectedConversationOption.value;
  if (!current) {
    return next;
  }
  if (!next.some((option) => destinationsMatch(option.value, current.value))) {
    next.unshift(current);
  }
  return next;
});
const explicitDestinationHex = computed(() =>
  safeTrim(selectedConversation.value?.destinationHex) || safeTrim(selectedThreadDestinationHex.value),
);
const conversationCount = computed(() => messagingStore.conversations.length);
const selectedPeer = computed(() => {
  const destinationHex = safeLower(explicitDestinationHex.value);
  if (!destinationHex) {
    return null;
  }
  return nodeStore.discoveredByDestination[destinationHex]
    ?? Object.values(nodeStore.discoveredByDestination).find((peer) =>
      safeLower(peer.destination) === destinationHex
      || safeLower(peer.lxmfDestinationHex) === destinationHex,
    )
    ?? null;
});
const selectedPeerDisplayName = computed(() =>
  safeTrim(selectedPeer.value?.announcedName)
  || safeTrim(selectedPeer.value?.label)
  || safeTrim(selectedConversation.value?.displayName)
  || safeTrim(activeThreadConversation.value?.displayName)
  || selectedDestinationHex.value,
);
function findConversationForSelection(
  destinationHex: string,
  peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex"> | null = null,
) {
  const matches = messagingStore.conversations.filter((conversation) =>
    destinationsMatch(conversation.destinationHex, destinationHex)
    || destinationsMatch(conversation.destinationHex, peer?.destination ?? "")
    || destinationsMatch(conversation.destinationHex, peer?.lxmfDestinationHex ?? ""),
  );
  return matches.find((conversation) => !isDraftConversationId(conversation.conversationId))
    ?? matches[0]
    ?? null;
}

const activeThreadConversation = computed(() =>
  findConversationForSelection(explicitDestinationHex.value, selectedPeer.value),
);
const selectedDestinationHex = computed(() =>
  safeTrim(selectedConversation.value?.destinationHex)
  || safeTrim(activeThreadConversation.value?.destinationHex)
  || safeTrim(explicitDestinationHex.value),
);
const activeThreadMessages = computed(() => {
  const selectedConversationRecord = selectedConversation.value ?? activeThreadConversation.value;
  const destinationHex = selectedDestinationHex.value;
  if (!selectedConversationRecord) {
    return messagingStore.messagesForDestination(destinationHex);
  }
  const conversationMessages = messagingStore.messagesForConversation(
    selectedConversationRecord.conversationId,
  );
  if (conversationMessages.length > 0) {
    return conversationMessages;
  }
  return messagingStore.messagesForDestination(destinationHex);
});
const sosMapTargetsByMessageId = computed<Record<string, SosMessageMapTarget>>(() => {
  const targets: Record<string, SosMessageMapTarget> = {};
  for (const alert of sosStore.alerts) {
    const messageIdHex = safeLower(alert.messageIdHex);
    if (!messageIdHex || alert.lat === undefined || alert.lon === undefined) {
      continue;
    }
    targets[messageIdHex] = {
      incidentId: alert.incidentId,
      sourceHex: alert.sourceHex,
      messageIdHex,
    };
  }
  return targets;
});

const targetLookupNames = computed(() =>
  [...new Set([
    selectedPeerDisplayName.value,
    selectedPeer.value?.label ?? "",
    selectedPeer.value?.announcedName ?? "",
  ]
    .map((value) => safeTrim(value))
    .filter((value) => value.length > 0)
    .map((value) => value.toLowerCase()))],
);
const selectedTargetMessage = computed(() =>
  messagesStore.messages.find((message) => {
    const callsign = safeLower(message.callsign);
    const sourceDisplayName = safeLower(message.source?.display_name);
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
    .map((value) => safeLower(value))
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
  const detail = safeTrim(status.detail);
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
  selectedThreadDestinationHex.value = "";
  mobilePane.value = "detail";
}

function showConversationList(): void {
  mobilePane.value = "list";
}

function handleThreadDestinationSelected(event: Event): void {
  const nextDestinationHex = safeTrim((event.target as HTMLSelectElement).value);
  selectedThreadDestinationHex.value = nextDestinationHex;
  if (!nextDestinationHex) {
    return;
  }
  const option = threadDestinationOptions.value.find((entry) =>
    destinationsMatch(entry.value, nextDestinationHex),
  );
  messagingStore.ensureConversationForDestination(nextDestinationHex, option?.displayName);
  mobilePane.value = "detail";
}

async function send(bodyUtf8: string): Promise<void> {
  const destinationHex = selectedDestinationHex.value;
  if (!destinationHex) {
    return;
  }
  await messagingStore.sendMessage(destinationHex, bodyUtf8);
  const matchingConversation = messagingStore.selectedConversation
    ?? findConversationForSelection(destinationHex, selectedPeer.value);
  if (matchingConversation) {
    messagingStore.selectConversation(matchingConversation.conversationId);
  }
}

async function handleViewSosOnMap(target: SosMessageMapTarget): Promise<void> {
  await router.push({
    path: "/telemetry",
    query: {
      incident: target.incidentId,
      source: target.sourceHex,
      ...(target.messageIdHex ? { message: target.messageIdHex } : {}),
    },
  });
}

watch(
  () => [
    route.query.conversation,
    route.query.message,
    messagingStore.hydrated,
  ],
  ([conversationQuery, messageQuery]) => {
    const conversationId = routeQueryString(conversationQuery);
    if (!conversationId) {
      return;
    }
    const messageIdHex = routeQueryString(messageQuery);
    void messagingStore
      .openConversationTarget(conversationId, messageIdHex || undefined)
      .then(() => {
        selectedThreadDestinationHex.value = "";
        mobilePane.value = "detail";
      });
  },
  { immediate: true },
);
</script>

<template>
  <section class="view" :class="`pane-${mobilePane}`">
    <header class="view-header">
      <div class="view-heading">
        <h1 class="view-title">Inbox</h1>
        <p class="sync-line header-sync-line">
          Sync status: <strong>{{ syncStatusLabel }}</strong>
        </p>
      </div>
    </header>

    <section class="inbox-layout" :class="`pane-${mobilePane}`">
      <section class="panel inbox-panel list-panel">
        <header class="inbox-panel-header">
        <div>
          <p class="panel-kicker">Conversations</p>
          <h2 class="panel-title">Encrypted Inbox</h2>
        </div>
        <label class="peer-thread-picker mobile-only">
          <select
            :value="selectedDestinationHex"
            aria-label="Select connected peer"
            class="thread-picker-select"
            :disabled="threadDestinationOptions.length === 0"
            @change="handleThreadDestinationSelected"
          >
            <option value="">Select connected peer</option>
            <option
              v-for="option in threadDestinationOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.displayName }}
            </option>
          </select>
        </label>
        </header>
        <p class="panel-copy">
          {{ conversationCount }} conversation{{ conversationCount === 1 ? "" : "s" }} available.
        </p>
        <ConversationList
          :items="messagingStore.conversations"
          :selected-conversation-id="activeConversationId"
          :active-sos-conversation-ids="sosStore.activeConversationIds"
          @select="handleSelectConversation"
        />
      </section>

      <section class="panel inbox-panel detail-panel">
        <ConversationThread
          :destination-hex="selectedDestinationHex"
          :display-name="selectedPeerDisplayName"
          :show-back-button="mobilePane === 'detail'"
          :target-status="targetStatusLabel"
          :target-team="targetTeamLabel"
          :target-latitude="targetLatitudeLabel"
          :target-longitude="targetLongitudeLabel"
          :target-message-id="messagingStore.selectedTargetMessageId"
          :sos-map-targets="sosMapTargetsByMessageId"
          :messages="activeThreadMessages"
          @back="showConversationList"
          @send="send"
          @view-sos-on-map="handleViewSosOnMap"
        />
      </section>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
  grid-template-rows: auto minmax(0, 1fr);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.view-header {
  align-items: baseline;
  display: flex;
  gap: 1rem;
  justify-content: space-between;
}

.view-heading {
  align-items: baseline;
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.view-title,
.header-sync-line {
  margin: 0;
}

.view-title {
  font-family: var(--font-headline);
  font-size: clamp(1.9rem, 3vw, 2.8rem);
}

.panel {
  background: rgb(9 24 52 / 86%);
  border: 1px solid rgb(72 114 184 / 33%);
  border-radius: 15px;
  padding: 0.95rem;
}

.sync-line {
  color: #cfe5ff;
  font-family: var(--font-ui);
  margin: 0;
}

.header-sync-line {
  color: #94add3;
}

.inbox-panel {
  display: grid;
  gap: 0.9rem;
  min-height: 0;
}

.inbox-panel-header {
  align-items: start;
  display: flex;
  gap: 0.85rem;
  justify-content: space-between;
}

.inbox-panel-header > div {
  min-width: 0;
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
  align-items: stretch;
  display: grid;
  gap: 1rem;
  grid-template-columns: minmax(16rem, 22rem) minmax(0, 1fr);
  height: 100%;
  min-height: 0;
}

.detail-panel {
  height: 100%;
  min-height: 0;
}

.mobile-only {
  display: none;
}

.peer-thread-picker {
  display: grid;
  flex: 1 1 12rem;
  min-width: 0;
  max-width: 100%;
}

.thread-picker-select {
  appearance: none;
  background:
    linear-gradient(145deg, rgb(8 27 57 / 92%), rgb(5 18 40 / 96%)),
    linear-gradient(110deg, rgb(0 168 255 / 14%), rgb(20 240 255 / 14%));
  border: 1px solid rgb(80 145 220 / 32%);
  border-radius: 11px;
  color: #dff2ff;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 0.92rem;
  box-sizing: border-box;
  min-height: 38px;
  padding: 0.62rem 2rem 0.62rem 0.82rem;
  width: 100%;
}

.thread-picker-select:disabled {
  cursor: default;
  opacity: 0.82;
}

.thread-picker-select:active {
  border-color: rgb(120 227 255 / 42%);
  transform: translateY(1px) scale(0.99);
}

@media (max-width: 900px) {
  .pane-detail .view-header {
    display: none;
  }

  .inbox-layout {
    grid-template-columns: 1fr;
  }

  .inbox-panel-header {
    flex-wrap: wrap;
  }

  .mobile-only {
    display: grid;
    width: 100%;
  }

  .pane-list .detail-panel,
  .pane-detail .list-panel {
    display: none;
  }
}
</style>
