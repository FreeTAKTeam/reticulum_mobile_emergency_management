<script setup lang="ts">
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import ConversationList from "../components/messaging/ConversationList.vue";
import ConversationThread from "../components/messaging/ConversationThread.vue";
import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import type { DiscoveredPeer } from "../types/domain";
import { registerBackNavigationHandler } from "../utils/androidBackNavigation";
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
const isPeerPickerVisible = shallowRef(false);
let visualMockRefreshInterval: number | undefined;
let unregisterBackNavigationHandler: (() => void) | undefined;

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

function isVisualMockMode(): boolean {
  return import.meta.env.DEV && route.query.mockChat === "1";
}

const selectedConversation = computed(() => messagingStore.selectedConversation);
const activeConversationId = computed(() =>
  selectedConversation.value?.conversationId ?? messagingStore.selectedConversationId,
);
const connectedPeerOptions = computed<ConnectedPeerOption[]>(() => {
  const seen = new Set<string>();
  return nodeStore.reachablePeers
    .filter((peer) => nodeStore.savedDestinations.has(peer.destination))
    .map((peer) => {
      const value = safeTrim(peer.lxmfDestinationHex) || safeTrim(peer.destination);
      const baseName = safeTrim(peer.announcedName) || safeTrim(peer.label) || value;
      const displayName = peer.activeLink ? `${baseName} (Connected)` : `${baseName} (Reachable)`;
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
      || safeLower(peer.lxmfDestinationHex) === destinationHex
      || safeLower(peer.identityHex) === destinationHex,
    )
    ?? null;
});
const selectedPeerDisplayName = computed(() =>
  safeTrim(selectedPeer.value?.announcedName)
  || safeTrim(selectedConversation.value?.displayName)
  || safeTrim(activeThreadConversation.value?.displayName)
  || safeTrim(selectedPeer.value?.label)
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
  return message.overallStatus ?? messagesStore.eamReadinessForCallsign(message.callsign)?.overallBand ?? "Unknown";
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
const targetEamHref = computed(() => {
  const callsign = safeTrim(selectedTargetMessage.value?.callsign) || safeTrim(selectedPeerDisplayName.value);
  if (!callsign) {
    return "";
  }
  const params = new URLSearchParams({ callsign });
  return `/messages?${params.toString()}`;
});
const targetMapHref = computed(() => {
  const position = targetTelemetryPosition.value;
  if (!position) {
    return "";
  }
  const params = new URLSearchParams({
    callsign: position.callsign,
    lat: String(position.lat),
    lon: String(position.lon),
  });
  return `/telemetry?${params.toString()}`;
});

function handleSelectConversation(conversationId: string): void {
  messagingStore.selectConversation(conversationId);
  selectedThreadDestinationHex.value = "";
  mobilePane.value = "detail";
}

async function handleDeleteConversation(conversationId: string): Promise<void> {
  const conversation = messagingStore.conversations.find(
    (candidate) => candidate.conversationId === conversationId,
  );
  const displayName = conversation?.displayName ?? "this conversation";
  if (!window.confirm(`Delete ${displayName} from this device?`)) {
    return;
  }
  const wasActive = activeConversationId.value === conversationId;
  await messagingStore.deleteConversation(conversationId);
  selectedThreadDestinationHex.value = "";
  if (mobilePane.value === "detail" && !wasActive) {
    return;
  }
  mobilePane.value = "list";
}

function showConversationList(): void {
  mobilePane.value = "list";
}

function handleAndroidBackNavigation(): boolean {
  if (isPeerPickerVisible.value) {
    isPeerPickerVisible.value = false;
    return true;
  }
  if (mobilePane.value === "detail") {
    showConversationList();
    return true;
  }
  return false;
}

function togglePeerPicker(): void {
  isPeerPickerVisible.value = !isPeerPickerVisible.value;
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
  isPeerPickerVisible.value = false;
  mobilePane.value = "detail";
}

async function send(bodyUtf8: string): Promise<void> {
  const destinationHex = selectedDestinationHex.value;
  if (!destinationHex) {
    return;
  }
  if (isVisualMockMode()) {
    messagingStore.appendVisualMockOutboundMessage(destinationHex, bodyUtf8);
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

onMounted(() => {
  unregisterBackNavigationHandler = registerBackNavigationHandler(handleAndroidBackNavigation);
  if (isVisualMockMode()) {
    messagingStore.applyVisualMockChatData();
    visualMockRefreshInterval = window.setInterval(() => {
      messagingStore.applyVisualMockChatData();
    }, 2000);
  }
});

onUnmounted(() => {
  unregisterBackNavigationHandler?.();
  unregisterBackNavigationHandler = undefined;
  if (visualMockRefreshInterval !== undefined) {
    window.clearInterval(visualMockRefreshInterval);
  }
});
</script>

<template>
  <section class="view" :class="`pane-${mobilePane}`">
    <header class="view-header">
      <div class="header-actions">
        <span class="utility-chip count-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 6h16" />
            <path d="M4 12h16" />
            <path d="M4 18h16" />
          </svg>
          <span>{{ conversationCount }} Threads</span>
        </span>
        <button
          class="utility-chip peer-chip"
          type="button"
          aria-label="Select reachable peer"
          :aria-expanded="isPeerPickerVisible"
          title="Select reachable peer"
          @click="togglePeerPicker"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M16 21v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2" />
            <circle cx="9.5" cy="7" r="3" />
            <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
            <path d="M16 3.13a3 3 0 0 1 0 5.74" />
          </svg>
          <span>Reachable Peers</span>
          <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m7 10 5 5 5-5" />
          </svg>
        </button>
        <button
          class="create-toggle utility-new"
          type="button"
          aria-label="Select reachable peer"
          :aria-expanded="isPeerPickerVisible"
          title="Select reachable peer"
          @click="togglePeerPicker"
        >
          <span aria-hidden="true">+</span>
        </button>
      </div>
    </header>

    <form
      v-show="isPeerPickerVisible"
      class="peer-picker-form"
      @submit.prevent
    >
      <select
        :value="selectedDestinationHex"
        aria-label="Select reachable peer"
        class="thread-picker-select"
        :disabled="threadDestinationOptions.length === 0"
        @change="handleThreadDestinationSelected"
      >
        <option value="">Select reachable peer</option>
        <option
          v-for="option in threadDestinationOptions"
          :key="option.value"
          :value="option.value"
        >
          {{ option.displayName }}
        </option>
      </select>
      <p v-if="threadDestinationOptions.length === 0" class="peer-picker-empty">
        No reachable saved peers available.
      </p>
    </form>

    <section class="inbox-layout" :class="`pane-${mobilePane}`">
      <section class="panel inbox-panel list-panel">
        <ConversationList
          :items="messagingStore.conversations"
          :selected-conversation-id="activeConversationId"
          :active-sos-conversation-ids="sosStore.activeConversationIds"
          @delete="handleDeleteConversation"
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
          :target-eam-href="targetEamHref"
          :target-map-href="targetMapHref"
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

<style scoped src="./InboxView.css"></style>
